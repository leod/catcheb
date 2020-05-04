use std::{collections::HashMap, net::SocketAddr};

use log::{info, warn};
use rand::seq::IteratorRandom;
use tokio::sync::{
    mpsc::{self, error::TryRecvError},
    oneshot,
};
use uuid::Uuid;

use comn::util::PingEstimation;

use crate::{
    game::{self, Game},
    webrtc::{RecvMessageRx, SendMessageTx},
};

pub struct Player {
    pub game_id: comn::GameId,
    pub player_id: comn::PlayerId,
    pub ping_estimation: PingEstimation,
    pub peer: Option<SocketAddr>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub max_num_games: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_num_games: 1000,
        }
    }
}

pub struct JoinMessage {
    pub request: comn::JoinRequest,
    pub reply_tx: oneshot::Sender<comn::JoinReply>,
}

// TODO: Check if we should make channels bounded
pub type JoinTx = mpsc::UnboundedSender<JoinMessage>;
pub type JoinRx = mpsc::UnboundedReceiver<JoinMessage>;

pub struct Runner {
    config: Config,

    games: HashMap<comn::GameId, Game>,
    players: HashMap<comn::PlayerToken, Player>,

    join_tx: JoinTx,
    join_rx: JoinRx,

    recv_message_rx: RecvMessageRx,
    send_message_tx: SendMessageTx,
}

impl Runner {
    pub fn new(
        config: Config,
        recv_message_rx: RecvMessageRx,
        send_message_tx: SendMessageTx,
    ) -> Self {
        let (join_tx, join_rx) = mpsc::unbounded_channel();
        Runner {
            config,
            games: HashMap::new(),
            players: HashMap::new(),
            join_tx,
            join_rx,
            recv_message_rx,
            send_message_tx,
        }
    }

    pub fn join_tx(&self) -> mpsc::UnboundedSender<JoinMessage> {
        self.join_tx.clone()
    }

    pub fn try_join_game(&mut self, request: comn::JoinRequest) -> comn::JoinReply {
        let (game_id, game) = if let Some(game_id) = request.game_id {
            // The player requested to join a specific game.
            match self.games.get_mut(&game_id) {
                Some(game) => {
                    if game.is_full() {
                        info!("Game is full");
                        return Err(comn::JoinError::FullGame);
                    } else {
                        (game_id, game)
                    }
                }
                None => {
                    info!("game_id is invalid");
                    return Err(comn::JoinError::InvalidGameId);
                }
            }
        } else {
            // The player wants to join just any game.
            let non_full_games = self
                .games
                .iter_mut()
                .filter(|(_game_id, game)| !game.is_full());

            match non_full_games.choose(&mut rand::thread_rng()) {
                Some((&game_id, game)) => (game_id, game),
                None => {
                    // All games are full.
                    assert!(self.games.len() <= self.config.max_num_games);
                    if self.games.len() == self.config.max_num_games {
                        // Reached game limit.
                        warn!(
                            "All games are full and we have reached the game limit of {}",
                            self.config.max_num_games
                        );
                        return Err(comn::JoinError::FullGame);
                    } else {
                        // Create a new game.
                        let game_id = comn::GameId(Uuid::new_v4());
                        let game = Game::new(comn::Settings::default());

                        self.games.insert(game_id, game);

                        info!(
                            "All games are full, created a new one with id {:?}",
                            game_id
                        );

                        (game_id, self.games.get_mut(&game_id).unwrap())
                    }
                }
            }
        };

        let player_token = comn::PlayerToken(Uuid::new_v4());
        let player_id = game.join(request.player_name);

        let player = Player {
            game_id,
            player_id,
            ping_estimation: PingEstimation::default(),
            peer: None,
        };

        assert!(!self.players.contains_key(&player_token));
        self.players.insert(player_token, player);

        Ok(comn::JoinSuccess {
            game_id,
            game_settings: game.settings().clone(),
            your_token: player_token,
            your_player_id: player_id,
        })
    }

    pub fn run(mut self) {
        loop {
            while let Some(join_message) = match self.join_rx.try_recv() {
                Ok(join_message) => Some(join_message),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Closed) => {
                    info!("join_rx closed, terminating thread");
                    return;
                }
            } {
                info!("Processing {:?}", join_message.request);

                let reply = self.try_join_game(join_message.request);

                if join_message.reply_tx.send(reply).is_err() {
                    info!("reply_tx closed, terminating thread");
                    return;
                }
            }

            while let Some(message_in) = match self.recv_message_rx.try_recv() {
                Ok(message_in) => Some(message_in),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Closed) => {
                    info!("recv_message_rx closed, terminating thread");
                    return;
                }
            } {
                info!("Received message from {:?}", message_in.peer);
            }

            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}
