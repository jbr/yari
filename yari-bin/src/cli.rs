use async_std::prelude::*;
use async_std::sync::{Arc, RwLock};
use std::{net::SocketAddr, path::PathBuf};
use structopt::StructOpt;
use tide::http::url::{ParseError, Url};
use tide::http::Method;
use tide::Result;
use yari::{
    persistence, rpc, server, state_machine::StateMachine, Config, ElectionThread, EphemeralState,
    Message,
};

fn parse_urls(s: &str) -> std::result::Result<Urls, ParseError> {
    let mut urls = vec![];
    for s in s.replace(",", " ").split_whitespace() {
        urls.push(Url::parse(s)?);
    }
    Ok(Urls(urls))
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

pub async fn cli<S: StateMachine>(state_machine: S) -> tide::Result<()> {
    exec(state_machine, Command::from_args()).await
}

async fn exec<S: StateMachine>(state_machine: S, command: Command) -> tide::Result<()> {
    match command {
        Command::Inspect(server_options) => {
            let statefile_path = extract_statefile_path(&server_options);
            if statefile_path.exists() {
                let mut raft_state = persistence::load_or_default(EphemeralState {
                    id: server_options.id.to_string(),
                    statefile_path,
                    config: config_from_options(&server_options)?,
                    state_machine,
                })
                .await;

                raft_state.commit().await;

                println!("{:#?}", raft_state);
                Ok(())
            } else {
                Err(tide::http::format_err!(
                    "no statefile found at ({})",
                    statefile_path.to_str().unwrap()
                ))
            }
        }

        Command::Bootstrap(server_options) => {
            let statefile = extract_statefile_path(&server_options);
            if statefile.exists() {
                Err(tide::http::format_err!(
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
                Err(tide::http::format_err!(
                    "cannot run join with an existing statefile ({})",
                    statefile.to_str().unwrap()
                ))
            } else {
                api_client_request(
                    client_options,
                    Method::Put,
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
                Err(tide::http::format_err!(
                    "no statefile found for resume at {}",
                    statefile.to_str().unwrap()
                ))
            }
        }

        Command::Ping(client_options) => {
            println!(
                "ping: {}",
                api_client_request(client_options, Method::Get, "/".into()).await?
            );
            Ok(())
        }

        Command::Add { id, client_options } => {
            api_client_request(
                client_options,
                Method::Put,
                format!("/servers/{}", &urlencoding::encode(id.as_str())),
            )
            .await?;
            Ok(())
        }

        Command::Remove { id, client_options } => {
            api_client_request(
                client_options,
                Method::Delete,
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
                Err(tide::http::format_err!("{}", &s))
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

fn config_from_options(options: &ServerOptions) -> tide::Result<Config> {
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
) -> tide::Result<()> {
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
        })
        .await;

        if bootstrap {
            raft_state.bootstrap().await
        }

        raft_state.commit().await;

        let arc_mutex = Arc::new(RwLock::new(raft_state));

        async_std::task::spawn(ElectionThread::spawn(arc_mutex.clone()))
            .race(async_std::task::spawn(async move {
                server::start(arc_mutex, bind).await.ok();
            }))
            .await;

        Ok(())
    } else {
        Err(tide::http::format_err!(
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
    let servers: Vec<Url> = options.servers.0.clone();
    let method = &method;
    let path = &path;

    for server in servers {
        match yari::rpc::request(method.clone(), server.clone().join(path).unwrap()).await {
            Ok(mut r) => {
                if r.status().is_success() {
                    return Ok(r
                        .body_string()
                        .await
                        .map_err(|_| tide::http::format_err!("utf8 error"))?);
                } else {
                    dbg!(r.body_string().await.unwrap());
                    dbg!(&r);
                }
            }

            Err(_) => {}
        }
    }

    Err(tide::http::format_err!("no server responded"))
}

async fn client<MessageType: Message>(options: &ClientOptions, message: MessageType) -> Result<()> {
    let request = &rpc::ClientRequest { message };
    for server in options.servers.0.iter() {
        let response = request.send(&server).await;
        dbg!(&response);
        if response.is_ok() {
            return Ok(());
        }
    }

    Err(tide::http::format_err!("no server responded"))
}
