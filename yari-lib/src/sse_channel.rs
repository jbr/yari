use broadcaster::BroadcastChannel;
use crate::RaftState;
use serde::Serialize;
use crate::eventstream::{Event, EventStream};

#[derive(Clone, Debug)]
pub struct SSEChannel(BroadcastChannel<SSEvent>);

impl Default for SSEChannel {
    fn default() -> Self {
        Self(BroadcastChannel::new())
    }
}

impl SSEChannel {
    pub async fn send(&self, item: SSEvent) -> anyhow::Result<()> {
        Ok(self.0.send(&item).await?)
    }

    pub async fn recv(&mut self) -> Option<SSEvent> {
        self.0.recv().await
    }

    pub fn into_inner(self) -> BroadcastChannel<SSEvent> {
        self.0
    }

    pub fn into_response(self) -> tide::Response {
        self.into_inner().into_response()
    }

    pub async fn state<X, Y: Serialize>(&self, raft: &RaftState<X, Y>) {
        self.send(SSEvent::State(serde_json::to_string(raft).unwrap()))
            .await
            .expect("channel.state")
    }

    pub async fn log(&self, message: String) {
        self.send(SSEvent::Log(message)).await.expect("channel.log")
    }
}

#[derive(Debug, Clone)]
pub enum SSEvent {
    State(String),
    Log(String),
}

impl Event for SSEvent {
    fn name(&self) -> &str {
        match self {
            SSEvent::State(_) => "state".into(),
            SSEvent::Log(_) => "log".into(),
        }
    }

    fn data(&self) -> &[u8] {
        match self {
            SSEvent::State(s) => s.as_bytes(),
            SSEvent::Log(l) => l.as_bytes()
        }
    }

    fn id(&self) -> Option<&str> {
        None
    }
}
