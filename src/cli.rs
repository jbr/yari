use crate::state_machine::StateMachine;
use crate::{persistence, rpc, server, Config, ElectionThread, Message, UnknownResult};
use rand::prelude::*;
use reqwest::{blocking::Client, Method};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use structopt::StructOpt;
use url::Url;

#[derive(Debug, StructOpt)]

struct ClientOptions {
    #[structopt(short, long, default_value = "10")]
    retries: u32,

    #[structopt(short, long)]
    no_follow: bool,

    #[structopt(short, long, required = true)]
    servers: Vec<Url>,
}
#[derive(Debug, StructOpt)]

struct ServerOptions {
    #[structopt(long, parse(from_os_str))]
    statefile: Option<PathBuf>,

    #[structopt(short, long, parse(from_os_str))]
    config: Option<PathBuf>,

    #[structopt(short, long)]
    bind: Option<SocketAddr>,

    id: Url,
}

#[structopt(name = "yari", about = "yet another raft implementation.")]
#[derive(Debug, StructOpt)]
enum Command {
    Join {
        #[structopt(flatten)]
        server_options: ServerOptions,

        #[structopt(flatten)]
        client_options: ClientOptions,
    },

    Resume(ServerOptions),

    Bootstrap(ServerOptions),

    Add {
        #[structopt(flatten)]
        client_options: ClientOptions,

        id: Url,
    },

    Remove {
        #[structopt(flatten)]
        client_options: ClientOptions,

        id: Url,
    },

    Client {
        #[structopt(flatten)]
        client_options: ClientOptions,
        ext: Vec<String>,
    },
}

pub fn cli<S: StateMachine>(state_machine: S) -> UnknownResult {
    exec(state_machine, Command::from_args())
}

fn exec<S: StateMachine>(state_machine: S, command: Command) -> UnknownResult {
    match command {
        Command::Bootstrap(server_options) => {
            let statefile = extract_statefile_path(&server_options);
            if statefile.exists() {
                Err(format!(
                    "cannot run bootstrap with an existing statefile ({})",
                    statefile.to_str().unwrap()
                )
                .into())
            } else {
                start_server(&server_options, true, state_machine)
            }
        }

        Command::Join {
            client_options,
            server_options,
        } => {
            let statefile = extract_statefile_path(&server_options);

            if statefile.exists() {
                Err(format!(
                    "cannot run join with an existing statefile ({})",
                    statefile.to_str().unwrap()
                )
                .into())
            } else {
                api_client_request(
                    &client_options,
                    Method::PUT,
                    format!(
                        "/servers/{}",
                        &urlencoding::encode(server_options.id.as_str())
                    ),
                )?;

                println!("added");
                start_server(&server_options, false, state_machine)
            }
        }

        Command::Resume(server_options) => {
            let statefile = extract_statefile_path(&server_options);

            if statefile.exists() {
                start_server(&server_options, false, state_machine)
            } else {
                Err(format!(
                    "no statefile found for resume at {}",
                    statefile.to_str().unwrap()
                )
                .into())
            }
        }

        Command::Add { id, client_options } => api_client_request(
            &client_options,
            Method::PUT,
            format!("/servers/{}", &urlencoding::encode(id.as_str())),
        ),

        Command::Remove { id, client_options } => api_client_request(
            &client_options,
            Method::DELETE,
            format!("/servers/{}", &urlencoding::encode(id.as_str())),
        ),

        Command::Client {
            ext,
            client_options,
        } => {
            if let Some(message) = state_machine.cli(ext) {
                client(&client_options, message)
            } else {
                Ok(())
            }
        }
    }
}

fn extract_statefile_path(server_options: &ServerOptions) -> PathBuf {
    server_options
        .statefile
        .as_ref()
        .cloned()
        .unwrap_or_else(|| persistence::path(&server_options.id).unwrap())
}

fn start_server<S: StateMachine>(
    options: &ServerOptions,
    bootstrap: bool,
    state_machine: S,
) -> UnknownResult {
    let config = Config::parse(options.config.as_ref().cloned().unwrap_or_else(|| {
        let mut path = std::env::current_dir().expect("current dir");
        path.push("config.toml");
        path
    }))?;

    let bind = options.bind.or_else(|| {
        options
            .id
            .socket_addrs(|| None)
            .ok()
            .and_then(|mut addrs| addrs.pop())
    });

    if let Some(bind) = bind {
        let statefile_path = extract_statefile_path(&options);

        let mut raft_state = persistence::load(&statefile_path)
            .unwrap_or_default()
            .with_ephemeral_state(
                options.id.to_string(),
                statefile_path,
                config,
                state_machine,
            );

        if bootstrap {
            raft_state.bootstrap()
        }

        raft_state.commit();

        let arc_mutex = Arc::new(Mutex::new(raft_state));

        ElectionThread::spawn(&arc_mutex);
        server::start(arc_mutex, bind)?;

        Ok(())
    } else {
        Err("could not determine address and port to bind. specify -b or --bind with a socket address".into())
    }
}

fn api_client_request(options: &ClientOptions, method: Method, path: String) -> UnknownResult {
    let mut rng = rand::thread_rng();
    let mut servers = options.servers.clone();
    servers.shuffle(&mut rng);

    servers
        .iter()
        .find_map(|server| {
            let response = Client::new()
                .request(method.clone(), server.clone().join(&path).unwrap())
                .send();

            match response {
                Ok(r) => {
                    println!("{:#?}", r);
                    Some(())
                }
                Err(e) => {
                    eprintln!("{:#?}", e);
                    None
                }
            }
        })
        .ok_or_else(|| "error".into())
}

fn client(options: &ClientOptions, message: Message) -> UnknownResult<()> {
    let mut retry_count = options.retries;
    let mut rng = rand::thread_rng();
    let request = rpc::ClientRequest { message };
    loop {
        if retry_count == 0 {
            break Err("ran out of retries".into());
        }

        let server = options
            .servers
            .choose(&mut rng)
            .expect("no server specified");

        let response = request.send(server);

        match response {
            Err(_) => {
                retry_count -= 1;
            }
            Ok(r) => {
                let body = r.text().unwrap();
                println!("{}", body);
                break Ok(());
            }
        }
    }
}
