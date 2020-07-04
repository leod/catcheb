use std::iter::once;

use nalgebra as na;
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
            x_edge: self.size.x * Vector::new(angle.cos(), angle.sin()),
            y_edge: self.size.y * Vector::new(-angle.sin(), angle.cos()),
        }
    }

    pub fn to_rect(&self) -> Rect {
        Rect {
            center: self.center(),
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

pub fn smooth_to_target_f32(factor: f32, start: f32, target: f32, dt: f32) -> f32 {
    target - (target - start) * (-factor * dt).exp()
}

pub fn angle_dist(alpha: f32, beta: f32) -> f32 {
    (alpha - beta).sin().atan2((alpha - beta).cos())
}

pub fn interp_angle(alpha: f32, beta: f32, t: f32) -> f32 {
    alpha + t * angle_dist(beta, alpha)
}

// Awesome resource:
// https://www.codeproject.com/Articles/15573/2D-Polygon-Collision-Detection

pub struct Collision {
    pub resolution_vector: Vector,
}

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

    pub fn collision(&self, other: &Shape, delta: Vector) -> Option<Collision> {
        match other {
            Shape::Rect(other) => rect_collision(self, other, delta),
            Shape::AaRect(other) => rect_collision(self, &other.to_rect(), delta),
            Shape::Circle(other) => {
                let theta = self.x_edge.y.atan2(self.x_edge.x);
                let rotation = na::Rotation2::new(theta);
                let inv_rotation = na::Rotation2::new(-theta);

                let aa_rect_origin = AaRect::new_center(
                    Point::origin(),
                    Vector::new(self.x_edge.norm(), self.y_edge.norm()),
                );
                let other_shifted = Circle {
                    center: inv_rotation * (other.center - self.center.coords),
                    radius: other.radius,
                };

                aa_rect_circle_collision(&aa_rect_origin, &other_shifted, inv_rotation * delta).map(
                    |collision| Collision {
                        resolution_vector: rotation * collision.resolution_vector,
                    },
                )
            }
        }
    }
}

pub fn rect_collision(a: &Rect, b: &Rect, delta: Vector) -> Option<Collision> {
    let edges = [
        a.x_edge, a.y_edge, b.x_edge, b.y_edge, -a.x_edge, -a.y_edge, -b.x_edge, -b.y_edge,
    ];

    let mut intersecting = true;
    let mut will_intersect = true;

    let mut min_interval_distance = std::f32::INFINITY;
    let mut translation_axis = Vector::zeros();

    for &edge in &edges {
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
        })
    } else {
        None
    }
}

pub fn aa_rect_circle_collision(
    rect: &AaRect,
    circle: &Circle,
    delta: Vector,
) -> Option<Collision> {
    // https://math.stackexchange.com/questions/227494/do-an-axis-aligned-rectangle-and-a-circle-overlap

    let moved_top_left = rect.top_left + delta;

    let p_star = Point::new(
        circle
            .center
            .x
            .max(moved_top_left.x)
            .min(moved_top_left.x + rect.size.x),
        circle
            .center
            .y
            .max(moved_top_left.y)
            .min(moved_top_left.y + rect.size.y),
    );

    let delta = p_star - circle.center;
    let dist_sq = delta.norm_squared();

    if dist_sq < circle.radius * circle.radius {
        let dist = dist_sq.sqrt();
        let normal = if dist < 0.01 {
            Vector::new(-1.0, 0.0)
        } else {
            delta / dist
        };

        Some(Collision {
            resolution_vector: normal * (circle.radius - dist + 1.0),
        })
    } else {
        None
    }

    // More general solution:
    // https://stackoverflow.com/questions/18704999/how-to-fix-circle-and-rectangle-overlap-in-collision-response/18790389#18790389
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

#[derive(Debug, Clone, PartialEq)]
pub struct Ray {
    pub origin: Point,
    pub dir: Vector,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayIntersections(pub Option<(f32, f32)>);

impl Ray {
    pub fn intersections(&self, other: &Shape) -> RayIntersections {
        match other {
            Shape::AaRect(aa_rect) => {
                // https://gamedev.stackexchange.com/questions/18436/most-efficient-aabb-vs-ray-collision-algorithms
                // https://www.scratchapixel.com/lessons/3d-basic-rendering/minimal-ray-tracer-rendering-simple-shapes/ray-box-intersection

                let min = aa_rect.top_left;
                let max = min + aa_rect.size;

                let t0x = (min.x - self.origin.x) / self.dir.x;
                let t0y = (min.y - self.origin.y) / self.dir.y;
                let t1x = (max.x - self.origin.x) / self.dir.x;
                let t1y = (max.y - self.origin.y) / self.dir.y;

                let t_min = t0x.min(t1x).max(t0y.min(t1y));
                let t_max = t0x.max(t1x).min(t0y.max(t1y));

                if t_min > t_max {
                    RayIntersections(None)
                } else {
                    Self::collect_times(t_min, t_max)
                }
            }
            Shape::Rect(rect) => {
                // Rotate both ray and rectangle so that the rectangle is axis-
                // aligned, with the rectangle at origin.

                let aa_rect_origin = AaRect::new_center(
                    Point::origin(),
                    Vector::new(rect.x_edge.norm(), rect.y_edge.norm()),
                );

                let theta = rect.x_edge.y.atan2(rect.x_edge.x);
                let rotation = na::Rotation2::new(-theta);

                let ray_rotated = Ray {
                    origin: rotation * (self.origin - rect.center.coords),
                    dir: rotation * self.dir,
                };

                ray_rotated.intersections(&Shape::AaRect(aa_rect_origin))
            }
            Shape::Circle(circle) => {
                // https://stackoverflow.com/questions/1073336/circle-line-segment-collision-detection-algorithm

                let d = self.dir;
                let f = self.origin - circle.center;
                let r = circle.radius;

                let a = d.dot(&d);
                let b = 2.0 * f.dot(&d);
                let c = f.dot(&f) - r * r;

                let discriminant = b * b - 4.0 * a * c;

                if discriminant >= 0.0 {
                    let discriminant = discriminant.sqrt();

                    let t1 = (-b - discriminant) / (2.0 * a);
                    let t2 = (-b + discriminant) / (2.0 * a);

                    Self::collect_times(t1, t2)
                } else {
                    RayIntersections(None)
                }
            }
        }
    }

    fn collect_times(t1: f32, t2: f32) -> RayIntersections {
        RayIntersections(if t1 < 0.0 {
            if t2 < 0.0 {
                None
            } else {
                Some((t2, t2))
            }
        } else {
            if t2 < 0.0 {
                Some((t1, t1))
            } else {
                Some((t1, t2))
            }
        })
    }
}

pub struct RayIntersectionsIter(Option<(f32, f32)>, usize);

impl RayIntersections {
    pub fn iter(&self) -> RayIntersectionsIter {
        RayIntersectionsIter(self.0, 0)
    }

    pub fn first(&self) -> Option<f32> {
        self.0.map(|(t1, t2)| t1.min(t2))
    }
}

impl Iterator for RayIntersectionsIter {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        match self.0 {
            Some((t1, t2)) => {
                if self.1 == 0 {
                    self.1 = 1;
                    Some(t1)
                } else if self.1 == 1 {
                    self.1 = 2;
                    Some(t2)
                } else {
                    None
                }
            }
            None => None,
        }
    }
}
