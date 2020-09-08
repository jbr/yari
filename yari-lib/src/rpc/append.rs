use crate::{
    log::LogEntry,
    raft::{Index, Message, Term},
    Okay,
};

use serde::{Deserialize, Serialize};
use surf::{Body, Client};
use url::Url;

#[derive(Serialize, Deserialize, Debug)]
pub struct AppendResponse {
    pub term: Term,
    pub success: bool,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct AppendRequest<MessageType> {
    pub term: Term,
    pub leader_id: String,
    pub previous_log_index: Option<Index>,
    pub previous_log_term: Option<Term>,
    pub entries: Option<Vec<LogEntry<MessageType>>>,
    pub leader_commit_index: Index,
}

pub async fn append<MT: Message>(
    server: &str,
    append_request: &AppendRequest<MT>,
) -> tide::Result<AppendResponse> {
    Client::new()
        .post(Url::parse(&server)?.join("/append")?)
        .body(Body::from_json(&append_request)?)
        .recv_json::<AppendResponse>()
        .await?
        .okay()
}

impl<'a, MessageType: Message> AppendRequest<MessageType> {
    pub async fn send(&self, server: &str) -> tide::Result<AppendResponse> {
        append(server, &self).await
    }
}
