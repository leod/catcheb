pub mod game;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinRequest {
    pub game_id: Option<Uuid>,
    pub player_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinReply {
    pub game_id: Uuid,
    pub your_token_id: Uuid,
    pub your_player_id: game::PlayerId,
}
