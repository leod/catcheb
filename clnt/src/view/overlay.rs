use quicksilver::{
    geom::{Rectangle, Vector},
    graphics::{Color, FontRenderer, Graphics, Image},
};

use comn::game::run::{HOOK_COOLDOWN, PLAYER_DASH_COOLDOWN};

use crate::view::Resources;

const PADDING: f32 = 10.0;
const MARGIN: f32 = 20.0;
const HEIGHT: f32 = ICON_SIZE;
const ICON_SIZE: f32 = 32.0;

pub fn box_thing(gfx: &mut Graphics, pos: Vector, size: Vector) -> quicksilver::Result<()> {
    gfx.fill_rect(
        &Rectangle::new(pos, size),
        Color::from_rgba(240, 240, 240, 1.0),
    );
    gfx.stroke_rect(&Rectangle::new(pos, size), Color::BLACK);
    Ok(())
}

pub fn render(
    gfx: &mut Graphics,
    resources: &mut Resources,
    entity: Option<&comn::PlayerEntity>,
    window_size: Vector,
) -> quicksilver::Result<()> {
    if let Some(entity) = entity {
        box_thing(
            gfx,
            Vector::new(PADDING, window_size.y - HEIGHT - 2.0 * PADDING - MARGIN),
            Vector::new(2.0 * (ICON_SIZE + MARGIN), HEIGHT + 2.0 * PADDING + 10.0),
        )?;
        render_ability(
            gfx,
            &mut resources.font_small,
            &resources.icon_hook,
            "shift",
            entity.hook.is_some(),
            entity.hook_cooldown / HOOK_COOLDOWN,
            Vector::new(MARGIN, window_size.y - HEIGHT - PADDING - MARGIN),
        )?;
        render_ability(
            gfx,
            &mut resources.font_small,
            &resources.icon_dash,
            "space",
            entity.dash.is_some(),
            entity.dash_cooldown / PLAYER_DASH_COOLDOWN,
            Vector::new(
                MARGIN + 1.0 * (MARGIN + ICON_SIZE),
                window_size.y - HEIGHT - PADDING - MARGIN,
            ),
        )?;
    }
    Ok(())
}

fn render_ability(
    gfx: &mut Graphics,
    font_small: &mut FontRenderer,
    image: &Image,
    key: &str,
    active: bool,
    cooldown: f32,
    pos: Vector,
) -> quicksilver::Result<()> {
    let (tint, outline) = if active {
        (
            Color::from_rgba(80, 220, 100, 1.0),
            Color::from_rgba(80, 220, 100, 1.0),
        )
    } else if cooldown > 0.0 {
        //Color::from_rgba(128, 128, 128, 1.0)
        (
            Color::from_rgba(255, 255, 255, 1.0),
            Color::from_rgba(54, 169, 254, 1.0),
        )
    } else {
        (
            Color::from_rgba(255, 255, 255, 1.0),
            Color::from_rgba(128, 128, 128, 1.0),
        )
    };
    gfx.fill_rect(
        &Rectangle::new(
            pos - Vector::new(2.0, 2.0),
            Vector::new(ICON_SIZE + 4.0, ICON_SIZE + 4.0),
        ),
        outline,
    );
    let key_size = font_small.layout_glyphs(gfx, key, None, |_, _| ()).unwrap();
    font_small.draw(
        gfx,
        key,
        Color::BLACK,
        pos + Vector::new((ICON_SIZE - key_size.x) / 2.0, ICON_SIZE + 13.0),
    )?;
    gfx.draw_image_tinted(
        image,
        Rectangle::new(pos, Vector::new(ICON_SIZE, ICON_SIZE)),
        tint,
    );
    if cooldown > 0.0 {
        gfx.fill_rect(
            &Rectangle::new(pos, Vector::new(cooldown * ICON_SIZE, ICON_SIZE)),
            Color::from_rgba(54, 169, 254, 1.0),
        );
    }

    Ok(())
}
