mod camera;
mod event_list;
mod overlay;
mod render;
mod resources;
mod scoreboard;

use std::{
    collections::{BTreeMap, HashSet},
    time::Duration,
};

use instant::Instant;
use quicksilver::{geom::Vector, graphics::Graphics, input::Key};

use camera::Camera;
use event_list::EventList;

pub use resources::Resources;

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub event_list: event_list::Config,
    pub camera: camera::Config,
}

pub struct View {
    my_player_id: comn::PlayerId,
    resources: Resources,
    event_list: EventList,
    camera: Camera,
    window_size: comn::Vector,
    window_scale_factor: f32,
}

impl View {
    pub fn new(
        config: Config,
        settings: comn::Settings,
        my_player_id: comn::PlayerId,
        resources: Resources,
        window_size: comn::Vector,
        window_scale_factor: f32,
    ) -> Self {
        let event_list = EventList::new(config.event_list);
        let camera = Camera::new(config.camera, settings.size);

        Self {
            my_player_id,
            resources,
            event_list,
            camera,
            window_size,
            window_scale_factor,
        }
    }

    pub fn resources_mut(&mut self) -> &mut Resources {
        &mut self.resources
    }

    pub fn set_window_size(&mut self, size: comn::Vector, scale_factor: f32) {
        self.window_size = size;
        self.window_scale_factor = scale_factor;
    }

    pub fn update(
        &mut self,
        now: Instant,
        dt: Duration,
        pressed_keys: &HashSet<Key>,
        state: Option<&comn::Game>,
        game_events: &[comn::Event],
        game_time: comn::GameTime,
    ) {
        let follow_entity = state.and_then(|state| {
            state
                .get_player_entity(self.my_player_id)
                .map(|(_id, e)| comn::Entity::Player(e.clone()))
        });

        self.camera.update(
            dt,
            &pressed_keys,
            follow_entity,
            game_time,
            self.window_size * self.window_scale_factor,
        );

        for event in game_events {
            self.event_list.push(now, event.clone());
        }
    }

    pub fn render(
        &mut self,
        now: Instant,
        gfx: &mut Graphics,
        state: Option<&comn::Game>,
        next_entities: &BTreeMap<comn::EntityId, (comn::GameTime, comn::Entity)>,
        game_time: comn::GameTime,
    ) -> quicksilver::Result<()> {
        if let Some(state) = state {
            render::render_game(
                gfx,
                &mut self.resources,
                state,
                next_entities,
                game_time,
                self.my_player_id,
                self.camera.transform(),
            )?;

            coarse_prof::profile!("overlay");
            overlay::render(
                gfx,
                &mut self.resources,
                state.get_player_entity(self.my_player_id).map(|(_, e)| e),
                Vector::new(self.window_size.x, self.window_size.y),
            )?;
        }

        coarse_prof::profile!("text");
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
                Vector::new(1010.0, 10.0),
                Vector::new(300.0, 300.0),
            )?;
        }

        Ok(())
    }
}
