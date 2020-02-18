mod election_thread;
mod followers;
mod message;
mod roles;
mod servers;

use crate::config::Config;
use crate::log::Log;
pub use crate::log::LogEntry;
use crate::persistence;
use crate::rpc::{AppendRequest, AppendResponse, VoteRequest, VoteResponse};
pub use crate::state_machine::*;
pub use election_thread::ElectionThread;
pub use followers::{FollowerState, Followers};
pub use message::Message;
pub use roles::{Candidate, Leader};
use serde::{Deserialize, Serialize};
pub use servers::Servers;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

pub type UnknownResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
pub type Term = u64;
pub type Index = usize;

#[derive(Debug)]
pub enum Role {
    Leader,
    Follower,
    Candidate,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct RaftState {
    pub id: String,
    log: Log,
    current_term: Term,
    voted_for: Option<String>,

    #[serde(skip)]
    pub statefile_path: PathBuf,

    #[serde(skip)]
    state_machine: Box<dyn StateMachine>,

    #[serde(skip)]
    pub update_timer: Option<Sender<()>>,

    #[serde(skip)]
    commit_index: Index,

    #[serde(skip)]
    last_applied_index: Index,

    #[serde(skip)]
    pub follower_state: Option<Followers>,

    #[serde(skip)]
    pub config: Config,

    #[serde(skip)]
    servers: Servers,

    #[serde(skip)]
    immediate_commit_index: Index,

    #[serde(skip)]
    pub leader_id_for_client_redirection: Option<String>,
}

impl RaftState {
    pub fn with_ephemeral_state<S: StateMachine + 'static>(
        mut self,
        id: String,
        statefile_path: PathBuf,
        config: Config,
        state_machine: S,
    ) -> Self {
        self.id = id;
        self.statefile_path = statefile_path;
        self.config = config;
        self.state_machine = Box::new(state_machine);
        self
    }

    pub fn client_append(&mut self, message: &Message) -> &LogEntry {
        self.log.client_append(self.current_term, Some(message))
    }

    pub fn member_add(&mut self, id: &str) {
        if let Some(message) = self.servers.member_add(&id) {
            self.client_append(&message);
        }
    }

    pub fn member_remove(&mut self, id: &str) {
        if let Some(message) = self.servers.member_remove(&id) {
            self.client_append(&message);
        }
    }

    pub fn bootstrap(&mut self) {
        if let Some(message) = self.servers.member_add(&self.id) {
            self.client_append(&message);
        }
    }

    fn become_follower(&mut self) {
        self.follower_state = None;
        self.leader_id_for_client_redirection = None;
        if self.is_candidate() {
            self.voted_for = None;
        }
    }

    fn is_candidate(&self) -> bool {
        self.voted_for.contains(&self.id)
    }

    pub fn is_leader(&self) -> bool {
        self.follower_state.is_some()
    }

    pub fn role(&self) -> Role {
        if self.is_leader() {
            Role::Leader
        } else if self.is_candidate() {
            Role::Candidate
        } else {
            Role::Follower
        }
    }

    pub fn commit(&mut self) {
        if let Some(entries) = self
            .log
            .entries_starting_at(self.immediate_commit_index + 1)
        {
            for entry in entries {
                if let Some(message) = &entry.message {
                    self.servers.visit(message);
                    self.state_machine.visit(message);
                }

                self.immediate_commit_index = entry.index;
            }
        }

        while self.commit_index > self.last_applied_index {
            let next_to_apply = self.last_applied_index + 1;
            let log_entry = self.log.get(next_to_apply).unwrap();
            if let Some(message) = &log_entry.message {
                self.servers.apply(message);
                log_entry.store_result(self.state_machine.apply(message));
            }

            self.last_applied_index = next_to_apply;
        }

        if let Some(followers) = self.follower_state.as_mut() {
            followers.update_from_servers(&self.servers, &self.id, self.log.next_index());
            if let Some(message) = self.servers.new_config.take() {
                self.client_append(&message);
            }
        }
    }

    fn apply_rules(&mut self, request_term: Term) -> UnknownResult {
        if request_term > self.current_term {
            self.voted_for = None;
            self.follower_state = None;
            self.current_term = request_term;
        }

        self.commit();
        persistence::persist(&self)?;
        Ok(())
    }

    fn extend_timer(&mut self) {
        self.update_timer
            .as_ref()
            .unwrap()
            .send(())
            .expect("sending message to leader thread");
    }

    pub fn append(&mut self, request: AppendRequest<'_>) -> AppendResponse {
        self.extend_timer();
        if self.is_candidate() {
            self.become_follower();
        }

        self.leader_id_for_client_redirection = Some(request.leader_id.into());

        let leader_commit = request.leader_commit_index;
        let request_term = request.term;
        let success = if request.term >= self.current_term {
            self.log.append(request)
        } else {
            false
        };

        if leader_commit > self.commit_index {
            // confirm that this is correct. paper says last *new*
            // index, but I can't think of a situation where last
            // index would be less than last new index
            self.commit_index = leader_commit.min(self.log.last_index().unwrap_or(0));
        }

        let current_term = self.current_term;

        self.apply_rules(request_term).ok();

        AppendResponse {
            success,
            term: current_term,
        }
    }

    pub fn vote(&mut self, request: VoteRequest<'_>) -> VoteResponse {
        self.extend_timer();

        let vote_granted = request.term >= self.current_term
            && (self.voted_for.is_none() || self.voted_for.contains(&request.candidate_id))
            && (request.last_log_index >= self.log.last_index()
                && request.last_log_term >= self.log.last_term());

        if vote_granted {
            self.voted_for = Some(request.candidate_id.into());
            println!(
                "term {}: I don't know about you, but I'm voting for {}",
                self.current_term,
                self.voted_for.as_ref().unwrap()
            );
        }

        let current_term = self.current_term;

        self.apply_rules(request.term).ok();

        VoteResponse {
            term: current_term,
            vote_granted,
        }
    }
}
