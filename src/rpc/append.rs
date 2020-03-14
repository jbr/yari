use crate::{
    log::LogEntry,
    raft::{Index, Message, Term},
    Okay,
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use url::Url;
use surf::Client;

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
) -> Result<AppendResponse> {
    Client::new()
        .post(Url::parse(&server)?.join("/append")?)
        .body_json(&append_request)?
        .recv_json::<AppendResponse>()
        .await
        .map_err(|e| anyhow!("{}", e))?
        .okay()
}

impl<'a, MessageType: Message> AppendRequest<MessageType> {
    pub async fn send(&self, server: &str) -> Result<AppendResponse> {
        append(server, &self).await
    }
}
