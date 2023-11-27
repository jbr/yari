use crate::{
    raft::{Index, Term},
    TermIndex,
};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{Debug, Formatter, Result},
    hash::{Hash, Hasher},
};

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct LogEntry<MessageType> {
    pub index: Index,
    pub term: Term,
    pub message: MessageType,
}

impl<MT> Hash for LogEntry<MT> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.term.hash(state);
    }
}

impl<MT> PartialEq for LogEntry<MT> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.term == other.term
    }
}

impl<MT> From<&LogEntry<MT>> for TermIndex {
    fn from(value: &LogEntry<MT>) -> Self {
        Self(value.term, value.index)
    }
}

impl<MT> Eq for LogEntry<MT> {}

impl<MessageType: Debug> Debug for LogEntry<MessageType> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{},{}:{:?}", self.term, self.index, self.message)
    }
}
