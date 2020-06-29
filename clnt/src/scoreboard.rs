use quicksilver::{
    geom::Vector,
    graphics::{Color, FontRenderer, Graphics},
};

pub fn render(
    gfx: &mut Graphics,
    font: &mut FontRenderer,
    state: &comn::Game,
    pos: Vector,
    _size: Vector,
) -> quicksilver::Result<()> {
    let mut players: Vec<_> = state.players.clone().into_iter().collect();
    players.sort_by_key(|(_, player)| player.food);

    let x0 = pos.x;
    let x1 = pos.x + 50.0;
    let x2 = pos.x + 200.0;

    font.draw(gfx, "id", Color::BLUE, Vector::new(x0, pos.y))?;
    font.draw(gfx, "name", Color::BLUE, Vector::new(x1, pos.y))?;
    font.draw(gfx, "food", Color::BLUE, Vector::new(x2, pos.y))?;

    for (i, (player_id, player)) in players.into_iter().rev().enumerate() {
        let y = pos.y + (i + 1) as f32 * 12.0;
        font.draw(
            gfx,
            &player_id.0.to_string(),
            Color::BLACK,
            Vector::new(x0, y),
        )?;
        font.draw(gfx, &player.name, Color::BLACK, Vector::new(x1, y))?;
        font.draw(
            gfx,
            &player.food.to_string(),
            Color::BLACK,
            Vector::new(x2, y),
        )?;
    }

    Ok(())
}
