mod response;
use crate::raft::{DynBoxedResult, Leader, RaftState};
use crate::rpc::{AppendRequest, ClientRequest, VoteRequest};
use response::Response;
use rocket::config::{Config as RocketConfig, Environment, LoggingLevel};
use rocket::{delete, post, put, routes, State};
use rocket_contrib::json::{Json, JsonError};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use url::Url;

type MutexedRaftState<'a> = State<'a, Arc<Mutex<RaftState>>>;
type JsonResult<'a, T> = Result<Json<T>, JsonError<'a>>;

#[post("/append", format = "json", data = "<append_request>")]
fn append(state: MutexedRaftState, append_request: JsonResult<AppendRequest>) -> Response {
    let append_request = append_request?.into_inner();
    state.lock()?.append(append_request).into()
}

#[post("/vote", format = "json", data = "<vote_request>")]
fn vote<'a>(state: MutexedRaftState, vote_request: JsonResult<VoteRequest>) -> Response {
    let vote_request = vote_request?.into_inner();
    state.lock()?.vote(vote_request).into()
}

#[post("/client", format = "json", data = "<client_request>")]
fn client<'a>(state: MutexedRaftState, client_request: JsonResult<ClientRequest>) -> Response {
    let result = {
        let client_request = &*client_request?;
        state.lock()?.client(client_request).clone()
    };

    match result {
        Err(Some(e)) => Url::parse(&e)?.join("/client")?.into(),
        Err(None) => Response::Unavailable,
        Ok(log_entry) => log_entry
            .block_until_committed()
            .unwrap_or_else(|| "".into())
            .into(),
    }
}

#[put("/servers/<id>")]
fn add_server(id: String, state: MutexedRaftState) -> Response {
    let mut raft = state.lock()?;
    if raft.is_leader() {
        raft.member_add(&id);
        Response::Success
    } else {
        let redirect = raft.leader_id_for_client_redirection.as_ref()?;
        Url::parse(&redirect)?
            .join("/servers/")?
            .join(&urlencoding::encode(&id))?
            .into()
    }
}

#[delete("/servers/<id>")]
fn remove_server(id: String, state: MutexedRaftState) -> Response {
    let mut raft = state.lock()?;
    if raft.is_leader() {
        raft.member_remove(&id);
        Response::Success
    } else {
        let redirect = raft.leader_id_for_client_redirection.as_ref()?;
        Url::parse(&redirect)?
            .join("/servers/")?
            .join(&urlencoding::encode(&id))?
            .into()
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
