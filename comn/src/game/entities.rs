use serde::{Deserialize, Serialize};

use crate::{
    game::{run, EntityId, PlayerId, Point, Vector},
    geom::{AaRect, Circle, Rect, Shape},
    GameError, GameResult, GameTime,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Entity {
    Player(PlayerEntity),
    Bullet(Bullet),
    DangerGuy(DangerGuy),
    Turret(Turret),
    Wall(Wall),
    FoodSpawn(FoodSpawn),
}

impl Entity {
    pub fn player(&self) -> GameResult<&PlayerEntity> {
        if let Entity::Player(e) = self {
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
            Entity::Wall(entity) => entity.pos(),
            Entity::FoodSpawn(entity) => entity.pos,
        }
    }

    pub fn interp(&self, other: &Entity, alpha: f32) -> Entity {
        match (self, other) {
            (Entity::Player(this), Entity::Player(other)) => {
                Entity::Player(this.interp(other, alpha))
            }
            _ => self.clone(),
        }
    }

    pub fn can_hook_attach(&self) -> bool {
        match self {
            Entity::Bullet(_) => false,
            _ => true,
        }
    }

    pub fn shape(&self, time: f32) -> Shape {
        match self {
            Entity::Player(entity) => entity.shape(),
            Entity::Bullet(entity) => entity.shape(time),
            Entity::DangerGuy(entity) => entity.shape(time),
            Entity::Turret(entity) => entity.shape(),
            Entity::Wall(entity) => entity.shape(),
            Entity::FoodSpawn(entity) => entity.shape(time),
        }
    }

    pub fn intersection_shape(&self, time: f32) -> Shape {
        match self {
            Entity::Player(entity) => entity.shape(),
            Entity::Bullet(entity) => entity.shape(time),
            Entity::DangerGuy(entity) => entity.shape(time),
            Entity::Turret(entity) => entity.shape(),
            Entity::Wall(entity) => entity.shape(),
            Entity::FoodSpawn(entity) => entity.intersection_shape(time),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HookState {
    Shooting {
        start_time: GameTime,
        start_pos: Point,
        vel: Vector,
    },
    Attached {
        start_time: GameTime,
        target: EntityId,
        offset: Vector,
    },
    Contracting {
        start_time: GameTime,
        duration: GameTime,
        start_pos: Point,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hook {
    pub state: HookState,
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
    pub hook: Option<Hook>,
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
            hook: None,
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

    pub fn shape(&self) -> Shape {
        Shape::Rect(self.rect())
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
    pub speed: (f32, f32),
    pub wait_time: (GameTime, GameTime),
    pub phase: f32,
    pub is_hot: bool,
}

impl DangerGuy {
    pub fn walk_time(&self) -> (GameTime, GameTime) {
        (
            (self.end_pos - self.start_pos).norm() / self.speed.0,
            (self.end_pos - self.start_pos).norm() / self.speed.1,
        )
    }

    pub fn period(&self) -> GameTime {
        self.wait_time.0 + self.walk_time().0 + self.wait_time.1 + self.walk_time().1
    }

    pub fn delta(&self) -> Vector {
        self.end_pos - self.start_pos
    }

    pub fn tau(&self, t: GameTime) -> GameTime {
        ((t - self.phase) / self.period()).fract() * self.period()
    }

    pub fn pos(&self, t: GameTime) -> Point {
        let delta = self.delta();
        let tau = self.tau(t);

        // TODO: Simplify, maybe pareen?
        if tau < self.wait_time.0 {
            self.start_pos
        } else if tau <= self.wait_time.0 + self.walk_time().0 {
            self.start_pos + (tau - self.wait_time.0) / self.walk_time().0 * delta
        } else if tau < self.wait_time.0 + self.wait_time.1 + self.walk_time().0 {
            self.end_pos
        } else {
            self.end_pos
                - (tau - self.wait_time.0 - self.wait_time.1 - self.walk_time().0)
                    / self.walk_time().1
                    * delta
        }

        /*pareen::seq! {
            self.wait_time => self.start_pos,
            self.walk_time() => pareen::lerp(self.start_pos, self.end_pos).scale_time(1.0 / self.walk_time()),
            self.wait_time => self.end_pos,
            self.walk_time() => pareen::lerp(self.end_pos, self.start_pos).scale_time(1.0 / self.walk_time()),
        }
        .repeat(self.period())
        .eval(t)*/
    }

    pub fn dir(&self, t: GameTime) -> Vector {
        let delta = self.delta();
        let tau = self.tau(t);

        if tau <= self.wait_time.0 + self.walk_time().0 {
            delta
        } else {
            -delta
        }
    }

    pub fn aa_rect(&self, t: GameTime) -> AaRect {
        AaRect::new_center(self.pos(t), self.size)
    }

    pub fn shape(&self, t: GameTime) -> Shape {
        Shape::AaRect(self.aa_rect(t))
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

    pub fn shape(&self, t: GameTime) -> Shape {
        Shape::Circle(Circle {
            center: self.pos(t),
            radius: 1.0,
        })
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

    pub fn shape(&self) -> Shape {
        Shape::Circle(Circle {
            center: self.pos,
            radius: run::TURRET_RADIUS,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Wall {
    pub rect: AaRect,
}

impl Wall {
    pub fn pos(&self) -> Point {
        self.rect.center()
    }

    pub fn shape(&self) -> Shape {
        Shape::AaRect(self.rect)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FoodSpawn {
    pub pos: Point,
    pub has_food: bool,
    pub respawn_time: Option<GameTime>,
}

impl FoodSpawn {
    pub fn rect(&self, time: GameTime) -> Rect {
        AaRect::new_center(self.pos, Vector::new(run::FOOD_SIZE, run::FOOD_SIZE))
            .rotate(time * run::FOOD_ROTATION_SPEED)
    }

    pub fn shape(&self, time: GameTime) -> Shape {
        Shape::Rect(self.rect(time))
    }

    pub fn intersection_shape(&self, _: GameTime) -> Shape {
        Shape::Circle(Circle {
            center: self.pos,
            radius: run::FOOD_SIZE * 2.0f32.sqrt(),
        })
    }
}

impl_opaque_diff!(Entity);
impl_opaque_diff!(Bullet);
impl_opaque_diff!(PlayerEntity);
impl_opaque_diff!(DangerGuy);
impl_opaque_diff!(Turret);
impl_opaque_diff!(Wall);
impl_opaque_diff!(FoodSpawn);
