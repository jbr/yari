use async_std::sync::{Arc, RwLock};
use async_std::{prelude::*, task};
use futures_util::stream::FuturesUnordered;
use yari::server;

const PORTS: &[u64] = &[8000, 8001, 8002, 8003, 8004];
fn main() -> tide::Result<()> {
    env_logger::init();

    task::block_on(async {
        let mut rafts: Vec<yari::raft::Raft<yari::state_machine::in_memory_kv::InMemoryKV>> = PORTS
            .iter()
            .map(|port| {
                yari::persistence::load_or_default(yari::raft::EphemeralState {
                    id: format!("http://127.0.0.1:{}", port),
                    statefile_path: std::path::PathBuf::from(format!("./state/{}.yari", port)),
                    ..Default::default()
                })
            })
            .collect::<FuturesUnordered<_>>()
            .collect::<Vec<_>>()
            .await;

        if let [leader, followers @ ..] = &mut rafts[..] {
            log::info!("bootstrapping {} as initial leader", leader.id());

            leader.bootstrap().await;
            leader.commit().await;
            for follower in followers {
                log::info!("adding {} to {}", follower.id(), leader.id());
                leader.member_add(follower.id()).await;
                leader.commit().await;
            }
        } else {
            log::error!("substrng");
        }
        dbg!(&rafts[0]);

        let mut fu: FuturesUnordered<_> = rafts
            .into_iter()
            .map(|mut raft_state| {
                task::spawn(async move {
                    raft_state.commit().await;
                    let id = raft_state.id().to_string();
                    log::info!("r");
                    let raft_state = Arc::new(RwLock::new(raft_state));
                    server::start(raft_state, id).await
                })
            })
            .collect();

        for l in fu.next().await {
            match l {
                Err(e) => {
                    dbg!(e);
                }
                Ok(_) => {}
            }
        }
        Ok(())
    })
}
