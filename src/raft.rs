use crate::append::{AppendRequest, AppendResponse};
use crate::client::{ClientRequest, ClientResponse};
use crate::config::Config;
use crate::log::Log;
use crate::state_machine::{StateMachine, StringAppendStateMachine};
use crate::vote::{VoteRequest, VoteResponse};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
pub type UnknownResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
pub type Term = u64;
pub type Index = usize;

#[derive(Debug)]
pub struct FollowerState {
    identifier: String,
    next_index: Index,
    match_index: Index,
}

impl FollowerState {
    pub fn has_item_at_index(&self, n: Index) -> bool {
        self.match_index >= n
    }
}

trait Followers {
    fn quorum(&self, count_self_as: bool) -> usize;
    fn meets_quorum<P>(&self, count_self_as: bool, predicate: P) -> bool
    where
        P: FnMut(&&FollowerState) -> bool;
    fn quorum_has_item_at_index(&self, n: Index) -> bool;
}

trait AtLeastN<I>
where
    I: Iterator,
{
    fn at_least<P>(self, n: usize, predicate: P) -> bool
    where
        P: FnMut(&I::Item) -> bool;
}

impl<I: Iterator> AtLeastN<I> for I {
    fn at_least<P>(self, n: usize, predicate: P) -> bool
    where
        P: FnMut(&I::Item) -> bool,
    {
        self.filter(predicate).take(n).count() == n
    }
}

impl Followers for Vec<FollowerState> {
    fn quorum(&self, count_self_as: bool) -> usize {
        let add_for_self = if count_self_as { 1 } else { 0 };
        (self.len() + add_for_self) / 2
    }

    fn meets_quorum<P>(&self, count_self_as: bool, predicate: P) -> bool
    where
        P: FnMut(&&FollowerState) -> bool,
    {
        let quorum = self.quorum(count_self_as);
        self.iter().at_least(quorum, predicate)
    }

    fn quorum_has_item_at_index(&self, n: Index) -> bool {
        self.meets_quorum(true, |follower| follower.has_item_at_index(n))
    }
}

