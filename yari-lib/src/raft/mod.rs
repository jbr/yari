mod election_thread;
mod followers;
mod message;
mod servers;

pub use crate::log::LogEntry;
pub use crate::state_machine::*;
use crate::{
    config::Config,
    log::Log,
    message_board::MessageBoard,
    persistence,
    rpc::{AppendRequest, AppendResponse, RaftClient, VoteRequest, VoteResponse},
};
use async_channel::{Receiver, Sender};
pub use election_thread::ElectionThread;
pub use followers::{FollowerState, Followers};
use serde::{Deserialize, Serialize};
pub use servers::{RaftMessage, Servers};
use std::{path::PathBuf, time::Duration};

pub type DynBoxedResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
pub type Term = u64;
pub type Index = usize;

use RaftMessage::{Blank, ServerConfigChange, StateMachineMessage};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct TermIndex(pub Term, pub Index);

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

#[derive(Debug, Clone)]
pub struct InterruptChannel {
    sender: Sender<()>,
    receiver: Option<Receiver<()>>,
}

impl Default for InterruptChannel {
    fn default() -> Self {
        let (sender, receiver) = async_channel::unbounded();
        Self {
            sender,
            receiver: Some(receiver),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RaftState<SM, MT, AR> {
    id: String,
    log: Log<RaftMessage<MT>>,
    current_term: Term,
    voted_for: Option<String>,

    #[serde(skip)]
    client: RaftClient<SM>,

    #[serde(skip)]
    statefile_path: PathBuf,

    #[serde(skip)]
    state_machine: SM,

    #[serde(skip)]
    message_board: MessageBoard<TermIndex, AR>,

    #[serde(skip)]
    interrupt_channel: InterruptChannel,

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
    leader_id_for_client_redirection: Option<String>,
    // #[serde(skip)]
    // channel: SSEChannel,
}

impl<SM: StateMachine> Default for RaftState<SM, SM::MessageType, SM::ApplyResult> {
    fn default() -> Self {
        Self {
            id: String::default(),
            log: Log::default(),
            current_term: Term::default(),
            voted_for: None,
            statefile_path: PathBuf::default(),
            state_machine: SM::default(),
            interrupt_channel: InterruptChannel::default(),
            commit_index: Index::default(),
            last_applied_index: Index::default(),
            follower_state: None,
            config: Config::default(),
            servers: Servers::default(),
            client: RaftClient::new(),
            immediate_commit_index: Index::default(),
            leader_id_for_client_redirection: None,
            message_board: MessageBoard::default(),
            // channel: SSEChannel::default(),
        }
    }
}

#[derive(Default)]
pub struct EphemeralState<SM: StateMachine> {
    pub id: String,
    pub state_machine: SM,
    pub config: Config,
    pub statefile_path: PathBuf,
}

impl<SM: StateMachine> RaftState<SM, SM::MessageType, SM::ApplyResult> {
    pub fn with_ephemeral_state(mut self, eph: EphemeralState<SM>) -> Self {
        self.id = eph.id;
        self.statefile_path = eph.statefile_path;
        self.config = eph.config;
        self.state_machine = eph.state_machine;
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn leader_id_for_client_redirection(&self) -> Option<&str> {
        self.leader_id_for_client_redirection.as_deref()
    }

    pub fn set_leader_id_for_client_redirection(&mut self, id: Option<String>) {
        self.leader_id_for_client_redirection = id;
    }

    // pub fn channel(&self) -> &SSEChannel {
    //     &self.channel
    // }

    // pub fn set_channel(&mut self, channel: SSEChannel) {
    //     self.channel = channel;
    // }

    pub fn client(&self) -> RaftClient<SM> {
        self.client.clone()
    }

    pub fn statefile_path(&self) -> &PathBuf {
        &self.statefile_path
    }

    pub fn set_statefile_path(&mut self, statefile_path: PathBuf) {
        self.statefile_path = statefile_path;
    }

    pub fn client_append(&mut self, message: RaftMessage<SM::MessageType>) -> TermIndex {
        self.log.client_append(self.current_term, message)
    }

    pub fn receive_applied_result(
        &mut self,
        term_index: TermIndex,
    ) -> async_broadcast::Receiver<SM::ApplyResult> {
        self.message_board.listen(term_index)
    }

    pub fn member_add(&mut self, id: &str) {
        log::info!("about to add member: {}", id);
        if let Some(message) = self.servers.member_add(id) {
            log::info!("server config change: {:?}", message);
            self.client_append(message.into());
        }
    }

    pub fn member_remove(&mut self, id: &str) {
        if let Some(message) = self.servers.member_remove(id) {
            self.client_append(message.into());
        }
    }

    pub fn bootstrap(&mut self) {
        if let Some(message) = self.servers.member_add(&self.id) {
            self.client_append(message.into());
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
        self.voted_for.as_deref() == Some(&self.id)
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
        log::info!("commit");
        //        self.interrupt_timer().await;
        if let Some(entries) = self
            .log
            .entries_starting_at(self.immediate_commit_index + 1)
        {
            for entry in entries {
                match &entry.message {
                    ServerConfigChange(message) => self.servers.visit(message),
                    StateMachineMessage(message) => self.state_machine.visit(message),
                    Blank => (),
                }

                self.immediate_commit_index = entry.index;
            }
        }

        while self.commit_index > self.last_applied_index {
            let next_to_apply = self.last_applied_index + 1;
            let log_entry = self.log.get(next_to_apply).unwrap();
            let term_index = log_entry.into();
            match &log_entry.message {
                ServerConfigChange(message) => {
                    self.servers.apply(message);
                }

                StateMachineMessage(message) => {
                    let apply_result = self.state_machine.apply(message);
                    if self.is_leader() {
                        self.message_board
                            .post(&term_index, apply_result)
                            .await
                            .unwrap();
                    }
                }

                Blank => (),
            }

            self.last_applied_index = next_to_apply;
        }

        if let Some(followers) = self.follower_state.as_mut() {
            followers.update_from_servers(&self.servers, &self.id, self.log.next_index());
            if let Some(message) = self.servers.new_config.take() {
                self.client_append(message.into());
            }
        }
    }

    async fn apply_rules(&mut self, request_term: Term) -> DynBoxedResult {
        log::info!("apply rules");
        if request_term > self.current_term {
            self.voted_for = None;
            self.follower_state = None;
            self.current_term = request_term;
        }

        self.commit().await;
        persistence::persist(self).await?;
        Ok(())
    }

    async fn interrupt(&self) {
        log::trace!("interrupted");
        self.interrupt_channel.sender.send(()).await.unwrap();
    }

    pub fn take_interrupt_channel(&mut self) -> Option<Receiver<()>> {
        self.interrupt_channel.receiver.take()
    }

    fn generate_election_timeout(&self) -> Duration {
        Duration::from_millis(fastrand::u64(self.config.timeout()))
    }

    pub async fn append(
        &mut self,
        request: AppendRequest<RaftMessage<SM::MessageType>>,
    ) -> AppendResponse {
        log::info!("append");
        self.interrupt().await;
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

        self.apply_rules(request_term).await.unwrap();

        AppendResponse {
            success,
            term: current_term,
        }
    }

    pub async fn vote(&mut self, request: VoteRequest) -> VoteResponse {
        self.interrupt().await;

        let vote_granted = request.term >= self.current_term
            && (self.voted_for.is_none() || self.voted_for == Some(request.candidate_id.clone()))
            && (request.last_log_index >= self.log.last_index()
                && request.last_log_term >= self.log.last_term());

        if vote_granted {
            self.voted_for = Some(request.candidate_id);
            log::debug!(
                "{}: term {} voting for {}",
                self.id(),
                self.current_term,
                self.voted_for.as_ref().unwrap()
            );
        }

        let current_term = self.current_term;

        self.apply_rules(request.term).await.unwrap();

        VoteResponse {
            term: current_term,
            vote_granted,
        }
    }

    async fn start_election(&mut self) -> ElectionResult {
        if self.servers.contains(&self.id) {
            self.current_term += 1;
            log::trace!(
                "{}: starting election, term: {}",
                self.id(),
                self.current_term
            );
            self.voted_for = Some(self.id.clone());
            self.leader_id_for_client_redirection = None;

            let followers = Followers::from_servers(&self.servers, &self.id, self.log.next_index());

            let vote_request = VoteRequest {
                candidate_id: self.id.clone(),
                last_log_index: self.log.last_index(),
                last_log_term: self.log.last_term(),
                term: self.current_term,
            };

            let include_self = self.servers.contains(&self.id);

            let quorum = followers
                .meets_quorum_async(include_self, |follower| {
                    let client = self.client.clone();
                    let i = follower.identifier.clone();
                    let vr = vote_request.clone();
                    async move {
                        match client.request_vote(&i, &vr).await {
                            Ok(response) => response.vote_granted,
                            _ => false,
                        }
                    }
                })
                .await;

            if quorum {
                self.voted_for = None;
                self.follower_state = Some(followers);
                self.log.client_append(self.current_term, Blank);
                self.send_appends_or_heartbeats().await;
                ElectionResult::Elected
            } else {
                ElectionResult::FailedQuorum
            }
        } else {
            ElectionResult::Ineligible
        }
    }

    // pub async fn client_append_or_redirect(
    //     &mut self,
    //     client_request: crate::rpc::ClientRequest<<SM as StateMachine>::MessageType>,
    // ) -> Result<crate::Result<SM::ApplyResult>, Option<String>> {
    //     log::trace!("client append");
    //     if self.is_leader() {
    //         Ok(self
    //             .client_append(StateMachineMessage(client_request.message))
    //             .await)
    //     } else {
    //         Err(self.leader_id_for_client_redirection.clone())
    //     }
    // }

    fn update_commit_index(&mut self) {
        log::trace!("update commit index");
        if let Some(last_index) = self.log.last_index_in_term(self.current_term) {
            if let Some(followers) = &self.follower_state {
                if self.commit_index != last_index {
                    let new_commit_index = (self.commit_index + 1..=last_index)
                        .rev()
                        .find(|n| followers.quorum_has_item_at_index(*n));

                    if let Some(commit_index) = new_commit_index {
                        log::debug!(
                            "{}: updating commit index from {} to {}",
                            self.id(),
                            self.commit_index,
                            commit_index
                        );
                        self.commit_index = commit_index;
                    }
                }
            }
        }
    }

    pub async fn send_appends_or_heartbeats(&mut self) {
        log::trace!("send appends or heartbeats");

        let mut step_down = false;

        if let Some(followers) = self.follower_state.as_mut() {
            let mut any_change_in_match_indexes = followers.is_empty();
            for follower in followers.iter_mut() {
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

                    let append_response = self
                        .client
                        .append(&follower.identifier, &append_request)
                        .await;

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
                log::info!("change in match indexes");
                self.update_commit_index();
                self.commit().await;
                persistence::persist(self).await.unwrap();
            }

            if step_down || !self.servers.contains(&self.id) {
                log::debug!("{}: stepping down", self.id());
                self.become_follower();
            }
        }
    }
}
