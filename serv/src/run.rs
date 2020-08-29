use rand::{seq::IteratorRandom, Rng};

use comn::{
    entities::{Bullet, Food},
    game::run::{
        RunContext, BULLET_MOVE_SPEED, BULLET_RADIUS, FOOD_MAX_LIFETIME, ROCKET_RADIUS,
        TURRET_RANGE,
    },
    geom::{self, Ray},
    DeathReason, Entity, EntityId, Event, Game, GameResult, GameTime, PlayerEntity, PlayerState,
    Turret, Vector,
};

const PLAYER_MAX_LOSE_FOOD: u32 = 5;
const PLAYER_MIN_LOSE_FOOD: u32 = 1;
const FOOD_SPEED_MIN_FACTOR: f32 = 5.0;
const FOOD_SPEED_MAX_FACTOR: f32 = 10.0;

const FOOD_MIN_SPEED: f32 = 300.0;
const FOOD_MAX_SPEED: f32 = 700.0;

const TURRET_TURN_FACTOR: f32 = 0.1;
const TURRET_SHOOT_ANGLE: f32 = 0.3;
const TURRET_SPAWN_OFFSET: f32 = 12.0;
const TURRET_SHOOT_PERIOD: GameTime = 2.5;

pub fn run_tick(state: &mut Game, context: &mut RunContext) -> GameResult<()> {
    assert!(!context.is_predicting);

    if let Some(catcher) = state.catcher {
        let catcher_alive = state
            .players
            .get(&catcher)
            .map_or(false, |player| player.state == PlayerState::Alive);
        if !catcher_alive {
            state.catcher = None;
        }
    }

    if state.catcher.is_none() {
        // TODO: Random
        let mut rng = rand::thread_rng();
        state.catcher = state
            .players
            .iter()
            .filter(|(_, player)| !player.name.contains("bot")) // TODO: remove bot discrimination
            .filter(|(_, player)| player.state == PlayerState::Alive)
            .map(|(player_id, _)| *player_id)
            .choose(&mut rng);
        if let Some(catcher) = state.catcher {
            context
                .events
                .push(Event::NewCatcher { player_id: catcher });
        }
    }

    let mut updates = Vec::new();

    for (entity_id, entity) in state.entities.iter() {
        let mut entity = entity.clone();
        let update = update_entity(state, *entity_id, &mut entity, context);

        if update {
            updates.push((*entity_id, entity));
        }
    }

    state.entities.extend(updates);

    Ok(())
}

fn update_entity(
    state: &Game,
    entity_id: EntityId,
    entity: &mut Entity,
    context: &mut RunContext,
) -> bool {
    let dt = state.settings.tick_period();

    match entity {
        Entity::Bullet(bullet) => {
            if state.any_solid_neutral_contains_circle(
                entity_id,
                bullet.owner,
                bullet.pos(state.game_time()),
                BULLET_RADIUS,
            ) {
                context.removed_entities.insert(entity_id);
            }
            false
        }
        Entity::Rocket(rocket) => {
            if state.any_solid_neutral_contains_circle(
                entity_id,
                rocket.owner,
                rocket.pos(state.game_time()),
                ROCKET_RADIUS,
            ) {
                //context.removed_entities.insert(entity_id);
            }
            false
        }
        Entity::Turret(turret) => {
            update_turret(state, entity_id, turret, context);
            true
        }
        Entity::FoodSpawn(spawn) if !spawn.has_food => {
            if let Some(respawn_time) = spawn.respawn_time {
                if state.game_time() >= respawn_time {
                    spawn.has_food = true;
                    spawn.respawn_time = None;
                    return true;
                }
            }
            false
        }
        Entity::Food(food) => {
            if state.game_time() - food.start_time > FOOD_MAX_LIFETIME {
                context.removed_entities.insert(entity_id);
            } else {
                for entity_b in state.entities.values() {
                    if entity_b.is_wall_like()
                        && entity_b
                            .shape(state.game_time())
                            .contains_point(food.pos(state.game_time()))
                    {
                        // Replace the Food by a non-moving one
                        context.removed_entities.insert(entity_id);
                        context.new_entities.push(Entity::Food(Food {
                            start_pos: food.pos(state.game_time() - dt / 2.0),
                            start_vel: Vector::zeros(),
                            ..food.clone()
                        }));
                        break;
                    }
                }
            }
            false
        }
        _ => false,
    }
}

