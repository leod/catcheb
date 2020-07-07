use std::collections::{BTreeMap, BTreeSet};

use rand::{seq::IteratorRandom, Rng};

use crate::entities::{Bullet, Dash, Food};
use crate::{
    geom::{self, Ray},
    DeathReason, Entity, EntityId, Event, Game, GameError, GameResult, GameTime, Hook, Input,
    PlayerEntity, PlayerId, PlayerMap, PlayerState, Vector,
};

pub const PLAYER_MOVE_SPEED: f32 = 300.0;
pub const PLAYER_SIT_W: f32 = 40.0;
pub const PLAYER_SIT_L: f32 = 40.0;
pub const PLAYER_MOVE_W: f32 = 56.6;
pub const PLAYER_MOVE_L: f32 = 28.2;
pub const PLAYER_SHOOT_PERIOD: GameTime = 0.3;
pub const PLAYER_ACCEL_FACTOR: f32 = 30.0;
pub const PLAYER_DASH_COOLDOWN: f32 = 2.5;
pub const PLAYER_DASH_DURATION: GameTime = 0.6;
pub const PLAYER_DASH_ACCEL_FACTOR: f32 = 40.0;
pub const PLAYER_DASH_SPEED: f32 = 850.0;
pub const PLAYER_MAX_LOSE_FOOD: u32 = 5;
pub const PLAYER_MIN_LOSE_FOOD: u32 = 1;
pub const PLAYER_TURN_FACTOR: f32 = 0.35;
pub const PLAYER_DASH_TURN_FACTOR: f32 = 0.8;
pub const PLAYER_SIZE_SKEW_FACTOR: f32 = 20.0;
pub const PLAYER_SIZE_SKEW: f32 = 0.5;
pub const PLAYER_TURN_DURATION: GameTime = 0.5;
pub const PLAYER_CATCHER_SIZE_SCALE: f32 = 1.5;
pub const PLAYER_SIZE_SCALE_FACTOR: f32 = 10.0;
pub const PLAYER_CATCH_FOOD: u32 = 10;
pub const PLAYER_TAKE_FOOD_SIZE_BUMP: f32 = 25.0;
pub const PLAYER_SIZE_BUMP_FACTOR: f32 = 20.0;
pub const PLAYER_TARGET_SIZE_BUMP_FACTOR: f32 = 30.0;
pub const PLAYER_MAX_SIZE_BUMP: f32 = 50.0;

