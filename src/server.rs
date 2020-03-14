mod response;
use self::response::Response;
use crate::raft::{Raft, ServerMessageOrStateMachineMessage, StateMachine};
use crate::rpc::{AppendRequest, ClientRequest, VoteRequest};
use async_std::sync::{Arc, Mutex};
use serde_json::json;
use std::net::SocketAddr;
use tide::Request;
use url::Url;

async fn append<SM: StateMachine>(mut request: Request<Arc<Mutex<Raft<SM>>>>) -> Response {
    let request_body: AppendRequest<ServerMessageOrStateMachineMessage<SM::MessageType>> =
        request.body_json().await.unwrap();
    let state: &Arc<Mutex<Raft<SM>>> = request.state();
    state.lock().await.append(request_body).await.into()
}

async fn vote<SM: StateMachine>(mut request: Request<Arc<Mutex<Raft<SM>>>>) -> Response {
    let vote_request: VoteRequest = request.body_json().await.unwrap();
    let state: &Arc<Mutex<Raft<SM>>> = request.state();
    state.lock().await.vote(vote_request).await.into()
}

async fn client<SM: StateMachine>(mut request: Request<Arc<Mutex<Raft<SM>>>>) -> Response {
    let client_request: ClientRequest<SM::MessageType> = request.body_json().await.unwrap();
    let state: &Arc<Mutex<Raft<SM>>> = request.state();
    let result = state.lock().await.client(client_request);

    match result {
        Err(Some(leader)) => Url::parse(&leader)?.join("/client")?.into(),
        Err(None) => Response::Unavailable,
        Ok(le) => {
            let result = le.recv().await?;
            json!({ "result": result }).into()
        }
    }
}

fn leader_redirect<SM: StateMachine>(id: &str, raft: &Raft<SM>) -> Response {
    let redirect = raft.leader_id_for_client_redirection.as_ref()?;
    Url::parse(&redirect)?
        .join("/servers/")?
        .join(&urlencoding::encode(id))?
        .into()
}

async fn add_server<SM: StateMachine>(request: Request<Arc<Mutex<Raft<SM>>>>) -> Response {
    let state: &Arc<Mutex<Raft<SM>>> = request.state();
    let mut raft = state.lock().await;
    let id: String = request.param("id").unwrap();
    dbg!(&id);
    if raft.is_leader() {
        raft.member_add(&urlencoding::decode(&id)?);
        dbg!(&raft);
        Response::Success
    } else {
        leader_redirect(&id, &*raft)
    }
}

async fn remove_server<SM: StateMachine>(request: Request<Arc<Mutex<Raft<SM>>>>) -> Response {
    let state: &Arc<Mutex<Raft<SM>>> = request.state();
    let mut raft = state.lock().await;
    let id: String = request.param("id").unwrap();
    if raft.is_leader() {
        raft.member_remove(&urlencoding::decode(&id)?);
        Response::Success
    } else {
        leader_redirect(&id, &*raft)
    }
}

async fn hello<SM: StateMachine>(_r: Request<Arc<Mutex<Raft<SM>>>>) -> Response {
    "hi".into()
}

pub async fn start<SM: StateMachine>(
    state: Arc<Mutex<Raft<SM>>>,
    address: SocketAddr,
) -> Result<(), std::io::Error> {
    let mut server = tide::with_state(state.clone());

    server.at("/").get(hello::<SM>);
    server.at("/append").post(append::<SM>);
    server.at("/vote").post(vote::<SM>);
    server.at("/client").post(client::<SM>);
    server
        .at("/servers/:id")
        .put(add_server::<SM>)
        .delete(remove_server::<SM>);

    println!("starting server on {:?}", &address);
    server.listen(address).await?;
    Ok(())
}
