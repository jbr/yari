use crate::raft::{DynBoxedResult, RaftState};
use bincode::{deserialize_from, serialize_into};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::{env, path::PathBuf};
use url::Url;

pub fn path(id: &Url) -> DynBoxedResult<PathBuf> {
    let mut path = env::current_dir()?;
    let name = id
        .port()
        .map(|port| port.to_string())
        .unwrap_or_else(|| id.host_str().unwrap().to_owned());

    path.push(format!("{}.yari", name));

    Ok(path)
}

#[derive(Serialize)]
enum VersionedSaveFileSerialize<'a> {
    V0(&'a RaftState),
}

#[derive(Deserialize)]
enum VersionedSaveFileDeserialize {
    V0(RaftState),
}

impl VersionedSaveFileDeserialize {
    fn into_current_raft(self) -> Result<RaftState, Box<dyn std::error::Error>> {
        match self {
            Self::V0(r) => Ok(r),
        }
    }
}

pub fn persist(raft: &RaftState) -> DynBoxedResult {
    let path = &raft.statefile_path;
    let file = OpenOptions::new().write(true).create(true).open(path)?;
    let versioned = VersionedSaveFileSerialize::V0(&raft);

    serialize_into(file, &versioned)?;

    Ok(())
}

pub fn load(path: &PathBuf) -> DynBoxedResult<RaftState> {
    let file = File::open(path)?;
    let save_file: VersionedSaveFileDeserialize = deserialize_from(file)?;
    let raft = save_file.into_current_raft()?;
    Ok(raft)
}
