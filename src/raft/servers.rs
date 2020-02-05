use crate::{JsonMessage, JsonStateMachine, Message};
use serde::{Deserialize, Serialize};
use std::collections::{hash_set::IntoIter, HashSet};
use std::fmt::{Debug, Formatter, Result as FmtResult};

#[derive(Default, Debug)]
pub struct Servers {
    set: HashSet<String>,
    pub new_config: Option<Message>,
}

#[derive(Serialize, Deserialize)]
pub struct ServerConfigChange {
    current: HashSet<String>,
    new: Option<HashSet<String>>,
}

impl JsonMessage for ServerConfigChange {
    const VARIETY: &'static str = "SCC";
}

impl Debug for ServerConfigChange {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{:?}->{:?}", self.current, self.new)
    }
}

impl Servers {
    pub fn member_add(&self, id: &str) -> Option<Message> {
        let mut new = self.set.clone();
        new.insert(id.into());

        ServerConfigChange {
            current: self.set.clone(),
            new: Some(new),
        }
        .to_message()
        .ok()
    }

    pub fn member_remove(&self, id: &str) -> Option<Message> {
        let mut new = self.set.clone();
        new.remove(id);

        ServerConfigChange {
            current: self.set.clone(),
            new: Some(new),
        }
        .to_message()
        .ok()
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

impl JsonStateMachine for Servers {
    type MessageType = ServerConfigChange;

    fn do_apply(&mut self, scc: &ServerConfigChange) {
        self.new_config = if let Some(new) = &scc.new {
            ServerConfigChange {
                current: new.clone(),
                new: None,
            }
            .to_message()
            .ok()
        } else {
            None
        }
    }

    fn do_visit(&mut self, scc: &ServerConfigChange) {
        self.set = if let Some(new) = &scc.new {
            scc.current.clone().union(&new).cloned().collect()
        } else {
            scc.current.clone()
        };
    }
}
