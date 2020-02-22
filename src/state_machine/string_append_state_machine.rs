use crate::{JsonMessage, JsonStateMachine};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug)]
pub struct StringAppendStateMachine {
    state: String,
}

impl StringAppendStateMachine {
    const DIVIDER: &'static str = "\n";
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StringAppendMessage(String);
impl JsonMessage for StringAppendMessage {
    const VARIETY: &'static str = "StringAppendMessage";

    fn from_cli(input: Vec<String>) -> Option<Self> {
        Some(Self(input.join(" ")))
    }
}

impl JsonStateMachine for StringAppendStateMachine {
    type MessageType = StringAppendMessage;

    fn do_apply(&mut self, m: &StringAppendMessage) -> Option<String> {
        self.state.push_str(&m.0);
        self.state.push_str(Self::DIVIDER);
        Some(self.state.clone())
    }
}
