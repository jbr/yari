pub mod config;
pub mod log;
pub mod message_board;
pub mod persistence;
pub mod raft;
pub mod rpc;
pub mod server;
pub mod state_machine;

pub use crate::log::*;
pub use config::*;
pub use raft::*;

//pub use sse_channel::*;

pub trait Okay<S> {
    fn okay(self) -> tide::Result<S>;
}

impl<T> Okay<T> for T
where
    T: Sized,
{
    fn okay(self) -> tide::Result<Self> {
        Ok(self)
    }
}
