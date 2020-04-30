// Increase recursion_limit for `futures::select` macro
#![recursion_limit = "1024"]

mod game;
mod http_server;
mod runner;
mod webrtc_server;

use std::path::PathBuf;

use tokio::sync::mpsc;

use clap::Arg;

#[derive(Clone, Debug)]
pub struct Config {
    pub http_server: http_server::Config,
    pub webrtc_server: webrtc_server::Config,
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
            Arg::with_name("webrtc_listen_address")
                .long("webrtc_listen_address")
                .takes_value(true)
                .required(true)
                .help("listen on the specified address/port for WebRTC"),
        )
        .arg(
            Arg::with_name("webrtc_public_address")
                .long("webrtc_public_address")
                .takes_value(true)
                .required(true)
                .help("public address/port for WebRTC"),
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
    let webrtc_server_config = webrtc_server::Config {
        listen_addr: matches
            .value_of("webrtc_listen_address")
            .unwrap()
            .parse()
            .expect("could not parse WebRTC listen address/port"),
        public_addr: matches
            .value_of("webrtc_public_address")
            .unwrap()
            .parse()
            .expect("could not parse WebRTC public address/port"),
    };
    let config = Config {
        http_server: http_server_config,
        webrtc_server: webrtc_server_config,
        runner: runner::Config::default(),
    };

    let (recv_message_tx, recv_message_rx) = webrtc_server::recv_message_channel();
    let webrtc_server = webrtc_server::Server::new(config.webrtc_server, recv_message_tx)
        .await
        .unwrap();
    let send_message_tx = webrtc_server.send_message_tx();

    let runner = runner::Runner::new(config.runner, recv_message_rx, send_message_tx);
    let join_tx = runner.join_tx();
    let runner_thread = tokio::task::spawn_blocking(move || {
        runner.run();
    });

    let http_server = http_server::Server::new(config.http_server, join_tx);

    let (_, http_server_result, _) =
        futures::join!(runner_thread, http_server.serve(), webrtc_server.serve(),);
    http_server_result.expect("HTTP server died");
}
