use crate::{
    persistence, rpc, server, state_machine::StateMachine, Config, ElectionThread, EphemeralState,
    Message,
};
use anyhow::{anyhow, Result};
use async_std::sync::{Arc, Mutex};
use std::{net::SocketAddr, path::PathBuf};
use structopt::StructOpt;
use tide::http::Method;
use url::{ParseError, Url};
use async_macros::join;

fn parse_urls(s: &str) -> Result<Urls, ParseError> {
    s.replace(",", " ")
        .split_whitespace()
        .map(|s| Url::parse(s))
        .collect::<Result<Vec<Url>, ParseError>>()
        .map(|u| Urls(u))
}

#[derive(Debug)]
struct Urls(Vec<Url>);

#[derive(Debug, StructOpt)]
struct ClientOptions {
    #[structopt(short, long, default_value = "10")]
    retries: u32,

    #[structopt(short, long)]
    no_follow: bool,

    #[structopt(short, long, parse(try_from_str = parse_urls), required = true, env = "YARI_SERVERS")]
    servers: Urls,
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
    Inspect(ServerOptions),

    Join {
        #[structopt(flatten)]
        server_options: ServerOptions,

        #[structopt(flatten)]
        client_options: ClientOptions,
    },

    Resume(ServerOptions),

    Bootstrap(ServerOptions),

    Ping(ClientOptions),

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

pub async fn cli<S: StateMachine>(state_machine: S) -> Result<()> {
    exec(state_machine, Command::from_args()).await
}

async fn exec<S: StateMachine>(state_machine: S, command: Command) -> Result<()> {
    match command {
        Command::Inspect(server_options) => {
            let statefile_path = extract_statefile_path(&server_options);
            if statefile_path.exists() {
                let mut raft_state = persistence::load_or_default(EphemeralState {
                    id: server_options.id.to_string(),
                    statefile_path,
                    config: config_from_options(&server_options)?,
                    state_machine,
                });

                raft_state.commit().await;

                println!("{:#?}", raft_state);
                Ok(())
            } else {
                Err(anyhow!(
                    "no statefile found at ({})",
                    statefile_path.to_str().unwrap()
                ))
            }
        }

        Command::Bootstrap(server_options) => {
            let statefile = extract_statefile_path(&server_options);
            if statefile.exists() {
                Err(anyhow!(
                    "cannot run bootstrap with an existing statefile ({})",
                    statefile.to_str().unwrap()
                ))
            } else {
                start_server(&server_options, true, state_machine).await
            }
        }

        Command::Join {
            client_options,
            server_options,
        } => {
            let statefile = extract_statefile_path(&server_options);

            if statefile.exists() {
                Err(anyhow!(
                    "cannot run join with an existing statefile ({})",
                    statefile.to_str().unwrap()
                ))
            } else {
                api_client_request(
                    client_options,
                    Method::PUT,
                    format!(
                        "/servers/{}",
                        &urlencoding::encode(server_options.id.as_str())
                    ),
                )
                .await?;

                println!("added");
                start_server(&server_options, false, state_machine).await
            }
        }

        Command::Resume(server_options) => {
            let statefile = extract_statefile_path(&server_options);

            if statefile.exists() {
                start_server(&server_options, false, state_machine).await
            } else {
                Err(anyhow!(
                    "no statefile found for resume at {}",
                    statefile.to_str().unwrap()
                ))
            }
        }

        Command::Ping(client_options) => {
            println!(
                "ping: {}",
                api_client_request(client_options, Method::GET, "/".into()).await?
            );
            Ok(())
        }

        Command::Add { id, client_options } => {
            api_client_request(
                client_options,
                Method::PUT,
                format!("/servers/{}", &urlencoding::encode(id.as_str())),
            )
            .await?;
            Ok(())
        }

        Command::Remove { id, client_options } => {
            api_client_request(
                client_options,
                Method::DELETE,
                format!("/servers/{}", &urlencoding::encode(id.as_str())),
            )
            .await?;
            Ok(())
        }

        Command::Client {
            ext,
            client_options,
        } => match state_machine.cli(ext) {
            Ok(Some(message)) => {
                client(&client_options, message).await?;
                Ok(())
            }
            Ok(None) => Ok(()),
            Err(s) => {
                eprintln!("{}", &s);
                Err(anyhow!("{}", &s))
            }
        },
    }
}

fn extract_statefile_path(server_options: &ServerOptions) -> PathBuf {
    server_options
        .statefile
        .as_ref()
        .cloned()
        .unwrap_or_else(|| persistence::path(&server_options.id).unwrap())
}

fn config_from_options(options: &ServerOptions) -> Result<Config> {
    Config::parse(options.config.as_ref().cloned().unwrap_or_else(|| {
        let mut path = std::env::current_dir().unwrap();
        path.push("config.toml");
        path
    }))
}

async fn start_server<S: StateMachine>(
    options: &ServerOptions,
    bootstrap: bool,
    state_machine: S,
) -> Result<()> {
    let config = config_from_options(options)?;

    let bind = options.bind.or_else(|| {
        options
            .id
            .socket_addrs(|| None)
            .ok()
            .and_then(|mut addrs| addrs.pop())
    });

    if let Some(bind) = bind {
        let statefile_path = extract_statefile_path(&options);

        let mut raft_state = persistence::load_or_default(EphemeralState {
            id: options.id.to_string(),
            statefile_path,
            config,
            state_machine,
        });

        if bootstrap {
            raft_state.bootstrap()
        }

        raft_state.commit().await;

        let arc_mutex = Arc::new(Mutex::new(raft_state));
        let cmc = arc_mutex.clone();

        let et = ElectionThread::spawn(&cmc);
        let s = server::start(arc_mutex, bind);
        let (_, _) = join!(et, s).await;

        Ok(())
    } else {
        Err(anyhow!(
            "could not determine address and port to bind.\n\
                     specify -b or --bind with a socket address"
        ))
    }
}

async fn api_client_request(
    options: ClientOptions,
    method: Method,
    path: String,
) -> Result<String> {
    //    let mut rng = rand::thread_rng();
    let servers: Vec<Url> = options.servers.0.clone();
    //    servers.shuffle(&mut rng);
    dbg!(&servers);

    let method = &method;
    let path = &path;

    for server in servers {
        let response = rpc::request(method.clone(), server.clone().join(path).unwrap()).await;

        dbg!(&response);

        if let Ok(mut r) = response {
            if r.status() == 200 {
                return Ok(r.body_string().await.map_err(|_| anyhow!("utf8 error"))?);
            } else {
                dbg!(r.body_string().await.unwrap());
                dbg!(&r);
            }
        }
    }

    Err(anyhow!("no server responded"))
}

async fn client<MessageType: Message>(options: &ClientOptions, message: MessageType) -> Result<()> {
    //    let mut rng = rand::thread_rng();
    let request = &rpc::ClientRequest { message };
    dbg!(&request);

    for server in options.servers.0.iter() {
        let response = request.send(&server).await;
        dbg!(&response);
        if response.is_ok() {
            return Ok(());
        }
    }

    Err(anyhow!("no server responded"))
}
