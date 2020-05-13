use crate::{Entity, EntityId, Game, GameError, GameResult, Input, PlayerEntity, PlayerId, Vector};

pub const PLAYER_MOVE_SPEED: f32 = 300.0;
pub const PLAYER_SIT_W: f32 = 50.0;
pub const PLAYER_SIT_L: f32 = 50.0;
pub const PLAYER_MOVE_W: f32 = 70.0;
pub const PLAYER_MOVE_L: f32 = 35.714;

impl Game {
    pub fn run_player_input(&mut self, player_id: PlayerId, input: &Input) -> GameResult<()> {
        let delta_s = self.settings.tick_delta_s();
        let map_size = self.settings.size;

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
        }

        Ok(())
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
