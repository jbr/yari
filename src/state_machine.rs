mod noop_state_machine;
mod string_append_state_machine;
use crate::raft::Message;
pub use noop_state_machine::*;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;
pub use string_append_state_machine::*;

pub trait JsonMessage: Debug + Serialize + DeserializeOwned {
    const VARIETY: &'static str;

    fn from_message(m: &Message) -> Result<Option<Self>, serde_json::Error> {
        if m.variety == Self::VARIETY {
            let result: Self = serde_json::from_str(&m.content)?;
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    fn to_message(&self) -> Result<Message, serde_json::Error> {
        let content = serde_json::to_string(self)?;
        Ok(Message {
            variety: Self::VARIETY.to_string(),
            content,
        })
    }

    fn from_cli(_input: Vec<String>) -> Option<Self> {
        None
    }
}

pub trait JsonStateMachine: Send + Debug + Sync + 'static {
    type MessageType: JsonMessage;

    fn do_apply(&mut self, _: &Self::MessageType) {}
    fn do_visit(&mut self, _: &Self::MessageType) {}
}

pub trait StateMachine: Send + Debug + Sync + 'static {
    fn visit(&mut self, _m: &Message) {}
    fn apply(&mut self, _m: &Message) {}
    fn cli(&self, _input: Vec<String>) -> Option<Message> {
        None
    }
}

impl Default for Box<dyn StateMachine> {
    fn default() -> Self {
        Box::new(NoopStateMachine)
    }
}

impl<SM, MT> StateMachine for SM
where
    MT: JsonMessage,
    SM: JsonStateMachine<MessageType = MT>,
{
    fn apply(&mut self, m: &Message) {
        if let Ok(Some(message)) = MT::from_message(&m) {
            self.do_apply(&message);
        }
    }

    fn visit(&mut self, m: &Message) {
        if let Ok(Some(message)) = MT::from_message(m) {
            self.do_visit(&message);
        }
    }

    fn cli(&self, input: Vec<String>) -> Option<Message> {
        MT::from_cli(input).and_then(|m| m.to_message().ok())
    }
}
