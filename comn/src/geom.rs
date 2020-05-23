use crate::{Point, Vector};

#[derive(Debug, Clone)]
pub struct AaRect {
    pub top_left: Point,
    pub size: Vector,
}

impl AaRect {
    pub fn new_top_left(top_left: Point, size: Vector) -> Self {
        Self { top_left, size }
    }

    pub fn contains_point(&self, point: Point) -> bool {
        point.x >= self.top_left.x
            && point.y >= self.top_left.y
            && point.x <= self.top_left.x + self.size.x
            && point.y <= self.top_left.y + self.size.y
    }
}
