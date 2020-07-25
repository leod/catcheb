use quicksilver::{
    geom::{Circle, Transform},
    graphics::{Color, Graphics},
};

use comn::{Event, Game, GameTime};

const NUM_CIRCLE_PARTICLES: usize = 16;
const CIRCLE_DURATION: GameTime = 0.3;

pub struct ActiveEvent {
    pub start_time: GameTime,
    pub event: Event,
}

impl ActiveEvent {
    pub fn is_active(&self, current_time: GameTime) -> bool {
        self.start_time + event_duration(&self.event) >= current_time
    }

    pub fn render(
        &self,
        gfx: &mut Graphics,
        state: &Game,
        game_time: GameTime,
        camera_transform: Transform,
    ) {
        use Event::*;

        gfx.set_transform(camera_transform);

        match self.event {
            PlayerAteFood { player_id } => {
                if let Some((_, player)) = state.get_player_view_entity(player_id) {
                    let dt = game_time - self.start_time;

                    for i in 0..NUM_CIRCLE_PARTICLES {
                        let angle =
                            (i as f32 / NUM_CIRCLE_PARTICLES as f32) * std::f32::consts::PI * 2.0;
                        let dir = comn::Vector::new(angle.cos(), angle.sin());
                        //let tau = (-(dt * 6.0 - 1.2).powi(2)).exp();
                        let tau = (dt * std::f32::consts::PI / CIRCLE_DURATION).sin().powi(2);
                        let pos = player.pos + dir * (tau * 40.0 + 50.0);
                        let pos: mint::Vector2<f32> = pos.coords.into();
                        gfx.fill_circle(
                            &Circle::new(pos.into(), 10.0),
                            Color {
                                a: tau,
                                ..crate::view::render::color_food()
                            },
                        );
                    }
                }
            }
            _ => unreachable!(),
        }

        gfx.set_transform(Transform::IDENTITY);
    }
}

pub fn event_duration(event: &Event) -> f32 {
    use Event::*;

    match event {
        PlayerAteFood { .. } => CIRCLE_DURATION,
        _ => 0.0,
    }
}
