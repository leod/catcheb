use crate::{Entity, EntityId, Game, Input, PlayerEntity, PlayerId, Vector};

#[derive(Debug, Clone)]
pub enum Error {
    InvalidEntityId(EntityId),
}

pub type Result<T> = std::result::Result<T, Error>;

pub const PLAYER_MOVE_SPEED: f32 = 300.0;

impl Game {
    pub fn run_player_input(&mut self, player_id: PlayerId, input: &Input) -> Result<()> {
        let delta_s = self.settings.tick_delta_s();

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
        }

        Ok(())
    }

    pub fn get_entity(&mut self, entity_id: EntityId) -> Result<&Entity> {
        self.entities
            .get(&entity_id)
            .ok_or_else(|| Error::InvalidEntityId(entity_id))
    }

    pub fn get_player_entity(
        &mut self,
        player_id: PlayerId,
    ) -> Result<Option<(EntityId, &PlayerEntity)>> {
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
    ) -> Result<Option<(EntityId, &mut PlayerEntity)>> {
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
