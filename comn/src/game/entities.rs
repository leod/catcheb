use serde::{Deserialize, Serialize};

use crate::game::{PlayerId, Point, Vector};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerEntity {
    pub owner: PlayerId,
    pub pos: Point,
    pub angle: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DangerGuy {
    pub start_pos: Point,
    pub end_pos: Point,
    pub size: Vector,
    pub speed: f32,
}

impl DangerGuy {
    pub fn period(&self) -> f32 {
        (2.0 * (self.end_pos - self.start_pos).norm()) / self.speed
    }

    pub fn pos(&self, t: f32) -> Point {
        let tau = (t / self.period()).fract();
        let delta = self.end_pos - self.start_pos;

        if tau < 0.5 {
            self.start_pos + tau * delta
        } else {
            self.end_pos - tau * delta
        }
    }
}
