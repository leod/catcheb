use std::collections::{BTreeMap, BTreeSet};

use rand::Rng;

use crate::entities::{Bullet, Food};
use crate::{
    geom::{self, Ray},
    DeathReason, Entity, EntityId, Event, Game, GameError, GameResult, GameTime, Hook, HookState,
    Input, Matrix, PlayerEntity, PlayerId, Vector,
};

pub const PLAYER_MOVE_SPEED: f32 = 300.0;
pub const PLAYER_SIT_W: f32 = 40.0;
pub const PLAYER_SIT_L: f32 = 40.0;
pub const PLAYER_MOVE_W: f32 = 56.6;
pub const PLAYER_MOVE_L: f32 = 28.2;
pub const PLAYER_SHOOT_PERIOD: GameTime = 0.3;
pub const PLAYER_TRANSITION_SPEED: f32 = 4.0;
pub const PLAYER_ACCEL_FACTOR: f32 = 30.0;
pub const PLAYER_DASH_COOLDOWN: f32 = 2.5;
pub const PLAYER_DASH_DURATION: GameTime = 0.6;
pub const PLAYER_DASH_ACCEL_FACTOR: f32 = 40.0;
pub const PLAYER_DASH_SPEED: f32 = 850.0;
pub const PLAYER_MAX_LOSE_FOOD: u32 = 5;
pub const PLAYER_MIN_LOSE_FOOD: u32 = 1;
pub const PLAYER_TURN_FACTOR: f32 = 0.5;

pub const HOOK_SHOOT_SPEED: f32 = 1200.0;
pub const HOOK_MAX_SHOOT_DURATION: f32 = 0.6;
pub const HOOK_MIN_DISTANCE: f32 = 40.0;
pub const HOOK_PULL_SPEED: f32 = 700.0;
pub const HOOK_MAX_CONTRACT_DURATION: f32 = 0.2;
pub const HOOK_CONTRACT_SPEED: f32 = 2000.0;

pub const BULLET_MOVE_SPEED: f32 = 300.0;
pub const BULLET_RADIUS: f32 = 8.0;
pub const MAGAZINE_SIZE: u32 = 15;
pub const RELOAD_DURATION: GameTime = 2.0;

pub const TURRET_RADIUS: f32 = 30.0;
pub const TURRET_RANGE: f32 = 400.0;
pub const TURRET_SHOOT_PERIOD: GameTime = 1.3;
pub const TURRET_SHOOT_ANGLE: f32 = 0.3;
pub const TURRET_MAX_TURN_SPEED: f32 = 2.0;
pub const TURRET_TURN_FACTOR: f32 = 0.1;
pub const TURRET_SPAWN_OFFSET: f32 = 12.0;

pub const FOOD_SIZE: f32 = 20.0;
pub const FOOD_ROTATION_SPEED: f32 = 3.0;
pub const FOOD_RESPAWN_DURATION: f32 = 5.0;
pub const FOOD_MAX_LIFETIME: f32 = 10.0;
pub const FOOD_MIN_SPEED: f32 = 300.0;
pub const FOOD_MAX_SPEED: f32 = 700.0;
pub const FOOD_SPEED_MIN_FACTOR: f32 = 5.0;
pub const FOOD_SPEED_MAX_FACTOR: f32 = 10.0;

#[derive(Clone, Debug, Default)]
pub struct RunContext {
    pub is_predicting: bool,
    pub events: Vec<Event>,
    pub new_entities: Vec<Entity>,
    pub removed_entities: BTreeSet<EntityId>,
    pub killed_players: BTreeMap<PlayerId, DeathReason>,
}

