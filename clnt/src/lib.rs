mod game;
mod webrtc;

use std::collections::HashSet;

use log::{debug, info};

use js_sys::Date;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::JsFuture;

use quicksilver::{
    geom::{Rectangle, Transform, Vector},
    graphics::{Color, FontRenderer, Graphics, VectorFont},
    lifecycle::{run, Event, EventStream, Key, Settings, Window},
    Timer,
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
    pub font: FontRenderer,
}

impl Resources {
    pub async fn load(gfx: &mut Graphics) -> quicksilver::Result<Self> {
        let ttf = VectorFont::load("Munro-2LYe.ttf").await?;
        let font = ttf.to_renderer(gfx, 36.0)?;

        Ok(Self { ttf, font })
    }
}

pub fn render_game(
    gfx: &mut Graphics,
    resources: &mut Resources,
    state: &comn::Game,
) -> quicksilver::Result<()> {
    gfx.clear(Color::WHITE);

    for entity in state.entities.values() {
        match entity {
            comn::Entity::Player(player) => {
                let pos: mint::Vector2<f32> = player.pos.coords.into();
                let size = if let Some(angle) = player.angle {
                    gfx.set_transform(
                        Transform::rotate(angle.to_degrees()).then(Transform::translate(pos)),
                    );
                    Vector::new(70.0, 35.714)
                } else {
                    gfx.set_transform(Transform::translate(pos));
                    Vector::new(50.0, 50.0)
                };
                let rect = Rectangle::new(-size / 2.0, size);

                gfx.fill_rect(&rect, Color::BLUE);
                gfx.stroke_rect(&rect, Color::RED);

                gfx.set_transform(Transform::IDENTITY);
                resources
                    .font
                    .draw(gfx, &player.owner.0.to_string(), Color::BLACK, pos.into())?;
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
    let webrtc_client = webrtc::Client::connect(Default::default()).await.unwrap();

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

        game.update().await;

        while input_timer.tick() {
            game.player_input(&current_input(&pressed_keys));
        }

        let now_time_ms = Date::new_0().get_time();
        let delta_s = ((now_time_ms - last_time_ms) / 1000.0) as f32;
        last_time_ms = now_time_ms;

        render_game(&mut gfx, &mut resources, &game.state())?;
        gfx.present(&window)?;
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
