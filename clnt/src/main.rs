use std::collections::HashSet;

use stdweb::web::Date;

use quicksilver::{
    geom::{Rectangle, Transform, Vector},
    graphics::{Color, Graphics},
    lifecycle::{run, Event, EventStream, Key, Settings, Window},
    Result,
};

fn main() {
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

async fn app(window: Window, mut gfx: Graphics, mut events: EventStream) -> Result<()> {
    let mut pos = Vector::new(350.0, 100.0);

    let mut pressed_keys: HashSet<Key> = HashSet::new();
    let mut last_time_ms = Date::new().get_time();

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

        let now_time_ms = Date::new().get_time();
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
