use crate::raft::{EphemeralState, Raft, StateMachine};
use async_std::fs::{File, OpenOptions};
use async_std::prelude::*;
use std::{env, path::PathBuf};
use tide::Result;
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

pub async fn load_or_default<SM: StateMachine>(eph: EphemeralState<SM>) -> Raft<SM> {
    load::<SM>(&eph.statefile_path)
        .await
        .unwrap_or_default()
        .with_ephemeral_state(eph)
}

pub async fn persist<SM: StateMachine>(raft: &Raft<SM>) -> Result<()> {
    let path = raft.statefile_path();
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(path)
        .await?;

    let data = bincode::serialize(&raft)?;
    file.write_all(&data).await?;

    Ok(())
}

pub async fn load<SM: StateMachine>(path: &PathBuf) -> Result<Raft<SM>> {
    let mut file = File::open(path).await?;
    let mut string_contents = vec![];
    file.read_to_end(&mut string_contents).await?;
    let raft: Raft<SM> = bincode::deserialize(&string_contents)?;
    Ok(raft)
}
