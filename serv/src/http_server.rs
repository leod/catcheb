use std::{future::Future, net::SocketAddr, path::PathBuf, sync::Arc};

use futures::TryStreamExt;
use log::{info, warn};
use tokio::{fs::File, io::AsyncReadExt, stream::StreamExt, sync::oneshot};

use hyper::{server::conn::AddrStream, Body, Method, Request, Response, StatusCode};

use crate::runner::{JoinMessage, JoinTx};

static INTERNAL_SERVER_ERROR: &[u8] = b"Internal Server Error";
static NOT_FOUND: &[u8] = b"Not Found";
static BAD_REQUEST: &[u8] = b"Bad Request";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub listen_addr: SocketAddr,
    pub clnt_dir: PathBuf,
}

#[derive(Clone)]
pub struct Server {
    config: Arc<Config>,
    join_tx: JoinTx,
}

impl Server {
    pub fn new(config: Config, join_tx: JoinTx) -> Self {
        Self {
            config: Arc::new(config),
            join_tx,
        }
    }

    pub fn serve(&self) -> impl Future<Output = Result<(), hyper::Error>> + '_ {
        info!("Starting HTTP server at {:?}", self.config.listen_addr);
        info!("Will serve client directory {:?}", self.config.clnt_dir);

        let make_service = hyper::service::make_service_fn(move |_: &AddrStream| {
            let config = self.config.clone();
            let join_tx = self.join_tx.clone();

            async move {
                Ok::<_, hyper::Error>(hyper::service::service_fn(move |req| {
                    service(config.clone(), join_tx.clone(), req)
                }))
            }
        });

        hyper::Server::bind(&self.config.listen_addr).serve(make_service)
    }
}

async fn service(
    config: Arc<Config>,
    join_tx: JoinTx,
    req: Request<Body>,
) -> Result<Response<Body>, hyper::Error> {
    match (req.method(), req.uri().path()) {
        // Serve static files
        (&Method::GET, "/") | (&Method::GET, "/index.html") => {
            send_file(config, "index.html", "text/html").await
        }
        (&Method::GET, "/clnt.js") => send_file(config, "clnt.js", "text/javascript").await,
        (&Method::GET, "/clnt_bg.wasm") => {
            send_file(config, "clnt_bg.wasm", "application/wasm").await
        }

        // Join a game
        (&Method::POST, "/join") => {
            // FIXME: Does this allow attackers to OOM the server by sending an infinite request?
            let body = req
                .into_body()
                .map(|chunk| chunk.map(|chunk| chunk.as_ref().to_vec()))
                .try_concat()
                .await?;

            let join_request = serde_json::from_slice(body.as_slice());

            info!("Got join request {:?}", join_request);
            info!("str: {}", std::str::from_utf8(body.as_slice()).unwrap());

            let join_request = if let Ok(join_request) = join_request {
                join_request
            } else {
                return Ok(bad_request());
            };

            let (reply_tx, reply_rx) = oneshot::channel();
            let join_message = JoinMessage {
                request: join_request,
                reply_tx,
            };

            if !join_tx.send(join_message).is_ok() {
                warn!("Receiver of join_tx was dropped, ignoring join request");
                return Ok(internal_server_error());
            }

            if let Ok(join_reply) = reply_rx.await {
                Ok(Response::builder()
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&join_reply).unwrap().into())
                    .unwrap())
            } else {
                warn!("Sender of reply_tx was dropped, ignoring join request");
                Ok(internal_server_error())
            }
        }

        // Return 404 Not Found for other routes
        _ => Ok(not_found()),
    }
}

/// Serve a file.
///
/// TODO: We'll need to cache the files eventually, but for now reloading
/// allows for quicker development.
///
/// Source: https://github.com/hyperium/hyper/blob/master/examples/send_file.rs
async fn send_file(
    config: Arc<Config>,
    filename: &str,
    content_type: &str,
) -> Result<Response<Body>, hyper::Error> {
    // Serve a file by asynchronously reading it entirely into memory.
    // Uses tokio_fs to open file asynchronously, then tokio::io::AsyncReadExt
    // to read into memory asynchronously.

    let filename = config.clnt_dir.join(filename);

    if let Ok(mut file) = File::open(&filename).await {
        let mut buf = Vec::new();

        if let Ok(_) = file.read_to_end(&mut buf).await {
            let response = Response::builder()
                .header("Content-Type", content_type)
                .body(buf.into())
                .unwrap();
            Ok(response)
        } else {
            warn!("Could not open file for reading: {:?}", filename);
            Ok(internal_server_error())
        }
    } else {
        Ok(not_found())
    }
}

fn bad_request() -> Response<Body> {
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .body(BAD_REQUEST.into())
        .unwrap()
}

fn not_found() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(NOT_FOUND.into())
        .unwrap()
}

fn internal_server_error() -> Response<Body> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(INTERNAL_SERVER_ERROR.into())
        .unwrap()
}
