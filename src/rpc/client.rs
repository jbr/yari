use crate::Message;
use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct ClientResponse {
    pub raft_success: bool,
    pub leader_id: Option<String>,
    pub state_machine_response: Option<String>,
    pub state_machine_error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientRequest {
    pub message: Message,
}

impl ClientRequest {
    pub fn send(&self, server: &Url) -> Result<Response, reqwest::Error> {
        client_append(server, &self)
    }
}

pub fn client_append(server: &Url, request: &ClientRequest) -> Result<Response, reqwest::Error> {
    Client::new()
        .post(&server.clone().join("/client").unwrap().to_string())
        .timeout(std::time::Duration::from_secs(15))
        .json(&request)
        .send()
}
