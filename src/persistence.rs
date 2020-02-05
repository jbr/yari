use crate::raft::{RaftState, UnknownResult};
use serde::{Deserialize, Serialize};
use std::{env, path::PathBuf};
use url::Url;

pub fn path(id: &Url) -> UnknownResult<PathBuf> {
    let mut path = env::current_dir()?;
    let name = id
        .port()
        .map(|port| port.to_string())
        .unwrap_or_else(|| id.host_str().unwrap().to_owned());

    path.push(format!("{}.json", name));

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
    fn to_current_raft(self) -> Result<RaftState, Box<dyn std::error::Error>> {
        match self {
            Self::V0(r) => Ok(r),
        }
    }
}

pub fn persist(raft: &RaftState) -> UnknownResult {
    let path = &raft.statefile_path;

    let versioned = VersionedSaveFileSerialize::V0(&raft);

    if let Ok(string) = serde_json::to_string(&versioned) {
        std::fs::write(path, string)?
    }

    Ok(())
}

pub fn load(path: &PathBuf) -> UnknownResult<RaftState> {
    let save_file: VersionedSaveFileDeserialize =
        serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let raft = save_file.to_current_raft()?;
    dbg!(&raft);
    Ok(raft)
}
