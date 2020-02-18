use crate::raft::{DynBoxedResult, Leader, LogEntry, RaftState};
use crate::rpc::{AppendRequest, AppendResponse, ClientRequest, VoteRequest, VoteResponse};
use rocket::config::{Config as RocketConfig, Environment, LoggingLevel};
use rocket::http::Status;
use rocket::response::{status::Custom, Redirect, Response};
use rocket::{delete, post, put, routes, State};
use rocket_contrib::json::{Json, JsonError};
use serde_json::json;
use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use url::Url;

fn handle_malformed_request<In>(
    json_parse_result: Result<Json<In>, JsonError<'_>>,
) -> Result<In, Custom<String>> {
    match json_parse_result {
        Ok(x) => Ok(x.into_inner()),
        Err(JsonError::Parse(input, e)) => Err(Custom(
            Status::UnprocessableEntity,
            json!({ "error": e.to_string(), "input": input }).to_string(),
        )),
        Err(JsonError::Io(e)) => Err(Custom(
            Status::BadRequest,
            json!({ "error": e.to_string() }).to_string(),
        )),
    }
}

fn handle_busy_server<'a, F, T>(
    state: State<'_, Arc<Mutex<RaftState>>>,
    f: F,
) -> Result<Json<T>, Custom<String>>
where
    F: (FnOnce(MutexGuard<RaftState>) -> T) + 'a,
{
    let arc: Arc<Mutex<_>> = state.inner().clone();
    let lock = arc.try_lock();
    match lock {
        Ok(raft_state) => Ok(Json(f(raft_state))),
        _ => Err(Custom(
            Status::ServiceUnavailable,
            json!({ "error": "unable to get a mutex lock" }).to_string(),
        )),
    }
}

#[post("/append", format = "json", data = "<append_request>")]
fn append(
    state: State<'_, Arc<Mutex<RaftState>>>,
    append_request: Result<Json<AppendRequest<'_>>, JsonError<'_>>,
) -> Result<Json<AppendResponse>, Custom<String>> {
    handle_malformed_request(append_request)
        .and_then(|request| handle_busy_server(state, move |mut raft| raft.append(request)))
}

#[post("/vote", format = "json", data = "<vote_request>")]
fn vote(
    state: State<'_, Arc<Mutex<RaftState>>>,
    vote_request: Result<Json<VoteRequest<'_>>, JsonError<'_>>,
) -> Result<Json<VoteResponse>, Custom<String>> {
    handle_malformed_request(vote_request)
        .and_then(|request| handle_busy_server(state, move |mut raft| raft.vote(request)))
}

#[post("/client", format = "json", data = "<client_request>")]
fn client(
    state: State<'_, Arc<Mutex<RaftState>>>,
    client_request: Result<Json<ClientRequest>, JsonError<'_>>,
) -> Response<'static> {
    let request = handle_malformed_request(client_request).unwrap();
    let arc: Arc<Mutex<_>> = state.inner().clone();
    let result: Result<LogEntry, Option<String>> = loop {
        match arc.try_lock() {
            Ok(mut state) => break state.client(&request),
            Err(_) => {
                eprintln!("waiting for lock");
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    };

    match result {
        Ok(log_entry) => {
            let apply_result = log_entry.block_until_committed();

            Response::build()
                .sized_body(Cursor::new(apply_result.unwrap_or_else(|| "".into())))
                .finalize()
        }

        Err(Some(url)) => Response::build()
            .status(Status::TemporaryRedirect)
            .raw_header("Location", url)
            .finalize(),

        Err(None) => Response::build()
            .status(Status::TemporaryRedirect)
            .finalize(),
    }
}

#[put("/servers/<id>")]
fn add_server(
    id: String,
    state: State<'_, Arc<Mutex<RaftState>>>,
) -> Result<Result<(), Redirect>, Status> {
    let arc: Arc<Mutex<_>> = state.inner().clone();
    let lock = arc.try_lock();
    if let Ok(mut raft) = lock {
        if raft.is_leader() {
            raft.member_add(&id);
            println!("i'm the leader, wanna add {}", id);
            Ok(Ok(()))
        } else if let Some(redirect) = raft.leader_id_for_client_redirection.as_ref() {
            let url = Url::parse(&redirect)
                .and_then(|u| u.join("/servers/"))
                .and_then(|u| u.join(&urlencoding::encode(&id)))
                .map_err(|_| Status::InternalServerError)?;

            Ok(Err(Redirect::temporary(url.into_string())))
        } else {
            Err(Status::InternalServerError)
        }
    } else {
        Err(Status::InternalServerError)
    }
}

#[delete("/servers/<id>")]
fn remove_server(
    id: String,
    state: State<'_, Arc<Mutex<RaftState>>>,
) -> Result<Result<(), Redirect>, Status> {
    let arc: Arc<Mutex<_>> = state.inner().clone();
    let lock = arc.try_lock();
    if let Ok(mut raft) = lock {
        if raft.is_leader() {
            raft.member_remove(&id);
            println!("i'm the leader, removing {}", id);
            Ok(Ok(()))
        } else if let Some(redirect) = raft.leader_id_for_client_redirection.as_ref() {
            let url = Url::parse(&redirect)
                .expect("could not parse url")
                .join("/servers/")
                .unwrap()
                .join(&urlencoding::encode(&id))
                .unwrap();
            Ok(Err(Redirect::temporary(url.into_string())))
        } else {
            Err(Status::InternalServerError)
        }
    } else {
        Err(Status::InternalServerError)
    }
}

pub fn start(state: Arc<Mutex<RaftState>>, address: SocketAddr) -> DynBoxedResult<()> {
    let rocket_config = RocketConfig::build(Environment::Development)
        .address(address.ip().to_string())
        .port(address.port())
        .log_level(LoggingLevel::Off)
        .finalize()?;

    rocket::custom(rocket_config)
        .manage(state)
        .mount(
            "/",
            routes![append, vote, client, add_server, remove_server],
        )
        .launch();

    Ok(())
}
