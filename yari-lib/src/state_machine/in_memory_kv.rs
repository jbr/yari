use crate::{Message, StateMachine};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tide::Result;

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct InMemoryKV {
    inner: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum KVMessage {
    Set(String, String),
    Get(String),
    Del(String),
    Keys(Option<String>),
}

impl Message for KVMessage {
    fn from_cli(input: Vec<String>) -> Result<Option<Self>> {
        let command: &str = input
            .get(0)
            .ok_or(tide::http::format_err!("no command provided"))?;

        match (command, input.len() - 1) {
            ("get", 1) => Ok(Some(Self::Get(input.get(1).unwrap().clone()))),
            ("set", 2) => Ok(Some(Self::Set(input.get(1).unwrap().clone(), input.get(2).unwrap().clone()))),
            ("del", 1) => Ok(Some(Self::Del(input.get(1).unwrap().clone()))),
            ("keys", 0..=1) => Ok(Some(Self::Keys(input.get(1).cloned()))),
            (command, arity) => Err(tide::http::format_err!("{} with {} arguments not recognized as a command.\n\
                                             try get @key), set @key, del @key, or keys (optional @prefix)", command, arity)),
        }
    }
}

impl StateMachine for InMemoryKV {
    type MessageType = KVMessage;

    fn apply(&mut self, m: &KVMessage) -> Option<String> {
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

            KVMessage::Keys(Some(k)) => Some(
                self.inner
                    .keys()
                    .filter(|key| key.starts_with(k))
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),

            KVMessage::Keys(None) => {
                Some(self.inner.keys().cloned().collect::<Vec<_>>().join("\n"))
            }
        }
    }
}
