use std::collections::HashMap;

use log::info;
use tokio::sync::{
    mpsc::{self, error::TryRecvError},
    oneshot,
};
use uuid::Uuid;

use crate::{
    game::Game,
    webrtc::{RecvMessageRx, SendMessageTx},
};
use comn::{game, JoinReply, JoinRequest, JoinSuccess};

#[derive(Default, Debug, Clone)]
pub struct Config {}

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

    pub fn run(mut self) {
        loop {
            while let Some(join_message) = {
                match self.join_rx.try_recv() {
                    Ok(join_message) => Some(join_message),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Closed) => {
                        info!("join_rx closed, terminating thread");
                        return;
                    }
                }
            } {
                info!("Processing {:?}", join_message.request);

                let reply = Ok(JoinSuccess {
                    game_id: Uuid::new_v4(),
                    your_token_id: Uuid::new_v4(),
                    your_player_id: game::PlayerId(0),
                });

                if join_message.reply_tx.send(reply).is_err() {
                    info!("reply_tx closed, terminating thread");
                    return;
                }
            }

            std::thread::sleep_ms(5);
        }
    }
}
