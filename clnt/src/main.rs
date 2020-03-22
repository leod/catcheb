use std::collections::HashSet;

use stdweb::web::Date;

use quicksilver::{
    geom::{Rectangle, Vector},
    graphics::{Color, Graphics},
    lifecycle::{run, EventStream, Settings, Window, Event, Key},
    Result,
};

fn main() {
    run(
        Settings {
            size: Vector::new(800.0, 600.0).into(),
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
            delta = delta.normalize();
            pos += delta * 300.0 * delta_s;
        }

        // Clear the screen to a blank, white color
        gfx.clear(Color::WHITE);
        // Paint a blue square with a red outline in the center of our screen
        // It should have a top-left of (350, 100) and a size of (150, 100)
        let rect = Rectangle::new(pos, Vector::new(100.0, 100.0));
        gfx.fill_rect(&rect, Color::BLUE);
        gfx.stroke_rect(&rect, Color::RED);
        // Send the data to be drawn
        gfx.present(&window)?;
    }
}
