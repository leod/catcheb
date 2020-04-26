use std::collections::HashMap;

use tokio::sync::{mpsc, oneshot};

use uuid::Uuid;

use crate::{
    game::Game,
    webrtc::{RecvMessageRx, SendMessageTx},
};
use comn::{JoinReply, JoinRequest};

pub struct JoinMessage {
    pub request: JoinRequest,
    pub reply_tx: oneshot::Sender<JoinReply>,
}

pub type JoinTx = mpsc::UnboundedSender<JoinMessage>;
pub type JoinRx = mpsc::UnboundedReceiver<JoinMessage>;

pub struct Runner {
    games: HashMap<Uuid, Game>,

    join_tx: JoinTx,
    join_rx: JoinRx,

    recv_message_rx: RecvMessageRx,
    send_message_tx: SendMessageTx,
}

impl Runner {
    pub fn new(
        recv_message_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        send_message_tx: mpsc::UnboundedSender<Vec<u8>>,
    ) -> Self {
        let (join_tx, join_rx) = mpsc::unbounded_channel();

        Runner {
            games: HashMap::new(),
            join_tx,
            join_rx,
            recv_message_rx,
            send_message_tx,
        }
    }

    pub fn join_tx(&self) -> mpsc::UnboundedSender<JoinMessage> {
        self.join_tx.clone()
    }

    pub fn run(&self) {
        loop {}
    }
}
