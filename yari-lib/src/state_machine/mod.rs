pub mod in_memory_kv;
pub mod noop_state_machine;
pub mod string_append_state_machine;
use crate::Result;
pub use noop_state_machine::*;
use serde::{de::DeserializeOwned, Serialize};
use std::{any::Any, fmt::Debug};

pub trait Message: Sized + Serialize + DeserializeOwned + Send + Debug + Clone + Sync {
    fn from_cli(_: Vec<String>) -> Result<Option<Self>> {
        Ok(None)
    }
}

pub trait StateMachine:
    Serialize + DeserializeOwned + Send + Debug + Sync + Default + 'static
{
    type MessageType: Message;
    type ApplyResult: Default + Serialize + DeserializeOwned + Clone + Send + Debug + Sync + 'static;

    fn visit(&mut self, _m: &Self::MessageType) {}

    fn apply(&mut self, _m: &Self::MessageType) -> Self::ApplyResult;

    fn cli(&self, v: Vec<String>) -> Result<Option<Self::MessageType>> {
        Self::MessageType::from_cli(v)
    }

    fn message_from(x: &dyn Any) -> Option<Self::MessageType> {
        x.downcast_ref::<Self::MessageType>().cloned()
    }
}
