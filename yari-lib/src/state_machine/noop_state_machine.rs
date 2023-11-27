use crate::{Message, StateMachine};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct NoopStateMachine;
impl Message for () {}
impl StateMachine for NoopStateMachine {
    type MessageType = ();
    type ApplyResult = ();

    fn apply(&mut self, _m: &Self::MessageType) -> Self::ApplyResult {}
}
