use std::collections::{BTreeMap, BTreeSet};

use crate::entities::Bullet;
use crate::{
    geom::{self, AaRect},
    DeathReason, Entity, EntityId, Event, Game, GameError, GameResult, GameTime, Input,
    PlayerEntity, PlayerId, Vector,
};

pub const PLAYER_MOVE_SPEED: f32 = 250.0;
pub const PLAYER_SIT_W: f32 = 40.0;
pub const PLAYER_SIT_L: f32 = 40.0;
pub const PLAYER_MOVE_W: f32 = 56.6;
pub const PLAYER_MOVE_L: f32 = 28.2;
pub const PLAYER_SHOOT_PERIOD: GameTime = 0.3;
pub const PLAYER_TRANSITION_SPEED: f32 = 4.0;
pub const PLAYER_ACCEL_FACTOR: f32 = 9.0;
pub const PLAYER_DASH_DURATION: GameTime = 1.5;

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
        let dt = self.settings.tick_period();
        let time = self.current_game_time();
        let input_time = input_state
            .map(|input_state| input_state.current_game_time())
            .unwrap_or(time);
        let map_size = self.settings.size;

        if let Some((_entity_id, ent)) = self.get_player_entity_mut(player_id) {
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

            ent.vel = geom::smooth_to_target_vector(
                PLAYER_ACCEL_FACTOR,
                ent.vel,
                delta.normalize() * PLAYER_MOVE_SPEED,
                dt,
            );
            ent.pos += ent.vel * dt;

            if delta.norm() > 0.0 {
                ent.angle = Some(delta.y.atan2(delta.x));
            } else {
                ent.angle = None;
            }

            ent.pos.x = ent
                .pos
                .x
                .min(map_size.x - PLAYER_SIT_W / 2.0)
                .max(PLAYER_SIT_W / 2.0);
            ent.pos.y = ent
                .pos
                .y
                .min(map_size.y - PLAYER_SIT_W / 2.0)
                .max(PLAYER_SIT_W / 2.0);

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

            let pos = ent.pos;
            let input_state = input_state.unwrap_or(self);

            for (entity_id, entity) in input_state.entities.iter() {
                match entity {
                    Entity::DangerGuy(danger_guy) => {
                        // TODO: Player geometry
                        if danger_guy.aa_rect(input_time).contains_point(pos) {
                            context
                                .killed_players
                                .insert(player_id, DeathReason::TouchedTheDanger);
                        }
                    }
                    Entity::Bullet(bullet) if bullet.owner != Some(player_id) => {
                        // TODO: Player geometry
                        let aa_rect =
                            AaRect::new_center(pos, Vector::new(PLAYER_SIT_W, PLAYER_SIT_L));

                        if aa_rect.contains_point(bullet.pos(input_time)) {
                            context.removed_entities.insert(*entity_id);
                            context
                                .killed_players
                                .insert(player_id, DeathReason::ShotBy(bullet.owner));
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

    pub fn get_player_entity_mut(
        &mut self,
        player_id: PlayerId,
    ) -> Option<(EntityId, &mut PlayerEntity)> {
        self.entities
            .iter_mut()
            .filter_map(|(&id, e)| {
                if let Entity::Player(ref mut e) = e {
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
