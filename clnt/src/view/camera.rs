use std::{collections::HashSet, time::Duration};

use nalgebra::Matrix3;

use webglee::Screen;

use comn::geom;

#[derive(Debug, Clone)]
pub struct Config {
    pub smooth_pos_factor: f32,
    pub max_smooth_dist: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            smooth_pos_factor: 5.0,
            max_smooth_dist: 300.0,
        }
    }
}

pub struct Camera {
    config: Config,
    pos: comn::Point,
    target: comn::Point,
    map_size: comn::Vector,
    scale: f32,
}

impl Camera {
    pub fn new(config: Config, map_size: comn::Vector) -> Self {
        Self {
            config,
            pos: comn::Point::origin(),
            target: comn::Point::origin(),
            map_size,
            scale: 0.75,
        }
    }

    pub fn update(
        &mut self,
        dt: Duration,
        follow_entity: Option<comn::Entity>,
        game_time: comn::GameTime,
    ) {
        //let offset = window_size / (2.0 * self.scale / window_scale_factor);

        self.target = follow_entity.map_or(self.target, |entity| entity.pos(game_time));

        // TODO: Put camera clipping back in
        /*self.target.x = self
            .target
            .x
            .max(offset.x - 200.0)
            .min(self.map_size.x - offset.x + 200.0);
        self.target.y = self
            .target
            .y
            .max(offset.y - 200.0)
            .min(self.map_size.y - offset.y + 200.0);*/

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
    }

    pub fn transform(&self, screen: &Screen) -> Matrix3<f32> {
        webglee::Camera {
            center: self.pos,
            zoom: self.scale,
            angle: 0.0,
        }
        .to_matrix(screen)
    }
}
