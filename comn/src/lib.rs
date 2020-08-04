// Needed for pareen stuff
#![type_length_limit = "600000000"]

#[macro_use]
pub mod util;
pub mod game;
pub mod geom;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use crate::{
    game::{
        entities::{DangerGuy, Hook, PlayerEntity, PlayerView, Turret},
        DeathReason, Entity, EntityId, EntityMap, Event, Game, Input, Item, Map, Matrix, Player,
        PlayerId, PlayerMap, PlayerState, Point, Settings, Tick, TickNum, Time, Vector,
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
    Tick(Tick),
    Disconnect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    Ping(SequenceNum),
    Pong(SequenceNum),
    Input(Vec<(TickNum, Input)>),
    // TODO: Send some kind of hash with the AckTick
    AckTick(TickNum),
    Disconnect,
}

pub const MAX_INPUTS_PER_MESSAGE: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedClientMessage(pub PlayerToken, pub ClientMessage);

impl ServerMessage {
    pub fn serialize(&self) -> Vec<u8> {
        //bincode::serialize(self).unwrap()
        rmp_serde::to_vec(self).unwrap()
    }

    pub fn deserialize(data: &[u8]) -> Option<Self> {
        //bincode::deserialize(data).ok()
        rmp_serde::from_read_ref(data).ok()
    }
}

impl SignedClientMessage {
    pub fn serialize(&self) -> Vec<u8> {
        //bincode::serialize(self).unwrap()
        rmp_serde::to_vec(self).unwrap()
    }

    pub fn deserialize(data: &[u8]) -> Option<Self> {
        //bincode::deserialize(data).ok()
        rmp_serde::from_read_ref(data).ok()
    }
}
