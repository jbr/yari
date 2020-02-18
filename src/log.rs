use crate::raft::{Index, Message, Term};
use crate::rpc::AppendRequest;
use serde::{Deserialize, Serialize};
mod log_entry;
pub use log_entry::LogEntry;

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Log {
    entries: Vec<LogEntry>,
}

impl Log {
    pub fn new() -> Self {
        Log { entries: vec![] }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains_term_at_index(
        &self,
        expected_term: Option<Term>,
        index: Option<Index>,
    ) -> bool {
        match (expected_term, index) {
            (None, None) => true,
            (Some(expected_term), Some(index)) => match self.get(index) {
                Some(LogEntry { term, .. }) => expected_term == *term,
                _ => false,
            },
            (_, _) => false,
        }
    }

    pub fn last_index_in_term(&self, term: Term) -> Option<Index> {
        self.entries.iter().rev().find_map(|entry| {
            if entry.term == term {
                Some(entry.index)
            } else {
                None
            }
        })
    }

    pub fn entries_starting_at(&self, index: Index) -> Option<&[LogEntry]> {
        self.last_index().and_then(|last_index| {
            if index <= last_index && index > 0 {
                Some(&self.entries[index - 1..])
            } else {
                None
            }
        })
    }

    pub fn previous_entry_to(&self, index: Index) -> Option<&LogEntry> {
        if index >= 1 {
            self.get(index - 1)
        } else {
            None
        }
    }

    pub fn get(&self, index: Index) -> Option<&LogEntry> {
        if index >= 1 {
            self.entries.get(index - 1)
        } else {
            None
        }
    }

    pub fn truncate(&mut self, index: Index) {
        self.entries.truncate(index - 1);
    }

    pub fn last_index(&self) -> Option<Index> {
        self.entries.last().map(|e| e.index)
    }

    pub fn next_index(&self) -> Index {
        self.last_index().map_or(1, |index| index + 1)
    }

    pub fn last_term(&self) -> Option<Term> {
        self.entries.last().map(|e| e.term)
    }

    pub fn first_conflicting_index(&self, new_entries: &Option<Vec<LogEntry>>) -> Option<Index> {
        new_entries.as_ref().and_then(|messages| {
            messages
                .iter()
                .find(|LogEntry { term, index, .. }| match self.get(*index) {
                    Some(entry @ LogEntry { .. }) => entry.term != *term,
                    None => false,
                })
                .map(|LogEntry { index, .. }| *index)
        })
    }

    pub fn append_new_entries_not_in_log(&mut self, new_entries: Option<Vec<LogEntry>>) {
        if let Some(entries) = new_entries {
            let current_last_index = self.last_index().unwrap_or(0);
            let new_entries_not_in_log = entries
                .into_iter()
                .filter(|LogEntry { index, .. }| *index > current_last_index);
            self.entries.extend(new_entries_not_in_log);
        }
    }

    pub fn client_append(&mut self, term: Term, message: Option<&Message>) -> &LogEntry {
        let index = self.next_index();
        let log_entry = LogEntry {
            message: message.cloned(),
            term,
            index,
            ..Default::default()
        };

        self.entries.push(log_entry);

        &self.entries.last().unwrap()
    }

    pub fn append(&mut self, request: AppendRequest<'_>) -> bool {
        if self.contains_term_at_index(request.previous_log_term, request.previous_log_index) {
            if let Some(index) = self.first_conflicting_index(&request.entries) {
                self.truncate(index);
            }

            self.append_new_entries_not_in_log(request.entries);
            true
        } else {
            false
        }
    }
}
