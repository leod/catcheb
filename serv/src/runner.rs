use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    sync::Arc,
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
    util::{diff::Diffable, stats, GameTimeEstimation, PingEstimation, Timer},
    GameTime,
};

use crate::{
    bot::Bot,
    game::Game,
    webrtc::{self, RecvMessageRx, SendMessageTx},
};

const PLAYER_INPUT_BUFFER: f32 = 1.5;
const MAX_PLAYER_INPUT_AGE: f32 = 1.0;
const MAX_DIFF_TICKS: u32 = 50;

#[derive(Debug, Clone)]
struct Player {
    /// Each player is in exactly one running game.
    game_id: comn::GameId,

    /// The player id is unique only in the game.
    player_id: comn::PlayerId,

    /// WebRTC peer address.
    peer: Option<SocketAddr>,

    /// Ping estimation.
    ping: PingEstimation,

    /// The last input that the player executed, if any. We remember this so
    /// that we can re-execute the input if we do not receive the packet for
    /// some tick.
    last_input: Option<(comn::TickNum, comn::Input)>,

    /// Inputs that we received from this player recently. The TickNum key is
    /// the tick that the player *saw* while it executed the input. In our
    /// server time, this tick will be somewhere in the past. Note that we try
    /// to buffer the inputs slightly (see `PLAYER_INPUT_BUFFER`), so that we
    /// can try to hide network jitter. Inputs are sorted by TickNum
    /// descending.
    inputs: Vec<(comn::TickNum, comn::Input)>,

    /// We estimate a function which maps from our `GameTime` to the input
    /// stream `GameTime`. This is used for buffering `inputs`.
    recv_input_time: GameTimeEstimation,

    /// Last tick that the player has acknowledged receiving from us. Used as
    /// the basis for delta encoding.
    last_ack_tick: Option<comn::TickNum>,

    /// Last states that we have sent to the player, ordered by the tick number
    /// ascending.
    last_sent: VecDeque<(Vec<comn::Event>, comn::Game)>,
}

