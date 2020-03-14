use crate::{StateMachine,Message};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct NoopStateMachine;
impl Message for () {}
impl StateMachine for NoopStateMachine {
    type MessageType = ();
}
