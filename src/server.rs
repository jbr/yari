use crate::append::{AppendRequest, AppendResponse};
use crate::client::{ClientRequest, ClientResponse};
use crate::config::YariConfig;
use crate::election_thread::ElectionThread;
use crate::raft::{RaftState, UnknownResult};
use crate::vote::{VoteRequest, VoteResponse};
use rocket::config::{Config, Environment, LoggingLevel};
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::{post, routes, State};
use rocket_contrib::json::{Json, JsonError};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

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
    F: (FnOnce(std::sync::MutexGuard<RaftState>) -> T) + 'a,
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
) -> Result<Json<ClientResponse>, Custom<String>> {
    handle_malformed_request(client_request)
        .and_then(|request| handle_busy_server(state, move |mut raft| raft.client(request)))
}

pub fn start(config: YariConfig, address: SocketAddr, id: String) -> UnknownResult<()> {
    let raft_state = RaftState::load_or_new(config, &id)?;
    ElectionThread::spawn(&raft_state);

    let rocket_config = Config::build(Environment::Development)
        .address(address.ip().to_string())
        .port(address.port())
        .log_level(LoggingLevel::Off)
        .finalize()?;

    rocket::custom(rocket_config)
        .manage(raft_state)
        .mount("/", routes![append, vote, client])
        .launch();

    Ok(())
}
