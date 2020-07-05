use std::{future::Future, net::SocketAddr, path::PathBuf, sync::Arc};

use log::{debug, info, warn};

use futures::TryStreamExt;
use tokio::{fs::File, io::AsyncReadExt, stream::StreamExt, sync::oneshot};

use hyper::{
    header::HeaderValue, server::conn::AddrStream, Body, Method, Request, Response, StatusCode,
};
use webrtc_unreliable::SessionEndpoint;

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
    session_endpoint: SessionEndpoint,
}

impl Server {
    pub fn new(config: Config, join_tx: JoinTx, session_endpoint: SessionEndpoint) -> Self {
        Self {
            config: Arc::new(config),
            join_tx,
            session_endpoint,
        }
    }

    pub fn serve(
        &self,
        shutdown_rx: oneshot::Receiver<()>,
    ) -> impl Future<Output = Result<(), hyper::Error>> + '_ {
        info!("Starting HTTP server at {:?}", self.config.listen_addr);
        info!("Will serve client directory {:?}", self.config.clnt_dir);

        let make_service = hyper::service::make_service_fn(move |addr_stream: &AddrStream| {
            let config = self.config.clone();
            let join_tx = self.join_tx.clone();
            let session_endpoint = self.session_endpoint.clone();
            let remote_addr = addr_stream.remote_addr();

            async move {
                Ok::<_, hyper::Error>(hyper::service::service_fn(move |req| {
                    service(
                        config.clone(),
                        join_tx.clone(),
                        session_endpoint.clone(),
                        remote_addr,
                        req,
                    )
                }))
            }
        });

        hyper::Server::bind(&self.config.listen_addr)
            .serve(make_service)
            .with_graceful_shutdown(async {
                shutdown_rx.await.expect("Failed to read shutdown_rx")
            })
    }
}

async fn service(
    config: Arc<Config>,
    join_tx: JoinTx,
    mut session_endpoint: SessionEndpoint,
    remote_addr: SocketAddr,
    req: Request<Body>,
) -> Result<Response<Body>, hyper::Error> {
    debug!("{}: {} {}", remote_addr, req.method(), req.uri().path());

    match (req.method(), req.uri().path()) {
        // Serve static files
        (&Method::GET, "/") | (&Method::GET, "/index.html") => {
            send_file(config, "index.html", "text/html").await
        }
        (&Method::GET, "/clnt.js") => send_file(config, "clnt.js.gz", "text/javascript").await,
        (&Method::GET, "/clnt_bg.wasm") => {
            send_file(config, "clnt_bg.wasm.gz", "application/wasm").await
        }
        (&Method::GET, "/Munro-2LYe.ttf") => send_file(config, "Munro-2LYe.ttf", "font/ttf").await,
        (&Method::GET, "/kongtext.ttf") => send_file(config, "kongtext.ttf", "font/ttf").await,
        (&Method::GET, "/hirsch.png") => send_file(config, "hirsch.png", "image/png").await,

        // Establish a WebRTC connection
        (&Method::POST, "/connect_webrtc") => {
            debug!("WebRTC session request from {}", remote_addr);

            match session_endpoint.http_session_request(req.into_body()).await {
                Ok(mut resp) => {
                    resp.headers_mut().insert(
                        hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN,
                        HeaderValue::from_static("*"),
                    );
                    Ok(resp.map(Body::from))
                }
                Err(_) => Ok(bad_request()),
            }
        }

        // Join a game
        (&Method::POST, "/join") => {
            // FIXME: Does this allow attackers to OOM the server by sending an infinite request?
            let body = req
                .into_body()
                .map(|chunk| chunk.map(|chunk| chunk.as_ref().to_vec()))
                .try_concat()
                .await?;

            let join_request = match serde_json::from_slice(body.as_slice()) {
                Ok(x) => x,
                Err(_) => return Ok(bad_request()),
            };

            let (reply_tx, reply_rx) = oneshot::channel();
            let join_message = JoinMessage {
                request: join_request,
                reply_tx,
            };

            if join_tx.send(join_message).is_err() {
                warn!("join_tx closed, ignoring join request");
                return Ok(internal_server_error());
            }

            if let Ok(join_reply) = reply_rx.await {
                Ok(Response::builder()
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&join_reply).unwrap().into())
                    .unwrap())
            } else {
                warn!("reply_rx closed, ignoring join request");
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

    let full_filename = config.clnt_dir.join(filename);

    if let Ok(mut file) = File::open(&full_filename).await {
        let mut buf = Vec::new();

        if file.read_to_end(&mut buf).await.is_ok() {
            let response = Response::builder().header("Content-Type", content_type);

            let response = if filename.ends_with(".gz") {
                response.header("Content-Encoding", "gzip")
            } else {
                response
            };

            Ok(response.body(buf.into()).unwrap())
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