#[derive(Debug)]
pub enum Role {
    Leader,
    Follower,
    Candidate,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RaftState {
    id: String,
    log: Log,
    current_term: Term,
    voted_for: Option<String>,

    #[serde(skip)]
    state_machine: StringAppendStateMachine,

    #[serde(skip)]
    pub update_timer: Option<std::sync::mpsc::Sender<()>>,

    #[serde(skip)]
    commit_index: Index,

    #[serde(skip)]
    last_applied_index: Index,

    #[serde(skip)]
    pub follower_state: Option<Vec<FollowerState>>,

    #[serde(skip)]
    pub config: Option<Config>,

    #[serde(skip)]
    pub leader_id_for_client_redirection: Option<String>,
}

impl RaftState {
    pub fn new(config: Config, id: &str) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(RaftState {
            state_machine: StringAppendStateMachine::default(),
            id: id.into(),
            log: Log::new(),
            current_term: 0,
            voted_for: None,
            commit_index: 0,
            last_applied_index: 0,
            follower_state: None,
            config: Some(config),
            update_timer: None,
            leader_id_for_client_redirection: None,
        }))
    }

    fn persistance_path(id: &str) -> UnknownResult<PathBuf> {
        let mut path = env::current_dir()?;
        path.push(format!("{}.json", id));
        Ok(path)
    }

    fn persist(&self) -> UnknownResult {
        let path = RaftState::persistance_path(&self.id)?;

        if let Ok(string) = serde_json::to_string(&self) {
            std::fs::write(path, string)?
        }
        Ok(())
    }

    fn load(id: &str) -> UnknownResult<Self> {
        let path = Self::persistance_path(id)?;
        let raft: RaftState = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        Ok(raft)
    }

    pub fn load_or_new(config: Config, id: &str) -> UnknownResult<Arc<Mutex<Self>>> {
        match RaftState::load(id) {
            Err(e) => {
                println!("e: {:?}", e);
                Ok(RaftState::new(config, id))
            }
            Ok(raft) => Ok(Arc::new(Mutex::new(RaftState {
                config: Some(config),
                ..raft
            }))),
        }
    }

    pub fn start_election(&mut self) -> UnknownResult<bool> {
        self.current_term += 1;
        println!("starting election, term: {}", self.current_term);
        self.voted_for = Some(self.id.clone());
        self.leader_id_for_client_redirection = None;

        let all_servers = &self.config.as_ref().unwrap().servers;

        let followers: Vec<_> = all_servers
            .iter()
            .filter(|id| id.to_string() != self.id)
            .map(|addr| FollowerState {
                identifier: addr.to_string(),
                next_index: self.log.next_index(),
                match_index: 0,
            })
            .collect();

        let vote_request = VoteRequest {
            candidate_id: &self.id,
            last_log_index: self.log.last_index(),
            last_log_term: self.log.last_term(),
            term: self.current_term,
        };

        let can_i_vote = all_servers
            .iter()
            .any(|server| server.to_string() == self.id);

        let quorum = followers.meets_quorum(can_i_vote, |follower| {
            match vote_request.send(&follower.identifier) {
                Ok(response) => response.vote_granted,
                _ => false,
            }
        });

        if quorum {
            self.voted_for = None;
            self.follower_state = Some(followers);
            self.log.client_append(self.current_term, None); //append blank noop
            self.send_appends_or_heartbeats();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn client(&mut self, client_request: ClientRequest) -> ClientResponse {
        if self.is_leader() {
            let message = Some(&client_request.message);
            self.log.client_append(self.current_term, message);

            ClientResponse {
                raft_success: true,
                state_machine_response: Some(self.state_machine.read().into()),
                ..Default::default()
            }
        } else {
            ClientResponse {
                raft_success: false,
                leader_id: self.leader_id_for_client_redirection.as_ref().cloned(),
                ..Default::default()
            }
        }
    }

    fn update_commit_index(&mut self) {
        if let Some(last_index) = self.log.last_index_in_term(self.current_term) {
            if let Some(followers) = &self.follower_state {
                if self.commit_index != last_index {
                    let new_commit_index = (self.commit_index..=last_index)
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

    pub fn send_appends_or_heartbeats(&mut self) {
        self.persist().expect("persist");

        if let Some(followers) = self.follower_state.as_mut() {
            let mut any_change_in_match_indexes = false;
            for mut follower in followers {
                loop {
                    let entries_to_send = self.log.entries_starting_at(follower.next_index);
                    let previous_entry = self.log.previous_entry_to(follower.next_index);
                    // if self.log.last_index() >= Some(follower.next_index) {
                    //     println!("i think i should have something to say");
                    //     println!("my last index: {:?}, their match index: {}, their next_index: {}", self.log.last_index(), follower.match_index, follower.next_index);
                    //     println!("but my entries to send are {:?}", entries_to_send);
                    // }

                    let append_request = AppendRequest {
                        term: self.current_term,
                        entries: entries_to_send,
                        leader_id: &self.id,
                        previous_log_index: previous_entry.map(|e| e.index),
                        previous_log_term: previous_entry.map(|e| e.term),
                        leader_commit_index: self.commit_index,
                    };

                    let append_response = append_request.send(&follower.identifier);

                    match append_response {
                        // TODO: if response term is greater than my term, i should step down
                        Ok(AppendResponse { success, .. }) if success == true => {
                            if let Some(entries_sent) = append_request.entries {
                                let last_entry_sent = entries_sent.last().unwrap();
                                let next_index = last_entry_sent.index + 1;
                                let match_index = last_entry_sent.index;
                                any_change_in_match_indexes |= follower.match_index != match_index;
                                follower.next_index = next_index;
                                follower.match_index = match_index;
                            }
                            break;
                        }

                        Ok(AppendResponse { .. }) => {
                            follower.next_index = 2.max(follower.next_index) - 1
                        }

                        Err(_) => break,
                    }
                }
            }

            if any_change_in_match_indexes {
                self.update_commit_index();
                self.commit();
            }
        }
    }

    pub fn become_follower(&mut self) {
        self.follower_state = None;
        self.leader_id_for_client_redirection = None;
        if self.is_candidate() {
            self.voted_for = None;
        }
    }

    pub fn is_candidate(&self) -> bool {
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
        while self.commit_index > self.last_applied_index {
            let next_to_apply = self.last_applied_index + 1;
            let log_entry = self.log.get(next_to_apply).unwrap();
            self.state_machine.apply(log_entry.message.as_ref()).ok();
            self.last_applied_index = next_to_apply;
        }
    }

    pub fn apply_rules(&mut self, request_term: Term) -> UnknownResult {
        if request_term > self.current_term {
            self.voted_for = None;
            self.follower_state = None;
            self.current_term = request_term;
        }

        self.commit();
        self.persist()?;

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

        let append_success = request.term >= self.current_term && self.log.append(request);

        if leader_commit > self.commit_index {
            // confirm that this is correct. paper says last *new*
            // index, but I can't think of a situation where last
            // index would be less than last new index
            self.commit_index = leader_commit.min(self.log.last_index().unwrap_or(0));
        }

        let current_term = self.current_term;

        self.apply_rules(request_term)
            .expect("apply rules failure, temporary panic");

        AppendResponse {
            success: append_success,
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

        if let Err(e) = self.apply_rules(request.term) {
            println!("{:?}", e);
        }

        VoteResponse {
            term: current_term,
            vote_granted,
        }
    }
}
