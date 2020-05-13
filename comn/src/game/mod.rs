pub mod entities;
pub mod run;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use nalgebra as na;

use entities::{DangerGuy, PlayerEntity};

pub type Vector = na::Vector2<f32>;
pub type Point = na::Point2<f32>;

#[derive(Debug, Clone)]
pub enum Error {
    InvalidEntityId(EntityId),
    UnexpectedEntityType,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub max_num_players: usize,
    pub ticks_per_second: usize,
    pub size: Vector,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_num_players: 16,
            ticks_per_second: 60,
            size: Vector::new(1280.0, 720.0),
        }
    }
}

impl Settings {
    pub fn tick_delta_s(&self) -> f32 {
        1.0 / (self.ticks_per_second as f32)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Time(pub f32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PlayerId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EntityId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TickNum(pub u32);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Input {
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
pub enum Entity {
    Player(PlayerEntity),
    Bullet {
        owner: PlayerId,
        pos: Point,
        dir: Vector,
        angle: f32,
    },
    Item {
        item: Item,
        pos: Point,
    },
    ItemSpawn {
        pos: Point,
    },
    Wall {
        pos: Point,
        size: Vector,
    },
    DangerGuy(DangerGuy),
}

impl Entity {
    pub fn danger_guy(&self) -> Result<DangerGuy> {
        if let Entity::DangerGuy(e) = self {
            Ok(e.clone())
        } else {
            Err(Error::UnexpectedEntityType)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    PlayerJoined { player_id: PlayerId, name: String },
    PlayerShotGun { player_id: PlayerId, dir: Vector },
    PlayerShotStunGun { player_id: PlayerId, dir: Vector },
    EntityRemoved { entity_id: EntityId },
    PlayerSpawned { pos: Point },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub settings: Settings,
    pub tick_num: TickNum,
    pub players: BTreeMap<PlayerId, Player>,
    pub entities: BTreeMap<EntityId, Entity>,
}

impl Game {
    pub fn new(settings: Settings) -> Self {
        let entities = Self::initial_entities(&settings);

        Self {
            settings,
            tick_num: TickNum(0),
            players: BTreeMap::new(),
            entities: entities
                .into_iter()
                .enumerate()
                .map(|(id, entity)| (EntityId(id as u32), entity))
                .collect(),
        }
    }

    pub fn initial_entities(settings: &Settings) -> Vec<Entity> {
        vec![
            Entity::DangerGuy(DangerGuy {
                start_pos: Point::new(200.0, 200.0),
                end_pos: Point::new(500.0, 200.0),
                size: Vector::new(100.0, 100.0),
                speed: 200.0,
            }),
            Entity::DangerGuy(DangerGuy {
                start_pos: Point::new(700.0, 600.0),
                end_pos: Point::new(700.0, 100.0),
                size: Vector::new(50.0, 100.0),
                speed: 400.0,
            }),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tick {
    pub entities: BTreeMap<EntityId, Entity>,
    pub events: Vec<Event>,
}
