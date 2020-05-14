mod game;
mod webrtc;

use std::collections::HashSet;

use log::{debug, info, warn};
//use instant::Instant;

use js_sys::Date;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::JsFuture;

use quicksilver::{
    geom::{Rectangle, Transform, Vector},
    graphics::{Color, FontRenderer, Graphics, VectorFont},
    lifecycle::{run, Event, EventStream, Key, Settings, Window},
    Timer,
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
        use_item: false,
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
        let ttf = VectorFont::load("Munro-2LYe.ttf").await?;
        let font_small = ttf.to_renderer(gfx, 18.0)?;
        let font = ttf.to_renderer(gfx, 36.0)?;
        let font_large = ttf.to_renderer(gfx, 58.0)?;

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
) -> quicksilver::Result<()> {
    gfx.clear(Color::WHITE);

    let time = state.tick_num.0 as f32 * state.settings.tick_duration().as_secs_f32();

    for entity in state.entities.values() {
        match entity {
            comn::Entity::Player(player) => {
                let pos: mint::Vector2<f32> = player.pos.coords.into();
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
            e => panic!("unhandled entity rendering: {:?}", e),
        }
    }

    Ok(())
}

async fn app(
    window: Window,
    mut gfx: Graphics,
    mut events: EventStream,
) -> quicksilver::Result<()> {
    info!("Starting up");

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
    let mut input_timer = Timer::time_per_second(game.settings().ticks_per_second as f32);

    let mut pressed_keys: HashSet<Key> = HashSet::new();
    let mut last_time_ms = Date::new_0().get_time();

    let mut delta_ms_var = stats::Var::default();
    let mut frame_ms_var = stats::Var::default();

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

        let now_time_ms = Date::new_0().get_time();
        let delta_ms = now_time_ms - last_time_ms;
        let delta_s = (delta_ms / 1000.0) as f32;
        last_time_ms = now_time_ms;

        game.update();

        //while input_timer.tick() {
        if input_timer.tick() {
            game.player_input(&current_input(&pressed_keys));
        }

        render_game(&mut gfx, &mut resources, &game.state())?;

        delta_ms_var.record(delta_ms as f32);

        resources.font_small.draw(
            &mut gfx,
            &format!("delta: {:.1}ms", delta_ms_var.mean().unwrap_or(-1.0)),
            Color::BLACK,
            Vector::new(10.0, 15.0),
        )?;
        resources.font_small.draw(
            &mut gfx,
            &format!("frame: {:.1}ms", frame_ms_var.mean().unwrap_or(-1.0)),
            Color::BLACK,
            Vector::new(10.0, 35.0),
        )?;
        resources.font_small.draw(
            &mut gfx,
            &format!(
                "ping: {:.1}ms",
                game.ping().estimate().as_secs_f32() * 1000.0
            ),
            Color::BLACK,
            Vector::new(10.0, 55.0),
        )?;

        gfx.present(&window)?;

        let end_time_ms = Date::new_0().get_time();
        frame_ms_var.record((end_time_ms - now_time_ms) as f32);
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
