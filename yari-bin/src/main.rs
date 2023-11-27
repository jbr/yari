mod cli;
use yari::state_machine::in_memory_kv::InMemoryKV;

fn main() {
    if !cfg!(debug_assertions) {
        human_panic::setup_panic!();
    }
    let state_machine = InMemoryKV::default();

    async_global_executor::block_on(cli::cli(state_machine));
}
