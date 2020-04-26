use serde::{Deserialize, Serialize};

use nalgebra as na;

pub type Vector = na::Vector2<f32>;
pub type Point = na::Point2<f32>;

pub struct Time(pub f32);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerId(pub u16);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityId(pub u32);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickNum(pub u32);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInput {
    pub move_left: bool,
    pub move_right: bool,
    pub move_up: bool,
    pub move_down: bool,
    pub use_item: bool,
    pub use_action: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Item {
    Gun { shots: u32 },
    StunGun,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position(Point);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Angle(f32);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityData {
    Player {
        owner: PlayerId,
        pos: Position,
        angle: Angle,
    },
    Bullet {
        owner: PlayerId,
        pos: Position,
        angle: Angle,
    },
    Item {
        item: Item,
        pos: Position,
    },
    ItemSpawn {
        pos: Position,
    },
    Wall {
        pos: Point,
        size: Vector,
    },
    DangerGuy {
        start_pos: Point,
        end_pos: Point,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    PlayerJoined { player_id: PlayerId, name: String },
    PlayerShotGun { player_id: PlayerId, dir: Vector },
    PlayerShotStunGun { player_id: PlayerId, dir: Vector },
    EntityRemoved { entity_id: EntityId },
    PlayerSpawned { pos: Point },
}

pub struct Tick {
    num: TickNum,
    events: Vec<Event>,
    entities: Vec<(EntityId, EntityData)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMsg {}
