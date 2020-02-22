mod response;
use crate::raft::{DynBoxedResult, Leader, RaftState};
use crate::rpc::{AppendRequest, ClientRequest, VoteRequest};
use response::Response;
use rocket::config::{Config as RocketConfig, Environment, LoggingLevel};
use rocket::{delete, post, put, routes, State};
use rocket_contrib::json::{Json, JsonError};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use url::Url;

type MutexedRaftState<'a> = State<'a, Arc<Mutex<RaftState>>>;
type JsonResult<'a, T> = Result<Json<T>, JsonError<'a>>;

#[post("/append", format = "json", data = "<append_request>")]
fn append(state: MutexedRaftState, append_request: JsonResult<AppendRequest>) -> Response {
    let append_request = append_request?.into_inner();
    Response::AppendResponse(state.lock()?.append(append_request))
}

#[post("/vote", format = "json", data = "<vote_request>")]
fn vote<'a>(state: MutexedRaftState, vote_request: JsonResult<VoteRequest>) -> Response {
    let vote_request = vote_request?.into_inner();
    Response::VoteResponse(state.lock()?.vote(vote_request))
}

#[post("/client", format = "json", data = "<client_request>")]
fn client<'a>(
    state: MutexedRaftState,
    client_request: JsonResult<ClientRequest>,
) -> Response {
    let result = {
        let client_request = &*client_request?;
        state.lock()?.client(client_request).clone()
    };

    match result {
        Err(Some(e)) => Response::Redirect(format!("{}{}", e, "client")),
        Err(None) => Response::Unavailable,
        Ok(log_entry) => Response::String(
            log_entry
                .block_until_committed()
                .unwrap_or_else(|| "".into()),
        ),
    }
}

#[put("/servers/<id>")]
fn add_server(id: String, state: MutexedRaftState) -> Response {
    let mut raft = state.lock()?;
    if raft.is_leader() {
        raft.member_add(&id);
        println!("i'm the leader, wanna add {}", id);
        Response::Json(json!({"success": true}))
    } else {
        let redirect = raft.leader_id_for_client_redirection.as_ref()?;
        let url = Url::parse(&redirect)
            .and_then(|u| u.join("/servers/"))
            .and_then(|u| u.join(&urlencoding::encode(&id)))?;

        Response::Redirect(url.into_string())
    }
}

#[delete("/servers/<id>")]
fn remove_server(id: String, state: MutexedRaftState) -> Response {
    if let Ok(mut raft) = state.lock() {
        if raft.is_leader() {
            raft.member_remove(&id);
            println!("i'm the leader, removing {}", id);
            Response::Ok
        } else if let Some(redirect) = raft.leader_id_for_client_redirection.as_ref() {
            let url = Url::parse(&redirect)
                .expect("could not parse url")
                .join("/servers/")
                .unwrap()
                .join(&urlencoding::encode(&id))
                .unwrap();
            Response::Redirect(url.into_string())
        } else {
            Response::InternalError(None)
        }
    } else {
        Response::InternalError(None)
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
