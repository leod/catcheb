mod camera;
mod event_list;
mod game;
mod join;
mod prediction;
mod render;
mod webrtc;

use std::{collections::HashSet, time::Duration};

use wasm_bindgen::prelude::wasm_bindgen;

use instant::Instant;
use log::info;

use quicksilver::{
    geom::Vector,
    graphics::{Color, Graphics},
    input::{Event, Input, Key},
    Settings, Window,
};

use comn::util::stats;

const SCREEN_SIZE: Vector = Vector { x: 800.0, y: 600.0 };

#[wasm_bindgen(start)]
pub fn main() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    quicksilver::run(
        Settings {
            size: SCREEN_SIZE,
            title: "Play Catcheb",
            resizable: true,
            log_level: log::Level::Debug,
            ..Settings::default()
        },
        app,
    );
}

pub fn current_input(pressed_keys: &HashSet<Key>) -> comn::Input {
    comn::Input {
        move_left: pressed_keys.contains(&Key::A),
        move_right: pressed_keys.contains(&Key::D),
        move_up: pressed_keys.contains(&Key::W),
        move_down: pressed_keys.contains(&Key::S),
        use_item: pressed_keys.contains(&Key::Space),
        use_action: false,
    }
}

/// Statistics for debugging.
#[derive(Default)]
struct Stats {
    dt_ms: stats::Var,
    smoothed_dt_ms: stats::Var,
    frame_ms: stats::Var,
}

#[derive(Debug, Clone, Default)]
struct Config {
    event_list: event_list::Config,
    camera: camera::Config,
}

async fn app(window: Window, mut gfx: Graphics, mut input: Input) -> quicksilver::Result<()> {
    info!("Starting up");

    let config = Config::default();
    let mut resources = render::Resources::load(&mut gfx).await?;

    // TODO: Graceful error handling in client
    let mut game = join::join_and_connect(
        comn::JoinRequest {
            game_id: None,
            player_name: "Pioneer".to_string(),
        },
        &mut input,
    )
    .await
    .unwrap();

    let mut camera = camera::Camera::new(config.camera, game.settings().size);

    let mut event_list = event_list::EventList::new(config.event_list);

    let mut stats = Stats::default();
    let mut show_stats = false;

    let mut pressed_keys: HashSet<Key> = HashSet::new();
    let mut last_time = Instant::now();
    let mut dt_smoothing = stats::Var::new(Duration::from_millis(100));
    let mut now = last_time;
    //let mut dt_smoothing = 16.6666667;

    loop {
        while let Some(event) = input.next_event().await {
            match event {
                Event::KeyboardInput(event) => {
                    if !pressed_keys.contains(&event.key()) {
                        if event.key() == Key::P {
                            show_stats = !show_stats;
                        }
                    }

                    if event.is_down() {
                        pressed_keys.insert(event.key());
                    } else {
                        pressed_keys.remove(&event.key());
                    }
                }
                Event::FocusChanged(event) if !event.is_focused() => {
                    pressed_keys.clear();
                }
                _ => (),
            }
        }

        if !game.is_good() {
            // TODO: Graceful error handling in client
            panic!("Game lost connection");
        }

        let start_time = Instant::now();
        let last_dt = start_time.duration_since(last_time);
        last_time = start_time;

        //dt_smoothing += 0.1 * (last_dt.as_secs_f32() - dt_smoothing);
        //let dt = Duration::from_secs_f32(dt_smoothing);

        // TODO: dt smoothing is just not a good idea
        dt_smoothing.record(last_dt.as_secs_f32());
        let smoothed_dt = last_dt;
        //Duration::from_secs_f32(dt_smoothing.mean().unwrap_or(last_dt.as_secs_f32()));
        now += smoothed_dt;

        let events = game.update(now, smoothed_dt, &current_input(&pressed_keys));

        for event in events {
            event_list.push(now, event);
        }

        {
            let follow_entity = game.state().and_then(|state| {
                state
                    .get_player_entity(game.my_player_id())
                    .map(|(_id, e)| comn::Entity::Player(e.clone()))
            });
            camera.update(
                smoothed_dt,
                &pressed_keys,
                follow_entity,
                game.interp_game_time(),
                comn::Vector::new(window.size().x, window.size().y) * window.scale_factor(),
            );
        }

        gfx.clear(Color::WHITE);

        if let Some(state) = game.state() {
            render::render_game(
                &mut gfx,
                &mut resources,
                &state,
                &game.next_entities(),
                game.interp_game_time(),
                game.my_player_id(),
                camera.transform(),
            )?;
        }

        let mut debug_y: f32 = 15.0;
        let mut debug = |s: &str| -> quicksilver::Result<()> {
            resources
                .font_small
                .draw(&mut gfx, s, Color::BLACK, Vector::new(10.0, debug_y))?;
            debug_y += 12.0;
            Ok(())
        };

        /*if let Some((_, my_entity)) = game
            .state()
            .and_then(|state| state.get_player_entity(game.my_player_id()).unwrap())
        {
            let cooldown = (my_entity.next_shot_time - game.interp_game_time()).max(0.0);
            debug(&format!("gun cooldown: {:>3.1}", cooldown))?;
            debug(&format!("shots left:   {}", my_entity.shots_left))?;
        } else {
            // lol
            debug("")?;
            debug("")?;
        }*/

        if show_stats {
            for _ in 0..33 {
                debug("")?;
            }

            debug(&format!(
                "ping:               {:>7.3}",
                game.ping().estimate().as_secs_f32() * 1000.0
            ))?;
            debug(&format!(
                "recv stddev:        {:>7.3}",
                1000.0 * game.stats().recv_delay_std_dev,
            ))?;
            debug(&format!(
                "loss (%):           {:>7.3}",
                game.stats().loss.estimate().map_or(100.0, |p| p * 100.0)
            ))?;
            debug(&format!(
                "skip loss (%):      {:>7.3}",
                game.stats()
                    .skip_loss
                    .estimate()
                    .map_or(100.0, |p| p * 100.0)
            ))?;
            debug(&format!(
                "recv rate (kB/s):   {:>7.3}",
                game.stats().recv_rate / 1000.0
            ))?;
            debug(&format!(
                "send rate (kB/s):   {:>7.3}",
                game.stats().send_rate / 1000.0
            ))?;
            debug("")?;
            debug("                        cur      min      max     mean   stddev")?;
            debug(&format!("dt (ms):           {}", stats.dt_ms))?;
            debug(&format!("smoothed dt (ms):  {}", stats.smoothed_dt_ms))?;
            debug(&format!("frame (ms):        {}", stats.frame_ms))?;
            debug(&format!("time lag (ms):     {}", game.stats().time_lag_ms))?;
            debug(&format!(
                "time lag dev (ms): {}",
                game.stats().time_lag_deviation_ms
            ))?;
            debug(&format!(
                "time warp:         {}",
                game.stats().time_warp_factor
            ))?;
            debug(&format!("tick interp:       {}", game.stats().tick_interp))?;
            debug(&format!("input delay:       {}", game.stats().input_delay))?;
        }

        event_list.render(
            now,
            &mut gfx,
            &mut resources.font_small,
            Vector::new(600.0, 30.0),
        )?;

        gfx.present(&window)?;

        // Keep some statistics for debugging...
        stats.dt_ms.record(last_dt.as_secs_f32() * 1000.0);
        stats
            .smoothed_dt_ms
            .record(smoothed_dt.as_secs_f32() * 1000.0);
        stats
            .frame_ms
            .record(Instant::now().duration_since(start_time).as_secs_f32() * 1000.0);
    }
}
