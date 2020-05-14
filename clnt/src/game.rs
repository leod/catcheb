use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use log::{debug, info, warn};

use comn::util::PingEstimation;

use crate::webrtc;

struct ServerTimeEstimation {
    tick_duration: Duration,
    recv_tick_times: VecDeque<(Instant, comn::TickNum)>,
}

impl ServerTimeEstimation {
    fn new(tick_duration: Duration) -> Self {
        Self {
            tick_duration,
            recv_tick_times: VecDeque::new(),
        }
    }

    fn record_tick(&mut self, recv_time: Instant, num: comn::TickNum) {
        self.recv_tick_times.push_back((recv_time, num));

        if self.recv_tick_times.len() > 10 {
            self.recv_tick_times.pop_front();
        }
    }
}

pub struct Game {
    state: comn::Game,
    my_token: comn::PlayerToken,
    my_player_id: comn::PlayerId,
    webrtc_client: webrtc::Client,
    ping: PingEstimation,
    server_time: ServerTimeEstimation,
}

impl Game {
    pub fn new(join: comn::JoinSuccess, webrtc_client: webrtc::Client) -> Self {
        Self {
            state: comn::Game::new(join.game_settings.clone()),
            my_token: join.your_token,
            my_player_id: join.your_player_id,
            webrtc_client,
            ping: PingEstimation::default(),
            server_time: ServerTimeEstimation::new(join.game_settings.tick_duration()),
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

    fn send(&self, message: comn::ClientMessage) {
        let signed_message = comn::SignedClientMessage(self.my_token, message);

        let data = signed_message.serialize();

        if let Err(err) = self.webrtc_client.send(&data) {
            warn!("Failed to send message: {:?}", err);
        }
    }
}
