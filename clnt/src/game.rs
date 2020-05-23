use instant::Instant;
use log::{debug, info, warn};
use std::{collections::BTreeMap, time::Duration};

use comn::util::{GameTimeEstimation, PingEstimation};

use crate::webrtc;

pub struct Game {
    my_token: comn::PlayerToken,
    my_player_id: comn::PlayerId,

    webrtc_client: webrtc::Client,

    state: comn::Game,
    next_tick: Option<(comn::TickNum, comn::Tick)>,
    received_ticks: BTreeMap<comn::TickNum, comn::Tick>,

    ping: PingEstimation,
    recv_tick_time: GameTimeEstimation,
    interp_game_time: comn::GameTime,
    next_time_warp_factor: f32,
}

impl Game {
    pub fn new(join: comn::JoinSuccess, webrtc_client: webrtc::Client) -> Self {
        Self {
            my_token: join.your_token,
            my_player_id: join.your_player_id,
            webrtc_client,
            state: comn::Game::new(join.game_settings.clone()),
            next_tick: None,
            received_ticks: BTreeMap::new(),
            ping: PingEstimation::default(),
            recv_tick_time: GameTimeEstimation::new(join.game_settings.tick_period()),
            interp_game_time: 0.0,
            next_time_warp_factor: 1.0,
        }
    }

    pub fn is_good(&self) -> bool {
        self.webrtc_client.status() == webrtc::Status::Open
    }

    pub fn target_time_lag(&self) -> comn::GameTime {
        self.state.settings.tick_period() * 3.0
    }

    pub fn next_time_warp_factor(&self) -> f32 {
        self.next_time_warp_factor
    }

    fn time_warp_factor(&self) -> f32 {
        if let Some(recv_game_time) = self.recv_tick_time.estimate(Instant::now()) {
            let current_time_lag = recv_game_time - self.interp_game_time;
            let time_lag_deviation = self.target_time_lag() - current_time_lag;

            0.5 + (2.0 - 0.5) / (1.0 + 2.0 * (time_lag_deviation / 0.05).exp())
        } else {
            0.0
        }
    }

    pub fn update(&mut self, dt: Duration, input: &comn::Input) {
        while let Some((recv_time, message)) = self.webrtc_client.take_message() {
            self.handle_message(recv_time, message);
        }
        if let Some(sequence_num) = self.ping.next_ping_sequence_num() {
            self.send(comn::ClientMessage::Ping(sequence_num));
        }

        // Advance our local game time, making sure to stay behind the
        // receive stream by our desired lag time. We do this so that
        // we have ticks between which we can interpolate.
        //
        // If we are off too far, slow down or speed up playback time.
        let new_interp_game_time =
            self.interp_game_time + dt.as_secs_f32() * self.next_time_warp_factor;

        // Don't let time run further than the ticks that we have received.
        let max_tick_num = self
            .received_ticks
            .keys()
            .rev()
            .next()
            .copied()
            .unwrap_or(comn::TickNum(0))
            .max(self.state.tick_num)
            .max(
                self.next_tick
                    .as_ref()
                    .map(|(tick_num, _)| *tick_num)
                    .unwrap_or(comn::TickNum(0)),
            );

        let new_interp_game_time =
            new_interp_game_time.min(self.state.tick_game_time(max_tick_num));

        // Start all the ticks that we have crossed while advancing our
        // local game time.
        let started_tick_nums: Vec<comn::TickNum> = self
            .received_ticks
            .keys()
            .copied()
            .filter(|tick_num| {
                let game_time = self.state.tick_game_time(*tick_num);
                self.state.tick_num < *tick_num && game_time <= new_interp_game_time
            })
            .collect();

        self.interp_game_time = new_interp_game_time;
        self.next_time_warp_factor = self.time_warp_factor();

        for tick_num in started_tick_nums.iter() {
            // TODO: Run events

            // TODO: Limit number of inputs to send, when skipping large numbers of ticks
            self.send(comn::ClientMessage::Input {
                tick_num: *tick_num,
                input: input.clone(),
            });
        }

        if let Some(last_tick_num) = started_tick_nums.last().copied() {
            self.snap_to_tick(last_tick_num, self.received_ticks[&last_tick_num].clone());

            if self
                .next_tick
                .as_ref()
                .map_or(false, |(next_tick_num, _)| *next_tick_num <= last_tick_num)
            {
                // We have reached the tick that we were interpolating into, so
                // we'll need to look for the next interpolation target.
                self.next_tick = None;
            }
        }

        // Do we have a tick to interpolate into ready?
        if self.next_tick.is_none() {
            let min_next_tick = self.received_ticks.iter().find(|(tick_num, _tick)| {
                **tick_num > self.state.tick_num && tick_num.0 - self.state.tick_num.0 <= 3
            });

            if let Some((next_tick_num, next_tick)) = min_next_tick {
                self.next_tick = Some((*next_tick_num, next_tick.clone()));
            }
        }

        // Prune received ticks that are no longer needed.
        let remove_tick_nums: Vec<comn::TickNum> = self
            .received_ticks
            .keys()
            .copied()
            .filter(|tick_num| *tick_num < self.state.tick_num)
            .collect();

        for tick_num in remove_tick_nums {
            self.received_ticks.remove(&tick_num);
        }
    }

