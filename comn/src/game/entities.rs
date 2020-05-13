use serde::{Deserialize, Serialize};

use crate::game::{PlayerId, Point};

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
}

impl DangerGuy {
    pub fn pos(&self, t: f32) -> Point {
        Point::new(0.0, 0.0)
    }
}
