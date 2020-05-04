use log::{debug, info, warn};

use comn::util::PingEstimation;

use crate::webrtc;

pub struct Game {
    game_settings: comn::Settings,
    my_token: comn::PlayerToken,
    my_player_id: comn::PlayerId,
    webrtc_client: webrtc::Client,
    ping_estimation: PingEstimation,
}

impl Game {
    pub fn new(join: comn::JoinSuccess, webrtc_client: webrtc::Client) -> Self {
        Self {
            game_settings: join.game_settings.clone(),
            my_token: join.your_token,
            my_player_id: join.your_player_id,
            webrtc_client,
            ping_estimation: PingEstimation::default(),
        }
    }

    pub fn is_good(&self) -> bool {
        self.webrtc_client.status() == webrtc::Status::Open
    }

    pub async fn update(&mut self) {
        while let Some(message) = self.webrtc_client.take_message().await {
            match message {
                comn::ServerMessage::Ping(sequence_num) => {
                    self.send(comn::ClientMessage::Pong(sequence_num));
                }
                comn::ServerMessage::Pong(sequence_num) => {
                    if self.ping_estimation.received_pong(sequence_num).is_err() {
                        warn!("Ignoring out-of-order pong {:?}", sequence_num);
                    } else {
                        debug!(
                            "Received pong -> estimation {:?}",
                            self.ping_estimation.estimate()
                        );
                    }
                }
                _ => panic!("TODO"),
            }
        }

        if let Some(sequence_num) = self.ping_estimation.next_ping_sequence_num() {
            self.send(comn::ClientMessage::Ping(sequence_num));
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
