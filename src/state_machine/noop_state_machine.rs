use crate::StateMachine;

#[derive(Debug)]
pub struct NoopStateMachine;
impl StateMachine for NoopStateMachine {}
