pub mod entities;
pub mod run;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use nalgebra as na;

use entities::{DangerGuy, Turret};

use crate::{
    geom,
    util::diff::{ApplyError, BTreeMapDiff, Diff, Diffable},
    GameTime,
};

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
            size: Vector::new(2400.0, 2400.0),
            spawn_points: vec![Point::new(50.0, 50.0), Point::new(50.0, 1300.0)],
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlayerState {
    Alive,
    Dead,
    Respawning { respawn_time: GameTime },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    pub state: PlayerState,
}

impl_opaque_diff!(Player);

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
                start_pos: Point::new(700.0, 100.0),
                end_pos: Point::new(1000.0, 100.0),
                size: Vector::new(160.0, 160.0),
                speed: 100.0,
                wait_time: 3.0,
            }),*/
            Entity::DangerGuy(DangerGuy {
                start_pos: Point::new(100.0, 1200.0),
                end_pos: Point::new(1500.0, 1200.0),
                size: Vector::new(30.0, 100.0),
                speed: 1000.0,
                wait_time: 2.0,
            }),
            Entity::DangerGuy(DangerGuy {
                start_pos: Point::new(1500.0, 1400.0),
                end_pos: Point::new(100.0, 1400.0),
                size: Vector::new(30.0, 100.0),
                speed: 1000.0,
                wait_time: 2.0,
            }),
            Entity::Turret(Turret {
                pos: Point::new(600.0, 400.0),
                target: None,
                angle: 0.0,
                next_shot_time: 0.0,
            }),
            Entity::Turret(Turret {
                pos: Point::new(1200.0, 400.0),
                target: None,
                angle: 0.0,
                next_shot_time: 0.0,
            }),
            Entity::Turret(Turret {
                pos: Point::new(900.0, 1000.0),
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
pub struct GameDiff {
    pub tick_num: TickNum,
    pub players: BTreeMapDiff<PlayerId, Player>,
    pub entities: BTreeMapDiff<EntityId, Entity>,
}

impl Diffable for Game {
    type Diff = GameDiff;

    fn diff(&self, other: &Self) -> Self::Diff {
        Self::Diff {
            tick_num: other.tick_num,
            players: self.players.diff(&other.players),
            entities: self.entities.diff(&other.entities),
        }
    }
}

impl Diff for GameDiff {
    type Value = Game;

    fn apply(self, value: &mut Self::Value) -> std::result::Result<(), ApplyError> {
        value.tick_num = self.tick_num;
        self.players.apply(&mut value.players)?;
        self.entities.apply(&mut value.entities)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tick {
    pub diff_base: Option<TickNum>,
    pub diff: GameDiff,
    pub events: Vec<(TickNum, Vec<Event>)>,
    pub your_last_input: Option<TickNum>,
}
