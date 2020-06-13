use std::iter::once;

use serde::{Deserialize, Serialize};

use crate::{Point, Vector};

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    AaRect(AaRect),
    Rect(Rect),
    Circle(Circle),
}

impl Shape {
    pub fn contains_point(&self, point: Point) -> bool {
        match self {
            Shape::AaRect(shape) => shape.contains_point(point),
            Shape::Rect(shape) => shape.contains_point(point),
            Shape::Circle(shape) => shape.contains_point(point),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

    pub fn center(&self) -> Point {
        self.top_left + self.size / 2.0
    }

    pub fn contains_point(&self, point: Point) -> bool {
        point.x >= self.top_left.x
            && point.y >= self.top_left.y
            && point.x <= self.top_left.x + self.size.x
            && point.y <= self.top_left.y + self.size.y
    }

    pub fn rotate(&self, angle: f32) -> Rect {
        Rect {
            center: self.center(),
            size: self.size,
            angle,
            x_edge: self.size.x * Vector::new(angle.cos(), angle.sin()),
            y_edge: self.size.y * Vector::new(-angle.sin(), angle.cos()),
        }
    }

    pub fn to_rect(&self) -> Rect {
        Rect {
            center: self.center(),
            size: self.size,
            angle: 0.0,
            x_edge: Vector::new(self.size.x, 0.0),
            y_edge: Vector::new(0.0, self.size.y),
        }
    }
}

pub fn smooth_to_target_point(factor: f32, start: Point, target: Point, dt: f32) -> Point {
    // p'(t) = factor * (target - p(t)), p(0) = start

    target - (target - start) * (-factor * dt).exp()
}

pub fn smooth_to_target_vector(factor: f32, start: Vector, target: Vector, dt: f32) -> Vector {
    target - (target - start) * (-factor * dt).exp()
}

// Awesome resource:
// https://www.codeproject.com/Articles/15573/2D-Polygon-Collision-Detection

pub struct AxisProjection {
    pub min: f32,
    pub max: f32,
}

impl AxisProjection {
    pub fn interval_distance(&self, other: &AxisProjection) -> f32 {
        // Calculate distance between two intervals, returning negative values
        // if the intervals overlap.
        if self.min < other.min {
            other.min - self.max
        } else {
            self.min - other.max
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rect {
    pub center: Point,
    pub size: Vector,
    pub angle: f32,
    pub x_edge: Vector,
    pub y_edge: Vector,
}

impl Rect {
    pub fn iter_points(&self) -> impl Iterator<Item = Point> {
        once(self.center - self.x_edge / 2.0 - self.y_edge / 2.0)
            .chain(once(self.center + self.x_edge / 2.0 - self.y_edge / 2.0))
            .chain(once(self.center - self.x_edge / 2.0 + self.y_edge / 2.0))
            .chain(once(self.center + self.x_edge / 2.0 + self.y_edge / 2.0))
    }

    pub fn project_to_edge(&self, edge: Vector) -> AxisProjection {
        use std::cmp::Ordering::Equal;

        AxisProjection {
            min: self
                .iter_points()
                .map(|p| edge.dot(&p.coords))
                .min_by(|d1, d2| d1.partial_cmp(d2).unwrap_or(Equal))
                .unwrap(),
            max: self
                .iter_points()
                .map(|p| edge.dot(&p.coords))
                .max_by(|d1, d2| d1.partial_cmp(d2).unwrap_or(Equal))
                .unwrap(),
        }
    }

    pub fn contains_point(&self, point: Point) -> bool {
        // TODO: Needlessly inefficient
        let uv = nalgebra::Matrix2::from_columns(&[self.x_edge, self.y_edge])
            .try_inverse()
            .unwrap()
            * (self.center - point);

        uv.x >= -0.5 && uv.x <= 0.5 && uv.y >= -0.5 && uv.y <= 0.5
    }
}

pub struct Collision {
    pub resolution_vector: Vector,
    pub axis: Vector,
}

pub fn rect_collision(a: &Rect, b: &Rect, delta: Vector) -> Option<Collision> {
    let edges = once(a.x_edge)
        .chain(once(a.y_edge))
        .chain(once(b.x_edge))
        .chain(once(b.y_edge))
        .chain(once(-a.x_edge))
        .chain(once(-a.y_edge))
        .chain(once(-b.x_edge))
        .chain(once(-b.y_edge));

    let mut intersecting = true;
    let mut will_intersect = true;

    let mut min_interval_distance = std::f32::INFINITY;
    let mut translation_axis = Vector::zeros();

    for edge in edges {
        let axis = Vector::new(-edge.y, edge.x).normalize();
        //let axis = edge;

        // Are the polygons currently intersecting?
        let mut a_projection = a.project_to_edge(axis);
        let b_projection = b.project_to_edge(axis);

        if a_projection.interval_distance(&b_projection) > 0.0 {
            // By the separating axis theorem, the polygons do not overlap.
            intersecting = false;
        }

        // Given the delta movement, will the polygons intersect?
        let delta_projection = axis.dot(&delta);

        if delta_projection < 0.0 {
            a_projection.min += delta_projection;
        } else {
            a_projection.max += delta_projection;
        }

        let interval_distance = a_projection.interval_distance(&b_projection);
        if interval_distance > 0.0 {
            // Again by the separating axis theorem, the polygons will not
            // overlap.
            will_intersect = false;
        }

        // Early exit if we already found a separating axis.
        if !intersecting && !will_intersect {
            return None;
        }

        // Keep the axis with the minimum interval distance.
        let interval_distance = interval_distance.abs();
        if interval_distance < min_interval_distance {
            min_interval_distance = interval_distance;

            translation_axis = if (a.center - b.center).dot(&axis) < 0.0 {
                -axis
            } else {
                axis
            };
        }
    }

    if will_intersect {
        Some(Collision {
            resolution_vector: translation_axis * min_interval_distance,
            axis: translation_axis,
        })
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Circle {
    pub center: Point,
    pub radius: f32,
}

impl Circle {
    pub fn contains_point(&self, point: Point) -> bool {
        (self.center - point).norm_squared() <= self.radius * self.radius
    }
}
