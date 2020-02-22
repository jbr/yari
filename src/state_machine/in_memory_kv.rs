use crate::{JsonMessage, JsonStateMachine};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Default, Debug)]
pub struct InMemoryKV {
    inner: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum KVMessage {
    Set(String, String),
    Get(String),
    Del(String),
    Keys(Option<String>),
}
impl JsonMessage for KVMessage {
    const VARIETY: &'static str = "KV";

    fn from_cli(input: Vec<String>) -> Option<Self> {
        let command: &str = input.get(0)?;

        match (command, input.len() - 1) {
            ("get", 1) => Some(Self::Get(input.get(1)?.clone())),
            ("set", 2) => Some(Self::Set(input.get(1)?.clone(), input.get(2)?.clone())),
            ("del", 1) => Some(Self::Del(input.get(1)?.clone())),
            ("keys", 0..=1) => Some(Self::Keys(input.get(1).cloned())),
            _ => None,
        }
    }
}

impl JsonStateMachine for InMemoryKV {
    type MessageType = KVMessage;

    fn do_apply(&mut self, m: &KVMessage) -> Option<String> {
        match m {
            KVMessage::Set(k, v) => {
                self.inner.insert(k.to_string(), v.to_string());
                None
            }
            KVMessage::Get(k) => self.inner.get(k).cloned(),
            KVMessage::Del(k) => {
                self.inner.remove(k);
                None
            }
            KVMessage::Keys(Some(k)) => Some(self
                .inner
                .keys()
                .filter(|key| key.starts_with(k))
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")),
            KVMessage::Keys(None) => Some(self.inner.keys().cloned().collect::<Vec<_>>().join("\n")),
        }
    }
}
