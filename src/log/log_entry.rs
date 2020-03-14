use crate::raft::{Index, Term};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};


#[derive(Serialize, Deserialize, Default)]
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

impl<MT> Eq for LogEntry<MT> {}

impl<T: Clone> Clone for LogEntry<T> {
    fn clone(&self) -> Self {
        Self {
            index: self.index.clone(),
            term: self.term.clone(),
            message: self.message.clone(),
        }
    }
}

impl<MessageType: std::fmt::Debug> std::fmt::Debug for LogEntry<MessageType> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{}:{:?}", self.term, self.index, self.message)
    }
}
