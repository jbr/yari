mod response;
use self::response::Response;
use crate::raft::{Raft, ServerMessageOrStateMachineMessage, StateMachine};
use crate::rpc::{AppendRequest, ClientRequest, VoteRequest};
use async_std::sync::{Arc, RwLock};
use serde_json::json;
use std::net::SocketAddr;
use tide::Request;
use url::Url;

async fn append<SM: StateMachine>(mut request: Request<WebState<SM>>) -> Response {
    let request_body: AppendRequest<ServerMessageOrStateMachineMessage<SM::MessageType>> =
        request.body_json().await.unwrap();
    let state = request.state().clone();
    let mut state = state.write().await;
    state.append(request_body).await.into()
}

async fn vote<SM: StateMachine>(mut request: Request<WebState<SM>>) -> Response {
    let vote_request: VoteRequest = request.body_json().await.unwrap();
    let state = request.state().clone();
    let mut state = state.write().await;
    state.vote(vote_request).await.into()
}

async fn client<SM: StateMachine>(mut request: Request<WebState<SM>>) -> Response {
    let client_request: ClientRequest<SM::MessageType> = request.body_json().await.unwrap();
    let state = request.state().clone();
    let mut state = state.write().await;
    let result = state.client(client_request).await;

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

async fn add_server<SM: StateMachine>(request: Request<WebState<SM>>) -> Response {
    let raft = request.state().clone();
    let mut raft = raft.write().await;
    let id: String = request.param("id").unwrap();

    if raft.is_leader() {
        raft.member_add(&urlencoding::decode(&id)?).await;
        Response::Success
    } else {
        leader_redirect(&id, &*raft)
    }
}

async fn remove_server<SM: StateMachine>(request: Request<WebState<SM>>) -> Response {
    let raft = request.state().clone();
    let mut raft = raft.write().await;
    let id: String = request.param("id").unwrap();

    if raft.is_leader() {
        raft.member_remove(&urlencoding::decode(&id)?).await;
        Response::Success
    } else {
        leader_redirect(&id, &*raft)
    }
}

async fn serve_static(path: String) -> tide::Response {
    let mut file_path = std::path::PathBuf::from("./web/build/");
    file_path.push(path);
    dbg!(&file_path);

    if file_path.exists() {
        let f = async_std::fs::File::open(&file_path).await.unwrap();
        let content_type = mime_guess::from_path(file_path).first().unwrap();

        tide::Response::with_reader(200, async_std::io::BufReader::new(f))
            .set_mime(content_type)
    } else {
        tide::Response::new(tide::http_types::StatusCode::NotFound)
    }
}

async fn web<SM: StateMachine>(r: Request<WebState<SM>>) -> tide::Response {
    let path = r.param("path").unwrap();
    serve_static(path).await
}

async fn index<SM: StateMachine>(_r: Request<WebState<SM>>) -> tide::Response {
    serve_static("index.html".to_string()).await
}

async fn sse<SM: StateMachine>(r: Request<WebState<SM>>) -> tide::Response {
    eprintln!("sse connected");
    let state = r.state().clone();
    let channel = state.read().await.channel.clone();
    channel
        .into_response()
        .set_header(tide::http_types::headers::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
}

type WebState<SM> = Arc<RwLock<Raft<SM>>>;

pub async fn start<SM: StateMachine>(
    state: Arc<RwLock<Raft<SM>>>,
    address: SocketAddr,
) -> Result<(), std::io::Error> {
    let mut server = tide::with_state(state);
    server.at("/append").post(append::<SM>);
    server.at("/vote").post(vote::<SM>);
    server.at("/client").post(client::<SM>);
    server.at("/sse").get(sse::<SM>);
    server
        .at("/servers/:id")
        .put(add_server::<SM>)
        .delete(remove_server::<SM>);
    server.at("/*path").get(web::<SM>);
    server.at("").get(index::<SM>);
    server.at("/").get(index::<SM>);

    println!("starting server on {:?}", &address);
    server.listen(address).await?;

    Ok(())
}
