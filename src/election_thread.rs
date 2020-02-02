use crate::config::YariConfig;
use crate::raft::{RaftState, Role, UnknownResult};
use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use delegate::delegate;

pub struct ElectionThread {
    raft_state: Arc<Mutex<RaftState>>,
    rx: Option<Receiver<()>>,
}
#[derive(PartialEq)]
enum TimerState {
    TimedOut,
    Interrupted,
}

impl ElectionThread {
    fn new(amr: Arc<Mutex<RaftState>>) -> Self {
        Self {
            raft_state: amr,
            rx: None,
        }
    }

    delegate! {
        to self.raft_state.clone().lock().unwrap() {
            fn start_election(&self) -> UnknownResult<bool>;
            fn role(&self) -> Role;
            fn send_appends_or_heartbeats(&self);
        }
    }

    fn establish_channel(&self) -> Receiver<()> {
        let mut raft_state = self.raft_state.lock().unwrap();
        let (tx, rx) = channel::<()>();
        raft_state.update_timer = Some(tx);
        rx
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
        self.send_appends_or_heartbeats();
    }

    fn follower_loop(&self) {
        let mut rng = thread_rng();

        let distribution: Uniform<u64> = self.config().timeout().into();
        let duration = Duration::from_millis(distribution.sample(&mut rng));
        if let TimerState::TimedOut = self.wait(duration) {
            match self.start_election() {
                Ok(true) => println!("successfully got elected"),
                Ok(false) => println!("failed to get elected"),
                Err(e) => {
                    println!("error in attempt to get elected: {:?}", e);
                }
            }
        }
    }

    fn spawn_on_self(mut self) {
        self.rx = Some(self.establish_channel());
        std::thread::spawn(move || loop {
            let role = self.role();
            match role {
                Role::Leader => self.leader_loop(),
                Role::Follower | Role::Candidate => self.follower_loop(),
            }
        });
    }

    pub fn spawn(state: &Arc<Mutex<RaftState>>) {
        Self::new(state.clone()).spawn_on_self()
    }
}


