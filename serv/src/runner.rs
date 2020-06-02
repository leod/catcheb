use std::{
    collections::HashMap,
    net::SocketAddr,
    time::{Duration, Instant},
};

use log::{debug, info, warn};
use rand::seq::IteratorRandom;
use tokio::sync::{
    mpsc::{self, error::TryRecvError},
    oneshot,
};
use uuid::Uuid;

use comn::{
    util::{stats, GameTimeEstimation, PingEstimation, Timer},
    GameTime,
};

use crate::{
    game::Game,
    webrtc::{self, RecvMessageRx, SendMessageTx},
};

pub const PLAYER_INPUT_BUFFER: usize = 2;
pub const MAX_PLAYER_INPUT_AGE: f32 = 1.0;

#[derive(Debug, Clone)]
pub struct Player {
    pub game_id: comn::GameId,
    pub player_id: comn::PlayerId,
    pub peer: Option<SocketAddr>,
    pub ping: PingEstimation,
    pub last_input: Option<(comn::TickNum, comn::Input)>,
    pub inputs: Vec<(comn::TickNum, comn::Input)>,
    pub recv_input_time: GameTimeEstimation,
}

impl Player {
    pub fn new(input_period: GameTime, game_id: comn::GameId, player_id: comn::PlayerId) -> Self {
        Self {
            game_id,
            player_id,
            peer: None,
            ping: PingEstimation::default(),
            last_input: None,
            inputs: Vec::new(),
            recv_input_time: GameTimeEstimation::new(input_period),
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

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub num_players: stats::Var,
    pub num_games: stats::Var,
    pub num_inputs_per_player_tick: stats::Var,
    pub input_delay: stats::Var,
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
    tick_timer: Timer,

    stats: Stats,
    print_stats_timer: Timer,
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
            stats: Stats::default(),
            print_stats_timer: Timer::with_duration(Duration::from_secs(5)),
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

        let player = Player::new(game.settings().tick_period(), game_id, player_id);

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
            self.run_update();

            if self.print_stats_timer.exhaust().is_some() {
                debug!("num players:          {}", self.stats.num_players);
                debug!("num games:            {}", self.stats.num_games);
                debug!(
                    "inputs per game tick: {}",
                    self.stats.num_inputs_per_player_tick
                );
                debug!("input delay:          {}", self.stats.input_delay,);
            }

            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    pub fn run_update(&mut self) {
        // Handle incoming join requests via HTTP channel.
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

        // Handle incoming messages via WebRTC channel.
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

        // Disconnect players.
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
            info!("Player with token {:?} timed out", player_token);
            self.games
                .get_mut(&player.game_id)
                .unwrap()
                .remove_player(player.player_id);
        }

        // Ping players.
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

        // Run the game.
        while self.tick_timer.tick() {
            self.run_tick();
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
                        /*debug!(
                            "Received pong from {:?} -> estimation {:?}",
                            peer,
                            player.ping.estimate()
                        );*/
                    }
                }
                comn::ClientMessage::Input(inputs) => {
                    let game = &self.games[&player.game_id].state();

                    if inputs.len() == 0 || inputs.len() > comn::MAX_INPUTS_PER_MESSAGE {
                        warn!(
                            "Received invalid number of inputs ({}) from {:?}, ignoring",
                            inputs.len(),
                            message.0,
                        );
                        return;
                    }

                    let max_input_num = inputs.iter().map(|(tick_num, _)| *tick_num).max();

                    for (tick_num, input) in inputs {
                        if tick_num > game.tick_num {
                            // Clients try to stay behind the server in time, since
                            // they always interpolate behind old received state.
                            // Thus, with a correct client, this case should not
                            // happen. Ignoring input here may help prevent speed
                            // hacking.
                            warn!(
                                "Ignoring input {:?} by player {:?}, which is ahead of our tick num {:?}",
                                tick_num,
                                message.0,
                                game.tick_num,
                            );
                            continue;
                        }

                        {
                            let input_age =
                                game.current_game_time() - game.tick_game_time(tick_num);

                            if input_age > MAX_PLAYER_INPUT_AGE {
                                // TODO: Inform the client if they are lagging behind too much?
                                warn!(
                                    "Ignoring input {:?} by player {:?}, which is too old with age {}",
                                    tick_num, player.game_id, input_age,
                                );
                                continue;
                            }
                        }

                        // Ignore inputs for ticks that we have already
                        // performed for this player. This case is expected to
                        // happen regularly, since clients resend old inputs in
                        // order to tape over packet loss.
                        if player
                            .last_input
                            .as_ref()
                            .map_or(false, |(last_num, _)| tick_num <= *last_num)
                        {
                            continue;
                        }

                        // Sorted insert of the new input, so that inputs are
                        // sorted by tick number descending.
                        match player
                            .inputs
                            .binary_search_by(|(ex_tick_num, _)| tick_num.cmp(ex_tick_num))
                        {
                            Ok(_) => {
                                // We have received input for the same tick
                                // more than once, just ignore.
                            }
                            Err(pos) => {
                                player.inputs.insert(pos, (tick_num, input));
                            }
                        }

                        // Keep track of when we receive player input, so that
                        // we can predict when to receive the next player input.
                        // This results in a mapping from our game time to the
                        // receive game time.
                        if Some(tick_num) == max_input_num {
                            player.recv_input_time.record_tick(
                                game.current_game_time(),
                                game.tick_game_time(tick_num),
                            );
                        }
                    }
                }
            }
        } else {
            warn!("Received message with unknown token, ignoring");
        }
    }

    fn run_tick(&mut self) {
        let mut tick_inputs: HashMap<_, _> = self
            .games
            .keys()
            .map(|game_id| (*game_id, Vec::new()))
            .collect();

        // Collect player inputs to run.
        for (player_token, player) in self.players.iter_mut() {
            let game = &self.games[&player.game_id].state();
            let buffered_input_time = player
                .recv_input_time
                .estimate(
                    game.current_game_time()
                        - PLAYER_INPUT_BUFFER as GameTime * game.settings.tick_period(),
                )
                .unwrap_or(0.0);

            /*info!(
                "at {} have {:?} vs {:?}",
                game.current_game_time(),
                buffered_input_time,
                player.inputs.last().map(|(a, _)| game.tick_game_time(*a))
            );*/

            let mut player_tick_inputs = Vec::new();
            while let Some((oldest_tick_num, oldest_input)) = player.inputs.last().cloned() {
                if buffered_input_time >= game.tick_game_time(oldest_tick_num) {
                    player_tick_inputs.push((oldest_tick_num, oldest_input));
                    player.inputs.pop();
                } else {
                    break;
                }
            }

            for (input_num, _) in player_tick_inputs.iter() {
                self.stats
                    .input_delay
                    .record((game.tick_num.0 - input_num.0) as f32);
            }

            if player_tick_inputs.is_empty() {
                // We did not receive the correct input in time, just reuse
                // the previous one.
                if let Some((last_input_num, last_input)) = player.last_input.clone() {
                    player_tick_inputs.push((last_input_num.next(), last_input));
                    debug!("Reusing input for player {:?}", player_token);
                }
            }

            player.last_input = player_tick_inputs.last().cloned();

            tick_inputs.get_mut(&player.game_id).unwrap().extend(
                player_tick_inputs
                    .into_iter()
                    .map(|(tick_num, input)| (player.player_id, tick_num, input)),
            );
        }

        // Record some statistics for monitoring.
        self.stats.num_players.record(self.players.len() as f32);
        self.stats.num_games.record(self.games.len() as f32);
        self.stats.num_inputs_per_player_tick.record(
            tick_inputs
                .values()
                .map(|inputs| inputs.len() as f32)
                .sum::<f32>()
                / self.players.len() as f32,
        );

        // Update the games.
        for (game_id, game) in self.games.iter_mut() {
            game.run_tick(tick_inputs[game_id].as_slice());
        }

        // Send out tick messages.
        let mut messages = Vec::new();
        for player in self.players.values() {
            if let Some(peer) = player.peer {
                // TODO: Delta encode tick state.
                let game = &self.games[&player.game_id];
                let tick = comn::Tick {
                    state: game.state.clone(),
                    events: game.last_events.clone(),
                    your_last_input: player.last_input.clone().map(|(tick_num, _)| tick_num),
                };

                messages.push((peer, comn::ServerMessage::Tick(tick)));
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
