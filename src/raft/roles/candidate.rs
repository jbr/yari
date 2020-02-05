use crate::raft::{Followers, Leader, RaftState};
use crate::rpc::VoteRequest;
pub trait Candidate {
    fn start_election(&mut self) -> ElectionResult;
}

pub enum ElectionResult {
    Elected,
    FailedQuorum,
    Ineligible
}

impl Candidate for RaftState {
    fn start_election(&mut self) -> ElectionResult {
        if self.servers.contains(&self.id) {
            self.current_term += 1;
            println!("starting election, term: {}", self.current_term);
            self.voted_for = Some(self.id.clone());
            self.leader_id_for_client_redirection = None;

            let followers = Followers::from_servers(&self.servers, &self.id, self.log.next_index());

            let vote_request = VoteRequest {
                candidate_id: &self.id,
                last_log_index: self.log.last_index(),
                last_log_term: self.log.last_term(),
                term: self.current_term,
            };

            let can_i_vote = self.servers.contains(&self.id);

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
                ElectionResult::Elected
            } else {
                ElectionResult::FailedQuorum
            }
        } else {
            ElectionResult::Ineligible
        }
    }
}
