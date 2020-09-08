use crate::{raft::Message, Okay};
use serde::{Deserialize, Serialize};
use surf::{Body, Client};
use url::Url;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct ClientResponse {
    result: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientRequest<M> {
    pub message: M,
}

impl<M: Message> ClientRequest<M> {
    pub async fn send(&self, server: &Url) -> tide::Result<ClientResponse> {
        client_append(server, &self).await
    }
}

pub async fn client_append<MessageType: Message>(
    server: &Url,
    request: &ClientRequest<MessageType>,
) -> tide::Result<ClientResponse> {
    Client::new()
        .post(server.join("/client").unwrap())
        .body(Body::from_json(&request).unwrap())
        .recv_json::<ClientResponse>()
        .await
        .map_err(|e| tide::http::format_err!("{}", e))?
        .okay()
}
