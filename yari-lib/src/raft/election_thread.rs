use crate::{
    Config,
    ElectionResult,
    Raft,
    Role::{self, *},
    //    SSEChannel,
    StateMachine,
};

use async_std::{
    future::timeout,
    sync::{Arc, Receiver, RwLock},
    task,
};

use std::time::Duration;

pub struct ElectionThread<SM: StateMachine> {
    raft_state: Arc<RwLock<Raft<SM>>>,
    //    channel: SSEChannel,
    rx: Receiver<()>,
}

#[derive(PartialEq, Debug)]
enum TimerState {
    TimedOut,
    Interrupted,
}

impl<SM: StateMachine> ElectionThread<SM> {
    async fn new(amr: Arc<RwLock<Raft<SM>>>) -> Self {
        let (rx, _channel) = {
            let cloned = amr.clone();
            let raft_state = cloned.read().await;
            (raft_state.interrupt_receiver(), ())
        };

        Self {
            raft_state: amr,
            rx,
            //            channel,
        }
    }

    async fn wait(&self, duration: Duration) -> TimerState {
        log::trace!("waiting {:?}", duration);
        //        self.log(&format!("waiting {:?}", duration)).await;
        match timeout(duration, self.rx.recv()).await {
            Ok(_) => TimerState::Interrupted,
            Err(_) => TimerState::TimedOut,
        }
    }

    async fn config(&self) -> Config {
        self.raft_state.clone().read().await.config
    }

    async fn leader_loop(&self) {
        let heartbeat_interval = self.config().await.heartbeat_interval();
        self.wait(heartbeat_interval).await;
        self.send_appends_or_heartbeats().await;
    }

    async fn solitary_loop(&self) {
        self.send_appends_or_heartbeats().await;
        self.wait_indefinitely().await;
    }

    async fn generate_election_timeout(&self) -> Duration {
        self.raft_state
            .clone()
            .read()
            .await
            .generate_election_timeout()
    }

    async fn log(&self, string: &str) {
        log::debug!("{}: {}", self.raft_state.read().await.id(), string);
        //        self.channel.log(string.to_owned()).await;
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

    async fn wait_indefinitely(&self) {
        self.log("waiting indefinitely").await;
        self.rx.recv().await.expect("receiving");
        self.log("interrupted!").await;
    }

    async fn role(&self) -> Role {
        self.raft_state.clone().read().await.role()
    }

    async fn start_election(&self) -> ElectionResult {
        self.raft_state.clone().write().await.start_election().await
    }

    async fn send_appends_or_heartbeats(&self) {
        self.raft_state
            .clone()
            .write()
            .await
            .send_appends_or_heartbeats()
            .await
    }

    async fn run(&self) {
        self.log("starting election thread").await;
        loop {
            //            self.channel.state(&*self.raft_state.read().await).await;
            match self.role().await {
                Leader => self.leader_loop().await,
                Solitary => self.solitary_loop().await,
                Follower | Candidate => self.follower_loop().await,
            }
        }
    }

    pub async fn spawn(state: Arc<RwLock<Raft<SM>>>) {
        Self::new(state).await.run().await
    }
}