pub const HOOK_SHOOT_SPEED: f32 = 1200.0;
pub const HOOK_MAX_SHOOT_DURATION: f32 = 0.6;
pub const HOOK_MIN_DISTANCE: f32 = 40.0;
pub const HOOK_PULL_SPEED: f32 = 700.0;
pub const HOOK_MAX_CONTRACT_DURATION: f32 = 0.2;
pub const HOOK_CONTRACT_SPEED: f32 = 2000.0;
pub const HOOK_COOLDOWN: f32 = 0.5;

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
        assert!(!context.is_predicting);

        let time = self.game_time();
        let dt = self.settings.tick_period();

        if let Some(catcher) = self.catcher {
            let catcher_alive = self
                .players
                .get(&catcher)
                .map_or(false, |player| player.state == PlayerState::Alive);
            if !catcher_alive {
                self.catcher = None;
            }
        }

        if self.catcher.is_none() {
            // TODO: Random
            let mut rng = rand::thread_rng();
            self.catcher = self
                .players
                .iter()
                .filter(|(_, player)| player.state == PlayerState::Alive)
                .map(|(player_id, _)| *player_id)
                .choose(&mut rng);
            if let Some(catcher) = self.catcher {
                context
                    .events
                    .push(Event::NewCatcher { player_id: catcher });
            }
        }

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
                        let angle_dist = geom::angle_dist(target_angle, turret.angle);
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
                    } else {
                        for entity_b in entities.values() {
                            if entity_b.is_wall_like()
                                && entity_b.shape(time).contains_point(food.pos(time))
                            {
                                // Replace the Food by a non-moving one
                                context.removed_entities.insert(*entity_id);
                                context.new_entities.push(Entity::Food(Food {
                                    start_pos: food.pos(time - dt / 2.0),
                                    start_vel: Vector::zeros(),
                                    ..food.clone()
                                }));
                                break;
                            }
                        }
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
            coarse_prof::profile!("run_player_input");

            let mut ent = ent.clone();

            self.run_player_entity_input(input, input_state, context, entity_id, &mut ent)?;

            self.entities.insert(entity_id, Entity::Player(ent));
        }

        Ok(())
    }

    fn run_player_entity_input(
        &mut self,
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
        let prev_target_angle = ent.target_angle;
        let mut any_move_key = false;

        if let Some(dash) = ent.dash.as_ref() {
            // Movement is constricted while dashing.
            ent.target_angle = dash.dir.y.atan2(dash.dir.x);
        } else {
            // Normal movement when not dashing.
            let mut delta = Vector::new(0.0, 0.0);
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
                any_move_key = true;
            }
        }

        // Smooth turning and scaling
        ent.turn_time_left = (ent.turn_time_left - dt).max(0.0);

        if (ent.target_angle - prev_target_angle).abs() >= 0.001 {
            let angle_dist = geom::angle_dist(ent.target_angle, prev_target_angle);
            if (angle_dist.abs() - std::f32::consts::PI).abs() < 0.01 {
                ent.angle += ent.target_angle - prev_target_angle;
            } else {
                ent.turn_time_left = PLAYER_TURN_DURATION;
            }
        }
        {
            let angle_dist = geom::angle_dist(ent.target_angle, ent.angle);
            let time_since_turn =
                (PLAYER_TURN_DURATION - ent.turn_time_left).min(PLAYER_TURN_DURATION);
            let factor = if ent.dash.is_some() {
                PLAYER_DASH_TURN_FACTOR
            } else {
                PLAYER_TURN_FACTOR
            };
            ent.angle += angle_dist * factor;

            let turn_scale = if let Some(dash) = ent.dash.as_ref() {
                let dash_delta = PLAYER_DASH_DURATION - dash.time_left;
                (dash_delta * std::f32::consts::PI / PLAYER_TURN_DURATION)
                    .cos()
                    .powf(2.0)
            } else {
                (time_since_turn * std::f32::consts::PI / PLAYER_TURN_DURATION)
                    .cos()
                    .powf(2.0)
                    * 0.8
                    + 0.2
            };
            let move_scale = if let Some(Hook::Attached { .. }) = ent.hook.as_ref() {
                0.5
            } else {
                ent.vel.norm() / PLAYER_MOVE_SPEED
            };
            let target_size_skew = PLAYER_SIZE_SKEW * move_scale * turn_scale;

            ent.size_skew = geom::smooth_to_target_f32(
                PLAYER_SIZE_SKEW_FACTOR,
                ent.size_skew,
                target_size_skew,
                dt,
            );
        }
        {
            let is_catcher = self.catcher == Some(ent.owner);
            let target_size_scale = if is_catcher {
                PLAYER_CATCHER_SIZE_SCALE
            } else {
                1.0
            };
            ent.size_bump = geom::smooth_to_target_f32(
                PLAYER_SIZE_BUMP_FACTOR,
                ent.size_bump,
                ent.target_size_bump,
                dt,
            );
            ent.target_size_bump = geom::smooth_to_target_f32(
                PLAYER_TARGET_SIZE_BUMP_FACTOR,
                ent.target_size_bump,
                0.0,
                dt,
            );
            ent.size_scale = geom::smooth_to_target_f32(
                PLAYER_SIZE_SCALE_FACTOR,
                ent.size_scale,
                target_size_scale,
                dt,
            );
        }

        // Acceleration
        {
            let target_vel = if let Some(dash) = ent.dash.as_ref() {
                dash.dir * PLAYER_DASH_SPEED
            } else {
                Vector::new(ent.angle.cos(), ent.angle.sin())
                    * PLAYER_MOVE_SPEED
                    * (any_move_key as usize as f32)
            };
            let factor = if ent.dash.is_some() {
                PLAYER_DASH_ACCEL_FACTOR
            } else {
                PLAYER_ACCEL_FACTOR
            };
            ent.vel = geom::smooth_to_target_vector(factor, ent.vel, target_vel, dt);
            ent.vel = geom::smooth_to_target_vector(PLAYER_ACCEL_FACTOR, ent.vel, target_vel, dt);
            if (ent.vel - target_vel).norm() < 0.01 {
                ent.vel = target_vel;
            }
        }

        // Experimental hook stuff
        ent.hook_cooldown = (ent.hook_cooldown - dt).max(0.0);
        ent.hook = if let Some(hook) = ent.hook.clone() {
            match hook {
                Hook::Shooting {
                    pos,
                    vel,
                    time_left,
                } => {
                    let next_time_left = (time_left - dt).max(0.0);

                    if !input.use_action || next_time_left <= 0.0 {
                        Some(Hook::Contracting { pos })
                    } else {
                        let pos_delta = dt * vel;
                        let ray = Ray {
                            origin: ent.pos,
                            dir: pos + pos_delta - ent.pos,
                        };

                        let hook = input_state
                            .entities
                            .iter()
                            .filter(|(other_id, other_ent)| {
                                **other_id != entity_id && other_ent.can_hook_attach()
                            })
                            .filter_map(|(other_id, other_ent)| {
                                ray.intersections(&other_ent.shape(input_time))
                                    .first()
                                    .filter(|t| *t <= 1.0)
                                    .map(|t| (other_id, other_ent, t))
                            })
                            .min_by(|(_, _, t1), (_, _, t2)| t1.partial_cmp(t2).unwrap())
                            .map_or(
                                Hook::Shooting {
                                    pos: pos + pos_delta,
                                    vel,
                                    time_left: next_time_left,
                                },
                                |(other_id, other_ent, t)| Hook::Attached {
                                    target: *other_id,
                                    offset: ray.origin + t * ray.dir - other_ent.pos(input_time),
                                },
                            );

                        Some(hook)
                    }
                }
                Hook::Attached { target, offset } => {
                    input_state.entities.get(&target).and_then(|target_ent| {
                        let hook_pos = target_ent.pos(input_time) + offset;

                        if !input.use_action || (hook_pos - ent.pos).norm() < HOOK_MIN_DISTANCE {
                            Some(Hook::Contracting { pos: hook_pos })
                        } else {
                            ent.vel += (hook_pos - ent.pos).normalize() * HOOK_PULL_SPEED;

                            Some(Hook::Attached { target, offset })
                        }
                    })
                }
                Hook::Contracting { pos } => {
                    let new_pos = geom::smooth_to_target_point(5.0, ent.pos, pos, dt);

                    if (new_pos - ent.pos).norm() < 5.0 {
                        ent.hook_cooldown = HOOK_COOLDOWN;

                        None
                    } else {
                        Some(Hook::Contracting { pos: new_pos })
                    }
                }
            }
        } else if input.use_action && ent.hook.is_none() && ent.hook_cooldown == 0.0 {
            let vel = Vector::new(ent.angle.cos(), ent.angle.sin()) * HOOK_SHOOT_SPEED;
            Some(Hook::Shooting {
                pos: ent.pos + vel * 0.05,
                vel,
                time_left: HOOK_MAX_SHOOT_DURATION,
            })
        } else {
            None
        };

        // Check for collisions
        let mut offset = ent.vel * dt;
        let mut flip_axis = None;

        let mut caught_players = BTreeSet::new();

        // TODO: Should probably use auth state for player-player collisions?
        for (other_entity_id, other_entity) in input_state.entities.iter() {
            let (other_shape, flip) = match other_entity {
                Entity::Player(other_ent) if other_ent.owner != ent.owner => {
                    (Some(other_ent.shape()), false)
                }
                Entity::PlayerView(other_ent) if other_ent.owner != ent.owner => {
                    (Some(other_ent.shape()), false)
                }
                Entity::Wall(other_ent) => (Some(other_ent.shape()), true),
                Entity::DangerGuy(other_ent) if !other_ent.is_hot => {
                    //Some(other_ent.aa_rect(input_time + self.settings.tick_period()).to_rect())
                    (Some(other_ent.shape(self.game_time())), true)
                }
                Entity::Turret(other_ent) => (Some(other_ent.shape()), true),
                _ => (None, false),
            };

            let collision =
                other_shape.and_then(|other_shape| ent.rect().collision(&other_shape, offset));

            if let Some(collision) = collision {
                let mut collide = true;

                if let Entity::Player(_) | Entity::PlayerView(_) = other_entity {
                    // TODO: Decide whom to favor regarding catching... or if
                    // we should even make it happen over a longer duration.
                    if self.catcher == Some(ent.owner) {
                        if ent.dash.is_some() {
                            caught_players.insert(*other_entity_id);
                        }

                        // To prevent prediction errors, we disable collision
                        // even some time _after_ dashing as the catcher.
                        // (The prediction error happens because we cannot
                        // predict locally that we caught the other player, so
                        // we collide if the dash stops while we are still on
                        // top.)
                        if PLAYER_DASH_COOLDOWN - ent.dash_cooldown < 1.5 * PLAYER_DASH_DURATION {
                            collide = false;
                        }
                    }
                }

                if collide {
                    offset += collision.resolution_vector;
                    if flip {
                        flip_axis = Some(collision.resolution_vector.normalize());
                    }
                }
            }
        }

        // Allow reflecting off walls when dashing
        if let (Some(dash), Some(flip_axis)) = (ent.dash.as_mut(), flip_axis) {
            let reflected_dash_dir = dash.dir - 2.0 * dash.dir.dot(&flip_axis) * flip_axis;
            dash.dir = reflected_dash_dir;
            ent.vel = ent.vel - 2.0 * ent.vel.dot(&flip_axis) * flip_axis;
            ent.turn_time_left = PLAYER_TURN_DURATION;
            ent.angle = ent.vel.y.atan2(ent.vel.x);
            ent.target_angle = reflected_dash_dir.y.atan2(reflected_dash_dir.x);
            offset += flip_axis * 10.0;
        }

        ent.pos += offset;

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

        // Start or dashing
        ent.dash_cooldown = (ent.dash_cooldown - dt).max(0.0);
        ent.dash = if let Some(mut dash) = ent.dash.clone() {
            dash.time_left -= dt;

            if dash.time_left <= 0.0 {
                ent.dash_cooldown = PLAYER_DASH_COOLDOWN;
                None
            } else {
                Some(dash)
            }
        } else if input.use_item && ent.dash_cooldown == 0.0 {
            Some(Dash {
                time_left: PLAYER_DASH_DURATION,
                dir: Vector::new(ent.angle.cos(), ent.angle.sin()),
            })
        } else {
            None
        };

        // Shooting
        /*if input_time >= ent.next_shot_time {
            if ent.shots_left == 0 {
                ent.shots_left = MAGAZINE_SIZE;
            }

            if delta.norm() > 0.0 && input.use_item {
                context.new_entities.push(Entity::Bullet(Bullet {
                    owner: Some(ent.owner),
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
                Entity::Bullet(bullet) if bullet.owner != Some(ent.owner) => {
                    if ent.rect().contains_point(bullet.pos(input_time)) {
                        context.removed_entities.insert(*entity_id);
                        killed = Some(DeathReason::ShotBy(bullet.owner));
                    }
                }
                _ => (),
            }
        }

        // Dying
        if let Some(reason) = killed {
            self.kill_player(entity_id, reason, context)?;
        }

        if !context.is_predicting {
            for caught_entity_id in caught_players {
                // If we are doing reconciliation, the entity might no longer exist in auth state.
                if self.entities.contains_key(&caught_entity_id) {
                    self.kill_player(caught_entity_id, DeathReason::CaughtBy(ent.owner), context)?;
                    Self::take_food(&mut self.players, ent, PLAYER_CATCH_FOOD);
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
                            Self::take_food(&mut self.players, ent, 1);
                        }
                    }
                    Entity::Food(food) => {
                        if context.removed_entities.contains(entity_id) {
                            // Already eaten or removed; prevent flickering.
                            continue;
                        }

                        if geom::rect_collision(
                            &food.rect(input_time),
                            &ent.rect(),
                            Vector::zeros(),
                        )
                        .is_some()
                        {
                            Self::take_food(&mut self.players, ent, food.amount);
                            context.removed_entities.insert(*entity_id);
                        }
                    }
                    _ => (),
                }
            }
        }

        Ok(())
    }

    fn take_food(players: &mut PlayerMap, ent: &mut PlayerEntity, amount: u32) {
        players.get_mut(&ent.owner).unwrap().food += amount;
        ent.target_size_bump += PLAYER_TAKE_FOOD_SIZE_BUMP * amount as f32;
        ent.target_size_bump = ent.target_size_bump.min(PLAYER_MAX_SIZE_BUMP);
    }

    fn kill_player(
        &mut self,
        entity_id: EntityId,
        reason: DeathReason,
        context: &mut RunContext,
    ) -> GameResult<()> {
        let ent = self.get_entity(entity_id)?.player()?.clone();
        context.killed_players.insert(ent.owner, reason);

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

            if self.catcher == Some(ent.owner) {
                // Choose a new catcher
                self.catcher = self
                    .entities
                    .iter()
                    .filter(|(other_id, _)| **other_id != entity_id)
                    .filter_map(|(_, other_entity)| {
                        other_entity.player().ok().map(|other_player| {
                            (other_player.owner, (ent.pos - other_player.pos).norm())
                        })
                    })
                    .min_by(|(_, dist1), (_, dist2)| dist1.partial_cmp(dist2).unwrap())
                    .map(|(other_owner, _)| other_owner);
                if let Some(catcher) = self.catcher {
                    context
                        .events
                        .push(Event::NewCatcher { player_id: catcher });
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
