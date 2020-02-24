use crate::persistence;
use crate::raft::{LogEntry, RaftState};
use crate::rpc::{AppendRequest, AppendResponse, ClientRequest};

pub trait Leader {
    fn client(&mut self, client_request: &ClientRequest) -> Result<LogEntry, Option<String>>;
    fn send_appends_or_heartbeats(&mut self);
    fn update_commit_index(&mut self);
}

impl Leader for RaftState {
    fn client(&mut self, client_request: &ClientRequest) -> Result<LogEntry, Option<String>> {
        if self.is_leader() {
            let log_entry = self.client_append(&client_request.message);
            Ok(log_entry.clone())
        } else {
            Err(self.leader_id_for_client_redirection.clone())
        }
    }

    fn update_commit_index(&mut self) {
        if let Some(last_index) = self.log.last_index_in_term(self.current_term) {
            if let Some(followers) = &self.follower_state {
                if self.commit_index != last_index {
                    let new_commit_index = (self.commit_index + 1 ..= last_index)
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

    fn send_appends_or_heartbeats(&mut self) {
        persistence::persist(&self).expect("persistence");
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
                        leader_id: &self.id,
                        previous_log_index: previous_entry.map(|e| e.index),
                        previous_log_term: previous_entry.map(|e| e.term),
                        leader_commit_index: self.commit_index,
                    };

                    let append_response = append_request.send(&follower.identifier);

                    match append_response {
                        Ok(AppendResponse { term, success, .. }) if success => {
                            if term > self.current_term {
                                step_down = true;
                            } else {
                                if let Some(entries_sent) = append_request.entries {
                                    let last_entry_sent = entries_sent.last().unwrap();
                                    let next_index = last_entry_sent.index + 1;
                                    let match_index = last_entry_sent.index;
                                    any_change_in_match_indexes |=
                                        follower.match_index != match_index;
                                    follower.next_index = next_index;
                                    follower.match_index = match_index;
                                }
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
                self.commit();
            }

            if step_down || !self.servers.contains(&self.id) {
                println!("stepping down");
                self.become_follower();
            }
        }
    }
}
