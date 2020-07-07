use std::{collections::VecDeque, time::Duration};

use instant::Instant;

use quicksilver::{
    geom::Vector,
    graphics::{Color, FontRenderer, Graphics},
};

use comn::{DeathReason, Event};

use crate::view::overlay;

#[derive(Debug, Clone)]
pub struct Config {
    pub num_lines: usize,
    pub max_age: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            num_lines: 4,
            max_age: Duration::from_secs(10),
        }
    }
}

pub struct EventList {
    config: Config,
    events: VecDeque<(Instant, Event)>,
}

impl EventList {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            events: VecDeque::new(),
        }
    }

    pub fn push(&mut self, now: Instant, event: Event) {
        self.events.push_back((now, event));

        if self.events.len() > self.config.num_lines {
            self.events.pop_front();
        }
    }

    pub fn render(
        &mut self,
        now: Instant,
        gfx: &mut Graphics,
        font: &mut FontRenderer,
        mut pos: Vector,
    ) -> quicksilver::Result<()> {
        // Remove events that are too old.
        while let Some((oldest_time, _)) = self.events.front() {
            if now.duration_since(*oldest_time) <= self.config.max_age {
                break;
            }

            self.events.pop_front();
        }

        // Display events.
        if !self.events.is_empty() {
            overlay::box_thing(
                gfx,
                pos - Vector::new(0.0, 6.0),
                Vector::new(260.0, 12.0 * self.events.len() as f32 + 14.0),
            )?;
            pos += Vector::new(10.0, 10.0);
        }

        for (_, event) in self.events.iter() {
            if let Some(string) = Self::event_to_string(event) {
                font.draw(gfx, &string, Color::BLACK, pos)?;
            }
            pos.y += 12.0;
        }

        Ok(())
    }

    pub fn event_to_string(event: &Event) -> Option<String> {
        // TODO: Use player names
        match event {
            Event::PlayerDied { player_id, reason } => Some(match reason {
                DeathReason::ShotBy(Some(other_player_id)) => {
                    format!("{} shot {}", player_id.0, other_player_id.0)
                }
                DeathReason::ShotBy(None) => format!("{} rekt by turret lol", player_id.0),
                DeathReason::TouchedTheDanger => format!("{} touched the danger", player_id.0),
                DeathReason::CaughtBy(other_player_id) => {
                    format!("{} caught {}!!", other_player_id.0, player_id.0)
                }
            }),
            Event::NewCatcher { player_id } => Some(format!("{} is the new catcher", player_id.0)),
            _ => None,
        }
    }
}
