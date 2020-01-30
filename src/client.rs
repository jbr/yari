use crate::log::Message;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

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

pub fn client_append(
    server: String,
    message: Message,
    follow: bool,
) -> Result<ClientResponse, reqwest::Error> {
    let request = ClientRequest { message };
    let mut server = server;

    loop {
        let response = Client::new()
            .post(&format!("http://{}/client", server))
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
