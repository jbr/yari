#![feature(proc_macro_hygiene, decl_macro, option_result_contains, try_trait)]

mod config;
mod log;
mod persistence;
mod raft;
mod rpc;
mod server;
mod state_machine;
mod cli;
use std::error::Error;
pub use config::*;
pub use raft::*;
pub use log::*;
use state_machine::in_memory_kv::InMemoryKV;

fn main() -> Result<(), Box<dyn Error>> {
    if !cfg!(debug_assertions) {
        human_panic::setup_panic!();
    }

    let state_machine = InMemoryKV::default();

    cli::cli(state_machine)
}
