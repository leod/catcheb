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

    pub fn new_center(center: Point, size: Vector) -> Self {
        Self {
            top_left: center - size / 2.0,
            size,
        }
    }

    pub fn contains_point(&self, point: Point) -> bool {
        point.x >= self.top_left.x
            && point.y >= self.top_left.y
            && point.x <= self.top_left.x + self.size.x
            && point.y <= self.top_left.y + self.size.y
    }
}

pub fn smooth_to_target_point(factor: f32, start: Point, target: Point, dt: f32) -> Point {
    // p'(t) = factor * (target - p(t)), p(0) = start

    target - (target - start) * (-factor * dt).exp()
}

pub fn smooth_to_target_vector(factor: f32, start: Vector, target: Vector, dt: f32) -> Vector {
    target - (target - start) * (-factor * dt).exp()
}
