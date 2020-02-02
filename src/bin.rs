#![feature(proc_macro_hygiene, decl_macro, option_result_contains)]
mod append;
mod client;
mod config;
mod election_thread;
mod log;
mod raft;
mod server;
mod state_machine;
mod vote;
use config::YariConfig;
use raft::UnknownResult;
use rand::prelude::*;
use std::net::SocketAddr;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "yari", about = "yet another raft implementation.")]
enum YariCLI {
    Start {
        #[structopt(short, long, parse(from_os_str))]
        config: Option<PathBuf>,

        #[structopt(short, long)]
        address: SocketAddr,
    },

    Client {
        #[structopt(short, long, parse(from_os_str))]
        config: Option<PathBuf>,

        #[structopt(short, long)]
        server: Option<SocketAddr>,

        message: String,

        #[structopt(short, long)]
        retries: Option<u32>,
        #[structopt(short, long)]
        no_follow: bool,
    },
}

fn default_config() -> PathBuf {
    let mut path = std::env::current_dir().expect("current dir");
    path.push("config.toml");
    path
}

fn main() -> UnknownResult {
    let opt = YariCLI::from_args();
    match opt {
        YariCLI::Start { config, address } => {
            let config = YariConfig::parse(config.unwrap_or_else(default_config))?;
            server::start(config, address, address.to_string())?;
            Ok(())
        }
        YariCLI::Client {
            config,
            message,
            server,
            no_follow,
            retries,
        } => {
            let mut retry_count = retries.unwrap_or(10);
            let config = YariConfig::parse(config.unwrap_or_else(default_config))?;
            let mut rng = rand::thread_rng();
            loop {
                if retry_count == 0 {
                    break Err("ran out of retries".into());
                }
                let server = server.unwrap_or_else(|| {
                    *config
                        .servers
                        .choose(&mut rng)
                        .expect("no servers specified")
                });
                let response =
                    crate::client::client_append(server.to_string(), message.clone(), !no_follow);

                match response {
                    Err(_) => {
                        retry_count -= 1;
                    }
                    Ok(client::ClientResponse { raft_success, .. }) if raft_success == false => {
                        eprintln!("🔁 raft error, retrying");
                        retry_count -= 1;
                    }
                    Ok(client::ClientResponse {
                        state_machine_response: Some(response),
                        ..
                    }) => {
                        println!("👍 response: {}", response);
                        break Ok(());
                    }

                    Ok(client::ClientResponse {
                        state_machine_error: Some(response),
                        ..
                    }) => {
                        eprintln!("❌ error: {}", response);
                        break Ok(());
                    }
                    _ => break Err("🤷‍♂ unknown".into()),
                }
            }
        }
    }
}
