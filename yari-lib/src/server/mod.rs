use crate::{
    raft::{ElectionThread, RaftMessage, StateMachine},
    rpc::{AppendRequest, AppendResponse, ClientRequest, VoteRequest, VoteResponse},
    RaftState,
};
use async_lock::RwLock;
use serde_json::{json, Value};
use std::{net::SocketAddr, sync::Arc};
use trillium::{Conn, Status};
use trillium_api::{api, FromConn, Json, State};
use trillium_http::Stopper;
use trillium_redirect::Redirect;
use trillium_router::RouterConnExt;
use trillium_server_common::ServerHandle;
use url::Url;

trait RaftStateExt {
    fn raft_state<SM: StateMachine>(&self) -> WebState<SM>;
}

impl RaftStateExt for Conn {
    fn raft_state<SM: StateMachine>(&self) -> WebState<SM> {
        self.state::<WebState<SM>>().unwrap().clone()
    }
}

async fn append<SM: StateMachine>(
    conn: &mut Conn,
    Json(append_request): Json<AppendRequest<RaftMessage<SM::MessageType>>>,
) -> Json<AppendResponse> {
    let state = conn.raft_state::<SM>();
    let mut state = state.write().await;
    Json(state.append(append_request).await)
}

async fn vote<SM: StateMachine>(
    conn: &mut Conn,
    Json(vote_request): Json<VoteRequest>,
) -> Json<VoteResponse> {
    let state = conn.raft_state::<SM>();
    let mut state = state.write().await;
    Json(state.vote(vote_request).await)
}

async fn client<SM: StateMachine>(
    _: &mut Conn,
    (Json(client_request), WebRaftState(raft)): (
        Json<ClientRequest<SM::MessageType>>,
        WebRaftState<SM>,
    ),
) -> Result<Json<Value>, Result<Redirect, Status>> {
    if raft.read().await.is_leader() {
        let mut receiver = {
            let mut raft = raft.write().await;
            let term_index =
                raft.client_append(RaftMessage::StateMachineMessage(client_request.message));
            raft.receive_applied_result(term_index)
        };
        let apply_result = receiver.recv().await.unwrap();
        Ok(Json(json!({"result": apply_result})))
    } else if let Some(leader) = raft.read().await.leader_id_for_client_redirection() {
        Err(Ok(Redirect::to(leader.to_string())))
    } else {
        Err(Err(Status::ServiceUnavailable))
    }
}

fn leader_redirect<SM: StateMachine>(
    id: &str,
    raft: &RaftState<SM, SM::MessageType, SM::ApplyResult>,
) -> Result<Redirect, Status> {
    match raft.leader_id_for_client_redirection() {
        Some(redirect) => {
            let location = Url::parse(id)
                .and_then(|u| u.join("/servers/"))
                .and_then(|u| u.join(&urlencoding::encode(redirect)))
                .map_err(|_| Status::InternalServerError)?;
            Ok(Redirect::to(location.to_string()))
        }

        None => Err(Status::InternalServerError),
    }
}

struct Id(String);
#[trillium::async_trait]
impl FromConn for Id {
    async fn from_conn(conn: &mut Conn) -> Option<Self> {
        conn.param("id")
            .map(urlencoding::decode)
            .transpose()
            .ok()
            .flatten()
            .map(|c| Self(c.to_string()))
    }
}

async fn add_server<SM: StateMachine>(
    _: &mut Conn,
    (Id(id), State(raft)): (Id, State<WebState<SM>>),
) -> Result<Status, Result<Redirect, Status>> {
    let mut raft = raft.write().await;

    if raft.is_leader() {
        raft.member_add(&id);
        Ok(Status::Ok)
    } else {
        Err(leader_redirect(&id, &*raft))
    }
}

struct WebRaftState<SM: StateMachine>(WebState<SM>);
#[trillium::async_trait]
impl<SM: StateMachine> FromConn for WebRaftState<SM> {
    async fn from_conn(conn: &mut Conn) -> Option<Self> {
        conn.take_state().map(Self)
    }
}

async fn remove_server<SM: StateMachine>(
    _: &mut Conn,
    (Id(id), WebRaftState(raft)): (Id, WebRaftState<SM>),
) -> Result<Status, Result<Redirect, Status>> {
    let mut raft = raft.write().await;

    if raft.is_leader() {
        raft.member_remove(&id);
        Ok(Status::Ok)
    } else {
        Err(leader_redirect(&id, &*raft))
    }
}

async fn status<SM: StateMachine>(
    _: &mut Conn,
    WebRaftState(raft): WebRaftState<SM>,
) -> Json<Value> {
    let state = raft.read().await;
    Json(serde_json::to_value(&*state).unwrap())
}

type WebState<SM> = Arc<
    RwLock<RaftState<SM, <SM as StateMachine>::MessageType, <SM as StateMachine>::ApplyResult>>,
>;

pub async fn start<SM: StateMachine>(
    state: RaftState<SM, SM::MessageType, SM::ApplyResult>,
    socket_addr: SocketAddr,
) -> ServerHandle {
    let stopper = Stopper::new();
    let state = Arc::new(RwLock::new(state));
    log::info!("start");
    async_global_executor::spawn({
        let state = state.clone();
        let stopper = stopper.clone();
        async move {
            log::info!("spawning election task");
            stopper.stop_future(ElectionThread::spawn(state)).await;
            stopper.stop();
        }
    })
    .detach();

    trillium_smol::config()
        .with_stopper(stopper)
        .with_socketaddr(socket_addr)
        .spawn((
            trillium::state(state),
            trillium_logger::logger(),
            trillium_router::router()
                .get("/", api(status::<SM>))
                .post("/append", api(append::<SM>))
                .post("/vote", api(vote::<SM>))
                .post("/client", api(client::<SM>))
                .put("/servers/:id", api(add_server::<SM>))
                .delete("/servers/:id", api(remove_server::<SM>)),
        ))
}
