use std::collections::HashMap;

use tokio::sync::{oneshot, mpsc};

use uuid::Uuid;

use crate::game::Game;
use comn::game::{JoinRequest, JoinReply};

pub struct JoinMessage {
    pub request: JoinRequest,
    pub reply: oneshot::Sender<JoinReply>,
}

pub struct Runner {
    games: HashMap<Uuid, Game>,

    join_tx: mpsc::UnboundedSender<JoinMessage>,
    join_rx: mpsc::UnboundedReceiver<JoinMessage>,

    recv_msg_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    send_msg_tx: mpsc::UnboundedSender<Vec<u8>>,
}

impl Runner {
    fn new(
        recv_msg_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        send_msg_tx: mpsc::UnboundedSender<Vec<u8>>,
    ) -> Self {
        let (join_tx, join_rx)

        Runner {
        }
    }
}