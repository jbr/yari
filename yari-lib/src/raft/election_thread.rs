use crate::{
    Config, ElectionResult, RaftState,
    Role::{self, *},
    StateMachine,
};

use async_io::Timer;
use async_lock::RwLock;
use futures_lite::FutureExt;
//use futures_util::FutureExt;
use std::{sync::Arc, time::Duration};

pub struct ElectionThread<SM, MT, AR> {
    raft_state: Arc<RwLock<RaftState<SM, MT, AR>>>,
    interrupt_channel: super::Receiver<()>,
}

#[derive(PartialEq, Debug)]
enum TimerState {
    TimedOut,
    Interrupted,
}

type ArcRaft<SM> = Arc<
    RwLock<RaftState<SM, <SM as StateMachine>::MessageType, <SM as StateMachine>::ApplyResult>>,
>;

impl<SM: StateMachine> ElectionThread<SM, SM::MessageType, SM::ApplyResult> {
    async fn new(raft_state: ArcRaft<SM>) -> Self {
        let interrupt_channel = raft_state.write().await.take_interrupt_channel().unwrap();
        Self {
            raft_state,
            interrupt_channel,
        }
    }

    async fn wait(&self, duration: Duration) -> TimerState {
        log::trace!("waiting {:?}", duration);
        let receiver = &self.interrupt_channel;
        let recv = async {
            receiver.recv().await.unwrap();
            TimerState::Interrupted
        };
        let timer = async {
            Timer::after(duration).await;
            TimerState::TimedOut
        };
        recv.or(timer).await
    }

    async fn config(&self) -> Config {
        self.raft_state.read().await.config
    }

    async fn wait_heartbeat_interval(&self) {
        let heartbeat_interval = self.config().await.heartbeat_interval();
        self.wait(heartbeat_interval).await;
    }

    async fn leader_loop(&self) {
        self.wait_heartbeat_interval().await;
        self.send_appends_or_heartbeats().await;
    }

    // async fn solitary_loop(&self) {
    //     self.send_appends_or_heartbeats().await;
    //     self.wait_heartbeat_interval().await;
    // }

    async fn generate_election_timeout(&self) -> Duration {
        self.raft_state
            .clone()
            .read()
            .await
            .generate_election_timeout()
    }

    async fn log(&self, string: &str) {
        log::debug!("{}: {}", self.raft_state.read().await.id(), string);
    }

    async fn follower_loop(&self) {
        let duration = self.generate_election_timeout().await;
        if let TimerState::TimedOut = self.wait(duration).await {
            match self.start_election().await {
                ElectionResult::Elected => self.log("successfully got elected").await,
                ElectionResult::FailedQuorum => self.log("failed to get elected").await,
                ElectionResult::Ineligible => self.log("was ineligible to get elected").await,
            }
        }
    }

    async fn role(&self) -> Role {
        self.raft_state.clone().read().await.role()
    }

    async fn start_election(&self) -> ElectionResult {
        self.raft_state.clone().write().await.start_election().await
    }

    async fn send_appends_or_heartbeats(&self) {
        self.raft_state
            .write()
            .await
            .send_appends_or_heartbeats()
            .await
    }

    async fn run(&self) {
        self.log("starting election thread").await;
        loop {
            match self.role().await {
                Leader | Solitary => self.leader_loop().await,
                Follower | Candidate => self.follower_loop().await,
            }
        }
    }

    pub async fn spawn(state: ArcRaft<SM>) {
        Self::new(state).await.run().await
    }
}
