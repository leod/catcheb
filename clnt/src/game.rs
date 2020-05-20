use std::{collections::BTreeMap, time::Duration};

use instant::Instant;
use log::{debug, info, warn};

use comn::{
    util::{GameTimeEstimation, PingEstimation},
    GameTime,
};

use crate::webrtc;

pub struct Game {
    state: comn::Game,
    my_token: comn::PlayerToken,
    my_player_id: comn::PlayerId,
    webrtc_client: webrtc::Client,
    ping: PingEstimation,
    received_ticks: BTreeMap<comn::TickNum, comn::Tick>,
    recv_tick_time: GameTimeEstimation,
    interp_game_time: GameTime,
    target_time_lag: GameTime,
}

impl Game {
    pub fn new(join: comn::JoinSuccess, webrtc_client: webrtc::Client) -> Self {
        let target_time_lag = join.game_settings.tick_period() * 3.0;

        Self {
            state: comn::Game::new(join.game_settings.clone()),
            my_token: join.your_token,
            my_player_id: join.your_player_id,
            webrtc_client,
            ping: PingEstimation::default(),
            received_ticks: BTreeMap::new(),
            recv_tick_time: GameTimeEstimation::new(join.game_settings.tick_period()),
            interp_game_time: 0.0,
            target_time_lag,
        }
    }

    pub fn is_good(&self) -> bool {
        self.webrtc_client.status() == webrtc::Status::Open
    }

    pub fn time_warp_factor(&self) -> f32 {
        if self.received_ticks.is_empty() {
            return 0.0;
        }

        let recv_game_time = self.recv_tick_time.estimate(Instant::now());
        if let Some(recv_game_time) = recv_game_time {
            let current_time_lag = recv_game_time - self.interp_game_time;
            let time_lag_deviation = self.target_time_lag - current_time_lag;

            0.5 + (2.0 - 0.5) / (1.0 + 2.0 * (time_lag_deviation / 0.05).exp())
        } else {
            0.0
        }
    }

    pub fn update(&mut self, dt: std::time::Duration) {
        while let Some((recv_time, message)) = self.webrtc_client.take_message() {
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
                    let game_time = self.state.settings.tick_period() * tick_num.0 as f32;

                    self.recv_tick_time.record_tick(recv_time, game_time);

                    if game_time < self.interp_game_time {
                        debug!(
                            "Ignoring old tick of time {} vs our interp_game_time={}",
                            game_time, self.interp_game_time,
                        );
                    } else {
                        self.received_ticks.insert(tick_num, tick);
                    }
                }
            }
        }

        self.interp_game_time += dt.as_secs_f32() * self.time_warp_factor();

        if let Some(sequence_num) = self.ping.next_ping_sequence_num() {
            self.send(comn::ClientMessage::Ping(sequence_num));
        }

        let mut remove_num = None;
        if let Some((min_tick_num, min_tick)) = self.received_ticks.iter().next() {
            let min_tick_time = self.state.settings.tick_period() * min_tick_num.0 as f32;

            if self.interp_game_time >= min_tick_time {
                self.state.tick_num = *min_tick_num;
                self.state.entities = min_tick.entities.clone();
                remove_num = Some(*min_tick_num);
            }
        }
        if let Some(remove_num) = remove_num {
            self.received_ticks.remove(&remove_num);
        }
    }

    pub fn player_input(&mut self, input: &comn::Input) {
        // TODO: player_input tick_num
        let tick_num = comn::TickNum(0);

        self.send(comn::ClientMessage::Input {
            tick_num,
            input: input.clone(),
        });
    }

    pub fn state(&self) -> &comn::Game {
        &self.state
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

    fn send(&self, message: comn::ClientMessage) {
        let signed_message = comn::SignedClientMessage(self.my_token, message);

        let data = signed_message.serialize();

        if let Err(err) = self.webrtc_client.send(&data) {
            warn!("Failed to send message: {:?}", err);
        }
    }
}
