use std::{net::SocketAddr, time::Instant};

use log::{info, warn};

use futures::{select, FutureExt};
use tokio::sync::{mpsc, oneshot};

pub struct MessageIn {
    pub peer: SocketAddr,
    pub data: Vec<u8>,
    pub recv_time: Instant,
}

pub struct MessageOut {
    pub peer: SocketAddr,
    pub data: Vec<u8>,
}

// TODO: Check if we should make channels bounded
pub type RecvMessageTx = mpsc::UnboundedSender<MessageIn>;
pub type RecvMessageRx = mpsc::UnboundedReceiver<MessageIn>;
pub type SendMessageTx = mpsc::UnboundedSender<MessageOut>;
pub type SendMessageRx = mpsc::UnboundedReceiver<MessageOut>;

pub fn recv_message_channel() -> (RecvMessageTx, RecvMessageRx) {
    mpsc::unbounded_channel()
}

pub fn send_message_channel() -> (SendMessageTx, SendMessageRx) {
    mpsc::unbounded_channel()
}

#[derive(Debug, Clone)]
pub struct Config {
    pub listen_addr: SocketAddr,
}

pub struct Server {
    recv_message_tx: RecvMessageTx,
    send_message_rx: SendMessageRx,

    webrtc_server: webrtc_unreliable::Server,
}

impl Server {
    pub async fn new(
        config: Config,
        recv_message_tx: RecvMessageTx,
        send_message_rx: SendMessageRx,
    ) -> Result<Self, std::io::Error> {
        // Note that the `webrtc_unreliable::Server` actually takes two
        // addresses: the listen address and the public address. In practice,
        // it seems that both addresses must listen on the same port:
        // <https://github.com/kyren/webrtc-unreliable/issues/3#issuecomment-532905616>
        //
        // There might be some use in using a different IP for the two
        // addresses, but for now we'll just use the exact same address.
        let webrtc_server =
            webrtc_unreliable::Server::new(config.listen_addr, config.listen_addr).await?;

        Ok(Self {
            recv_message_tx,
            send_message_rx,
            webrtc_server,
        })
    }

    pub fn session_endpoint(&self) -> webrtc_unreliable::SessionEndpoint {
        self.webrtc_server.session_endpoint()
    }

    pub async fn serve(mut self, shutdown_rx: oneshot::Receiver<()>) {
        let mut shutdown_rx = shutdown_rx.fuse();

        // TODO: Check size of `message_buf` for receiving WebRTC messages
        let mut message_buf = vec![0; 0x10000];

        loop {
            select! {
                message_out = self.send_message_rx.recv().fuse() => {
                    match message_out {
                        Some(message_out) => {
                            if let Err(err) = self.webrtc_server.send(
                                    &message_out.data,
                                    webrtc_unreliable::MessageType::Binary,
                                    &message_out.peer,
                                )
                                .await
                            {
                                warn!(
                                    "Failed to send message to {}: {}",
                                    message_out.peer,
                                    err,
                                );
                            }
                        }
                        None => {
                            info!("send_message_rx closed, terminating");
                            return;
                        }
                    }
                }
                message_result = self.webrtc_server.recv(&mut message_buf).fuse() => {
                    match message_result {
                        Ok(message_result) => {
                            let message_in = MessageIn {
                                peer: message_result.remote_addr,
                                data: message_buf[0..message_result.message_len].to_vec(),
                                recv_time: Instant::now(),
                            };
                            if self.recv_message_tx.send(message_in).is_err() {
                                info!("recv_message_tx closed, terminating");
                                return;
                            }
                        }
                        Err(err) => {
                            warn!("Could not receive message: {}", err);
                        }
                    }
                }
                _ = shutdown_rx => {
                    return;
                }
            };
        }
    }
}