impl Player {
    fn new(input_period: GameTime, game_id: comn::GameId, player_id: comn::PlayerId) -> Self {
        Self {
            game_id,
            player_id,
            peer: None,
            ping: PingEstimation::default(),
            last_input: None,
            inputs: Vec::new(),
            recv_input_time: GameTimeEstimation::new(input_period),
            last_ack_tick: None,
            last_sent: VecDeque::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub max_num_games: usize,
    pub game_settings: comn::Settings,
}

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub num_players: stats::Var,
    pub num_games: stats::Var,
    pub num_inputs_per_player_tick: stats::Var,
    pub input_delay: stats::Var,
    pub last_sent_len: stats::Var,
    pub tick_message_size: stats::Var,
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

    shutdown_rx: oneshot::Receiver<()>,
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
        shutdown_rx: oneshot::Receiver<()>,
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
            shutdown_rx,
            shutdown: false,
            tick_timer,
            stats: Stats::default(),
            print_stats_timer: Timer::with_duration(Duration::from_secs(5)),
        }
    }

    pub fn join_tx(&self) -> mpsc::UnboundedSender<JoinMessage> {
        self.join_tx.clone()
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
                debug!("input delay:          {}", self.stats.input_delay);
                debug!("last sent len:        {}", self.stats.last_sent_len);
                debug!("tick message size:    {}", self.stats.tick_message_size);
            }

            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    fn run_update(&mut self) {
        // Handle external shutdown requests.
        if self.shutdown_rx.try_recv().is_ok() {
            info!("Sending disconnect messages to clients...");

            // Send unreliable disconnect messages a few times to increase
            // chance of arrival.
            let peers: Vec<_> = self
                .players
                .values()
                .filter_map(|player| player.peer)
                .collect();

            for _ in 0..3 {
                for &peer in &peers {
                    self.send(peer, comn::ServerMessage::Disconnect);
                }
            }

            // Wait a little bit to allow WebRTC to send packages.
            std::thread::sleep(Duration::from_secs(1));

            info!("Finished shutting down");

            self.shutdown = true;
            return;
        }

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
                if player.ping.is_timeout(Instant::now()) {
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
            if let Some(sequence_num) = player.ping.next_ping_sequence_num(Instant::now()) {
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

    fn handle_message(
        &mut self,
        peer: SocketAddr,
        recv_time: Instant,
        message: comn::SignedClientMessage,
    ) {
        let player = if let Some(player) = self.players.get_mut(&message.0) {
            player
        } else {
            warn!("Received message with unknown token, ignoring");
            return;
        };

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
                    warn!("Ignoring pong with invalid sequence number from {:?}", peer);
                }
            }
            comn::ClientMessage::Input(inputs) => {
                self.record_player_input(message.0, &inputs);
            }
            comn::ClientMessage::AckTick(ack_num) => {
                self.record_player_ack_tick(message.0, ack_num);
            }
            comn::ClientMessage::Disconnect => {
                debug!("Player {:?} disconnected", message.0);

                let game = self.games.get_mut(&player.game_id).unwrap();
                game.remove_player(player.player_id);
                self.players.remove(&message.0);
            }
        }
    }

    fn run_tick(&mut self) {
        let tick_inputs = self.collect_player_inputs_for_tick();

        // Record some statistics for monitoring.
        self.stats.num_players.record(self.players.len() as f32);
        self.stats.num_games.record(self.games.len() as f32);
        self.stats.num_inputs_per_player_tick.record(
            tick_inputs
                .values()
                .map(|inputs| inputs.len() as f32)
                .sum::<f32>()
                / (self.players.len() as f32 * self.games.len() as f32),
        );

        // Update the games given the player inputs.
        for (game_id, game) in self.games.iter_mut() {
            game.run_tick(tick_inputs[game_id].as_slice());
        }

        // Send out tick messages.
        let mut messages = Vec::new();
        for player in self.players.values_mut() {
            if let Some(peer) = player.peer {
                let game = &self.games[&player.game_id];
                let tick = Self::prepare_tick_for_player(player, game);
                messages.push((peer, comn::ServerMessage::Tick(tick)));

                self.stats
                    .last_sent_len
                    .record(player.last_sent.len() as f32);
            }
        }

        for (peer, message) in messages {
            // FIXME: Here, we will run into a problem as soon as a state
            // update is larger than the MTU of WebRTC (1200 Bytes, AFAIK).
            // We'll need to do one of two things:
            // 1. Make the tick smaller by removing the least important
            //    updates. For example, remove the oldest events, or the
            //    entities that are the farthest away.
            // 2. Implement sending fragmented packets.
            self.send(peer, message);
        }
    }

    fn send(&mut self, peer: SocketAddr, message: comn::ServerMessage) {
        let data = message.serialize();

        if let comn::ServerMessage::Tick(_) = message {
            self.stats.tick_message_size.record(data.len() as f32);
        }

        let message_out = webrtc::MessageOut { peer, data };

        if self.send_message_tx.send(message_out).is_err() {
            info!("send_message_tx closed, will terminate thread");
        }
    }

    fn record_player_input(
        &mut self,
        player_token: comn::PlayerToken,
        inputs: &[(comn::TickNum, comn::Input)],
    ) {
        let player = self.players.get_mut(&player_token).unwrap();
        let game = &self.games[&player.game_id].state;

        if inputs.is_empty() || inputs.len() > comn::MAX_INPUTS_PER_MESSAGE {
            warn!(
                "Received invalid number of inputs ({}) from {:?}, ignoring",
                inputs.len(),
                player_token,
            );
            return;
        }

        // Keep track of when we receive player input, so that we can predict
        // when we'll receive the next player input. This results in a mapping
        // from our game time to the player's input game time as we receive it.
        {
            let max_input_num = inputs.iter().map(|(tick_num, _)| *tick_num).max().unwrap();
            player
                .recv_input_time
                .record_tick(game.game_time(), game.tick_game_time(max_input_num));
        }

        for (input_num, input) in inputs {
            // Ignore inputs that are too far in the past or even ahead of our
            // time (the latter case should not happen for a correct client).
            {
                let input_age = game.game_time() - game.tick_game_time(*input_num);

                if input_age < 0.0 || input_age > MAX_PLAYER_INPUT_AGE {
                    // TODO: Inform the client if they are lagging behind too much?
                    /*warn!(
                        "Received input {:?} by player {:?} with age {}, ignoring",
                        input_num, player_token, input_age,
                    );*/
                    continue;
                }
            }

            // Ignore inputs for ticks that we have already performed for this
            // player. This case is expected to happen regularly, since clients
            // resend old inputs in order to tape over packet loss.
            if player
                .last_input
                .as_ref()
                .map_or(false, |(last_input_num, _)| *input_num <= *last_input_num)
            {
                continue;
            }

            // Sorted insert of the new input, so that inputs are sorted by tick
            // number descending.
            match player
                .inputs
                .binary_search_by(|(other_input_num, _)| input_num.cmp(other_input_num))
            {
                Ok(_) => {
                    // We have received input for the same tick
                    // more than once, just ignore.
                }
                Err(pos) => {
                    player.inputs.insert(pos, (*input_num, input.clone()));
                }
            }
        }
    }

    fn record_player_ack_tick(&mut self, player_token: comn::PlayerToken, ack_num: comn::TickNum) {
        let player = self.players.get_mut(&player_token).unwrap();
        let game = &self.games[&player.game_id].state;

        if ack_num > game.tick_num {
            warn!(
                "Received AckTick from {:?} which is ahead of us ({:?} vs {:?}), ignoring",
                player_token, game.tick_num, ack_num,
            );
        } else if player
            .last_ack_tick
            .map_or(true, |last_ack_num| ack_num > last_ack_num)
        {
            player.last_ack_tick = Some(ack_num);

            // We can now forget all the states that are older than the one
            // whose acknowledgment we just received.
            while player
                .last_sent
                .front()
                .map_or(false, |(_events, state)| state.tick_num < ack_num)
            {
                player.last_sent.pop_front();
            }
        }
    }

    fn try_join_game(&mut self, request: comn::JoinRequest) -> comn::JoinReply {
        let game_id = self.get_non_full_game_to_join(request.game_id)?;
        let game = self.games.get_mut(&game_id).unwrap();
        assert!(!game.is_full());

        let player_token = comn::PlayerToken(Uuid::new_v4());
        assert!(!self.players.contains_key(&player_token));

        let player_id = game.join(request.player_name, None);
        let player = Player::new(game.settings().tick_period(), game_id, player_id);
        self.players.insert(player_token, player);

        Ok(comn::JoinSuccess {
            game_id,
            game_settings: game.settings().clone(),
            your_token: player_token,
            your_player_id: player_id,
        })
    }

    fn get_non_full_game_to_join(
        &mut self,
        game_id: Option<comn::GameId>,
    ) -> Result<comn::GameId, comn::JoinError> {
        if let Some(game_id) = game_id {
            // The player requested to join a specific game.
            if let Some(game) = self.games.get(&game_id) {
                if game.is_full() {
                    info!("Game is full");
                    Err(comn::JoinError::FullGame)
                } else {
                    Ok(game_id)
                }
            } else {
                info!("game_id is invalid");
                Err(comn::JoinError::InvalidGameId)
            }
        } else {
            // The player wants to join just any game.
            let non_full_games = self.games.iter().filter(|(_, game)| !game.is_full());

            if let Some((game_id, _)) = non_full_games.choose(&mut rand::thread_rng()) {
                Ok(*game_id)
            } else if self.games.len() == self.config.max_num_games {
                // All games are full, and we have reached the game limit.
                // Reject the join request.
                warn!(
                    "All games are full and we have reached the game limit of {}",
                    self.config.max_num_games
                );
                Err(comn::JoinError::FullGame)
            } else {
                // We still have capacity, create a new game.
                assert!(self.games.len() < self.config.max_num_games);

                let game_id = self.add_game();
                info!(
                    "All games are full, created a new one with id {:?}",
                    game_id
                );
                Ok(game_id)
            }
        }
    }

    fn add_game(&mut self) -> comn::GameId {
        let game_id = comn::GameId(Uuid::new_v4());
        let mut game = Game::new(Arc::new(self.config.game_settings.clone()));

        /*for i in 0..2 {
            game.join(format!("random_bot{}", i), Some(Bot::random()));
        }*/

        for i in 0..2 {
            game.join(format!("left_right_bot{}", i), Some(Bot::left_right(2.0)));
        }

        assert!(!self.games.contains_key(&game_id));
        self.games.insert(game_id, game);

        game_id
    }

    fn collect_player_inputs_for_tick(
        &mut self,
    ) -> HashMap<comn::GameId, Vec<(comn::PlayerId, comn::TickNum, comn::Input)>> {
        let mut tick_inputs: HashMap<_, _> = self
            .games
            .keys()
            .map(|game_id| (*game_id, Vec::new()))
            .collect();

        for player in self.players.values_mut() {
            let game = &self.games[&player.game_id].state;

            // We explicitly buffer player inputs for some time, so that we can
            // deal with jitter.
            let lag = PLAYER_INPUT_BUFFER * game.settings.tick_period();
            let buffered_input_time = player
                .recv_input_time
                .estimate(game.game_time() - lag)
                .unwrap_or(0.0);

            let mut player_tick_inputs = Vec::new();
            while let Some((oldest_tick_num, oldest_input)) = player.inputs.last().cloned() {
                if buffered_input_time < game.tick_game_time(oldest_tick_num) {
                    // This input is not ready to be used yet. Same for any
                    // newer input.
                    break;
                }

                self.stats
                    .input_delay
                    .record((game.tick_num.0 - oldest_tick_num.0) as f32);

                player_tick_inputs.push((player.player_id, oldest_tick_num, oldest_input));
                player.inputs.pop();
            }

            if player_tick_inputs.is_empty() {
                // We did not receive the matching input in time, just reuse the
                // previous one.
                if let Some((last_input_num, last_input)) = player.last_input.clone() {
                    debug!("Reusing input for player {:?}", player.player_id);

                    player_tick_inputs.push((player.player_id, last_input_num.next(), last_input));
                }
            }

            player.last_input = player_tick_inputs
                .last()
                .map(|(_, input_num, input)| (*input_num, input.clone()));

            tick_inputs
                .get_mut(&player.game_id)
                .unwrap()
                .extend(player_tick_inputs.into_iter());
        }

        tick_inputs
    }

    fn prepare_tick_for_player(player: &mut Player, game: &Game) -> comn::Tick {
        let mut state = game.state.clone();
        game.prepare_state_for_player(player.player_id, &mut state);

        let mut events = vec![(game.state.tick_num, game.last_events.clone())];

        // Attempt to do delta encoding w.r.t. a previous state if
        // possible.
        let ack_num_and_sent_state = player
            .last_ack_tick
            .and_then(|ack_num| {
                player
                    .last_sent
                    .front()
                    .as_ref()
                    .map(|(_, sent_state)| (ack_num, sent_state))
            })
            .filter(|(ack_num, sent_state)| {
                *ack_num == sent_state.tick_num && ack_num.0 + MAX_DIFF_TICKS > state.tick_num.0
            });

        let (diff_base, diff) = if let Some((ack_num, sent_state)) = ack_num_and_sent_state {
            // Okay, we know that the player has acknowledged a tick for which
            // we also still have the state. We can use this state as the basis
            // for delta encoding.

            // Re-send all the events that happened since the base tick.
            for (sent_events, sent_state) in player.last_sent.iter() {
                if !sent_events.is_empty() {
                    events.push((sent_state.tick_num, sent_events.clone()));
                }
            }

            (Some(ack_num), sent_state.diff(&state))
        } else {
            // We cannot do delta encoding.
            info!(
                "Sending tick {:?} from scratch to {:?} (last ack: {:?})",
                game.state.tick_num, player.player_id, player.last_ack_tick,
            );
            let base_state = comn::Game::new(game.state.settings.clone());
            (None, base_state.diff(&state))
        };

        // Remember the state we're sending, so that we may use it as the basis
        // for delta encoding in the future (assuming that we will receive the
        // client's receival acknowledgement).
        player
            .last_sent
            .push_back((game.last_events.clone(), state.clone()));

        // Prune the state memory. This should be rarely necessary, since we
        // already prune states when we receive acknowledgements.
        if player.last_sent.len() > MAX_DIFF_TICKS as usize {
            /*warn!(
                "Player {:?}'s state memory grew too long ({}), pruning",
                player.player_id,
                player.last_sent.len(),
            );*/

            player.last_sent.pop_front();
        }

        comn::Tick {
            diff_base,
            diff,
            events,
            your_last_input_num: player.last_input.clone().map(|(num, _)| num),
        }
    }
}
