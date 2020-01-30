use crate::log::LogEntry;
use crate::raft::{Index, Term};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct AppendResponse {
    pub term: Term,
    pub success: bool,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct AppendRequest<'a> {
    pub term: Term,
    pub leader_id: &'a str,
    pub previous_log_index: Option<Index>,
    pub previous_log_term: Option<Term>,
    pub entries: Option<Vec<LogEntry>>,
    pub leader_commit_index: Index,
}

pub fn append(
    server: &str,
    append_request: &AppendRequest<'_>,
) -> Result<AppendResponse, reqwest::Error> {
    Client::new()
        .post(&format!("http://{}/append", server))
        .json(&append_request)
        .send()?
        .json::<AppendResponse>()
}

impl<'a> AppendRequest<'a> {
    pub fn send(&self, server: &str) -> Result<AppendResponse, reqwest::Error> {
        append(server, &self)
    }
}
