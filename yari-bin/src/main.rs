mod cli;
use yari::state_machine::in_memory_kv::InMemoryKV;

#[async_std::main]
async fn main() -> tide::Result<()> {
    if !cfg!(debug_assertions) {
        human_panic::setup_panic!();
    }

    let state_machine = InMemoryKV::default();

    cli::cli(state_machine).await
}
