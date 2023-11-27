use clap::Parser;
use clap_verbosity_flag::Verbosity;
use std::{net::SocketAddr, path::PathBuf};
use yari::{
    persistence,
    rpc::{ClientRequest, RaftClient},
    server,
    state_machine::StateMachine,
    url::Url,
    Config, EphemeralState, ServerHandle,
};

#[allow(dead_code)]
#[derive(Debug, Parser)]
struct ClientOptions {
    #[arg(short, long, default_value = "10")]
    retries: u32,

    #[arg(short, long)]
    no_follow: bool,

    #[arg(
        short,
        long,
        value_delimiter = ',',
        required = true,
        env = "YARI_SERVERS"
    )]
    servers: Vec<Url>,
}

#[derive(Debug, Parser)]
struct ServerOptions {
    #[arg(long)]
    statefile: Option<PathBuf>,

    #[arg(short, long)]
    config: Option<PathBuf>,

    #[arg(short, long)]
    bind: Option<SocketAddr>,

    url: Url,
}

#[derive(Debug, Parser)]
#[command(author, version, about)]
enum Command {
    Inspect {
        #[command(flatten)]
        server_options: ServerOptions,
        #[command(flatten)]
        verbosity: Verbosity,
    },

    Join {
        #[command(flatten)]
        server_options: ServerOptions,
        #[command(flatten)]
        client_options: ClientOptions,
        #[command(flatten)]
        verbosity: Verbosity,
    },

    Resume {
        #[command(flatten)]
        server_options: ServerOptions,
        #[command(flatten)]
        verbosity: Verbosity,
    },

    Bootstrap {
        #[command(flatten)]
        server_options: ServerOptions,
        #[command(flatten)]
        verbosity: Verbosity,
    },

    Ping {
        #[command(flatten)]
        client_options: ClientOptions,
        #[command(flatten)]
        verbosity: clap_verbosity_flag::Verbosity,
    },

    Add {
        #[command(flatten)]
        client_options: ClientOptions,

        url: Url,

        #[command(flatten)]
        verbosity: Verbosity,
    },

    Remove {
        #[command(flatten)]
        client_options: ClientOptions,

        url: Url,

        #[command(flatten)]
        verbosity: Verbosity,
    },

    Client {
        #[command(flatten)]
        client_options: ClientOptions,
        #[command(flatten)]
        verbosity: Verbosity,
        ext: Vec<String>,
    },
}

impl Command {
    fn verbosity(&self) -> &Verbosity {
        match self {
            Command::Inspect { verbosity, .. } => verbosity,
            Command::Join { verbosity, .. } => verbosity,
            Command::Resume { verbosity, .. } => verbosity,
            Command::Bootstrap { verbosity, .. } => verbosity,
            Command::Ping { verbosity, .. } => verbosity,
            Command::Add { verbosity, .. } => verbosity,
            Command::Remove { verbosity, .. } => verbosity,
            Command::Client { verbosity, .. } => verbosity,
        }
    }
}
pub async fn cli<S: StateMachine>(state_machine: S) {
    exec(state_machine, Command::parse()).await
}

