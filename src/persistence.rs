use crate::raft::{EphemeralState, Raft, StateMachine};
use anyhow::{Context, Result};
use bincode::{deserialize_from, serialize_into};
use std::fs::{File, OpenOptions};
use std::{env, path::PathBuf};
use url::Url;

pub fn path(id: &Url) -> Result<PathBuf> {
    let mut path = env::current_dir()?;
    let name = id
        .port()
        .map(|port| port.to_string())
        .unwrap_or_else(|| id.host_str().unwrap().to_owned());

    path.push(format!("{}.yari", name));

    Ok(path)
}

pub fn load_or_default<SM: StateMachine>(eph: EphemeralState<SM>) -> Raft<SM> {
    load::<SM>(&eph.statefile_path)
        .unwrap_or_default()
        .with_ephemeral_state(eph)
}

pub fn persist<SM: StateMachine>(raft: &Raft<SM>) -> Result<()> {
    let path = &raft.statefile_path;
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(path)
        .with_context(|| format!("failed to open {:?}", path))?;

    let cloned = raft.clone();

    serialize_into(file, cloned)?;

    Ok(())
}

pub fn load<SM: StateMachine>(path: &PathBuf) -> Result<Raft<SM>> {
    let file = File::open(path)?;
    let raft: Raft<SM> = deserialize_from(file)?;
    Ok(raft)
}
