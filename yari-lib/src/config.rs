use crate::Result;
use serde::Deserialize;
use std::{fs::File, io::prelude::*, ops::Range, path::PathBuf, time::Duration};

#[derive(Debug, Deserialize, Clone, Copy)]
struct TimeoutConfig {
    min: u64,
    max: u64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self { min: 150, max: 300 }
    }
}

impl From<TimeoutConfig> for Range<u64> {
    fn from(val: TimeoutConfig) -> Self {
        val.min..val.max
    }
}

#[derive(Debug, Deserialize, Clone, Default, Copy)]
pub struct Config {
    timeout: TimeoutConfig,
    heartbeat_interval: Option<u64>,
}

impl Config {
    pub fn parse(path: PathBuf) -> Result<Self> {
        let mut file = File::open(path)?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)?;
        Ok(toml::from_str(&buf)?)
    }

    pub fn timeout(&self) -> Range<u64> {
        self.timeout.into()
    }

    pub fn heartbeat_interval(&self) -> Duration {
        Duration::from_millis(self.heartbeat_interval.unwrap_or(self.timeout.min / 2))
    }
}
