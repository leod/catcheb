use std::{collections::HashMap, net::SocketAddr, time::Instant};

use log::{debug, info, warn};
use rand::seq::IteratorRandom;
use tokio::sync::{
    mpsc::{self, error::TryRecvError},
    oneshot,
};
use uuid::Uuid;

use comn::util::PingEstimation;

use crate::{
    game::Game,
    webrtc::{self, RecvMessageRx, SendMessageTx},
};

#[derive(Debug, Clone)]
pub struct Player {
    pub game_id: comn::GameId,
    pub player_id: comn::PlayerId,
    pub ping: PingEstimation,
    pub peer: Option<SocketAddr>,
    pub inputs: Vec<(comn::TickNum, comn::Input)>,
}

impl Player {
    pub fn new(game_id: comn::GameId, player_id: comn::PlayerId) -> Self {
        Self {
            game_id,
            player_id,
            ping: PingEstimation::default(),
            peer: None,
            inputs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub max_num_games: usize,
    pub game_settings: comn::Settings,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_num_games: 1000,
            game_settings: comn::Settings::default(),
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

    shutdown: bool,
    tick_timer: comn::util::Timer,
}

impl Runner {
    pub fn new(
        config: Config,
        recv_message_rx: RecvMessageRx,
        send_message_tx: SendMessageTx,
    ) -> Self {
        let (join_tx, join_rx) = mpsc::unbounded_channel();
        let tick_timer =
            comn::util::Timer::time_per_second(config.game_settings.ticks_per_second as f32);
        Runner {
            config,
            games: HashMap::new(),
            players: HashMap::new(),
            join_tx,
            join_rx,
            recv_message_rx,
            send_message_tx,
            shutdown: false,
            tick_timer,
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
                        let game = Game::new(self.config.game_settings.clone());

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

        let player = Player::new(game_id, player_id);

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
        while !self.shutdown {
            // Handle incoming join requests via HTTP channel
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

            // Handle incoming messages via WebRTC channel
            while let Some(message_in) = match self.recv_message_rx.try_recv() {
                Ok(message_in) => Some(message_in),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Closed) => {
                    info!("recv_message_rx closed, terminating thread");
                    return;
                }
            } {
                let signed_message = comn::SignedClientMessage::deserialize(&message_in.data);

                match signed_message {
                    Some(signed_message) => {
                        /*debug!(
                            "Received message from {:?}: {:?}",
                            message_in.peer, signed_message
                        );*/
                        self.handle_message(message_in.peer, message_in.recv_time, signed_message);
                    }
                    None => {
                        warn!(
                            "Failed to deserialize message from {:?}, ignoring",
                            message_in.peer,
                        );
                    }
                }
            }

            // Disconnect players
            let remove_player_tokens: Vec<comn::PlayerToken> = self
                .players
                .iter()
                .filter_map(|(player_token, player)| {
                    if player.ping.is_timeout() {
                        Some(*player_token)
                    } else {
                        None
                    }
                })
                .collect();

            for player_token in remove_player_tokens {
                let player = self.players.remove(&player_token).unwrap();
                info!(
                    "Player {:?} with token {:?} timed out",
                    player, player_token
                );
                self.games
                    .get_mut(&player.game_id)
                    .unwrap()
                    .remove_player(player.player_id);
            }

            // Ping players
            let mut messages = Vec::new();

            for player in self.players.values_mut() {
                if let Some(sequence_num) = player.ping.next_ping_sequence_num() {
                    if let Some(peer) = player.peer {
                        messages.push((peer, comn::ServerMessage::Ping(sequence_num)));
                    }
                }
            }

            for (peer, message) in messages {
                self.send(peer, message);
            }

            // Run the game
            while self.tick_timer.tick() {
                self.run_tick();
            }

            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    pub fn handle_message(
        &mut self,
        peer: SocketAddr,
        recv_time: Instant,
        message: comn::SignedClientMessage,
    ) {
        if let Some(player) = self.players.get_mut(&message.0) {
            if Some(peer) != player.peer {
                debug!("Changing peer from {:?} to {:?}", player.peer, peer);
                player.peer = Some(peer);
            }

            match message.1 {
                comn::ClientMessage::Ping(sequence_num) => {
                    self.send(peer, comn::ServerMessage::Pong(sequence_num));
                }
                comn::ClientMessage::Pong(sequence_num) => {
                    if player.ping.record_pong(recv_time, sequence_num).is_err() {
                        warn!("Ignoring out-of-order pong from {:?}", peer);
                    } else {
                        debug!(
                            "Received pong from {:?} -> estimation {:?}",
                            peer,
                            player.ping.estimate()
                        );
                    }
                }
                comn::ClientMessage::Input { tick_num, input } => {
                    player.inputs.push((tick_num, input));
                }
            }
        } else {
            warn!("Received message with unknown token, ignoring");
        }
    }

    fn run_tick(&mut self) {
        let mut inputs = HashMap::new();

        for &game_id in self.games.keys() {
            inputs.insert(game_id, Vec::new());
        }

        for player in self.players.values_mut() {
            // TODO: Consider player input timing
            for (_tick_num, input) in player.inputs.iter() {
                inputs
                    .get_mut(&player.game_id)
                    .unwrap()
                    .push((player.player_id, input.clone()));
            }
            player.inputs.clear();
        }

        for (game_id, game) in self.games.iter_mut() {
            let inputs = inputs[game_id].as_slice();
            //debug!("Updating {:?} with {} inputs", game_id, inputs.len());

            game.run_tick(inputs);
        }

        let mut messages = Vec::new();
        for player in self.players.values() {
            if let Some(peer) = player.peer {
                // TODO: Sending state properly
                let state = self.games[&player.game_id].state();
                let tick = comn::Tick {
                    entities: state.entities.clone(),
                    events: Vec::new(),
                    last_inputs: Default::default(), // TODO: send last_inputs
                };

                messages.push((
                    peer,
                    comn::ServerMessage::Tick {
                        tick_num: state.tick_num,
                        tick,
                    },
                ));
            }
        }

        for (peer, message) in messages {
            self.send(peer, message);
        }
    }

    pub fn send(&mut self, peer: SocketAddr, message: comn::ServerMessage) {
        let data = message.serialize();
        let message_out = webrtc::MessageOut { peer, data };

        if self.send_message_tx.send(message_out).is_err() {
            info!("send_message_tx closed, will terminate thread");
        }
    }
}
