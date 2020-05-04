mod game;
mod webrtc;

use std::collections::HashSet;

use log::{debug, info};

use js_sys::Date;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::JsFuture;

use quicksilver::{
    geom::{Rectangle, Transform, Vector},
    graphics::{Color, Graphics},
    lifecycle::{run, Event, EventStream, Key, Settings, Window},
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

async fn app(
    window: Window,
    mut gfx: Graphics,
    mut events: EventStream,
) -> quicksilver::Result<()> {
    info!("Starting up");

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

    let mut pos = Vector::new(350.0, 100.0);

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

        game.update();

        let now_time_ms = Date::new_0().get_time();
        let delta_s = ((now_time_ms - last_time_ms) / 1000.0) as f32;
        last_time_ms = now_time_ms;

        let mut delta = Vector::new(0.0, 0.0);

        if pressed_keys.contains(&Key::W) {
            delta.y -= 1.0;
        }
        if pressed_keys.contains(&Key::S) {
            delta.y += 1.0;
        }
        if pressed_keys.contains(&Key::A) {
            delta.x -= 1.0;
        }
        if pressed_keys.contains(&Key::D) {
            delta.x += 1.0;
        }

        if delta.len2() > 0.0 {
            pos += delta.normalize() * 300.0 * delta_s;
        }

        let size = if delta.len2() > 0.0 {
            let angle = delta.y.atan2(delta.x).to_degrees();
            gfx.set_transform(Transform::rotate(angle).then(Transform::translate(pos)));
            Vector::new(70.0, 35.714)
        } else {
            gfx.set_transform(Transform::translate(pos));
            Vector::new(50.0, 50.0)
        };

        gfx.clear(Color::WHITE);

        let rect = Rectangle::new(-size / 2.0, size);

        gfx.fill_rect(&rect, Color::BLUE);
        gfx.stroke_rect(&rect, Color::RED);

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