    pub fn state(&self) -> &comn::Game {
        &self.state
    }

    pub fn next_tick(&self) -> Option<&(comn::TickNum, comn::Tick)> {
        self.next_tick.as_ref()
    }

    pub fn next_state(&self) -> BTreeMap<comn::EntityId, (comn::GameTime, comn::Entity)> {
        let mut next_state = BTreeMap::new();

        if let Some((next_tick_num, next_tick)) = self.next_tick() {
            let next_game_time = self.state.tick_game_time(*next_tick_num);

            next_state.extend(
                next_tick
                    .entities
                    .clone()
                    .into_iter()
                    .map(|(entity_id, entity)| (entity_id, (next_game_time, entity))),
            );
        }

        next_state
    }

    pub fn settings(&self) -> &comn::Settings {
        &self.state.settings
    }

    pub fn ping(&self) -> &PingEstimation {
        &self.ping
    }

    pub fn recv_tick_time(&self) -> &GameTimeEstimation {
        &self.recv_tick_time
    }

    pub fn interp_game_time(&self) -> f32 {
        self.interp_game_time
    }

    fn handle_message(&mut self, recv_time: Instant, message: comn::ServerMessage) {
        match message {
            comn::ServerMessage::Ping(_) => {
                // Handled in on_message callback to get better ping
                // estimates.
            }
            comn::ServerMessage::Pong(sequence_num) => {
                if self.ping.record_pong(recv_time, sequence_num).is_err() {
                    debug!("Ignoring out-of-order pong {:?}", sequence_num);
                } else {
                    debug!("Received pong -> estimation {:?}", self.ping.estimate());
                }
            }
            comn::ServerMessage::Tick { tick_num, tick } => {
                let recv_game_time = self.state.tick_game_time(tick_num);

                if recv_game_time < self.interp_game_time {
                    debug!(
                        "Ignoring old tick of time {} vs our interp_game_time={}",
                        recv_game_time, self.interp_game_time,
                    );
                } else {
                    if !self.recv_tick_time.has_started() {
                        // If this is the first tick we have recevied from the server, reset
                        // to the correct time
                        self.interp_game_time = recv_game_time;
                        self.snap_to_tick(tick_num, tick.clone());

                        // TODO: Run events

                        info!("Starting tick stream at recv_game_time={}", recv_game_time);
                    }

                    self.received_ticks.insert(tick_num, tick);
                }

                self.recv_tick_time.record_tick(recv_time, recv_game_time);
            }
        }
    }

    fn send(&self, message: comn::ClientMessage) {
        let signed_message = comn::SignedClientMessage(self.my_token, message);

        let data = signed_message.serialize();

        if let Err(err) = self.webrtc_client.send(&data) {
            warn!("Failed to send message: {:?}", err);
        }
    }

    fn snap_to_tick(&mut self, tick_num: comn::TickNum, tick: comn::Tick) {
        self.state.tick_num = tick_num;
        self.state.entities = tick.entities;
    }
}
