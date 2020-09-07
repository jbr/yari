#![feature(
    proc_macro_hygiene,
    decl_macro,
    option_result_contains,
    try_trait,
    async_closure,
    associated_type_bounds,
    specialization
)]

mod at_least;
mod cli;
mod config;
mod log;
mod message_board;
mod persistence;
mod raft;
mod rpc;
mod server;
mod state_machine;
mod sse_channel;
mod eventstream;

pub use config::*;
pub use crate::log::*;
pub use raft::*;
pub use sse_channel::*;


use anyhow::Result;
use state_machine::in_memory_kv::InMemoryKV;

pub trait Okay<S> {
    fn okay(self) -> Result<S>;
}

impl<T> Okay<T> for T
where
    T: Sized,
{
    fn okay(self) -> Result<Self> {
        Ok(self)
    }
}

#[async_std::main]
async fn main() -> Result<()> {
    if !cfg!(debug_assertions) {
        human_panic::setup_panic!();
    }

    let state_machine = InMemoryKV::default();

    cli::cli(state_machine).await
}
