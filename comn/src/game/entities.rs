use serde::{Deserialize, Serialize};

use crate::{
    game::{run, EntityId, PlayerId, Point, Vector},
    geom::{AaRect, Rect},
    GameError, GameResult, GameTime,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Entity {
    Player(PlayerEntity),
    Bullet(Bullet),
    DangerGuy(DangerGuy),
    Turret(Turret),
}

impl Entity {
    pub fn player(&self) -> GameResult<&PlayerEntity> {
        if let Entity::Player(e) = self {
            Ok(e)
        } else {
            Err(GameError::UnexpectedEntityType)
        }
    }

    pub fn danger_guy(&self) -> GameResult<&DangerGuy> {
        if let Entity::DangerGuy(e) = self {
            Ok(e)
        } else {
            Err(GameError::UnexpectedEntityType)
        }
    }

    pub fn pos(&self, time: GameTime) -> Point {
        match self {
            Entity::Player(entity) => entity.pos,
            Entity::Bullet(entity) => entity.pos(time),
            Entity::DangerGuy(entity) => entity.pos(time),
            Entity::Turret(entity) => entity.pos,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerEntity {
    pub owner: PlayerId,
    pub pos: Point,
    pub vel: Vector,
    pub angle: Option<f32>,
    pub next_shot_time: GameTime,
    pub shots_left: u32,
    pub last_dash: Option<(GameTime, Vector)>,

    // TODO: Redundant state needed for display
    pub is_dashing: bool,
}

impl PlayerEntity {
    pub fn new(owner: PlayerId, pos: Point) -> Self {
        Self {
            owner,
            pos,
            vel: Vector::zeros(),
            angle: Some(0.0),
            next_shot_time: 0.0,
            shots_left: run::MAGAZINE_SIZE,
            last_dash: None,
            is_dashing: false,
        }
    }

    pub fn rect(&self) -> Rect {
        if let Some(angle) = self.angle {
            AaRect::new_center(
                self.pos,
                Vector::new(run::PLAYER_MOVE_W, run::PLAYER_MOVE_L),
            )
            .rotate(angle)
        } else {
            AaRect::new_center(self.pos, Vector::new(run::PLAYER_SIT_W, run::PLAYER_SIT_L))
                .to_rect()
        }
    }

    pub fn interp(&self, other: &PlayerEntity, alpha: f32) -> PlayerEntity {
        // TODO: Interpolate player properties other than just the position
        PlayerEntity {
            pos: self.pos + alpha * (other.pos - self.pos),
            ..self.clone()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

    pub fn delta(&self) -> Vector {
        self.end_pos - self.start_pos
    }

    pub fn tau(&self, t: GameTime) -> GameTime {
        (t / self.period()).fract() * self.period()
    }

    pub fn pos(&self, t: GameTime) -> Point {
        let delta = self.delta();
        let tau = self.tau(t);

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

    pub fn dir(&self, t: GameTime) -> Vector {
        let delta = self.delta();
        let tau = self.tau(t);

        if tau <= self.wait_time + self.walk_time() {
            delta
        } else {
            -delta
        }
    }

    pub fn aa_rect(&self, t: GameTime) -> AaRect {
        AaRect::new_center(self.pos(t), self.size)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bullet {
    pub owner: Option<PlayerId>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Turret {
    pub pos: Point,
    pub target: Option<EntityId>,
    pub angle: f32,
    pub next_shot_time: GameTime,
}

impl Turret {
    pub fn angle_to_pos(&self, pos: Point) -> f32 {
        let d = pos - self.pos;
        d.y.atan2(d.x)
    }
}

impl_opaque_diff!(Entity);
impl_opaque_diff!(Bullet);
impl_opaque_diff!(PlayerEntity);
impl_opaque_diff!(DangerGuy);
impl_opaque_diff!(Turret);
