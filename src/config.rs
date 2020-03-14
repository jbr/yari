use anyhow::Result;
use serde::Deserialize;
use std::fs::File;
use std::io::prelude::*;
use std::ops::Range;
use std::path::PathBuf;
use std::time::Duration;

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

impl Into<Range<u64>> for TimeoutConfig {
    fn into(self) -> Range<u64> {
        self.min..self.max
    }
}

#[derive(Debug, Deserialize, Clone, Default, Copy)]
pub struct Config {
    timeout: TimeoutConfig,
    heartbeat_interval: Option<u64>,
}

impl Config {
    pub fn parse(path: PathBuf) -> Result<Self> {
        let mut file = File::open(path).unwrap();
        let mut buf = String::new();
        file.read_to_string(&mut buf)?;
        let config: Config = toml::from_str(&buf)?;
        Ok(config)
    }

    pub fn timeout(&self) -> Range<u64> {
        self.timeout.into()
    }

    pub fn heartbeat_interval(&self) -> Duration {
        Duration::from_millis(self.heartbeat_interval.unwrap_or(self.timeout.min / 2))
    }
}
