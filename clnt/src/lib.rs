mod join;
mod prediction;
mod runner;
//mod view;
mod webrtc;

use std::{cell::RefCell, collections::HashSet, rc::Rc};

use wasm_bindgen::{
    prelude::{wasm_bindgen, Closure},
    JsCast, JsValue, UnwrapThrowExt,
};

use instant::Instant;
use log::info;

use webglee::{Context, Event, InputState, Key};

use comn::util::stats;

//use crate::view::View;

fn current_input(input_state: &InputState) -> comn::Input {
    comn::Input {
        move_left: input_state.key(Key::A),
        move_right: input_state.key(Key::D),
        move_up: input_state.key(Key::W),
        move_down: input_state.key(Key::S),
        dash: input_state.key(Key::Space),
        use_action: input_state.key(Key::LShift),
        shoot: input_state.key(Key::Q),
    }
}

/// Statistics for debugging.
#[derive(Default)]
struct Stats {
    dt_ms: stats::Var,
    frame_ms: stats::Var,
}

#[wasm_bindgen(start)]
pub fn main() {
    wasm_bindgen_futures::spawn_local(async {
        start().await.unwrap_throw();
    });
}

pub async fn start() -> Result<(), JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Debug).unwrap();

    info!("Starting up");

    let ctx = Context::from_canvas_id("canvas").unwrap();

    /*let config = view::Config::default();
    let resources = view::Resources::load(&mut gfx).await?;*/

    // TODO: Graceful error handling in client
    let runner = join::join_and_connect(comn::JoinRequest {
        game_id: None,
        player_name: "Pioneer".to_string(),
    })
    .await
    .expect("Failed to connect");

    Ok(())

    /*let mut view = View::new(
        config,
        runner.settings().clone(),
        runner.my_player_id(),
        resources,
        comn::Vector::new(window.size().x, window.size().y),
        window.scale_factor(),
    );*/

    /*let mut stats = Stats::default();
    let mut show_stats = false;
    let mut lag_frames: usize = 0;

    let mut pressed_keys: HashSet<Key> = HashSet::new();
    let mut last_time = Instant::now();

    // Wrap the Runner in RefCell so that it can be used in Window callback
    let runner = Rc::new(RefCell::new(runner));
    let on_before_unload = Closure::wrap(Box::new({
        let runner = runner.clone();
        move |_: &web_sys::Event| {
            info!("Disconnecting...");
            runner.borrow_mut().disconnect();
        }
    }) as Box<dyn FnMut(&web_sys::Event)>);

    web_sys::window()
        .expect("Failed to get Window")
        .set_onbeforeunload(Some(on_before_unload.as_ref().unchecked_ref()));

    ctx.main_loop(move |mut ctx, dt, events, _running| {
        coarse_prof::profile!("loop");

        for event in events {
            match event {
                Event::KeyPressed(key) => match key {
                    Key::K => {
                        show_stats = !show_stats;
                    }
                    Key::P => {
                        let mut writer = std::io::Cursor::new(Vec::new());
                        coarse_prof::write(&mut writer).unwrap();
                        coarse_prof::reset();
                        log::info!(
                            "{}",
                            std::str::from_utf8(&writer.into_inner()).unwrap()
                        );
                    }
                    Key::L => {
                        lag_frames = 30;
                    }
                    _ => (),
                }
                _ => (),
            }
        }

        coarse_prof::profile!("frame");

        let mut runner = runner.borrow_mut();

        if lag_frames > 0 {
            lag_frames -= 1;
            return;
        }

        let start_time = Instant::now();
        let last_dt = start_time.duration_since(last_time);
        last_time = start_time;

        let game_events = if runner.is_good() {
            coarse_prof::profile!("update");

            runner.update(start_time, last_dt, &current_input(ctx.input_state()))
        } else {
            Vec::new()
        };

        let state = runner.state();

        /*{
            coarse_prof::profile!("update_view");

            view.set_window_size(
                comn::Vector::new(window.size().x, window.size().y),
                window.scale_factor(),
            );
            view.update(
                start_time,
                last_dt,
                &pressed_keys,
                state.as_ref(),
                &game_events,
                runner.interp_game_time(),
            );
        }*/

        coarse_prof::profile!("render");
        /*gfx.clear(Color::from_hex("D4D6B9"));

        {
            coarse_prof::profile!("view");

            view.render(
                start_time,
                &mut gfx,
                state.as_ref(),
                &runner.next_entities(),
                runner.interp_game_time(),
            )?;
        }

        if !runner.is_good() {
            view.resources_mut().font.draw(
                &mut gfx,
                "Lost connection to server",
                Color::RED,
                Vector::new(250.0, 25.0),
            )?;
        }

        let mut debug_y: f32 = window.size().y * window.scale_factor() - 200.0;
        let mut debug = |s: &str| -> quicksilver::Result<()> {
            view.resources_mut().font_small.draw(
                &mut gfx,
                s,
                Color::BLACK,
                Vector::new(
                    window.size().x * window.scale_factor() / 2.0 - 100.0,
                    debug_y,
                ),
            )?;
            debug_y += 12.0;
            Ok(())
        };

        if show_stats {
            coarse_prof::profile!("stats");

            debug(&format!(
                "ping:               {:>7.3}",
                runner.ping().estimate().as_secs_f32() * 1000.0
            ))?;
            debug(&format!(
                "recv stddev:        {:>7.3}",
                1000.0 * runner.stats().recv_delay_std_dev,
            ))?;
            debug(&format!(
                "loss (%):           {:>7.3}",
                runner.stats().loss.estimate().map_or(100.0, |p| p * 100.0)
            ))?;
            debug(&format!(
                "skip loss (%):      {:>7.3}",
                runner
                    .stats()
                    .skip_loss
                    .estimate()
                    .map_or(100.0, |p| p * 100.0)
            ))?;
            debug(&format!(
                "recv rate (kB/s):   {:>7.3}",
                runner.stats().recv_rate / 1000.0
            ))?;
            debug(&format!(
                "send rate (kB/s):   {:>7.3}",
                runner.stats().send_rate / 1000.0
            ))?;
            debug("")?;
            debug("                        cur      min      max     mean   stddev")?;
            debug(&format!("dt (ms):           {}", stats.dt_ms))?;
            debug(&format!("frame (ms):        {}", stats.frame_ms))?;
            debug(&format!(
                "time lag (ms):     {}",
                runner.stats().time_lag_ms
            ))?;
            debug(&format!(
                "time lag dev (ms): {}",
                runner.stats().time_lag_deviation_ms
            ))?;
            debug(&format!(
                "time warp:         {}",
                runner.stats().time_warp_factor
            ))?;
            debug(&format!(
                "tick interp:       {}",
                runner.stats().tick_interp
            ))?;
            debug(&format!(
                "input delay:       {}",
                runner.stats().input_delay
            ))?;
        }

        {
            coarse_prof::profile!("present");
            gfx.present(&window)?;
        }*/

        // Keep some statistics for debugging...
        stats.dt_ms.record(last_dt.as_secs_f32() * 1000.0);
        stats
            .frame_ms
            .record(Instant::now().duration_since(start_time).as_secs_f32() * 1000.0);
    })
    .unwrap();*/
}
