use std::net::SocketAddr;

use log::warn;

use futures::{pin_mut, select, FutureExt, StreamExt};
use tokio::sync::mpsc;

type Message = (SocketAddr, Vec<u8>);

// TODO: Check if we should make channels bounded
pub type RecvMessageTx = mpsc::UnboundedSender<Message>;
pub type RecvMessageRx = mpsc::UnboundedReceiver<Message>;
pub type SendMessageTx = mpsc::UnboundedSender<Message>;
pub type SendMessageRx = mpsc::UnboundedReceiver<Message>;

pub fn recv_message_channel() -> (RecvMessageTx, RecvMessageRx) {
    mpsc::unbounded_channel()
}

#[derive(Debug, Clone)]
pub struct Config {
    pub listen_addr: SocketAddr,
}

pub struct Server {
    config: Config,

    recv_message_tx: RecvMessageTx,
    send_message_tx: SendMessageTx,
    send_message_rx: SendMessageRx,

    webrtc_server: webrtc_unreliable::Server,
}

impl Server {
    pub async fn new(
        config: Config,
        recv_message_tx: RecvMessageTx,
    ) -> Result<Self, std::io::Error> {
        let (send_message_tx, send_message_rx) = mpsc::unbounded_channel();

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
            config,
            recv_message_tx,
            send_message_tx,
            send_message_rx,
            webrtc_server,
        })
    }

    pub fn send_message_tx(&self) -> SendMessageTx {
        self.send_message_tx.clone()
    }

    pub fn session_endpoint(&self) -> webrtc_unreliable::SessionEndpoint {
        self.webrtc_server.session_endpoint()
    }

    pub async fn serve(mut self) {
        // TODO: Check size of `message_buf` for receiving WebRTC messages
        let mut message_buf = vec![0; 0x10000];

        loop {
            select! {
                send_message = self.send_message_rx.recv().fuse() => {
                    match send_message {
                        Some((remote_addr, message)) => {
                            if let Err(err) = self.webrtc_server.send(
                                    &message,
                                    webrtc_unreliable::MessageType::Binary,
                                    &remote_addr,
                                )
                                .await
                            {
                                warn!(
                                    "Failed to send message to {}: {}",
                                    remote_addr,
                                    err,
                                );
                            }
                        }
                        None => {
                            warn!("send_message_rx closed, terminating");
                            return;
                        }
                    }
                }
                message_result = self.webrtc_server.recv(&mut message_buf).fuse() => {
                    match message_result {
                        Ok(message_result) => {
                            let recv_message = (
                                message_result.remote_addr,
                                message_buf[0..message_result.message_len].to_vec()
                            );
                            if self.recv_message_tx.send(recv_message).is_err() {
                                warn!("recv_message_tx closed, terminating");
                            }
                        }
                        Err(err) => {
                            warn!("Could not receive message: {}", err);
                        }
                    }
                }
            };
        }
    }
}
