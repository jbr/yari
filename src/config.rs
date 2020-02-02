use crate::raft::UnknownResult;
use serde::Deserialize;
use std::fs::File;
use std::io::prelude::*;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

type ServerSpec = SocketAddr;

#[derive(Debug, Deserialize, Clone)]
struct TimeoutConfig {
    min: u64,
    max: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub servers: Vec<ServerSpec>,
    timeout: TimeoutConfig,
    heartbeat_interval: Option<u64>,
}

impl Config {
    pub fn parse(path: PathBuf) -> UnknownResult<Self> {
        let mut file = File::open(path).unwrap();
        let mut buf = String::new();
        file.read_to_string(&mut buf)?;
        let config: Config = toml::from_str(&buf)?;
        Ok(config)
    }

    pub fn timeout(&self) -> std::ops::Range<u64> {
        self.timeout.min..self.timeout.max
    }

    pub fn heartbeat_interval(&self) -> Duration {
        Duration::from_millis(self.heartbeat_interval.unwrap_or(self.timeout.min / 2))
    }
}
