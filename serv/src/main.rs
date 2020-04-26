mod game;
mod http_server;
mod runner;
mod webrtc;

use std::path::PathBuf;

use tokio::sync::mpsc;

use clap::Arg;

use runner::Runner;

#[derive(Clone, Debug)]
pub struct Config {
    pub http_server: http_server::Config,
    pub runner: runner::Config,
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
                .help("listen on the specified address/port for HTTP"),
        )
        .arg(
            Arg::with_name("clnt_dir")
                .long("clnt_dir")
                .takes_value(true)
                .default_value("clnt")
                .help("Directory containing static files to be served over HTTP"),
        )
        .get_matches();

    let http_server_config = http_server::Config {
        listen_addr: matches
            .value_of("http_address")
            .unwrap()
            .parse()
            .expect("could not parse HTTP address/port"),
        clnt_dir: PathBuf::from(matches.value_of("clnt_dir").unwrap()),
    };

    let config = Config {
        http_server: http_server_config,
        runner: runner::Config::default(),
    };

    let (recv_msg_tx, recv_msg_rx) = mpsc::unbounded_channel();
    let (send_msg_tx, send_msg_rx) = mpsc::unbounded_channel();

    let runner = runner::Runner::new(config.runner, recv_msg_rx, send_msg_tx);
    let join_tx = runner.join_tx();

    let runner_thread = tokio::task::spawn_blocking(move || {
        runner.run();
    });

    let http_server = http_server::Server::new(config.http_server, join_tx);

    let (_, result) = futures::join!(runner_thread, http_server.serve());
    result.expect("HTTP server died");
}
