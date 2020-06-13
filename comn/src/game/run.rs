use std::collections::{BTreeMap, BTreeSet};

use crate::entities::Bullet;
use crate::{
    geom::{self, AaRect},
    DeathReason, Entity, EntityId, Event, Game, GameError, GameResult, GameTime, Hook, HookState,
    Input, PlayerEntity, PlayerId, Vector,
};

pub const PLAYER_MOVE_SPEED: f32 = 250.0;
pub const PLAYER_SIT_W: f32 = 40.0;
pub const PLAYER_SIT_L: f32 = 40.0;
pub const PLAYER_MOVE_W: f32 = 56.6;
pub const PLAYER_MOVE_L: f32 = 28.2;
pub const PLAYER_SHOOT_PERIOD: GameTime = 0.3;
pub const PLAYER_TRANSITION_SPEED: f32 = 4.0;
pub const PLAYER_ACCEL_FACTOR: f32 = 40.0;
pub const PLAYER_DASH_COOLDOWN: f32 = 2.5;
pub const PLAYER_DASH_DURATION: GameTime = 0.5;
pub const PLAYER_DASH_ACCEL_FACTOR: f32 = 30.0;
pub const PLAYER_DASH_SPEED: f32 = 750.0;

pub const HOOK_SHOOT_SPEED: f32 = 600.0;
pub const HOOK_MAX_SHOOT_DURATION: f32 = 1.0;
pub const HOOK_MIN_DISTANCE: f32 = 0.05;

pub const BULLET_MOVE_SPEED: f32 = 300.0;
pub const BULLET_RADIUS: f32 = 8.0;
pub const MAGAZINE_SIZE: u32 = 15;
pub const RELOAD_DURATION: GameTime = 2.0;

pub const TURRET_RADIUS: f32 = 30.0;
pub const TURRET_RANGE: f32 = 400.0;
pub const TURRET_SHOOT_PERIOD: GameTime = 1.3;
pub const TURRET_SHOOT_ANGLE: f32 = 0.3;
pub const TURRET_MAX_TURN_SPEED: f32 = 2.0;

#[derive(Clone, Debug, Default)]
pub struct RunContext {
    pub events: Vec<Event>,
    pub new_entities: Vec<Entity>,
    pub removed_entities: BTreeSet<EntityId>,
    pub killed_players: BTreeMap<PlayerId, DeathReason>,
}

impl Game {
    pub fn run_tick(&mut self, context: &mut RunContext) -> GameResult<()> {
        let time = self.current_game_time();

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
                        turret.angle += angle_dist * 0.1;
                        //.min(TURRET_MAX_TURN_SPEED * tick_period)
                        //.max(TURRET_MAX_TURN_SPEED * tick_period);

                        if time >= turret.next_shot_time && angle_dist.abs() < TURRET_SHOOT_ANGLE {
                            turret.next_shot_time = time + TURRET_SHOOT_PERIOD;

                            let delta = Vector::new(turret.angle.cos(), turret.angle.sin());

                            context.new_entities.push(Entity::Bullet(Bullet {
                                owner: None,
                                start_time: time,
                                start_pos: turret.pos + 12.0 * delta,
                                vel: delta * BULLET_MOVE_SPEED,
                            }));
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
        let input_time = input_state.current_game_time();

        let cur_dash = ent.last_dash.filter(|(dash_time, _)| {
            input_time >= *dash_time && input_time <= dash_time + PLAYER_DASH_DURATION
        });

        // TODO: Redundant state needed for display
        ent.is_dashing = cur_dash.is_some();

        let mut delta = Vector::new(0.0, 0.0);

        if let Some((_dash_time, dash_dir)) = cur_dash {
            // Movement is constricted while dashing.
            let target_vel = dash_dir * PLAYER_DASH_SPEED;
            ent.vel =
                geom::smooth_to_target_vector(PLAYER_DASH_ACCEL_FACTOR, ent.vel, target_vel, dt);

            // TODO: State redundancy
            ent.angle = Some(dash_dir.y.atan2(dash_dir.x));
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

            // TODO: State redundancy
            ent.angle = None;
        }

        if let Some(hook) = ent.hook.clone() {
            match hook.state {
                HookState::Shooting {
                    start_time,
                    start_pos,
                    vel,
                } => {
                    if input_time - start_time > HOOK_MAX_SHOOT_DURATION {
                        ent.hook = None;
                    } else {
                        let pos = start_pos + (input_time - start_time) * vel;

                        for (target_id, target) in input_state.entities.iter() {
                            if entity_id != *target_id
                                && target.can_hook_attach()
                                && target.shape(input_time).contains_point(pos)
                            {
                                ent.hook = Some(Hook {
                                    state: HookState::Attached {
                                        target: *target_id,
                                        offset: pos - target.pos(input_time),
                                    },
                                });
                                break;
                            }
                        }
                    }
                }
                HookState::Attached { target, offset } => {
                    if let Some(target_entity) = input_state.entities.get(&target) {
                        let delta_to_target = target_entity.pos(input_time) + offset - ent.pos;
                        if delta_to_target.norm() < HOOK_MIN_DISTANCE {
                            ent.hook = None;
                        } else {
                            ent.vel += delta_to_target;
                        }
                    } else {
                        ent.hook = None;
                    }

                    if !input.use_action {
                        ent.hook = None;
                    }
                }
            }
        } else if input.use_action && delta.norm() > 0.0 && ent.hook.is_none() {
            ent.hook = Some(Hook {
                state: HookState::Shooting {
                    start_time: input_time,
                    start_pos: ent.pos,
                    vel: delta.normalize() * HOOK_SHOOT_SPEED,
                },
            });
        }

        let mut offset = ent.vel * dt;
        let mut flip_axis = None;

        for (_, entity) in input_state.entities.iter() {
            let other_shape = match entity {
                Entity::Player(other_ent) if other_ent.owner != player_id => Some(other_ent.rect()),
                Entity::Wall(other_ent) => Some(other_ent.rect.to_rect()),
                _ => None,
            };

            if let Some(collision) = other_shape
                .and_then(|other_shape| geom::rect_collision(&ent.rect(), &other_shape, offset))
            {
                offset += collision.resolution_vector;
                flip_axis = Some(collision.axis);
            }
        }

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

        if input.use_item
            && ent.last_dash.map_or(true, |(dash_time, _)| {
                dash_time + PLAYER_DASH_COOLDOWN <= input_time
            })
            && delta.norm() > 0.1
        {
            ent.last_dash = Some((input_time, delta));
        }

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

        for (entity_id, entity) in input_state.entities.iter() {
            match entity {
                Entity::DangerGuy(danger_guy) => {
                    if geom::rect_collision(
                        &danger_guy.aa_rect(input_time).to_rect(),
                        &ent.rect(),
                        Vector::zeros(),
                    )
                    .is_some()
                    {
                        context
                            .killed_players
                            .insert(player_id, DeathReason::TouchedTheDanger);
                    }
                }
                Entity::Bullet(bullet) if bullet.owner != Some(player_id) => {
                    if ent.rect().contains_point(bullet.pos(input_time)) {
                        context.removed_entities.insert(*entity_id);
                        context
                            .killed_players
                            .insert(player_id, DeathReason::ShotBy(bullet.owner));
                    }
                }
                _ => (),
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