async fn exec<S: StateMachine>(state_machine: S, command: Command) {
    env_logger::builder()
        .filter_module("yari", command.verbosity().log_level_filter())
        .init();

    let raft_client = RaftClient::<S>::default();
    match command {
        Command::Inspect { server_options, .. } => {
            let statefile_path = extract_statefile_path(&server_options);
            if statefile_path.exists() {
                let mut raft_state = persistence::load_or_default(EphemeralState {
                    id: server_options.url.to_string(),
                    statefile_path,
                    config: config_from_options(&server_options),
                    state_machine,
                })
                .await;

                raft_state.commit().await;

                println!("{raft_state:#?}");
            } else {
                panic!(
                    "no statefile found at ({})",
                    statefile_path.to_str().unwrap()
                )
            }
        }

        Command::Bootstrap { server_options, .. } => {
            let statefile = extract_statefile_path(&server_options);
            if statefile.exists() {
                panic!(
                    "cannot run bootstrap with an existing statefile ({})",
                    statefile.to_str().unwrap()
                );
            } else {
                start_server(&server_options, true, state_machine)
                    .await
                    .await;
            }
        }

        Command::Join {
            client_options,
            server_options,
            ..
        } => {
            let statefile = extract_statefile_path(&server_options);
            if statefile.exists() {
                panic!(
                    "cannot run join with an existing statefile ({})",
                    statefile.to_str().unwrap()
                );
            } else {
                let servers = client_options.servers;
                let handle = start_server(&server_options, false, state_machine).await;
                handle.info().await;
                for server in &servers {
                    if raft_client
                        .add(server, server_options.url.as_str())
                        .await
                        .is_ok()
                    {
                        println!("added to {}", server.as_str());
                        handle.await;
                        return;
                    } else {
                        println!("could not reach {}", server.as_str());
                    }
                }

                handle.stop().await;
                panic!("no server responded");
            }
        }

        Command::Resume { server_options, .. } => {
            let statefile = extract_statefile_path(&server_options);

            if statefile.exists() {
                start_server(&server_options, false, state_machine)
                    .await
                    .await;
            } else {
                panic!(
                    "no statefile found for resume at {}",
                    statefile.to_str().unwrap()
                );
            }
        }

        Command::Ping { client_options, .. } => {
            let servers = client_options.servers;
            for server in &servers {
                if let Ok(ping) = raft_client.ping(server).await {
                    println!("{ping} ({server})");
                    break;
                }
            }
            panic!("no server of {servers:?} responded");
        }

        Command::Add {
            url,
            client_options,
            ..
        } => {
            let servers = client_options.servers;
            for server in &servers {
                if raft_client.add(server, url.as_str()).await.is_ok() {
                    println!("added ({server})");
                    return;
                }
            }

            panic!("no server of {servers:?} responded");
        }

        Command::Remove {
            url,
            client_options,
            ..
        } => {
            let servers = client_options.servers;
            for server in &servers {
                if raft_client.remove(server, url.as_str()).await.is_ok() {
                    println!("removed ({server})");
                    return;
                }
            }

            panic!("no server of {servers:?} responded");
        }

        Command::Client {
            ext,
            client_options,
            ..
        } => match state_machine.cli(ext) {
            Ok(Some(message)) => {
                dbg!(&message);
                for server in client_options.servers {
                    if let Ok(result) = raft_client
                        .client_append(
                            &server,
                            &ClientRequest {
                                message: message.clone(),
                            },
                        )
                        .await
                    {
                        println!("{:?}", result.result);
                        return;
                    }
                }
            }
            Ok(None) => {}
            Err(s) => {
                panic!("{}", &s);
            }
        },
    }
}

fn extract_statefile_path(server_options: &ServerOptions) -> PathBuf {
    server_options
        .statefile
        .as_ref()
        .cloned()
        .unwrap_or_else(|| persistence::path(&server_options.url).unwrap())
}

fn config_from_options(options: &ServerOptions) -> Config {
    Config::parse(options.config.as_ref().cloned().unwrap_or_else(|| {
        let mut path = std::env::current_dir().unwrap();
        path.push("config.toml");
        path
    }))
    .unwrap_or_default()
}

async fn start_server<S: StateMachine>(
    options: &ServerOptions,
    bootstrap: bool,
    state_machine: S,
) -> ServerHandle {
    let config = config_from_options(options);

    let socket_addr = options.bind.or_else(|| {
        options
            .url
            .socket_addrs(|| None)
            .ok()
            .and_then(|mut addrs| addrs.pop())
    });

    if let Some(socket_addr) = socket_addr {
        let statefile_path = extract_statefile_path(options);

        let mut raft_state = persistence::load_or_default(EphemeralState {
            id: options.url.to_string(),
            statefile_path,
            config,
            state_machine,
        })
        .await;

        if bootstrap {
            raft_state.bootstrap();
        }

        raft_state.commit().await;

        server::start(raft_state, socket_addr).await
    } else {
        panic!(
            "could not determine address and port to bind.\n\
                     specify -b or --bind with a socket address"
        );
    }
}
