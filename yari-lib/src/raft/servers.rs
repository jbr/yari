use crate::raft::{Message, StateMachine};
use serde::{Deserialize, Serialize};
use std::collections::{hash_set::Iter, HashSet};
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

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(tag = "type")]
pub enum RaftMessage<MT> {
    ServerConfigChange(ServerConfigChange),
    StateMachineMessage(MT),
    #[default]
    Blank,
}

impl<MT> From<ServerConfigChange> for RaftMessage<MT> {
    fn from(value: ServerConfigChange) -> Self {
        Self::ServerConfigChange(value)
    }
}

impl<MT: Message> Message for RaftMessage<MT> {}

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
    type Item = &'a String;
    type IntoIter = Iter<'a, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.set.iter()
    }
}

impl StateMachine for Servers {
    type MessageType = ServerConfigChange;
    type ApplyResult = ();

    fn apply(&mut self, scc: &ServerConfigChange) {
        self.new_config = scc.new.as_ref().map(|new| ServerConfigChange {
            current: new.clone(),
            new: None,
        });
    }

    fn visit(&mut self, scc: &ServerConfigChange) {
        self.set = if let Some(new) = &scc.new {
            scc.current.union(new).cloned().collect()
        } else {
            scc.current.clone()
        };
    }
}
