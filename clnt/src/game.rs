use instant::Instant;
use log::{debug, info, warn};
use std::{collections::BTreeMap, time::Duration};

use comn::util::{GameTimeEstimation, PingEstimation};

use crate::{prediction::Prediction, webrtc};

pub struct Game {
    settings: comn::Settings,
    my_token: comn::PlayerToken,
    my_player_id: comn::PlayerId,

    webrtc_client: webrtc::Client,

    received_ticks: BTreeMap<comn::TickNum, comn::Tick>,
    prediction: Option<Prediction>,

    interp_game_time: comn::GameTime,
    next_tick_num: Option<comn::TickNum>,

    ping: PingEstimation,
    recv_tick_time: GameTimeEstimation,
    next_time_warp_factor: f32,
}

impl Game {
    pub fn new(join: comn::JoinSuccess, webrtc_client: webrtc::Client) -> Self {
        let prediction = Some(Prediction::new(join.your_player_id));
        let recv_tick_time = GameTimeEstimation::new(join.game_settings.tick_period());
        Self {
            settings: join.game_settings,
            my_token: join.your_token,
            my_player_id: join.your_player_id,
            webrtc_client,
            received_ticks: BTreeMap::new(),
            prediction,
            interp_game_time: 0.0,
            next_tick_num: None,
            ping: PingEstimation::default(),
            recv_tick_time,
            next_time_warp_factor: 1.0,
        }
    }

    pub fn my_player_id(&self) -> comn::PlayerId {
        self.my_player_id
    }

    pub fn is_good(&self) -> bool {
        self.webrtc_client.status() == webrtc::Status::Open
    }

    pub fn target_time_lag(&self) -> comn::GameTime {
        self.settings.tick_period() * 2.0
    }

    pub fn next_time_warp_factor(&self) -> f32 {
        self.next_time_warp_factor
    }

    pub fn next_tick_num(&self) -> Option<comn::TickNum> {
        self.next_tick_num
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

    pub fn update(&mut self, dt: Duration, input: &comn::Input) -> Vec<comn::Event> {
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
        // If we are off too far from our lag target, slow down or speed up
        // playback time.
        let new_interp_game_time =
            self.interp_game_time + dt.as_secs_f32() * self.next_time_warp_factor;

        // Don't let time run further than the ticks that we have received.
        // This is here so that we stop local time if the server drops or
        // starts lagging heavily.
        let max_tick_num = self
            .received_ticks
            .keys()
            .rev()
            .next()
            .copied()
            .unwrap_or(comn::TickNum(0))
            .max(self.tick_num())
            .max(self.next_tick_num.unwrap_or(comn::TickNum(0)));

        let new_interp_game_time =
            new_interp_game_time.min(self.settings.tick_game_time(max_tick_num));

        let prev_tick_num = self.tick_num();
        self.interp_game_time = new_interp_game_time;
        let new_tick_num = self.tick_num();
        self.next_time_warp_factor = self.time_warp_factor();

        let crossed_tick_nums: Vec<comn::TickNum> = (prev_tick_num.0 + 1..=new_tick_num.0)
            .map(|i| comn::TickNum(i))
            .collect();

        // Iterate over all the ticks that we have crossed, also including
        // those for which we did not anything from the server.
        let mut events = Vec::new();
        let mut last_server_tick_num = None;

        for tick_num in crossed_tick_nums.iter() {
            if let Some(tick) = self.received_ticks.get(tick_num) {
                events.extend(tick.events.clone().into_iter());
                last_server_tick_num = Some(*tick_num);
            }

            // TODO: Limit number of inputs to send, when skipping large numbers of ticks
            self.send(comn::ClientMessage::Input {
                tick_num: *tick_num,
                input: input.clone(),
            });

            if let Some(prediction) = self.prediction.as_mut() {
                prediction.record_tick_input(
                    *tick_num,
                    input.clone(),
                    self.received_ticks.get(tick_num),
                );
            }
        }

        if let Some(tick_num) = last_server_tick_num {
            if self
                .next_tick_num
                .map_or(false, |next_tick_num| next_tick_num <= tick_num)
            {
                // We have reached the tick that we were interpolating into, so
                // we'll need to look for the next interpolation target.
                self.next_tick_num = None;
            }
        }

        // Do we have a tick to interpolate into ready?
        if self.next_tick_num.is_none() {
            let min_ready_num = self.received_ticks.keys().find(|tick_num| {
                **tick_num > self.tick_num() && tick_num.0 - self.tick_num().0 <= 3
            });

            if let Some(min_ready_num) = min_ready_num {
                self.next_tick_num = Some(*min_ready_num);
            }
        }

        // Prune received ticks that are no longer needed.
        let remove_tick_nums: Vec<comn::TickNum> = self
            .received_ticks
            .keys()
            .copied()
            .filter(|tick_num| *tick_num < self.tick_num())
            .collect();

        for tick_num in remove_tick_nums {
            self.received_ticks.remove(&tick_num);
        }

        events
    }

    pub fn tick_num(&self) -> comn::TickNum {
        comn::TickNum((self.interp_game_time / self.settings.tick_period()) as u32)
    }

    pub fn state(&self) -> Option<&comn::Game> {
        if let Some(prediction) = self.prediction.as_ref() {
            prediction.predicted_state(self.tick_num())
        } else {
            self.received_ticks
                .get(&self.tick_num())
                .map(|tick| &tick.state)
        }
    }

    pub fn next_entities(&self) -> BTreeMap<comn::EntityId, (comn::GameTime, comn::Entity)> {
        let mut entities = BTreeMap::new();

        if let Some((recv_tick_num, recv_tick)) = self
            .next_tick_num
            .and_then(|key| self.received_ticks.get(&key).map(|value| (key, value)))
        {
            let recv_game_time = self.settings.tick_game_time(recv_tick_num);

            entities.extend(
                recv_tick
                    .state
                    .entities
                    .clone()
                    .into_iter()
                    .map(|(entity_id, entity)| (entity_id, (recv_game_time, entity))),
            );

            if let Some((prediction, predicted_state)) = self.prediction.as_ref().and_then(|p| {
                p.predicted_state(self.tick_num().next())
                    .map(|state| (p, state))
            }) {
                entities.extend(
                    predicted_state
                        .entities
                        .clone()
                        .into_iter()
                        .filter(|(_, entity)| prediction.is_predicted(entity))
                        .map(|(entity_id, entity)| {
                            (
                                entity_id,
                                (self.settings.tick_game_time(self.tick_num().next()), entity),
                            )
                        }),
                );
            }
        }

        entities
    }

    pub fn settings(&self) -> &comn::Settings {
        &self.settings
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
            comn::ServerMessage::Tick(tick) => {
                let recv_game_time = tick.state.current_game_time();

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

                        // TODO: Run events?

                        info!("Starting tick stream at recv_game_time={}", recv_game_time);
                    }

                    self.received_ticks.insert(tick.state.tick_num, tick);
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
}
