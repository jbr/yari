use crate::{
    raft::{EphemeralState, StateMachine},
    RaftState, Result,
};
use async_fs::File;
use futures_lite::AsyncReadExt;
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

pub async fn load_or_default<SM: StateMachine>(
    eph: EphemeralState<SM>,
) -> RaftState<SM, SM::MessageType, SM::ApplyResult> {
    load::<SM>(&eph.statefile_path)
        .await
        .unwrap_or_default()
        .with_ephemeral_state(eph)
}

pub async fn persist<SM: StateMachine>(
    _raft: &RaftState<SM, SM::MessageType, SM::ApplyResult>,
) -> Result<()> {
    // let path = raft.statefile_path();
    // let mut file = OpenOptions::new()
    //     .write(true)
    //     .create(true)
    //     .open(path)
    //     .await?;

    // let data = bincode::serialize(&raft).unwrap();
    // file.write_all(&data).await?;

    Ok(())
}

pub async fn load<SM: StateMachine>(
    path: &PathBuf,
) -> Result<RaftState<SM, SM::MessageType, SM::ApplyResult>> {
    let mut file = File::open(path).await?;
    let mut string_contents = vec![];
    file.read_to_end(&mut string_contents).await?;
    Ok(bincode::deserialize(&string_contents)?)
}