impl Game {
    pub fn run_tick(&mut self, context: &mut RunContext) -> GameResult<()> {
        let time = self.game_time();

        // TODO: clone
        let entities = self.entities.clone();

        for (entity_id, entity) in self.entities.iter_mut() {
            match entity {
                Entity::Bullet(bullet) => {
                    if !self.settings.aa_rect().contains_point(bullet.pos(time)) {
                        context.removed_entities.insert(*entity_id);
                        continue;
                    }

                    for (entity_id_b, entity_b) in entities.iter() {
                        if *entity_id == *entity_id_b {
                            continue;
                        }

                        match entity_b {
                            Entity::DangerGuy(danger_guy) => {
                                if danger_guy.aa_rect(time).contains_point(bullet.pos(time)) {
                                    context.removed_entities.insert(*entity_id);
                                }
                            }
                            Entity::Turret(turret) if bullet.owner.is_some() => {
                                if (bullet.pos(time) - turret.pos).norm()
                                    < TURRET_RADIUS + BULLET_RADIUS
                                {
                                    context.removed_entities.insert(*entity_id);
                                }
                            }
                            Entity::Wall(wall) => {
                                if wall.rect.contains_point(bullet.pos(time)) {
                                    context.removed_entities.insert(*entity_id);
                                    continue;
                                }
                            }
                            _ => (),
                        }
                    }
                }
                Entity::Turret(turret) => {
                    turret.target = entities
                        .iter()
                        .filter(|(other_id, _)| **other_id != *entity_id)
                        .filter_map(|(other_id, other_entity)| {
                            other_entity
                                .player()
                                .ok()
                                .map(|player| (other_id, (turret.pos - player.pos).norm()))
                        })
                        .filter(|(_, dist)| *dist <= TURRET_RANGE)
                        .min_by(|(_, dist1), (_, dist2)| dist1.partial_cmp(dist2).unwrap())
                        .map(|(other_id, _)| *other_id);

                    if let Some(target) = turret.target {
                        let target_pos = entities[&target].pos(time);
                        let target_angle = turret.angle_to_pos(target_pos);
                        let angle_dist = ((target_angle - turret.angle).sin())
                            .atan2((target_angle - turret.angle).cos());
                        turret.angle += angle_dist * TURRET_TURN_FACTOR;
                        //.min(TURRET_MAX_TURN_SPEED * tick_period)
                        //.max(TURRET_MAX_TURN_SPEED * tick_period);

                        if time >= turret.next_shot_time && angle_dist.abs() < TURRET_SHOOT_ANGLE {
                            turret.next_shot_time = time + TURRET_SHOOT_PERIOD;

                            let delta = Vector::new(turret.angle.cos(), turret.angle.sin());

                            context.new_entities.push(Entity::Bullet(Bullet {
                                owner: None,
                                start_time: time,
                                start_pos: turret.pos + TURRET_SPAWN_OFFSET * delta,
                                vel: delta * BULLET_MOVE_SPEED,
                            }));
                        }
                    }
                }
                Entity::FoodSpawn(spawn) if !spawn.has_food => {
                    if let Some(respawn_time) = spawn.respawn_time {
                        if time >= respawn_time {
                            spawn.has_food = true;
                            spawn.respawn_time = None;
                        }
                    }
                }
                Entity::Food(food) => {
                    if time - food.start_time > FOOD_MAX_LIFETIME {
                        context.removed_entities.insert(*entity_id);
                    }
                }
                _ => (),
            }
        }

        Ok(())
    }

    pub fn run_player_input(
        &mut self,
        player_id: PlayerId,
        input: &Input,
        input_state: Option<&Game>,
        context: &mut RunContext,
    ) -> GameResult<()> {
        if let Some((entity_id, ent)) = self.get_player_entity(player_id) {
            let mut ent = ent.clone();

            self.run_player_entity_input(
                player_id,
                input,
                input_state,
                context,
                entity_id,
                &mut ent,
            )?;

            self.entities.insert(entity_id, Entity::Player(ent));
        }

        Ok(())
    }