fn update_turret(state: &Game, entity_id: EntityId, turret: &mut Turret, context: &mut RunContext) {
    turret.target = state
        .entities
        .iter()
        .filter(|(other_id, _)| **other_id != entity_id)
        .filter_map(|(other_id, other_entity)| {
            other_entity.player().ok().map(|player| {
                (
                    other_id,
                    other_entity,
                    (turret.pos - player.pos).norm_squared(),
                )
            })
        })
        .filter(|(other_id, other_entity, dist)| {
            let ray = Ray {
                origin: turret.pos,
                dir: other_entity.pos(state.game_time()) - turret.pos,
            };

            *dist <= TURRET_RANGE * TURRET_RANGE
                && Game::trace_ray(
                    &ray,
                    state.game_time(),
                    state.entities.iter().filter(|(between_id, _)| {
                        **between_id != entity_id && **between_id != **other_id
                    }),
                )
                .map_or(true, |(t, _, _)| t > 1.0)
        })
        .min_by(|(_, _, dist1), (_, _, dist2)| dist1.partial_cmp(dist2).unwrap())
        .map(|(other_id, _, _)| *other_id);

    if let Some(target) = turret.target {
        let target_pos = state.entities[&target].pos(state.game_time());
        let target_angle = turret.angle_to_pos(target_pos);
        let angle_dist = geom::angle_dist(target_angle, turret.angle);
        turret.angle += angle_dist * TURRET_TURN_FACTOR;

        if state.game_time() >= turret.next_shot_time && angle_dist.abs() < TURRET_SHOOT_ANGLE {
            turret.next_shot_time = state.game_time() + TURRET_SHOOT_PERIOD;

            let delta = Vector::new(turret.angle.cos(), turret.angle.sin());

            context.new_entities.push(Entity::Bullet(Bullet {
                owner: None,
                start_time: state.game_time(),
                start_pos: turret.pos + TURRET_SPAWN_OFFSET * delta,
                vel: delta * BULLET_MOVE_SPEED,
            }));
        }
    }
}

pub fn on_kill_player(
    state: &mut Game,
    ent: &PlayerEntity,
    _reason: DeathReason,
    context: &mut RunContext,
) -> GameResult<()> {
    let player = state.players.get_mut(&ent.owner).unwrap();
    let spawn_food = player
        .food
        .min(PLAYER_MAX_LOSE_FOOD)
        .max(PLAYER_MIN_LOSE_FOOD);
    player.food -= spawn_food.min(player.food);

    for _ in 0..spawn_food {
        let angle = rand::thread_rng().gen::<f32>() * std::f32::consts::PI * 2.0;
        let speed = rand::thread_rng().gen_range(FOOD_MIN_SPEED, FOOD_MAX_SPEED);
        let start_vel = Vector::new(speed * angle.cos(), speed * angle.sin());
        let factor = rand::thread_rng().gen_range(FOOD_SPEED_MIN_FACTOR, FOOD_SPEED_MAX_FACTOR);

        let food = Food {
            start_time: state.game_time(),
            start_pos: ent.pos,
            start_vel,
            factor,
            amount: 1,
        };
        context.new_entities.push(Entity::Food(food));
    }

    if state.catcher == Some(ent.owner) {
        // Choose a new catcher
        state.catcher = state
            .entities
            .iter()
            .filter_map(|(_, other_entity)| {
                other_entity
                    .player()
                    .ok()
                    .map(|other_player| (other_player.owner, (ent.pos - other_player.pos).norm()))
            })
            .filter(|(other_owner, _)| *other_owner != ent.owner)
            .min_by(|(_, dist1), (_, dist2)| dist1.partial_cmp(dist2).unwrap())
            .map(|(other_owner, _)| other_owner);

        if let Some(catcher) = state.catcher {
            context
                .events
                .push(Event::NewCatcher { player_id: catcher });
        }
    }

    Ok(())
}
