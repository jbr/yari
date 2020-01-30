use crate::config::YariConfig;
use crate::raft::{RaftState, Role, UnknownResult};
use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct Leader {
    raft_state: Arc<Mutex<RaftState>>,
    rx: Option<Receiver<()>>,
}
#[derive(PartialEq)]
enum TimerState {
    TimedOut,
    Interrupted,
}

impl Leader {
    pub fn new(amr: Arc<Mutex<RaftState>>) -> Self {
        Self {
            raft_state: amr,
            rx: None,
        }
    }

    fn establish_channel(&self) -> Receiver<()> {
        let mut raft_state = self.raft_state.lock().unwrap();
        let (tx, rx) = channel::<()>();
        raft_state.update_timer = Some(tx);
        rx
    }

    fn role(&self) -> Role {
        self.raft_state.clone().lock().unwrap().role()
    }

    fn become_candidate(&self) -> UnknownResult<bool> {
        self.raft_state.clone().lock().unwrap().start_election()
    }

    fn send_appends(&self) {
        self.raft_state
            .clone()
            .lock()
            .unwrap()
            .send_appends_or_heartbeats()
    }

    fn wait(&self, duration: Duration) -> TimerState {
        match self.rx.as_ref().unwrap().recv_timeout(duration) {
            Ok(_) => TimerState::Interrupted,
            Err(_) => TimerState::TimedOut,
        }
    }

    fn config(&self) -> YariConfig {
        self.raft_state
            .clone()
            .lock()
            .unwrap()
            .config
            .as_ref()
            .unwrap()
            .clone()
    }

    fn leader_loop(&self) {
        let heartbeat_interval = self.config().heartbeat_interval();
        self.wait(heartbeat_interval);
        self.send_appends();
    }

    fn follower_loop(&self) {
        let mut rng = thread_rng();

        let distribution: Uniform<u64> = self.config().timeout().into();
        let duration = Duration::from_millis(distribution.sample(&mut rng));
        if let TimerState::TimedOut = self.wait(duration) {
            match self.become_candidate() {
                Ok(true) => println!("successfully got elected"),
                Ok(false) => println!("failed to get elected"),
                Err(e) => {
                    println!("error in attempt to get elected: {:?}", e);
                }
            }
        }
    }

    pub fn spawn(mut self) {
        self.rx = Some(self.establish_channel());
        std::thread::spawn(move || loop {
            let role = self.role();
            match role {
                Role::Leader => self.leader_loop(),
                Role::Follower | Role::Candidate => self.follower_loop(),
            }
        });
    }
}


