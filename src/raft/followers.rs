use crate::raft::{Index, Servers};
use std::collections::{
    hash_map::{Values, ValuesMut},
    HashMap
};
use crate::at_least::{AtLeastN,AtLeastNAsync};
use std::hash::{Hash, Hasher};
use async_std::future::Future;

#[derive(Debug, Clone)]
pub struct FollowerState {
    pub identifier: String,
    pub next_index: Index,
    pub match_index: Index,
}

impl Hash for FollowerState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.identifier.hash(state);
    }
}

impl PartialEq for FollowerState {
    fn eq(&self, other: &Self) -> bool {
        self.identifier == other.identifier
    }
}

impl Eq for FollowerState {}

impl FollowerState {
    pub fn has_item_at_index(&self, n: Index) -> bool {
        self.match_index >= n
    }
}

#[derive(Debug, Default)]
pub struct Followers(HashMap<String, FollowerState>);
impl Followers {
    pub fn from_servers(servers: &Servers, own_id: &str, next_index: Index) -> Self {
        let mut followers = Followers::default();
        for server in servers {
            if server != own_id {
                followers.add_follower(server.clone(), next_index)
            }
        }
        followers
    }

    pub fn update_from_servers(&mut self, servers: &Servers, own_id: &str, next_index: Index) {
        for follower in self.0.clone().keys() {
            if ! servers.contains(&follower) {
                self.0.remove(follower);
            }
        }

        for server in servers {
            if server != own_id {
                self.add_follower(server, next_index)
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> Values<String, FollowerState> {
        self.0.values()
    }

    pub fn iter_mut(&mut self) -> ValuesMut<String, FollowerState> {
        self.0.values_mut()
    }

    pub fn add_follower(&mut self, identifier: String, next_index: Index) {
        self.0
            .entry(identifier.clone())
            .or_insert_with(|| FollowerState {
                identifier,
                next_index,
                match_index: 0,
            });
    }

    pub fn remove_follower(&mut self, identifier: &str) {
        self.0.remove(identifier);
    }

    fn others_needed_for_quorum(&self, include_self: bool) -> usize {
        let count = self.0.len() as f32;
        if include_self {
            (((count + 1.0) / 2.0).ceil() - 1.0) as usize
        } else {
            (count / 2.0).ceil() as usize
        }
    }

    pub fn meets_quorum<P>(&self, include_self: bool, predicate: P) -> bool
    where
        P: FnMut(&&FollowerState) -> bool,
    {
        let quorum_size = self.others_needed_for_quorum(include_self);
        self.0.values().at_least(quorum_size, predicate)
    }


    pub async fn meets_quorum_async<P, F>(&self, include_self: bool, predicate: P) -> bool
    where
        P: Send + FnMut(&FollowerState) -> F,
        F: Send + Future<Output = bool>
    {
        let quorum_size = self.others_needed_for_quorum(include_self);
        self.0.values().at_least_async(quorum_size, predicate).await
    }



    pub fn quorum_has_item_at_index(&self, n: Index) -> bool {
        self.meets_quorum(true, |follower| follower.has_item_at_index(n))
    }
}