    fn run_player_entity_input(
        &mut self,
        player_id: PlayerId,
        input: &Input,
        input_state: Option<&Game>,
        context: &mut RunContext,
        entity_id: EntityId,
        ent: &mut PlayerEntity,
    ) -> GameResult<()> {
        let dt = self.settings.tick_period();
        let input_state = input_state.unwrap_or(self);
        let input_time = input_state.game_time();

        // Movement
        let cur_dash = ent.last_dash.filter(|(dash_time, _)| {
            input_time >= *dash_time && input_time <= dash_time + PLAYER_DASH_DURATION
        });

        let mut delta = Vector::new(0.0, 0.0);

        if let Some((_dash_time, dash_dir)) = cur_dash {
            // Movement is constricted while dashing.
            let target_vel = dash_dir * PLAYER_DASH_SPEED;
            ent.vel =
                geom::smooth_to_target_vector(PLAYER_DASH_ACCEL_FACTOR, ent.vel, target_vel, dt);

            ent.target_angle = dash_dir.y.atan2(dash_dir.x);
        } else {
            // Normal movement when not dashing.
            if input.move_left {
                delta.x -= 1.0;
            }
            if input.move_right {
                delta.x += 1.0;
            }
            if input.move_up {
                delta.y -= 1.0;
            }
            if input.move_down {
                delta.y += 1.0;
            }

            if delta.norm() > 0.0 {
                ent.target_angle = delta.y.atan2(delta.x);
            }

            delta = if delta.norm() > 0.0 {
                delta.normalize()
            } else {
                delta
            };

            let target_vel = delta * PLAYER_MOVE_SPEED;
            ent.vel = geom::smooth_to_target_vector(PLAYER_ACCEL_FACTOR, ent.vel, target_vel, dt);
            if (ent.vel - target_vel).norm() < 0.01 {
                ent.vel = target_vel;
            }
        }

        {
            let angle_dist =
                ((ent.target_angle - ent.angle).sin()).atan2((ent.target_angle - ent.angle).cos());
            ent.angle += angle_dist * PLAYER_TURN_FACTOR;
            ent.target_size_scale = 1.0
                + (0.4 * (-angle_dist.abs() * 1.0).exp() * ent.vel.norm() / PLAYER_MOVE_SPEED)
                    .min(0.6);
            ent.size_scale =
                geom::smooth_to_target_f32(30.0, ent.size_scale, ent.target_size_scale, dt);
        }

        // Experimental hook stuff
        if let Some(hook) = ent.hook.clone() {
            match hook.state {
                HookState::Shooting {
                    start_time,
                    start_pos,
                    vel,
                } => {
                    let pos = start_pos + (input_time - start_time) * vel;
                    let next_pos =
                        start_pos + (input_time - start_time + self.settings.tick_period()) * vel;
                    let pos_delta = next_pos - pos;
                    let pos_delta_norm = pos_delta.norm();
                    let ray = Ray {
                        origin: pos,
                        dir: pos_delta / pos_delta_norm,
                    };

                    if !input.use_action || input_time - start_time > HOOK_MAX_SHOOT_DURATION {
                        let duration = (pos - ent.pos).norm() / HOOK_CONTRACT_SPEED;
                        ent.hook = Some(Hook {
                            state: HookState::Contracting {
                                start_time: input_time,
                                start_pos: pos,
                                duration: duration.min(HOOK_MAX_CONTRACT_DURATION),
                            },
                        });
                    } else {
                        for (target_id, target) in input_state.entities.iter() {
                            if entity_id != *target_id && target.can_hook_attach() {
                                if let Some(intersection_t) = ray
                                    .intersects(&target.intersection_shape(input_time))
                                    .filter(|t| *t <= pos_delta_norm)
                                {
                                    let intersection_p = ray.origin + intersection_t * ray.dir;
                                    ent.hook = Some(Hook {
                                        state: HookState::Attached {
                                            start_time: input_time
                                                + intersection_t / pos_delta_norm
                                                    * self.settings.tick_period(),
                                            target: *target_id,
                                            offset: intersection_p - target.pos(input_time),
                                        },
                                    });
                                    break;
                                }
                            }
                        }
                    }
                }
                HookState::Attached {
                    start_time: _,
                    target,
                    offset,
                } => {
                    if let Some(target_entity) = input_state.entities.get(&target) {
                        let hook_pos = target_entity.pos(input_time) + offset;

                        if !input.use_action || (hook_pos - ent.pos).norm() < HOOK_MIN_DISTANCE {
                            let duration = (hook_pos - ent.pos).norm() / HOOK_CONTRACT_SPEED;
                            ent.hook = Some(Hook {
                                state: HookState::Contracting {
                                    start_time: input_time,
                                    start_pos: hook_pos,
                                    duration: duration.min(HOOK_MAX_CONTRACT_DURATION),
                                },
                            });
                        } else {
                            ent.vel += (hook_pos - ent.pos).normalize() * HOOK_PULL_SPEED;
                        }
                    } else {
                        ent.hook = None;
                    }
                }
                HookState::Contracting {
                    start_time,
                    duration,
                    ..
                } => {
                    if input_time - start_time >= duration {
                        ent.hook = None;
                    }
                }
            }
        } else if input.use_action && ent.hook.is_none() {
            // TODO: Trace ray when spawning hook?
            ent.hook = Some(Hook {
                state: HookState::Shooting {
                    start_time: input_time,
                    start_pos: ent.pos,
                    vel: Vector::new(ent.angle.cos(), ent.angle.sin()) * HOOK_SHOOT_SPEED,
                },
            });
        }

        // Check for collisions
        let mut offset = ent.vel * dt;
        let mut flip_axis = None;

        for (_, entity) in input_state.entities.iter() {
            let (other_shape, flip) = match entity {
                Entity::Player(other_ent) if other_ent.owner != player_id => {
                    (Some(other_ent.rect()), false)
                }
                Entity::Wall(other_ent) => (Some(other_ent.rect.to_rect()), true),
                Entity::DangerGuy(other_ent) if !other_ent.is_hot => {
                    //Some(other_ent.aa_rect(input_time + self.settings.tick_period()).to_rect())
                    (Some(other_ent.aa_rect(self.game_time()).to_rect()), true)
                }
                _ => (None, false),
            };

            if let Some(collision) = other_shape
                .and_then(|other_shape| geom::rect_collision(&ent.rect(), &other_shape, offset))
            {
                offset += collision.resolution_vector;
                if flip {
                    flip_axis = Some(collision.axis);
                }
            }
        }

        // Allow reflecting off walls when dashing
        if let (Some((dash_time, dash_dir)), Some(flip_axis)) = (cur_dash, flip_axis) {
            let reflected_dash_dir = dash_dir - 2.0 * dash_dir.dot(&flip_axis) * flip_axis;
            ent.last_dash = Some((dash_time, reflected_dash_dir));
            ent.vel = ent.vel - 2.0 * ent.vel.dot(&flip_axis) * flip_axis;
            offset += flip_axis * 0.1;
        }

        ent.pos += offset;

        /*if delta.norm() > 0.0 {
            ent.angle = Some(delta.y.atan2(delta.x));
        } else {
            ent.angle = None;
        }*/

        // Clip to map boundary
        ent.pos.x = ent
            .pos
            .x
            .min(self.settings.size.x - PLAYER_SIT_W / 2.0)
            .max(PLAYER_SIT_W / 2.0);
        ent.pos.y = ent
            .pos
            .y
            .min(self.settings.size.y - PLAYER_SIT_W / 2.0)
            .max(PLAYER_SIT_W / 2.0);

        // Start dashing
        if input.use_item
            && ent.last_dash.map_or(true, |(dash_time, _)| {
                dash_time + PLAYER_DASH_COOLDOWN <= input_time
            })
            && delta.norm() > 0.1
        {
            ent.last_dash = Some((input_time, delta));
        }

        // Shooting
        /*if input_time >= ent.next_shot_time {
            if ent.shots_left == 0 {
                ent.shots_left = MAGAZINE_SIZE;
            }

            if delta.norm() > 0.0 && input.use_item {
                context.new_entities.push(Entity::Bullet(Bullet {
                    owner: Some(player_id),
                    start_time: input_time,
                    start_pos: ent.pos,
                    vel: delta.normalize() * BULLET_MOVE_SPEED,
                }));

                ent.shots_left -= 1;

                if ent.shots_left == 0 {
                    ent.next_shot_time = input_time + RELOAD_DURATION;
                } else {
                    ent.next_shot_time = input_time + PLAYER_SHOOT_PERIOD;
                }
            }
        }*/

        // Check for death
        let mut killed = None;

        for (entity_id, entity) in input_state.entities.iter() {
            match entity {
                Entity::DangerGuy(danger_guy) if danger_guy.is_hot => {
                    if geom::rect_collision(
                        &danger_guy.aa_rect(input_time).to_rect(),
                        &ent.rect(),
                        Vector::zeros(),
                    )
                    .is_some()
                    {
                        killed = Some(DeathReason::TouchedTheDanger);
                    }
                }
                Entity::Bullet(bullet) if bullet.owner != Some(player_id) => {
                    if ent.rect().contains_point(bullet.pos(input_time)) {
                        context.removed_entities.insert(*entity_id);
                        killed = Some(DeathReason::ShotBy(bullet.owner));
                    }
                }
                _ => (),
            }
        }

        if let Some(killed) = killed {
            context.killed_players.insert(player_id, killed);

            if !context.is_predicting {
                let player = self.players.get_mut(&ent.owner).unwrap();
                let spawn_food = player
                    .food
                    .min(PLAYER_MAX_LOSE_FOOD)
                    .max(PLAYER_MIN_LOSE_FOOD);
                player.food -= spawn_food.min(player.food);

                for _ in 0..spawn_food {
                    // TODO: Random
                    let angle = rand::thread_rng().gen::<f32>() * std::f32::consts::PI * 2.0;
                    let speed = rand::thread_rng().gen_range(FOOD_MIN_SPEED, FOOD_MAX_SPEED);
                    let start_vel = Vector::new(speed * angle.cos(), speed * angle.sin());
                    let factor =
                        rand::thread_rng().gen_range(FOOD_SPEED_MIN_FACTOR, FOOD_SPEED_MAX_FACTOR);

                    let food = Food {
                        start_time: self.game_time(),
                        start_pos: ent.pos,
                        start_vel,
                        factor,
                        amount: 1,
                    };
                    context.new_entities.push(Entity::Food(food));
                }
            }
        }

        // Take food
        if !context.is_predicting {
            let time = self.game_time();
            for (entity_id, entity) in self.entities.iter_mut() {
                match entity {
                    Entity::FoodSpawn(spawn) if spawn.has_food => {
                        if geom::rect_collision(
                            &spawn.rect(input_time),
                            &ent.rect(),
                            Vector::zeros(),
                        )
                        .is_some()
                        {
                            spawn.has_food = false;
                            spawn.respawn_time = Some(time + FOOD_RESPAWN_DURATION);
                            self.players.get_mut(&ent.owner).unwrap().food += 1;
                        }
                    }
                    Entity::Food(food) => {
                        if geom::rect_collision(
                            &food.rect(input_time),
                            &ent.rect(),
                            Vector::zeros(),
                        )
                        .is_some()
                        {
                            self.players.get_mut(&ent.owner).unwrap().food += 1;
                            context.removed_entities.insert(*entity_id);
                        }
                    }
                    _ => (),
                }
            }
        }

        Ok(())
    }

    pub fn get_entity(&mut self, entity_id: EntityId) -> GameResult<&Entity> {
        self.entities
            .get(&entity_id)
            .ok_or_else(|| GameError::InvalidEntityId(entity_id))
    }

    pub fn get_player_entity(&self, player_id: PlayerId) -> Option<(EntityId, &PlayerEntity)> {
        self.entities
            .iter()
            .filter_map(|(&id, e)| {
                if let Entity::Player(ref e) = e {
                    if e.owner == player_id {
                        Some((id, e))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .next()
    }
}
