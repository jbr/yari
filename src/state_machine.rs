use crate::log::Message;
use crate::raft::UnknownResult;
use serde::{Deserialize, Serialize};

pub trait StateMachine<'a, Message, ApplyResult> {
    fn apply(&'a mut self, m: Option<&Message>) -> UnknownResult<ApplyResult>;
    fn read(&self) -> &str;
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct NoopStateMachine;

impl StateMachine<'_, String, &str> for NoopStateMachine {
    fn apply(&mut self, m: Option<&Message>) -> UnknownResult<&'static str> {
        println!("{:?}", m);
        Ok("ok")
    }

    fn read(&self) -> &str {
        "noop"
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct StringAppendStateMachine {
    #[serde(skip_deserializing)]
    state: String,
}

impl StringAppendStateMachine {
    const DIVIDER: &'static str = "\n";
}

impl StateMachine<'_, Message, String> for StringAppendStateMachine {
    fn apply(&mut self, m: Option<&Message>) -> UnknownResult<String> {
        if let Some(m) = m {
            println!("applying {:#?}", m);
            self.state.push_str(m);
            self.state.push_str(Self::DIVIDER);
        }
        Ok(self.state.clone())
    }

    fn read(&self) -> &str {
        &self.state
    }
}
