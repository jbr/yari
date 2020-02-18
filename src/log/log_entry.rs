use crate::raft::{Index, Message, Term};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Condvar, Mutex};

type LogEntryResult = Arc<(Mutex<(bool, Option<String>)>, Condvar)>;

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct LogEntry {
    pub index: Index,
    pub term: Term,
    pub message: Option<Message>,
    #[serde(skip)]
    pub apply_result: LogEntryResult,
}

impl LogEntry {
    pub fn block_until_committed(&self) -> Option<String> {
        eprintln!("blocking until committed: {:?}", &self);
        let (mutex, condvar) = &*self.apply_result;
        let mut result = mutex.lock().unwrap();
        while !result.0 {
            result = condvar.wait(result).unwrap();
        }
        result.1.clone()
    }

    pub fn store_result(&self, result: Option<String>) {
        let (mutex, condvar) = &*self.apply_result;
        *mutex.lock().unwrap() = (true, result);
        eprintln!("storing result: {:?}", &self);
        condvar.notify_one();
    }
}

impl std::fmt::Debug for LogEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(message) = &self.message {
            let (mutex, _condvar) = &*self.apply_result;
            let result = mutex.lock().unwrap();
            write!(f, "{},{}:{:?}:{:?}", self.term, self.index, message, result)
        } else {
            write!(f, "{},{} noop", self.term, self.index)
        }
    }
}
