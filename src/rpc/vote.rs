use crate::{
    raft::{Index, Term},
    Okay,
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use surf::Client;
use url::Url;

#[derive(Serialize, Deserialize, Debug)]
pub struct VoteResponse {
    pub term: Term,
    pub vote_granted: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct VoteRequest {
    pub term: Term,
    pub candidate_id: String,
    pub last_log_index: Option<Index>,
    pub last_log_term: Option<Term>,
}

pub async fn request_vote(server: &str, vote_request: &VoteRequest) -> Result<VoteResponse> {
    Client::new()
        .post(Url::parse(server)?.join("/vote")?)
        .body_json(&vote_request)?
        .recv_json::<VoteResponse>()
        .await
        .map_err(|e| anyhow!("{}", e))?
        .okay()
}

impl VoteRequest {
    pub async fn send(&self, server: &str) -> Result<VoteResponse> {
        request_vote(server, &self).await
    }
}
