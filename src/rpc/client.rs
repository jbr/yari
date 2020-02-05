use crate::Message;
use reqwest::blocking::Client;
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
    pub fn send(&self, server: &Url, follow: bool) -> Result<ClientResponse, reqwest::Error> {
        client_append(server, &self, follow)
    }
}

    

pub fn client_append(
    server: &Url,
    request: &ClientRequest,
    follow: bool,
) -> Result<ClientResponse, reqwest::Error> {
    let mut server = server.to_string();

    loop {
        let response = Client::new()
            .post(&format!("{}/client", server))
            .json(&request)
            .send()?
            .json::<ClientResponse>();

        match (follow, response) {
            (
                true,
                Ok(ClientResponse {
                    leader_id: Some(leader_id),
                    ..
                }),
            ) if leader_id != server => {
                server = leader_id;
                println!("⤳ redirecting to {}", &server);
            }
            (_, response) => {
                break response;
            }
        }
    }
}
