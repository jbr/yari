mod election_thread;
mod followers;
mod message;
mod servers;

use crate::SSEChannel;
use crate::config::Config;
use crate::log::Log;
pub use crate::log::LogEntry;
use crate::message_board::MessageBoard;
use crate::persistence;
use crate::rpc::{AppendRequest, AppendResponse, VoteRequest, VoteResponse};
pub use crate::state_machine::*;
use async_std::sync::{Receiver, Sender};
pub use election_thread::ElectionThread;
pub use followers::{FollowerState, Followers};
use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;
use serde::{Deserialize, Serialize};
pub use servers::{ServerMessageOrStateMachineMessage, Servers};
use std::path::PathBuf;
use std::time::Duration;

pub type DynBoxedResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
pub type Term = u64;
pub type Index = usize;

pub enum ElectionResult {
    Elected,
    FailedQuorum,
    Ineligible,
}

#[derive(Debug)]
pub enum Role {
    Solitary,
    Leader,
    Follower,
    Candidate,
}
use Role::*;

#[derive(Debug)]
struct InterruptChannel {
    sender: Sender<()>, receiver: Receiver<()>
}
impl Default for InterruptChannel {
    fn default() -> Self {
        let (sender, receiver) = async_std::sync::channel(1);
        Self{sender, receiver}
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RaftState<SM, MT> {
    pub id: String,
    log: Log<MT>,
    current_term: Term,
    voted_for: Option<String>,

    #[serde(skip)]
    pub statefile_path: PathBuf,

    #[serde(skip)]
    state_machine: SM,

    #[serde(skip)]
    message_board: MessageBoard<LogEntry<MT>, String>,

    #[serde(skip)]
    update_timer: InterruptChannel,

    #[serde(skip)]
    commit_index: Index,

    #[serde(skip)]
    last_applied_index: Index,

    #[serde(skip_deserializing)]
    follower_state: Option<Followers>,

    #[serde(skip)]
    config: Config,

    #[serde(skip_deserializing)]
    servers: Servers,

    #[serde(skip)]
    immediate_commit_index: Index,

    #[serde(skip)]
    pub leader_id_for_client_redirection: Option<String>,

    #[serde(skip)]
    pub channel: SSEChannel,
}


impl<SM: StateMachine> Default for RaftState<SM, MessageType<SM>> {
    fn default() -> Self {
        Self {
            id: String::default(),
            log: Log::default(),
            current_term: Term::default(),
            voted_for: None,
            statefile_path: PathBuf::default(),
            state_machine: SM::default(),
            update_timer: InterruptChannel::default(),
            commit_index: Index::default(),
            last_applied_index: Index::default(),
            follower_state: None,
            config: Config::default(),
            servers: Servers::default(),
            immediate_commit_index: Index::default(),
            leader_id_for_client_redirection: None,
            message_board: MessageBoard::new(),
            channel: SSEChannel::default(),
        }
    }
}

pub struct EphemeralState<SM: StateMachine> {
    pub id: String,
    pub state_machine: SM,
    pub config: Config,
    pub statefile_path: PathBuf,
}

pub type MessageType<SM> = ServerMessageOrStateMachineMessage<<SM as StateMachine>::MessageType>;
pub type Raft<SM> = RaftState<SM, MessageType<SM>>;

impl<SM: StateMachine> RaftState<SM, MessageType<SM>> {
    pub fn with_ephemeral_state(mut self, eph: EphemeralState<SM>) -> Self {
        self.id = eph.id;
        self.statefile_path = eph.statefile_path;
        self.config = eph.config;
        self.state_machine = eph.state_machine;
        self
    }

    pub async fn client_append_with_result(&mut self, message: MessageType<SM>) -> Receiver<String> {
        self.interrupt_timer().await;

        self.message_board
            .listen(self.log.client_append(self.current_term, message).clone())
    }

    pub async fn client_append_without_result(&mut self, message: MessageType<SM>) {
        self.log.client_append(self.current_term, message);
        self.interrupt_timer().await;
    }

    pub async fn member_add(&mut self, id: &str) {
        if let Some(message) = self.servers.member_add(&id) {
            let wrapped: MessageType<SM> =
                ServerMessageOrStateMachineMessage::ServerConfigChange(message);
            self.client_append_without_result(wrapped).await;
        }
    }

    pub async fn member_remove(&mut self, id: &str) {
        if let Some(message) = self.servers.member_remove(&id) {
            let wrapped: MessageType<SM> =
                ServerMessageOrStateMachineMessage::ServerConfigChange(message);
            self.client_append_without_result(wrapped).await;
        }
    }

    pub async fn bootstrap(&mut self) {
        if let Some(message) = self.servers.member_add(&self.id) {
            let wrapped: MessageType<SM> =
                ServerMessageOrStateMachineMessage::ServerConfigChange(message);
            self.client_append_without_result(wrapped).await;
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

    pub fn is_solitary(&self) -> bool {
        if let Some(followers) = &self.follower_state {
            followers.is_empty()
        } else {
            false
        }
    }

    pub fn role(&self) -> Role {
        if self.is_solitary() {
            Solitary
        } else if self.is_leader() {
            Leader
        } else if self.is_candidate() {
            Candidate
        } else {
            Follower
        }
    }

    pub async fn commit(&mut self) {
        if let Some(entries) = self
            .log
            .entries_starting_at(self.immediate_commit_index + 1)
        {
            for entry in entries {
                match &entry.message {
                    ServerMessageOrStateMachineMessage::ServerConfigChange(message) => 
                        self.servers.visit(message),

                    ServerMessageOrStateMachineMessage::StateMachineMessage(message) => 
                        self.state_machine.visit(message),

                    ServerMessageOrStateMachineMessage::Blank => (),
                }

                self.immediate_commit_index = entry.index;
            }
        }

        while self.commit_index > self.last_applied_index {
            let next_to_apply = self.last_applied_index + 1;
            let log_entry = self.log.get(next_to_apply).unwrap();
            match &log_entry.message {
                ServerMessageOrStateMachineMessage::ServerConfigChange(message) => {
                    self.servers.apply(message);
                }

                ServerMessageOrStateMachineMessage::StateMachineMessage(message) => {
                    let apply_result = self.state_machine.apply(message).unwrap_or_default();
                    self.message_board.post(&log_entry, apply_result).await.ok();
                }

                ServerMessageOrStateMachineMessage::Blank => (),
            }

            self.last_applied_index = next_to_apply;
        }

        if let Some(followers) = self.follower_state.as_mut() {
            followers.update_from_servers(&self.servers, &self.id, self.log.next_index());
            if let Some(message) = self.servers.new_config.take() {
                let wrapped: MessageType<SM> =
                    ServerMessageOrStateMachineMessage::ServerConfigChange(message);
                self.client_append_without_result(wrapped).await;
            }
        }
    }

    async fn apply_rules(&mut self, request_term: Term) -> DynBoxedResult {
        if request_term > self.current_term {
            self.voted_for = None;
            self.follower_state = None;
            self.current_term = request_term;
        }

        self.commit().await;
        persistence::persist(&self)?;
        Ok(())
    }

    async fn interrupt_timer(&self) {
        self.update_timer.sender.send(()).await;
    }

    pub fn interrupt_receiver(&self) -> Receiver<()> {
        self.update_timer.receiver.clone()
    }

    fn generate_election_timeout(&self) -> Duration {
        let mut rng = thread_rng();
        let distribution: Uniform<u64> = self.config.timeout().into();
        Duration::from_millis(distribution.sample(&mut rng))
    }

    pub async fn append(&mut self, request: AppendRequest<MessageType<SM>>) -> AppendResponse {
        self.interrupt_timer().await;
        if self.is_candidate() {
            self.become_follower();
        }

        self.leader_id_for_client_redirection = Some(request.leader_id.clone());

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

        self.apply_rules(request_term).await.ok();

        AppendResponse {
            success,
            term: current_term,
        }
    }

    pub async fn vote(&mut self, request: VoteRequest) -> VoteResponse {
        self.interrupt_timer().await;

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

        self.apply_rules(request.term).await.ok();

        VoteResponse {
            term: current_term,
            vote_granted,
        }
    }

    async fn start_election(&mut self) -> ElectionResult {
        if self.servers.contains(&self.id) {
            self.current_term += 1;
            println!("starting election, term: {}", self.current_term);
            self.voted_for = Some(self.id.clone());
            self.leader_id_for_client_redirection = None;

            let followers = Followers::from_servers(&self.servers, &self.id, self.log.next_index());

            let vote_request = VoteRequest {
                candidate_id: self.id.clone(),
                last_log_index: self.log.last_index(),
                last_log_term: self.log.last_term(),
                term: self.current_term,
            };

            let can_i_vote = self.servers.contains(&self.id);

            let quorum = followers
                .meets_quorum_async(can_i_vote, |follower| {
                    let i = follower.identifier.clone();
                    let vr = vote_request.clone();
                    async move {
                        match vr.send(&i).await {
                            Ok(response) => response.vote_granted,
                            _ => false,
                        }
                    }
                })
                .await;

            if quorum {
                self.voted_for = None;
                self.follower_state = Some(followers);
                self.log
                    .client_append(self.current_term, ServerMessageOrStateMachineMessage::Blank);
                self.send_appends_or_heartbeats().await;
                ElectionResult::Elected
            } else {
                ElectionResult::FailedQuorum
            }
        } else {
            ElectionResult::Ineligible
        }
    }

    pub async fn client(
        &mut self,
        client_request: crate::rpc::ClientRequest<<SM as StateMachine>::MessageType>,
    ) -> Result<Receiver<String>, Option<String>> {
        if self.is_leader() {
            Ok(self.client_append_with_result(
                ServerMessageOrStateMachineMessage::StateMachineMessage(client_request.message),
            ).await)
        } else {
            Err(self.leader_id_for_client_redirection.clone())
        }
    }

    fn update_commit_index(&mut self) {
        if let Some(last_index) = self.log.last_index_in_term(self.current_term) {
            if let Some(followers) = &self.follower_state {
                if self.commit_index != last_index {
                    let new_commit_index = (self.commit_index + 1..=last_index)
                        .rev()
                        .find(|n| followers.quorum_has_item_at_index(*n));

                    if let Some(commit_index) = new_commit_index {
                        println!(
                            "updating commit index from {} to {}",
                            self.commit_index, commit_index
                        );
                        self.commit_index = commit_index;
                    }
                }
            }
        }
    }

    pub async fn send_appends_or_heartbeats(&mut self) {
        let mut step_down = false;

        if let Some(followers) = self.follower_state.as_mut() {
            let mut any_change_in_match_indexes = followers.is_empty();
            for mut follower in followers.iter_mut() {
                loop {
                    let entries_to_send = self.log.entries_starting_at(follower.next_index);
                    let previous_entry = self.log.previous_entry_to(follower.next_index);

                    let append_request = AppendRequest {
                        term: self.current_term,
                        entries: entries_to_send.map(|e| e.to_vec()),
                        leader_id: self.id.clone(),
                        previous_log_index: previous_entry.map(|e| e.index),
                        previous_log_term: previous_entry.map(|e| e.term),
                        leader_commit_index: self.commit_index,
                    };

                    let append_response = append_request.send(&follower.identifier).await;

                    match append_response {
                        Ok(AppendResponse { term, success, .. }) if success => {
                            if term > self.current_term {
                                step_down = true;
                            } else if let Some(entries_sent) = append_request.entries {
                                let last_entry_sent = entries_sent.last().unwrap();
                                let next_index = last_entry_sent.index + 1;
                                let match_index = last_entry_sent.index;
                                any_change_in_match_indexes |= follower.match_index != match_index;
                                follower.next_index = next_index;
                                follower.match_index = match_index;
                            }

                            break;
                        }

                        Ok(AppendResponse { term, .. }) => {
                            if term > self.current_term {
                                step_down = true;
                            }
                            follower.next_index = 2.max(follower.next_index) - 1
                        }

                        Err(_) => break,
                    }
                }
            }

            if any_change_in_match_indexes {
                self.update_commit_index();
                self.commit().await;
                persistence::persist(&self).unwrap();
            }

            if step_down || !self.servers.contains(&self.id) {
                println!("stepping down");
                self.become_follower();
            }
        }
    }
}
