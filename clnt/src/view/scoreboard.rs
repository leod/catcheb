use quicksilver::{
    geom::Vector,
    graphics::{Color, FontRenderer, Graphics},
};

use crate::view::overlay;

pub const MAX_SCOREBOARD_ENTRIES: usize = 5;

pub fn render(
    gfx: &mut Graphics,
    font: &mut FontRenderer,
    state: &comn::Game,
    my_player_id: comn::PlayerId,
    mut pos: Vector,
    _size: Vector,
) -> quicksilver::Result<()> {
    let mut players: Vec<_> = state.players.clone().into_iter().collect();
    players.sort_by_key(|(_, player)| -(player.food as isize));
    while players.len() > MAX_SCOREBOARD_ENTRIES {
        players.pop();
    }

    if !players
        .iter()
        .any(|(player_id, _)| *player_id == my_player_id)
    {
        if let Some(me) = state.players.get(&my_player_id) {
            players.pop();
            players.push((my_player_id, me.clone()));
        }
    }

    overlay::box_thing(
        gfx,
        pos - Vector::new(0.0, 6.0),
        Vector::new(260.0, 12.0 * (players.len() as f32 + 1.0) + 14.0),
    )?;
    pos += Vector::new(10.0, 10.0);

    let x0 = pos.x;
    let x1 = pos.x + 50.0;
    let x2 = pos.x + 200.0;

    font.draw(gfx, "id", Color::BLUE, Vector::new(x0, pos.y))?;
    font.draw(gfx, "name", Color::BLUE, Vector::new(x1, pos.y))?;
    font.draw(gfx, "food", Color::BLUE, Vector::new(x2, pos.y))?;

    for (i, (player_id, player)) in players.into_iter().enumerate() {
        let y = pos.y + (i + 1) as f32 * 12.0;
        let color = if player_id == my_player_id {
            Color::ORANGE
        } else {
            Color::BLACK
        };
        font.draw(gfx, &player_id.0.to_string(), color, Vector::new(x0, y))?;
        font.draw(gfx, &player.name, color, Vector::new(x1, y))?;
        font.draw(gfx, &player.food.to_string(), color, Vector::new(x2, y))?;
    }

    Ok(())
}
