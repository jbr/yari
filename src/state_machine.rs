pub mod in_memory_kv;
pub mod noop_state_machine;
pub mod string_append_state_machine;
use anyhow::Result;
pub use noop_state_machine::*;
use serde::{de::DeserializeOwned, Serialize};
use std::any::Any;
use std::fmt::Debug;

pub trait Message: Serialize + DeserializeOwned + Send + Debug + Clone + Sync {
    fn from_cli(_v: Vec<String>) -> Result<Option<Self>> {
        Ok(None)
    }
}

pub trait StateMachine:
    Serialize + DeserializeOwned + Send + Debug + Sync + Default + 'static
{
    type MessageType: Message;

    fn visit(&mut self, _m: &Self::MessageType) {}

    fn apply(&mut self, _m: &Self::MessageType) -> Option<String> {
        None
    }

    fn cli(&self, v: Vec<String>) -> Result<Option<Self::MessageType>> {
        Self::MessageType::from_cli(v)
    }

    fn message_from(x: &dyn Any) -> Option<Self::MessageType> {
        x.downcast_ref::<Self::MessageType>().cloned()
    }
}

// impl<SM, MT> StateMachine for SM
// where
//     MT: JsonMessage,
//     SM: JsonStateMachine<MessageType = MT>,
// {
//     fn apply(&mut self, m: &Message) -> Option<String> {
//         if let Ok(Some(message)) = MT::from_message(&m) {
//             self.do_apply(&message)
//         } else {
//             None
//         }
//     }

//     fn visit(&mut self, m: &Message) {
//         if let Ok(Some(message)) = MT::from_message(m) {
//             self.do_visit(&message);
//         }
//     }

//     fn cli(&self, input: Vec<String>) -> Option<Message> {
//         MT::from_cli(input).and_then(|m| m.to_message().ok())
//     }
// }
