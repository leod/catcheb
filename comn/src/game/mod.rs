pub mod entities;
pub mod run;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use nalgebra as na;

use entities::{DangerGuy, Turret};

use crate::{geom, GameTime};

pub use entities::Entity;
pub use run::RunContext;

pub type Time = f32;
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
    pub spawn_points: Vec<Point>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_num_players: 16,
            ticks_per_second: 60,
            size: Vector::new(1600.0, 800.0),
            spawn_points: vec![Point::new(50.0, 50.0), Point::new(550.0, 400.0)],
        }
    }
}

impl Settings {
    pub fn tick_period(&self) -> GameTime {
        1.0 / (self.ticks_per_second as f32)
    }

    pub fn tick_game_time(&self, tick_num: TickNum) -> GameTime {
        self.tick_period() * tick_num.0 as f32
    }

    pub fn aa_rect(&self) -> geom::AaRect {
        geom::AaRect::new_top_left(Point::new(0.0, 0.0), self.size)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PlayerId(pub u32);

impl PlayerId {
    pub fn next(&self) -> PlayerId {
        PlayerId(self.0 + 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EntityId(pub u32);

impl EntityId {
    pub fn next(&self) -> EntityId {
        EntityId(self.0 + 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TickNum(pub u32);

impl TickNum {
    pub fn next(&self) -> TickNum {
        TickNum(self.0 + 1)
    }
}

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
pub enum DeathReason {
    ShotBy(Option<PlayerId>),
    TouchedTheDanger,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    PlayerShotGun {
        player_id: PlayerId,
        dir: Vector,
    },
    PlayerShotStunGun {
        player_id: PlayerId,
        dir: Vector,
    },
    PlayerSpawned {
        player_id: PlayerId,
        pos: Point,
    },
    PlayerDied {
        player_id: PlayerId,
        reason: DeathReason,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlayerState {
    Alive,
    Dead,
    Respawning { respawn_time: GameTime },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    pub state: PlayerState,
}

pub type PlayerMap = BTreeMap<PlayerId, Player>;
pub type EntityMap = BTreeMap<EntityId, Entity>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub settings: Settings,
    pub tick_num: TickNum,
    pub players: PlayerMap,
    pub entities: EntityMap,
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

    pub fn initial_entities(_settings: &Settings) -> Vec<Entity> {
        vec![
            /*Entity::DangerGuy(DangerGuy {
                start_pos: Point::new(200.0, 200.0),
                end_pos: Point::new(500.0, 200.0),
                size: Vector::new(100.0, 50.0),
                speed: 2000.0,
                wait_time: 2.0,
            }),
            Entity::DangerGuy(DangerGuy {
                start_pos: Point::new(200.0, 400.0),
                end_pos: Point::new(200.0, 100.0),
                size: Vector::new(30.0, 80.0),
                speed: 300.0,
                wait_time: 1.0,
            }),*/
            Entity::DangerGuy(DangerGuy {
                start_pos: Point::new(40.0, 500.0),
                end_pos: Point::new(760.0, 500.0),
                size: Vector::new(80.0, 30.0),
                speed: 2000.0,
                wait_time: 2.0,
            }),
            Entity::Turret(Turret {
                pos: Point::new(600.0, 100.0),
                target: None,
                angle: 0.0,
                next_shot_time: 0.0,
            }),
        ]
    }

    pub fn tick_game_time(&self, tick_num: TickNum) -> GameTime {
        self.settings.tick_game_time(tick_num)
    }

    pub fn current_game_time(&self) -> GameTime {
        self.tick_game_time(self.tick_num)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tick {
    pub state: Game,
    pub events: Vec<Event>,
    pub your_last_input: Option<TickNum>,
}
