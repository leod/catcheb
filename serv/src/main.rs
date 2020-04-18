mod game;
mod http_server;
mod runner;
mod webrtc;

use std::path::PathBuf;

use tokio::sync::mpsc;

use clap::Arg;

#[derive(Clone, Debug)]
pub struct Config {
    pub http_server: http_server::Config,
}

#[tokio::main]
async fn main() {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let matches = clap::App::new("serv")
        .arg(
            Arg::with_name("http_address")
                .long("http_address")
                .takes_value(true)
                .required(true)
                .help("listen on the specified address/port for HTTP")
        )
        .arg(
            Arg::with_name("clnt_deploy_dir")
                .long("clnt_deploy_dir")
                .takes_value(true)
                .required(true)
                .help("Directory in which the clnt has been deployed (containing static files to be served over HTTP")
        )
        .get_matches();

    let http_server_config = http_server::Config {
        listen_addr: matches
            .value_of("http_address")
            .unwrap()
            .parse()
            .expect("could not parse HTTP address/port"),
        clnt_deploy_dir: PathBuf::from(matches.value_of("clnt_deploy_dir").unwrap()),
    };

    let config = Config {
        http_server: http_server_config,
    };

    let (recv_msg_tx, recv_msg_rx) = mpsc::unbounded_channel();
    let (send_msg_tx, send_msg_rx) = mpsc::unbounded_channel();

    let runner = runner::Runner::new(recv_msg_rx, send_msg_tx);

    let http_server = http_server::Server::new(config.http_server, runner.join_tx());

    tokio::spawn(async move {
        http_server.serve().await.expect("HTTP server died");
    });
}
