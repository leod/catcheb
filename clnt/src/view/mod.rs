//mod active_event;
mod camera;
//mod event_list;
//mod overlay;
//mod particles;
mod render;
mod resources;
//mod scoreboard;
mod stage;

use std::{
    collections::BTreeMap,
    time::Duration,
};

use webglee::{
    draw::{ColVertex, TriBatch},
    InputState,
};

use instant::Instant;
use stage::Stage;

use camera::Camera;
/*use active_event::ActiveEvent;
use event_list::EventList;
use particles::Particles;*/

pub use resources::Resources;

#[derive(Debug, Clone, Default)]
pub struct Config {
    //pub event_list: event_list::Config,
    pub camera: camera::Config,
}

pub struct View {
    my_player_id: comn::PlayerId,
    resources: Resources,
    //event_list: EventList,
    camera: Camera,
    //ground_particles: Particles,
    //air_particles: Particles,
    last_game_time: Option<comn::GameTime>,
    //active_events: Vec<ActiveEvent>,
    stage: Stage,
}

impl View {
    pub fn new(
        ctx: &webglee::Context,
        config: Config,
        settings: comn::Settings,
        my_player_id: comn::PlayerId,
        resources: Resources,
    ) -> Result<Self, webglee::Error> {
        //let event_list = EventList::new(config.event_list);
        let camera = Camera::new(config.camera, settings.map.size);
        //let ground_particles = Particles::new();
        //let air_particles = Particles::new();

        Ok(Self {
            my_player_id,
            resources,
            //event_list,
            camera,
            //ground_particles,
            //air_particles,
            last_game_time: None,
            //active_events: Vec::new(),
            stage: Stage::new(ctx)?,
        })
    }

    pub fn resources_mut(&mut self) -> &mut Resources {
        &mut self.resources
    }

    pub fn update(
        &mut self,
        now: Instant,
        dt: Duration,
        input_state: &InputState,
        state: Option<&comn::Game>,
        game_events: &[comn::Event],
        game_time: comn::GameTime,
    ) {
        self.stage.clear();

        let game_dt = self
            .last_game_time
            .map_or(0.0, |last_game_time| game_time - last_game_time)
            .max(0.0);
        self.last_game_time = Some(game_time);

        let follow_entity = state.and_then(|state| {
            state
                .get_player_entity(self.my_player_id)
                .map(|(_id, e)| comn::Entity::Player(e.clone()))
        });

        self.camera.update(dt, follow_entity, game_time);
        /*self.ground_particles.update(game_dt);
        self.air_particles.update(game_dt);

        for event in game_events {
            self.event_list.push(now, event.clone());

            use comn::Event::*;
            match event {
                PlayerDied {
                    player_id: _,
                    pos,
                    reason: _,
                } => {
                    self.ground_particles.spawn_blood(*pos, 100.0);
                }
                _ => (),
            }

            let duration = active_event::event_duration(event);
            if duration > 0.0 {
                self.active_events.push(ActiveEvent {
                    start_time: game_time,
                    event: event.clone(),
                });
            }
        }*/

        if let Some(state) = state {
            for entity in state.entities.values() {
                match entity {
                    comn::Entity::Player(player) => {
                        self.update_player(game_dt, state, &player.to_view());
                    }
                    comn::Entity::PlayerView(player) => {
                        self.update_player(game_dt, state, player);
                    }
                    _ => (),
                }
            }
        }

        /*self.active_events
            .retain(|active_event| active_event.is_active(game_time));*/
    }

    fn update_player(
        &mut self,
        game_dt: comn::GameTime,
        state: &comn::Game,
        player: &comn::PlayerView,
    ) {
        if player.is_dashing {
            let num = (game_dt * 150.0) as usize;
            let (offset, size) = if Some(player.owner) == state.catcher {
                (50.0, 16.0)
            } else {
                (35.0, 12.5)
            };
            let start =
                player.pos - comn::Vector::new(player.angle.cos(), player.angle.sin()) * offset;
            /*self.air_particles.spawn_trail(
                start,
                player.angle,
                std::f32::consts::PI / 8.0,
                1000.0,
                Color::BLUE,
                size,
                num,
            );*/
        }
    }

    pub fn render(
        &mut self,
        now: Instant,
        state: Option<&comn::Game>,
        next_entities: &BTreeMap<comn::EntityId, (comn::GameTime, comn::Entity)>,
        game_time: comn::GameTime,
    ) -> Result<(), webglee::Error> {
        if let Some(state) = state {
            /*{
                coarse_prof::profile!("ground_particles");
                self.ground_particles.render(gfx, self.camera.transform());
            }*/

            {
                coarse_prof::profile!("game");
                render::render_game(
                    state,
                    next_entities,
                    game_time,
                    self.my_player_id,
                    &mut self.stage,
                );
            }

            /*{
                coarse_prof::profile!("air_particles");
                self.air_particles.render(gfx, self.camera.transform());
            }

            {
                coarse_prof::profile!("active_events");
                for active_event in &self.active_events {
                    active_event.render(gfx, state, game_time, self.camera.transform());
                }
            }

            coarse_prof::profile!("overlay");
            overlay::render(
                gfx,
                &mut self.resources,
                state.get_player_entity(self.my_player_id).map(|(_, e)| e),
                Vector::new(self.window_size.x, self.window_size.y) * self.window_scale_factor,
            )?;*/
        }

        /*coarse_prof::profile!("text");
        self.event_list.render(
            now,
            gfx,
            &mut self.resources.font_small,
            Vector::new(10.0, 10.0),
        )?;

        if let Some(state) = state {
            scoreboard::render(
                gfx,
                &mut self.resources.font_small,
                state,
                self.my_player_id,
                Vector::new(self.window_size.x * self.window_scale_factor - 270.0, 10.0),
                Vector::new(300.0, 300.0),
            )?;
        }*/

        Ok(())
    }
}