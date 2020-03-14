use crate::raft::{StateMachine, Message};
use serde::{Deserialize, Serialize};
use std::collections::{hash_set::IntoIter, HashSet};
use std::fmt::{Debug, Formatter, Result as FmtResult};

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Servers {
    set: HashSet<String>,
    pub new_config: Option<ServerConfigChange>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct ServerConfigChange {
    current: HashSet<String>,
    new: Option<HashSet<String>>,
}
impl Message for ServerConfigChange {}


#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ServerMessageOrStateMachineMessage<MT> {
    ServerConfigChange(ServerConfigChange),
    StateMachineMessage(MT),
    Blank,
}

impl<MT: Message> Message for ServerMessageOrStateMachineMessage<MT> {}

impl<MT> Default for ServerMessageOrStateMachineMessage<MT> {
    fn default() -> Self {
        Self::Blank
    }
}

impl Debug for ServerConfigChange {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{:?}->{:?}", self.current, self.new)
    }
}

impl Servers {
    pub fn member_add(&self, id: &str) -> Option<ServerConfigChange> {
        let mut new = self.set.clone();
        new.insert(id.into());

        Some(ServerConfigChange {
            current: self.set.clone(),
            new: Some(new),
        })
    }

    pub fn member_remove(&self, id: &str) -> Option<ServerConfigChange> {
        let mut new = self.set.clone();
        new.remove(id);

        Some(ServerConfigChange {
            current: self.set.clone(),
            new: Some(new),
        })
    }

    pub fn contains(&self, id: &str) -> bool {
        self.set.contains(id)
    }

    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }
}

impl<'a> IntoIterator for &'a Servers {
    type Item = String;
    type IntoIter = IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.set.clone().into_iter()
    }
}

impl StateMachine for Servers {
    type MessageType = ServerConfigChange;

    fn apply(&mut self, scc: &ServerConfigChange) -> Option<String> {
        self.new_config = if let Some(new) = &scc.new {
            Some(ServerConfigChange {
                current: new.clone(),
                new: None,
            })
        } else {
            None
        };

        None
    }

    fn visit(&mut self, scc: &ServerConfigChange) {
        self.set = if let Some(new) = &scc.new {
            scc.current.clone().union(&new).cloned().collect()
        } else {
            scc.current.clone()
        };
    }
}
