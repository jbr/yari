use crate::raft::{Index, Term};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct VoteResponse {
    pub term: Term,
    pub vote_granted: bool,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct VoteRequest<'a> {
    pub term: Term,
    pub candidate_id: &'a str,
    pub last_log_index: Option<Index>,
    pub last_log_term: Option<Term>,
}

pub fn request_vote(
    server: &str,
    vote_request: &VoteRequest<'_>,
) -> Result<VoteResponse, reqwest::Error> {
    Client::new()
        .post(&format!("{}/vote", server))
        .json(&vote_request)
        .send()?
        .json::<VoteResponse>()
}

impl<'a> VoteRequest<'a> {
    pub fn send(&self, server: &str) -> Result<VoteResponse, reqwest::Error> {
        request_vote(server, &self)
    }
}
