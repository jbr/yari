use crate::{Message, StateMachine};
use serde::{Deserialize, Serialize};
use tide::Result;

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct StringAppendStateMachine {
    state: String,
}

impl StringAppendStateMachine {
    const DIVIDER: &'static str = "\n";
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StringAppendMessage(String);
impl Message for StringAppendMessage {
    fn from_cli(input: Vec<String>) -> Result<Option<Self>> {
        Ok(Some(Self(input.join(" "))))
    }
}

impl StateMachine for StringAppendStateMachine {
    type MessageType = StringAppendMessage;

    fn apply(&mut self, m: &StringAppendMessage) -> Option<String> {
        self.state.push_str(&m.0);
        self.state.push_str(Self::DIVIDER);
        Some(self.state.clone())
    }
}
