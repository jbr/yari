use crate::{Message, Result, StateMachine};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct StringAppendStateMachine {
    state: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
pub struct StringAppendMessage(String);
impl Message for StringAppendMessage {
    fn from_cli(input: Vec<String>) -> Result<Option<Self>> {
        Ok(Some(Self(input.join(" "))))
    }
}

impl StateMachine for StringAppendStateMachine {
    type MessageType = StringAppendMessage;
    type ApplyResult = Vec<String>;

    fn apply(&mut self, m: &StringAppendMessage) -> Self::ApplyResult {
        self.state.push(m.0.clone());
        self.state.clone()
    }
}
