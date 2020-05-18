use std::{collections::VecDeque, time::Duration};

use instant::Instant;
use log::{debug, info, warn};

use comn::util::{stats, PingEstimation};

use crate::webrtc;

pub struct GameTimeEstimation {
    tick_duration: f32,
    recv_tick_times: VecDeque<(Instant, comn::TickNum)>,
}

impl GameTimeEstimation {
    pub fn new(ticks_per_second: usize) -> Self {
        Self {
            tick_duration: 1.0 / ticks_per_second as f32,
            recv_tick_times: VecDeque::new(),
        }
    }

    pub fn record_tick(&mut self, recv_time: Instant, num: comn::TickNum) {
        if let Some((last_time, last_num)) = self.recv_tick_times.back() {
            if num < *last_num {
                // Received packages out of order, just ignore
                return;
            }

            assert!(recv_time >= *last_time);
        }

        self.recv_tick_times.push_back((recv_time, num));

        if self.recv_tick_times.len() > 128 {
            self.recv_tick_times.pop_front();
        }
    }

    pub fn shifted_recv_tick_times(&self) -> Option<impl Iterator<Item = (f32, f32)> + '_> {
        self.recv_tick_times
            .front()
            .copied()
            .map(|(first_time, first_num)| {
                self.recv_tick_times.iter().map(move |(time, num)| {
                    let delta_time = time.duration_since(first_time).as_secs_f32();
                    let delta_game_time = self.tick_duration * (num.0 - first_num.0) as f32;

                    (delta_time, delta_game_time)
                })
            })
    }

    pub fn linear_regression(&self) -> Option<stats::LinearRegression> {
        self.shifted_recv_tick_times()
            .map(|samples| stats::linear_regression_with_beta(1.0, samples))
    }

    pub fn recv_delay_std_dev(&self) -> Option<f32> {
        self.shifted_recv_tick_times().map(|samples| {
            let samples: Vec<(f32, f32)> = samples.collect();
            let line = stats::linear_regression_with_beta(1.0, samples.iter().copied());

            let recv_delay = samples
                .iter()
                .map(|(delta_time, delta_game_time)| line.eval(*delta_time) - delta_game_time);

            stats::std_dev(recv_delay)
        })
    }

    pub fn estimate(&self, ping: &PingEstimation, now: Instant) -> Option<f32> {
        self.recv_tick_times
            .front()
            .and_then(|(first_time, first_num)| {
                self.shifted_recv_tick_times().map(|samples| {
                    let line = stats::linear_regression_with_beta(1.0, samples);
                    let delta_time = now.duration_since(*first_time).as_secs_f32();
                    let delta_game_time = line.eval(delta_time);

                    delta_game_time + self.tick_duration * first_num.0 as f32
                })
            })
    }
}

pub struct Game {
    state: comn::Game,
    my_token: comn::PlayerToken,
    my_player_id: comn::PlayerId,
    webrtc_client: webrtc::Client,
    ping: PingEstimation,
    recv_tick_time: GameTimeEstimation,
}

impl Game {
    pub fn new(join: comn::JoinSuccess, webrtc_client: webrtc::Client) -> Self {
        Self {
            state: comn::Game::new(join.game_settings.clone()),
            my_token: join.your_token,
            my_player_id: join.your_player_id,
            webrtc_client,
            ping: PingEstimation::default(),
            recv_tick_time: GameTimeEstimation::new(join.game_settings.ticks_per_second),
        }
    }

    pub fn is_good(&self) -> bool {
        self.webrtc_client.status() == webrtc::Status::Open
    }

    pub fn update(&mut self) {
        while let Some((recv_time, message)) = self.webrtc_client.take_message() {
            match message {
                comn::ServerMessage::Ping(_) => {
                    // Handled in on_message callback to get better ping
                    // estimates.
                }
                comn::ServerMessage::Pong(sequence_num) => {
                    if self.ping.record_pong(recv_time, sequence_num).is_err() {
                        warn!("Ignoring out-of-order pong {:?}", sequence_num);
                    } else {
                        debug!("Received pong -> estimation {:?}", self.ping.estimate());
                    }
                }
                comn::ServerMessage::Tick { tick_num, tick } => {
                    self.recv_tick_time.record_tick(recv_time, tick_num);
                    self.state.tick_num = tick_num;
                    self.state.entities = tick.entities;
                }
            }
        }

        if let Some(sequence_num) = self.ping.next_ping_sequence_num() {
            self.send(comn::ClientMessage::Ping(sequence_num));
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

    fn send(&self, message: comn::ClientMessage) {
        let signed_message = comn::SignedClientMessage(self.my_token, message);

        let data = signed_message.serialize();

        if let Err(err) = self.webrtc_client.send(&data) {
            warn!("Failed to send message: {:?}", err);
        }
    }
}
