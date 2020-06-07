use std::{collections::HashSet, time::Duration};

use quicksilver::{
    geom::{Transform, Vector},
    input::Key,
};

use comn::geom;

#[derive(Debug, Clone)]
pub struct Config {
    pub smooth_pos_factor: f32,
    pub max_smooth_dist: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            smooth_pos_factor: 10.0,
            max_smooth_dist: 100.0,
        }
    }
}

pub struct Camera {
    config: Config,
    pos: comn::Point,
    centered_pos: comn::Point,
    target: comn::Point,
    map_size: comn::Vector,
    scale: f32,
}

impl Camera {
    pub fn new(config: Config, map_size: comn::Vector) -> Self {
        Self {
            config,
            pos: comn::Point::origin(),
            centered_pos: comn::Point::origin(),
            target: comn::Point::origin(),
            map_size,
            scale: 0.6,
        }
    }

    pub fn update(
        &mut self,
        dt: Duration,
        _pressed_keys: &HashSet<Key>,
        follow_entity: Option<comn::Entity>,
        game_time: comn::GameTime,
        window_size: comn::Vector,
    ) {
        let offset = window_size / (2.0 * self.scale);

        self.target = follow_entity.map_or(self.target, |entity| entity.pos(game_time));
        self.target.x = self.target.x.max(offset.x).min(self.map_size.x - offset.x);
        self.target.y = self.target.y.max(offset.y).min(self.map_size.y - offset.y);

        self.pos = if (self.pos - self.target).norm() <= self.config.max_smooth_dist {
            geom::smooth_to_target_point(
                self.config.smooth_pos_factor,
                self.pos,
                self.target,
                dt.as_secs_f32(),
            )
        } else {
            // Camera is too far away, just snap to the target position.
            self.target
        };
        self.centered_pos = self.pos - offset;
    }

    pub fn transform(&self) -> Transform {
        let offset: mint::Vector2<f32> = (-self.centered_pos.coords).into();
        Transform::translate(offset.into())
            .then(Transform::scale(Vector::new(self.scale, self.scale)))
    }
}
