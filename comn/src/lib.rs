pub mod game;
pub mod geom;
pub mod util;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use crate::{
    game::{
        entities::{DangerGuy, PlayerEntity},
        Entity, EntityId, Game, Input, Item, Player, PlayerId, Point, Settings, Tick, TickNum,
        Time, Vector,
    },
    util::ping::SequenceNum,
};

pub use crate::game::entities;
pub use crate::game::Error as GameError;
pub use crate::game::Result as GameResult;
pub use crate::game::Time as GameTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GameId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlayerToken(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinRequest {
    pub game_id: Option<GameId>,
    pub player_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinSuccess {
    pub game_id: GameId,
    pub game_settings: Settings,
    pub your_token: PlayerToken,
    pub your_player_id: PlayerId,
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
    Ping(SequenceNum),
    Pong(SequenceNum),
    Tick { tick_num: TickNum, tick: Tick },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    Ping(SequenceNum),
    Pong(SequenceNum),
    Input { tick_num: TickNum, input: Input },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedClientMessage(pub PlayerToken, pub ClientMessage);

impl ServerMessage {
    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    pub fn deserialize(data: &[u8]) -> Option<Self> {
        bincode::deserialize(data).ok()
    }
}

impl SignedClientMessage {
    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    pub fn deserialize(data: &[u8]) -> Option<Self> {
        bincode::deserialize(data).ok()
    }
}
