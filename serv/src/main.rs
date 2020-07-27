// Increase recursion_limit for `futures::select` macro
#![recursion_limit = "1024"]
// Needed for pareen stuff
#![type_length_limit = "600000000"]

mod bot;
mod fake_bad_net;
mod game;
mod http;
mod runner;
mod tiled;
mod webrtc;

use std::{path::PathBuf, time::Duration};

use clap::Arg;
use log::{info, warn};

use tokio::sync::oneshot;

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
        .arg(
            Arg::with_name("map")
                .long("map")
                .takes_value(true)
                .default_value("maps/test.tmx")
                .help("Path to TMX map file"),
        )
        .get_matches();

    let game_map = tiled::load_map(matches.value_of("map").unwrap()).unwrap();
    let runner_config = runner::Config {
        max_num_games: 32,
        game_settings: comn::Settings {
            max_num_players: 64,
            ticks_per_second: 30,
            map: game_map,
        },
    };
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
        runner: runner_config,
    };

    let (recv_message_tx, recv_message_rx) = webrtc::recv_message_channel();
    let (send_message_tx, send_message_rx) = webrtc::send_message_channel();

    let fake_bad_net_config = Some((
        fake_bad_net::Config {
            lag_mean: Duration::from_millis(125),
            lag_std_dev: 0.0,
            loss: 0.00,
        },
        fake_bad_net::Config {
            lag_mean: Duration::from_millis(125),
            lag_std_dev: 0.0,
            loss: 0.00,
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

    let (shutdown_http_tx, shutdown_http_rx) = oneshot::channel();
    let (shutdown_runner_tx, shutdown_runner_rx) = oneshot::channel();
    let (shutdown_webrtc_tx, shutdown_webrtc_rx) = oneshot::channel();

    let webrtc_server = webrtc::Server::new(config.webrtc_server, recv_message_tx, send_message_rx)
        .await
        .expect("Error starting WebRTC server");
    let session_endpoint = webrtc_server.session_endpoint();

    let runner = runner::Runner::new(
        config.runner,
        recv_message_rx,
        send_message_tx,
        shutdown_runner_rx,
    );
    let join_tx = runner.join_tx();

    let http_server = http::Server::new(config.http_server, join_tx, session_endpoint);

    let runner_thread = tokio::task::spawn_blocking(move || runner.run());
    let http_server_task =
        tokio::task::spawn(async move { http_server.serve(shutdown_http_rx).await });
    let webrtc_server_task =
        tokio::task::spawn(async move { webrtc_server.serve(shutdown_webrtc_rx).await });

    // Shutdown handling...
    ctrlc::set_handler_mut({
        let mut shutdown_http_tx = Some(shutdown_http_tx);

        move || {
            info!("Received Ctrl-C signal, shutting down tasks");

            if let Some(shutdown_http_tx) = shutdown_http_tx.take() {
                shutdown_http_tx
                    .send(())
                    .expect("Failed to send shutdown to HTTP server");
            }
        }
    })
    .expect("Error setting Ctrl-C handler");

    if let Err(err) = http_server_task.await.expect("Failed to join HTTP server") {
        warn!("HTTP server died: {:?}", err);
    }

    info!("HTTP server terminated, shutting down runner thread");
    shutdown_runner_tx
        .send(())
        .expect("Failed to send shutdown to runner thread");

    runner_thread.await.expect("Failed to join runner thread");

    info!("Runner thread terminated, shutting down WebRTC server");
    if shutdown_webrtc_tx.send(()).is_err() {
        info!("WebRTC server has already shut down");
    }

    webrtc_server_task
        .await
        .expect("Failed to join WebRTC server");
}
