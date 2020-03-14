use crate::raft::{Config, ElectionResult, Raft, Role, StateMachine};
use async_std::{
    future::timeout,
    sync::{channel, Arc, Mutex, Receiver},
    task,
};
use std::time::Duration;

pub struct ElectionThread<SM: StateMachine> {
    raft_state: Arc<Mutex<Raft<SM>>>,
    rx: Receiver<()>,
}

#[derive(PartialEq, Debug)]
enum TimerState {
    TimedOut,
    Interrupted,
}

impl<SM: StateMachine> ElectionThread<SM> {
    async fn new(amr: Arc<Mutex<Raft<SM>>>) -> Self {
        let rx = {
            let cloned = amr.clone();
            let mut raft_state = cloned.lock().await;
            let (tx, rx) = channel::<()>(1);
            raft_state.update_timer = Some(tx);
            rx
        };

        Self {
            raft_state: amr,
            rx,
        }
    }

    async fn wait(&self, duration: Duration) -> TimerState {
        match timeout(duration, self.rx.recv()).await {
            Ok(_) => TimerState::Interrupted,
            Err(_) => TimerState::TimedOut,
        }
    }

    async fn config(&self) -> Config {
        self.raft_state.clone().lock().await.config
    }

    async fn leader_loop(&self) {
        let heartbeat_interval = self.config().await.heartbeat_interval();
        self.wait(heartbeat_interval).await;
        self.send_appends_or_heartbeats().await;
    }

    async fn generate_election_timeout(&self) -> Duration {
        self.raft_state
            .clone()
            .lock()
            .await
            .generate_election_timeout()
    }

    async fn follower_loop(&self) {
        let duration = self.generate_election_timeout().await;
        if let TimerState::TimedOut = self.wait(duration).await {
            match self.start_election().await {
                ElectionResult::Elected => println!("successfully got elected"),
                ElectionResult::FailedQuorum => println!("failed to get elected"),
                ElectionResult::Ineligible => (),
            }
        }
    }

    async fn role(&self) -> Role {
        self.raft_state.clone().lock().await.role()
    }

    async fn start_election(&self) -> ElectionResult {
        self.raft_state.clone().lock().await.start_election().await
    }

    async fn send_appends_or_heartbeats(&self) {
        self.raft_state
            .clone()
            .lock()
            .await
            .send_appends_or_heartbeats()
            .await
    }

    async fn run(&self) {
        println!("starting election thread");
        loop {
            match self.role().await {
                Role::Leader => self.leader_loop().await,
                Role::Follower | Role::Candidate => self.follower_loop().await,
            }
        }
    }

    pub async fn spawn(state: &Arc<Mutex<Raft<SM>>>) {
        let state = state.clone();
        task::spawn(async { Self::new(state).await.run().await });
    }
}
