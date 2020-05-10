use log::{debug, info, warn};

use comn::util::PingEstimation;

use crate::webrtc;

pub struct Game {
    state: comn::Game,
    my_token: comn::PlayerToken,
    my_player_id: comn::PlayerId,
    webrtc_client: webrtc::Client,
    ping_estimation: PingEstimation,
}

impl Game {
    pub fn new(join: comn::JoinSuccess, webrtc_client: webrtc::Client) -> Self {
        Self {
            state: comn::Game::new(join.game_settings.clone()),
            my_token: join.your_token,
            my_player_id: join.your_player_id,
            webrtc_client,
            ping_estimation: PingEstimation::default(),
        }
    }

    pub fn is_good(&self) -> bool {
        self.webrtc_client.status() == webrtc::Status::Open
    }

    pub fn update(&mut self) {
        while let Some((recv_time, message)) = self.webrtc_client.take_message() {
            match message {
                comn::ServerMessage::Ping(sequence_num) => {
                    self.send(comn::ClientMessage::Pong(sequence_num));
                }
                comn::ServerMessage::Pong(sequence_num) => {
                    if self
                        .ping_estimation
                        .received_pong(recv_time, sequence_num)
                        .is_err()
                    {
                        warn!("Ignoring out-of-order pong {:?}", sequence_num);
                    } else {
                        debug!(
                            "Received pong -> estimation {:?}",
                            self.ping_estimation.estimate()
                        );
                    }
                }
                comn::ServerMessage::Tick(tick) => {
                    self.state.entities = tick.entities;
                }
            }
        }

        if let Some(sequence_num) = self.ping_estimation.next_ping_sequence_num() {
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

    pub fn ping_estimation(&self) -> &PingEstimation {
        &self.ping_estimation
    }

    fn send(&self, message: comn::ClientMessage) {
        let signed_message = comn::SignedClientMessage(self.my_token, message);

        let data = signed_message.serialize();

        if let Err(err) = self.webrtc_client.send(&data) {
            warn!("Failed to send message: {:?}", err);
        }
    }
}
