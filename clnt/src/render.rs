use std::collections::BTreeMap;

use log::info;

use quicksilver::{
    geom::{Circle, Line, Rectangle, Transform, Vector},
    golem::TextureFilter,
    graphics::{
        //blend::{BlendEquation, BlendFunction, BlendMode, BlendOperation, BlendFactor, BlendChannel, BlendInput},
        Color,
        FontRenderer,
        Graphics,
        Image,
        VectorFont,
    },
    Settings, Window,
};

use comn::{
    game::run::{BULLET_RADIUS, TURRET_RADIUS},
    geom,
    util::join,
};

pub struct Resources {
    pub ttf: VectorFont,
    pub font_small: FontRenderer,
    pub font: FontRenderer,
    pub font_large: FontRenderer,
    pub hirsch: Image,
}

impl Resources {
    pub async fn load(gfx: &mut Graphics) -> quicksilver::Result<Self> {
        let ttf = VectorFont::load("kongtext.ttf").await?;
        let font_small = ttf.to_renderer(gfx, 9.0)?;
        let font = ttf.to_renderer(gfx, 18.0)?;
        let font_large = ttf.to_renderer(gfx, 40.0)?;
        let hirsch = Image::load(gfx, "hirsch.png").await?;
        hirsch.set_magnification(TextureFilter::Nearest)?;
        hirsch.set_minification(TextureFilter::Nearest)?;

        Ok(Self {
            ttf,
            font_small,
            font,
            font_large,
            hirsch,
        })
    }
}

pub fn interp_entities<'a>(
    state: &'a comn::Game,
    next_entities: &'a BTreeMap<comn::EntityId, (comn::GameTime, comn::Entity)>,
    time: comn::GameTime,
) -> impl Iterator<Item = comn::Entity> + 'a {
    join::full_join(state.entities.iter(), next_entities.iter()).filter_map(
        move |item| match item {
            join::Item::Left(_, entity) => Some(entity.clone()),
            join::Item::Right(_, _) => None,
            join::Item::Both(_, entity, (next_time, next_entity)) => {
                let tau =
                    (time - state.current_game_time()) / (next_time - state.current_game_time());
                Some(entity.interp(next_entity, tau))
            }
        },
    )
}

pub fn interp_entity(
    state: &comn::Game,
    next_entities: &BTreeMap<comn::EntityId, (comn::GameTime, comn::Entity)>,
    time: comn::GameTime,
    entity_id: comn::EntityId,
) -> Option<comn::Entity> {
    match (
        state.entities.get(&entity_id),
        next_entities.get(&entity_id),
    ) {
        (Some(entity), None) => Some(entity.clone()),
        (Some(entity), Some((next_time, next_entity))) => {
            let tau = (time - state.current_game_time()) / (next_time - state.current_game_time());
            Some(entity.interp(next_entity, tau))
        }
        (None, _) => None,
    }
}

