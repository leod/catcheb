// Increase recursion_limit for `futures::select` macro
#![recursion_limit = "1024"]

mod fake_bad_net;
mod game;
mod http;
mod runner;
mod webrtc;

use std::{path::PathBuf, time::Duration};

use clap::Arg;

use fake_bad_net::FakeBadNet;

#[derive(Clone, Debug)]
pub struct Config {
    pub http_server: http::Config,
    pub webrtc_server: webrtc::Config,
    pub runner: runner::Config,
}

#[tokio::main]
async fn main() {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("debug"));

    let matches = clap::App::new("serv")
        .arg(
            Arg::with_name("http_address")
                .long("http_address")
                .takes_value(true)
                .required(true)
                .help("listen on the specified address/port for HTTP"),
        )
        .arg(
            Arg::with_name("webrtc_address")
                .long("webrtc_address")
                .takes_value(true)
                .required(true)
                .help("listen on the specified address/port for WebRTC"),
        )
        .arg(
            Arg::with_name("clnt_dir")
                .long("clnt_dir")
                .takes_value(true)
                .default_value("clnt/static")
                .help("Directory containing static files to be served over HTTP"),
        )
        .get_matches();

    let http_server_config = http::Config {
        listen_addr: matches
            .value_of("http_address")
            .unwrap()
            .parse()
            .expect("could not parse HTTP address/port"),
        clnt_dir: PathBuf::from(matches.value_of("clnt_dir").unwrap()),
    };
    let webrtc_server_config = webrtc::Config {
        listen_addr: matches
            .value_of("webrtc_address")
            .unwrap()
            .parse()
            .expect("could not parse WebRTC address/port"),
    };
    let config = Config {
        http_server: http_server_config,
        webrtc_server: webrtc_server_config,
        runner: runner::Config::default(),
    };

    let (recv_message_tx, recv_message_rx) = webrtc::recv_message_channel();
    let (send_message_tx, send_message_rx) = webrtc::send_message_channel();

    let fake_bad_net_config = Some((
        fake_bad_net::Config {
            lag_mean: Duration::from_millis(25),
            lag_std_dev: 5.0,
            loss: 0.05,
        },
        fake_bad_net::Config {
            lag_mean: Duration::from_millis(25),
            lag_std_dev: 5.0,
            loss: 0.05,
        },
    ));
    let fake_bad_net_config = None;

    let (recv_message_rx, send_message_rx) = if let Some((config_in, config_out)) =
        fake_bad_net_config
    {
        let (lag_recv_message_tx, lag_recv_message_rx) = webrtc::recv_message_channel();
        let (lag_send_message_tx, lag_send_message_rx) = webrtc::send_message_channel();
        let fake_bad_net_recv = FakeBadNet::new(config_in, recv_message_rx, lag_recv_message_tx);
        let fake_bad_net_send = FakeBadNet::new(config_out, send_message_rx, lag_send_message_tx);
        tokio::spawn(fake_bad_net_recv.run());
        tokio::spawn(fake_bad_net_send.run());

        (lag_recv_message_rx, lag_send_message_rx)
    } else {
        (recv_message_rx, send_message_rx)
    };

    let webrtc_server = webrtc::Server::new(config.webrtc_server, recv_message_tx, send_message_rx)
        .await
        .unwrap();
    let session_endpoint = webrtc_server.session_endpoint();

    let runner = runner::Runner::new(config.runner, recv_message_rx, send_message_tx);
    let join_tx = runner.join_tx();
    let runner_thread = tokio::task::spawn_blocking(move || runner.run());

    let http_server = http::Server::new(config.http_server, join_tx, session_endpoint);

    let (_, http_server_result, _) =
        futures::join!(runner_thread, http_server.serve(), webrtc_server.serve(),);
    http_server_result.expect("HTTP server died");
}
