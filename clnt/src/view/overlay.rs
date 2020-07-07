use quicksilver::{
    geom::{Rectangle, Vector},
    graphics::{Color, FontRenderer, Graphics, Image},
};

use crate::view::Resources;

const MARGIN: f32 = 30.0;
const HEIGHT: f32 = ICON_SIZE;
const ICON_SIZE: f32 = 32.0;

pub fn render(
    gfx: &mut Graphics,
    resources: &mut Resources,
    entity: Option<&comn::PlayerEntity>,
    window_size: Vector,
) -> quicksilver::Result<()> {
    if let Some(entity) = entity {
        render_ability(
            gfx,
            &mut resources.font_small,
            &mut resources.font,
            &resources.icon_hook,
            "shift",
            entity.hook.is_some(),
            entity.hook_cooldown,
            Vector::new(MARGIN, window_size.y - HEIGHT - MARGIN),
        )?;
        render_ability(
            gfx,
            &mut resources.font_small,
            &mut resources.font,
            &resources.icon_dash,
            "space",
            entity.dash.is_some(),
            entity.dash_cooldown,
            Vector::new(
                MARGIN + 1.0 * (MARGIN + ICON_SIZE),
                window_size.y - HEIGHT - MARGIN,
            ),
        )?;
    }
    Ok(())
}

fn render_ability(
    gfx: &mut Graphics,
    font_small: &mut FontRenderer,
    font: &mut FontRenderer,
    image: &Image,
    key: &str,
    active: bool,
    cooldown: f32,
    pos: Vector,
) -> quicksilver::Result<()> {
    let tint = if active {
        Color::from_rgba(173, 216, 230, 1.0)
    } else if cooldown > 0.0 {
        Color::from_rgba(128, 128, 128, 1.0)
    } else {
        Color::from_rgba(255, 255, 255, 1.0)
    };
    let key_size = font_small.layout_glyphs(gfx, key, None, |_, _| ()).unwrap();
    font_small.draw(
        gfx,
        key,
        Color::BLACK,
        pos + Vector::new((ICON_SIZE - key_size.x) / 2.0, ICON_SIZE + 10.0),
    )?;
    gfx.draw_image_tinted(
        image,
        Rectangle::new(pos, Vector::new(ICON_SIZE, ICON_SIZE)),
        tint,
    );
    if cooldown > 0.0 {
        let t = ((cooldown * 10.0) as usize).to_string();
        let t_size = font_small.layout_glyphs(gfx, &t, None, |_, _| ()).unwrap();
        font.draw(
            gfx,
            &t,
            Color::GREEN,
            pos + Vector::new(1.0, 20.0), //Vector::new(ICON_SIZE / 2.0 - t_size.x, ICON_SIZE / 2.0 + 2.0 * t_size.y),
        )?;
    }

    Ok(())
}