pub fn render_game(
    gfx: &mut Graphics,
    resources: &mut Resources,
    state: &comn::Game,
    next_entities: &BTreeMap<comn::EntityId, (comn::GameTime, comn::Entity)>,
    time: comn::GameTime,
    my_player_id: comn::PlayerId,
    camera_transform: Transform,
) -> quicksilver::Result<()> {
    {
        gfx.set_transform(camera_transform);
        let map_size: mint::Vector2<f32> = state.settings.size.into();
        let map_rect = Rectangle::new(Vector::new(0.0, 0.0), map_size.into());
        gfx.fill_rect(&map_rect, Color::from_rgba(142, 182, 155, 1.0));
        gfx.stroke_rect(&map_rect, Color::BLACK);
    }

    {
        /*gfx.set_blend_mode(Some(BlendMode {
            equation: BlendEquation::Same(BlendOperation::Add),
            function: BlendFunction::Same {
                source: BlendFactor::Color {
                    input: BlendInput::Source,
                    channel: BlendChannel::Alpha,
                    is_inverse: false,
                },
                destination: BlendFactor::Color {
                    input: BlendInput::Source,
                    channel: BlendChannel::Alpha,
                    is_inverse: true,
                },
            },
            ..BlendMode::default()
        }));*/

        /*for (_, entity) in state.entities.iter() {
            match entity {
                comn::Entity::Turret(turret) => {
                    let origin: mint::Vector2<f32> = turret.pos.coords.into();
                    let circle = Circle::new(origin.into(), TURRET_RANGE);
                    gfx.set_transform(camera_transform);
                    gfx.stroke_circle(&circle, Color::from_rgba(255, 0, 0, 1.0));
                }
                _ => (),
            }
        }*/

        gfx.set_blend_mode(Some(Default::default()));
    }

    for entity in interp_entities(state, next_entities, time) {
        match entity {
            comn::Entity::Player(player) => {
                let pos: mint::Vector2<f32> = player.pos.coords.into();

                let color = if player.owner == my_player_id {
                    Color::BLUE
                } else {
                    Color::from_rgba(148, 0, 211, 1.0)
                };

                let transform = rect_to_transform(&player.rect());
                let rect = Rectangle::new(Vector::new(-0.5, -0.5), Vector::new(1.0, 1.0));

                gfx.set_transform(transform.then(camera_transform));
                gfx.fill_rect(&rect, color);
                gfx.stroke_rect(&rect, Color::BLACK);
                gfx.set_transform(camera_transform);

                if let Some(hook) = player.hook.as_ref() {
                    let (a, b) = match hook.state {
                        comn::HookState::Shooting {
                            start_time,
                            start_pos,
                            vel,
                        } => (player.pos, start_pos + (time - start_time) * vel),
                        comn::HookState::Attached { target, offset } => {
                            let b = interp_entity(state, next_entities, time, target)
                                .map_or(player.pos, |interp_target| {
                                    interp_target.pos(time) + offset
                                });
                            (player.pos, b)
                        }
                    };

                    let a: mint::Vector2<f32> = a.coords.into();
                    let b: mint::Vector2<f32> = b.coords.into();
                    gfx.stroke_path(&[a.into(), b.into()], Color::BLACK);
                    gfx.fill_circle(&Circle::new(b.into(), 5.0), Color::BLACK);
                }

                resources
                    .font
                    .draw(gfx, &player.owner.0.to_string(), Color::WHITE, pos.into())?;
            }
            comn::Entity::DangerGuy(danger_guy) => {
                let origin: mint::Vector2<f32> =
                    (danger_guy.pos(time) - danger_guy.size / 2.0).coords.into();
                let size: mint::Vector2<f32> = danger_guy.size.into();
                let rect = Rectangle::new(origin.into(), size.into());
                gfx.set_transform(camera_transform);

                // Awesome Hirsch, add back in once we have more images!
                /*let frame = pareen::constant(0)
                    .switch(danger_guy.wait_time - 0.6, 1)
                    .switch(danger_guy.wait_time - 0.4, 2)
                    .switch(danger_guy.wait_time - 0.2, 3)
                    .seq(
                        danger_guy.wait_time,
                        pareen::fun(|tau| 3 + (tau * danger_guy.speed / 40.0) as usize % 4),
                    )
                    .repeat(danger_guy.period() / 2.0)
                    .eval(danger_guy.tau(time)) as f32;

                let flip = danger_guy
                    .dir(time)
                    .normalize()
                    .dot(&comn::Vector::new(1.0, 0.0))
                    > 0.7;
                let sub_rect = if flip {
                    Rectangle::new(
                        Vector::new(17.0, 16.0 * frame + 1.0),
                        Vector::new(-16.0, 16.0),
                    )
                } else {
                    Rectangle::new(
                        Vector::new(1.0, 16.0 * frame + 1.0),
                        Vector::new(16.0, 16.0),
                    )
                };

                gfx.draw_subimage(&resources.hirsch, sub_rect, rect);*/

                gfx.fill_rect(&rect, Color::RED);
                gfx.stroke_rect(&rect, Color::BLACK);
            }
            comn::Entity::Bullet(bullet) => {
                let origin: mint::Vector2<f32> = bullet.pos(time).coords.into();
                let circle = Circle::new(origin.into(), BULLET_RADIUS);
                let color = if bullet.owner == Some(my_player_id) {
                    Color::ORANGE
                } else {
                    Color::MAGENTA
                };
                gfx.set_transform(camera_transform);
                gfx.fill_circle(&circle, color);
                gfx.stroke_circle(&circle, Color::BLACK);
            }
            comn::Entity::Turret(turret) => {
                let origin: mint::Vector2<f32> = turret.pos.coords.into();
                let circle = Circle::new(origin.into(), TURRET_RADIUS);
                gfx.set_transform(camera_transform);
                gfx.fill_circle(&circle, Color::from_rgba(128, 128, 128, 1.0));
                gfx.stroke_circle(&circle, Color::BLACK);

                let angle = turret.angle;

                gfx.set_transform(
                    Transform::rotate(angle.to_degrees())
                        .then(Transform::translate(origin.into()))
                        .then(camera_transform),
                );

                let rect = Rectangle::new(Vector::new(0.0, -5.0), Vector::new(40.0, 10.0));

                gfx.fill_rect(&rect, Color::BLACK);
            }
            comn::Entity::Wall(wall) => {
                let transform = rect_to_transform(&wall.rect.to_rect());
                let rect = Rectangle::new(Vector::new(-0.5, -0.5), Vector::new(1.0, 1.0));
                gfx.set_transform(transform.then(camera_transform));
                gfx.fill_rect(&rect, Color::BLACK);
            }
        }
    }

    gfx.set_transform(Transform::IDENTITY);

    Ok(())
}

fn rect_to_transform(rect: &geom::Rect) -> Transform {
    let size: mint::Vector2<f32> = rect.size.into();
    let center: mint::Vector2<f32> = rect.center.coords.into();

    Transform::translate(center.into())
        * Transform::rotate(rect.angle.to_degrees())
        * Transform::scale(size.into())
}
