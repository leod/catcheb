use log::{debug, info, warn};

use crate::webrtc;

pub struct Game {
    game_settings: comn::Settings,
    my_token: comn::PlayerToken,
    my_player_id: comn::PlayerId,
    webrtc_client: webrtc::Client,
}

impl Game {
    pub fn new(join: comn::JoinSuccess, webrtc_client: webrtc::Client) -> Self {
        Self {
            game_settings: join.game_settings.clone(),
            my_token: join.your_token,
            my_player_id: join.your_player_id,
            webrtc_client,
        }
    }

    pub fn is_good(&self) -> bool {
        self.webrtc_client.status() == webrtc::Status::Open
    }

    pub async fn update(&mut self) {
        self.send(comn::ClientMessage::Ping(comn::SequenceNum(0)));

        while let Some(message) = self.webrtc_client.take_message().await {
            match message {
                comn::ServerMessage::Ping(sequence_num) => {
                    self.send(comn::ClientMessage::Pong(sequence_num));
                }
                comn::ServerMessage::Pong(sequence_num) => {}
                _ => panic!("TODO"),
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
