use crate::{raft::Message, Okay};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use surf::Client;
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
    pub async fn send(&self, server: &Url) -> Result<ClientResponse> {
        client_append(server, &self).await
    }
}

pub async fn client_append<MessageType: Message>(
    server: &Url,
    request: &ClientRequest<MessageType>,
) -> Result<ClientResponse> {
    Client::new()
        .post(server.join("/client")?)
        .body_json(&request)?
        .recv_json::<ClientResponse>()
        .await
        .map_err(|e| anyhow!("{}", e))?
        .okay()
}
