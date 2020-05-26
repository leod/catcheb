mod event_list;
mod game;
mod prediction;
mod webrtc;

use std::collections::{BTreeMap, HashSet};

use instant::Instant;
use log::{debug, info, warn};

use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::JsFuture;

use quicksilver::{
    geom::{Circle, Rectangle, Transform, Vector},
    graphics::{Color, FontRenderer, Graphics, VectorFont},
    lifecycle::{run, Event, EventStream, Key, Settings, Window},
};

use comn::{
    game::run::{PLAYER_MOVE_L, PLAYER_MOVE_W, PLAYER_SIT_L, PLAYER_SIT_W},
    util::stats,
};

#[wasm_bindgen(start)]
pub fn main() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    run(
        Settings {
            size: Vector::new(1280.0, 720.0).into(),
            fullscreen: true,
            title: "Play Catcheb",
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

pub struct Resources {
    pub ttf: VectorFont,
    pub font_small: FontRenderer,
    pub font: FontRenderer,
    pub font_large: FontRenderer,
}

impl Resources {
    pub async fn load(gfx: &mut Graphics) -> quicksilver::Result<Self> {
        let ttf = VectorFont::load("kongtext.ttf").await?;
        let font_small = ttf.to_renderer(gfx, 9.0)?;
        let font = ttf.to_renderer(gfx, 18.0)?;
        let font_large = ttf.to_renderer(gfx, 40.0)?;

        Ok(Self {
            ttf,
            font_small,
            font,
            font_large,
        })
    }
}

pub fn render_game(
    gfx: &mut Graphics,
    resources: &mut Resources,
    state: &comn::Game,
    next_state: &BTreeMap<comn::EntityId, (comn::GameTime, comn::Entity)>,
    time: comn::GameTime,
) -> quicksilver::Result<()> {
    gfx.clear(Color::WHITE);

    let state_time = state.tick_game_time(state.tick_num);

    for (entity_id, entity) in state.entities.iter() {
        match entity {
            comn::Entity::Player(player) => {
                let pos = if let Some((next_time, next_entity)) = next_state.get(entity_id) {
                    let tau = (time - state_time) / (next_time - state_time);

                    if let Ok(next_player) = next_entity.player() {
                        let delta = next_player.pos - player.pos;
                        (player.pos + tau * delta).coords
                    } else {
                        player.pos.coords
                    }
                } else {
                    player.pos.coords
                };
                let pos: mint::Vector2<f32> = pos.into();

                let size = if let Some(angle) = player.angle {
                    gfx.set_transform(
                        Transform::rotate(angle.to_degrees()).then(Transform::translate(pos)),
                    );
                    Vector::new(PLAYER_MOVE_W, PLAYER_MOVE_L)
                } else {
                    gfx.set_transform(Transform::translate(pos));
                    Vector::new(PLAYER_SIT_W, PLAYER_SIT_L)
                };
                let rect = Rectangle::new(-size / 2.0, size);

                gfx.fill_rect(&rect, Color::BLUE);
                gfx.stroke_rect(&rect, Color::GREEN);

                gfx.set_transform(Transform::IDENTITY);
                resources
                    .font
                    .draw(gfx, &player.owner.0.to_string(), Color::BLACK, pos.into())?;
            }
            comn::Entity::DangerGuy(danger_guy) => {
                let origin: mint::Vector2<f32> =
                    (danger_guy.pos(time) - danger_guy.size / 2.0).coords.into();
                let size: mint::Vector2<f32> = danger_guy.size.into();
                let rect = Rectangle::new(origin, size);
                gfx.fill_rect(&rect, Color::RED);
            }
            comn::Entity::Bullet(bullet) => {
                //debug!("bullet at {:?}", bullet);
                let origin: mint::Vector2<f32> = bullet.pos(time).coords.into();
                let circle = Circle::new(origin, 10.0);
                gfx.fill_circle(&circle, Color::ORANGE);
            }
        }
    }

    Ok(())
}

#[derive(Default)]
struct Stats {
    dt_ms: stats::Var,
    frame_ms: stats::Var,
    time_lag_ms: stats::Var,
    time_warp_factor: stats::Var,
    tick_interp: stats::Var,
}

#[derive(Debug, Clone, Default)]
struct Config {
    event_list: event_list::Config,
}

async fn app(
    window: Window,
    mut gfx: Graphics,
    mut events: EventStream,
) -> quicksilver::Result<()> {
    info!("Starting up");

    let config = Config::default();
    let mut resources = Resources::load(&mut gfx).await?;

    // TODO: Graceful error handling in client
    let join_reply = join_request(comn::JoinRequest {
        game_id: None,
        player_name: "Pioneer".to_string(),
    })
    .await
    .unwrap();

    // TODO: Graceful error handling in client
    let join_success = join_reply.expect("Failed to join game");

    // TODO: Graceful error handling in client
    let my_token = join_success.your_token;
    let on_message = Box::new(
        move |client_data: &webrtc::Data, message: &comn::ServerMessage| {
            on_message(my_token, client_data, message)
        },
    );
    let webrtc_client = webrtc::Client::connect(Default::default(), on_message)
        .await
        .unwrap();

    while webrtc_client.status() == webrtc::Status::Connecting {
        events.next_event().await;
    }

    if webrtc_client.status() != webrtc::Status::Open {
        // TODO: Graceful error handling in client
        panic!(
            "Failed to establish WebRTC connection: {:?}",
            webrtc_client.status()
        );
    }

    let mut game = game::Game::new(join_success, webrtc_client);

    let mut pressed_keys: HashSet<Key> = HashSet::new();
    let mut last_time = Instant::now();

    let mut event_list = event_list::EventList::new(config.event_list);

    let mut stats = Stats::default();

    loop {
        while let Some(event) = events.next_event().await {
            match event {
                Event::KeyboardInput(event) => {
                    if event.is_down() {
                        pressed_keys.insert(event.key());
                    } else {
                        pressed_keys.remove(&event.key());
                    }
                }
                _ => (),
            }
        }

        if !game.is_good() {
            // TODO: Graceful error handling in client
            panic!("Game lost connection");
        }

        let start_time = Instant::now();
        let dt = start_time.duration_since(last_time);
        last_time = start_time;

        let recv_game_time = game
            .recv_tick_time()
            .estimate(Instant::now())
            .unwrap_or(-1.0);

        let events = game.update(dt, &current_input(&pressed_keys));

        for event in events {
            event_list.push(event);
        }

        stats.time_warp_factor.record(game.next_time_warp_factor());
        stats.dt_ms.record(dt.as_secs_f32() * 1000.0);
        stats
            .time_lag_ms
            .record((recv_game_time - game.interp_game_time()) * 1000.0);
        stats
            .tick_interp
            .record(game.next_tick().map_or(0.0, |(next_tick_num, _)| {
                (next_tick_num.0 - game.state().tick_num.0) as f32
            }));

        render_game(
            &mut gfx,
            &mut resources,
            &game.state(),
            &game.next_state(),
            game.interp_game_time(),
        )?;

        let mut debug_y: f32 = 15.0;
        let mut debug = |s: &str| -> quicksilver::Result<()> {
            resources
                .font_small
                .draw(&mut gfx, s, Color::BLACK, Vector::new(10.0, debug_y))?;
            debug_y += 12.0;
            Ok(())
        };

        debug(&format!(
            "ping (ms):     {:.1}",
            game.ping().estimate().as_secs_f32() * 1000.0
        ))?;
        debug(&format!(
            "recv std dev:  {:.2}",
            1000.0 * game.recv_tick_time().recv_delay_std_dev().unwrap_or(-1.0),
        ))?;
        debug("")?;
        debug(&format!("dt (ms):       {}", stats.dt_ms))?;
        debug(&format!("frame (ms):    {}", stats.frame_ms))?;
        debug(&format!("time lag (ms): {}", stats.time_lag_ms))?;
        debug(&format!("time warp:     {}", stats.time_warp_factor))?;
        debug(&format!("tick interp:   {}", stats.tick_interp))?;

        event_list.render(
            &mut gfx,
            &mut resources.font_small,
            Vector::new(800.0, 15.0),
        )?;

        gfx.present(&window)?;

        let end_time = Instant::now();

        stats
            .frame_ms
            .record(end_time.duration_since(start_time).as_secs_f32() * 1000.0);
    }
}

pub async fn join_request(request: comn::JoinRequest) -> Result<comn::JoinReply, JsValue> {
    let request_json = format!(
        "{{\"game_id\":{},\"player_name\":\"{}\"}}",
        request
            .game_id
            .map_or("null".to_owned(), |comn::GameId(id)| "\"".to_owned()
                + &id.to_string()
                + "\""),
        request.player_name,
    );

    let mut opts = web_sys::RequestInit::new();
    opts.method("POST");
    opts.mode(web_sys::RequestMode::SameOrigin);
    opts.body(Some(&JsValue::from_str(&request_json)));

    info!("Requesting to join game: {} ...", request_json);

    let request = web_sys::Request::new_with_str_and_init(&"/join", &opts)?;
    request.headers().set("Accept", "application/json")?;

    let window = web_sys::window().unwrap();
    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
    assert!(resp_value.is_instance_of::<web_sys::Response>());
    let resp: web_sys::Response = resp_value.dyn_into().unwrap();

    // Convert this other `Promise` into a rust `Future`.
    let reply = JsFuture::from(resp.json()?).await?;

    info!("Join reply: {:?}", reply);

    // Use serde to parse the JSON into a struct.
    Ok(reply.into_serde().unwrap())
}

pub fn on_message(
    my_token: comn::PlayerToken,
    client_data: &webrtc::Data,
    message: &comn::ServerMessage,
) {
    if let comn::ServerMessage::Ping(sequence_num) = message {
        let reply = comn::ClientMessage::Pong(*sequence_num);
        let signed_message = comn::SignedClientMessage(my_token, reply);
        let data = signed_message.serialize();
        if let Err(err) = client_data.send(&data) {
            warn!("Failed to send message: {:?}", err);
        }
    }
}
