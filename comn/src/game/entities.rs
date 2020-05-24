use serde::{Deserialize, Serialize};

use crate::{
    game::{PlayerId, Point, Vector},
    geom::AaRect,
    GameError, GameResult, GameTime,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Entity {
    Player(PlayerEntity),
    Bullet(Bullet),
    DangerGuy(DangerGuy),
}

impl Entity {
    pub fn player(&self) -> GameResult<PlayerEntity> {
        if let Entity::Player(e) = self {
            Ok(e.clone())
        } else {
            Err(GameError::UnexpectedEntityType)
        }
    }

    pub fn danger_guy(&self) -> GameResult<DangerGuy> {
        if let Entity::DangerGuy(e) = self {
            Ok(e.clone())
        } else {
            Err(GameError::UnexpectedEntityType)
        }
    }

    pub fn pos(&self, time: GameTime) -> Point {
        match self {
            Entity::Player(entity) => entity.pos,
            Entity::Bullet(entity) => entity.pos(time),
            Entity::DangerGuy(entity) => entity.pos(time),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerEntity {
    pub owner: PlayerId,
    pub pos: Point,
    pub angle: Option<f32>,
    pub last_shot_time: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DangerGuy {
    pub start_pos: Point,
    pub end_pos: Point,
    pub size: Vector,
    pub speed: f32,
    pub wait_time: GameTime,
}

impl DangerGuy {
    pub fn walk_time(&self) -> GameTime {
        (self.end_pos - self.start_pos).norm() / self.speed
    }

    pub fn period(&self) -> GameTime {
        2.0 * (self.walk_time() + self.wait_time)
    }

    pub fn pos(&self, t: GameTime) -> Point {
        let tau = (t / self.period()).fract() * self.period();
        let delta = self.end_pos - self.start_pos;

        // TODO: Simplify, maybe pareen?
        if tau < self.wait_time {
            self.start_pos
        } else if tau <= self.wait_time + self.walk_time() {
            self.start_pos + (tau - self.wait_time) / self.walk_time() * delta
        } else if tau < 2.0 * self.wait_time + self.walk_time() {
            self.end_pos
        } else {
            self.end_pos
                - (tau - 2.0 * self.wait_time - self.walk_time()) / self.walk_time() * delta
        }
    }

    pub fn aa_rect(&self, t: GameTime) -> AaRect {
        AaRect::new_center(self.pos(t), self.size)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bullet {
    pub owner: PlayerId,
    pub start_time: GameTime,
    pub start_pos: Point,
    pub vel: Vector,
}

impl Bullet {
    pub fn pos(&self, t: GameTime) -> Point {
        if t >= self.start_time {
            self.start_pos + self.vel * (t - self.start_time)
        } else {
            self.start_pos
        }
    }
}
