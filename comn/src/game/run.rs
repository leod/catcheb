use crate::entities::Bullet;
use crate::{Entity, EntityId, Game, GameError, GameResult, Input, PlayerEntity, PlayerId, Vector};

pub const PLAYER_MOVE_SPEED: f32 = 300.0;
pub const PLAYER_SIT_W: f32 = 50.0;
pub const PLAYER_SIT_L: f32 = 50.0;
pub const PLAYER_MOVE_W: f32 = 70.0;
pub const PLAYER_MOVE_L: f32 = 35.714;
pub const PLAYER_SHOOT_PERIOD: f32 = 0.3;
pub const BULLET_MOVE_SPEED: f32 = 900.0;

impl Game {
    pub fn run_tick(&mut self) -> GameResult<()> {
        let time = self.tick_game_time(self.tick_num);

        let mut remove_ids = Vec::new();

        for (entity_id, entity) in self.entities.iter() {
            match entity {
                Entity::Bullet(entity) => {
                    if !self.settings.aa_rect().contains_point(entity.pos(time)) {
                        remove_ids.push(*entity_id);
                        continue;
                    }

                    for (entity_id_b, entity_b) in self.entities.iter() {
                        if *entity_id == *entity_id_b {
                            continue;
                        }

                        match entity_b {
                            Entity::DangerGuy(entity_b) => {
                                if entity_b.aa_rect(time).contains_point(entity.pos(time)) {
                                    remove_ids.push(*entity_id);
                                    break;
                                }
                            }
                            _ => (),
                        }
                    }
                }
                _ => (),
            }
        }

        for id in remove_ids {
            self.entities.remove(&id);
        }

        Ok(())
    }

    pub fn run_player_input(
        &mut self,
        player_id: PlayerId,
        input: &Input,
    ) -> GameResult<Vec<Entity>> {
        let delta_s = self.settings.tick_period();
        let time = self.tick_game_time(self.tick_num);
        let map_size = self.settings.size;

        let mut new_entities = Vec::new();

        if let Some((_entity_id, player_entity)) = self.get_player_entity_mut(player_id)? {
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
                player_entity.pos += delta.normalize() * PLAYER_MOVE_SPEED * delta_s;
                player_entity.angle = Some(delta.y.atan2(delta.x));
            } else {
                player_entity.angle = None;
            }

            player_entity.pos.x = player_entity
                .pos
                .x
                .min(map_size.x - PLAYER_SIT_W / 2.0)
                .max(PLAYER_SIT_W / 2.0);
            player_entity.pos.y = player_entity
                .pos
                .y
                .min(map_size.y - PLAYER_SIT_W / 2.0)
                .max(PLAYER_SIT_W / 2.0);

            if delta.norm() > 0.0
                && input.use_item
                && time - player_entity.last_shot_time.unwrap_or(-1000.0) >= PLAYER_SHOOT_PERIOD
            {
                player_entity.last_shot_time = Some(time);
                new_entities.push(Entity::Bullet(Bullet {
                    owner: player_id,
                    start_time: time,
                    start_pos: player_entity.pos,
                    vel: delta.normalize() * BULLET_MOVE_SPEED,
                }));
            }
        }

        Ok(new_entities)
    }

    pub fn get_entity(&mut self, entity_id: EntityId) -> GameResult<&Entity> {
        self.entities
            .get(&entity_id)
            .ok_or_else(|| GameError::InvalidEntityId(entity_id))
    }

    pub fn get_player_entity(
        &mut self,
        player_id: PlayerId,
    ) -> GameResult<Option<(EntityId, &PlayerEntity)>> {
        Ok(self
            .entities
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
            .next())
    }

    pub fn get_player_entity_mut(
        &mut self,
        player_id: PlayerId,
    ) -> GameResult<Option<(EntityId, &mut PlayerEntity)>> {
        Ok(self
            .entities
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
            .next())
    }
}
