pub mod game;
pub mod util;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinRequest {
    pub game_id: Option<Uuid>,
    pub player_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinSuccess {
    pub game_id: Uuid,
    pub your_token_id: Uuid,
    pub your_player_id: game::PlayerId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JoinError {
    InvalidGameId,
    InvalidPlayerName,
    FullGame,
}

pub type JoinReply = Result<JoinSuccess, JoinError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    Pong,
}
