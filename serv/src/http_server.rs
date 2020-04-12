use std::path::PathBuf;
use std::net::SocketAddr;
use std::future::Future;

use tokio::fs::File;
use tokio::io::AsyncReadExt;

use hyper::{
    header::{self, HeaderValue},
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
    Body, Method, Response, StatusCode, Request,
};

static INTERNAL_SERVER_ERROR: &[u8] = b"Internal Server Error";
static NOT_FOUND: &[u8] = b"Not Found";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub listen_addr: SocketAddr,
    pub clnt_deploy_dir: PathBuf,
}

#[derive(Clone)]
pub struct Server {
    config: Config,
}

impl Server {
    pub fn new(config: Config) -> Self {
        Self {
            config,
        }
    }

    pub fn serve(&self) -> impl Future<Output=Result<(), hyper::Error>> + '_ {
        async move {
            let listen_addr = self.config.listen_addr.clone();
            let server = self.clone();
            let make_service = hyper::service::make_service_fn(move |addr_stream: &AddrStream| {
                let remote_addr = addr_stream.remote_addr();
                let server = server.clone();

                async move {
                    Ok::<_, hyper::Error>(hyper::service::service_fn(move |req| {
                        server.service(req)
                    }))
                }
            });

            hyper::Server::bind(&listen_addr)
                .serve(make_service)
                .await
        }
    }

    async fn service(&self, req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
        match (req.method(), req.uri().path()) {
            // Serve static files
            (&Method::GET, "") | (&Method::GET, "/index.html") => 
                self.send_file("index.html").await,
            (&Method::GET, "clnt.js") =>
                self.send_file("clnt.js").await,
            (&Method::GET, "clnt.wasm") =>
                self.send_file("clnt.wasm").await,

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
    async fn send_file(&self, filename: &str) -> Result<Response<Body>, hyper::Error> {
        // Serve a file by asynchronously reading it entirely into memory.
        // Uses tokio_fs to open file asynchronously, then tokio::io::AsyncReadExt
        // to read into memory asynchronously.

        let filename = self.config.clnt_deploy_dir.join(filename);

        if let Ok(mut file) = File::open(filename).await {
            let mut buf = Vec::new();

            if let Ok(_) = file.read_to_end(&mut buf).await {
                Ok(Response::new(buf.into()))
            } else {
                Ok(internal_server_error())
            }
        } else {
            Ok(not_found())
        }
    }
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