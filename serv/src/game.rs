use log::{debug, info};

pub struct Game {
    pub state: comn::Game,
    pub next_entity_id: comn::EntityId,
}

impl Game {
    pub fn new(settings: comn::Settings) -> Self {
        let state = comn::Game::new(settings);
        let next_entity_id = state
            .entities
            .keys()
            .copied()
            .map(|id| comn::EntityId(id.0 + 1))
            .max()
            .unwrap_or(comn::EntityId(0));

        Self {
            state,
            next_entity_id,
        }
    }

    pub fn is_full(&self) -> bool {
        assert!(self.state.players.len() <= self.settings().max_num_players);
        self.state.players.len() == self.settings().max_num_players
    }

    pub fn state(&self) -> &comn::Game {
        &self.state
    }

    pub fn settings(&self) -> &comn::Settings {
        &self.state.settings
    }

    pub fn join(&mut self, player_name: String) -> comn::PlayerId {
        // Runner takes care of not trying to join a full game.
        assert!(!self.is_full());

        let max_player_id = self
            .state
            .players
            .keys()
            .next_back()
            .cloned()
            .unwrap_or(comn::PlayerId(0));
        let player_id = comn::PlayerId(max_player_id.0 + 1);

        let player = comn::Player { name: player_name };
        info!(
            "New player {:?} with id {:?} joined game",
            player, player_id
        );

        self.state.players.insert(player_id, player);

        self.add_entity(comn::Entity::Player(comn::PlayerEntity {
            owner: player_id,
            pos: comn::Point::new(350.0, 100.0),
            angle: None,
        }));

        player_id
    }

    pub fn run_tick(&mut self, inputs: &[(comn::PlayerId, comn::Input)]) {
        for (player_id, input) in inputs {
            self.state.run_player_input(*player_id, input).unwrap();
        }

        self.state.tick_num = comn::TickNum(self.state.tick_num.0 + 1)
    }

    pub fn remove_player(&mut self, player_id: comn::PlayerId) {
        debug!("Removing player {:?}", player_id);
        self.state.players.remove(&player_id).unwrap();

        let remove_ids: Vec<comn::EntityId> = self
            .state
            .entities
            .iter()
            .filter_map(|(entity_id, entity)| {
                if let comn::Entity::Player(entity) = entity {
                    if entity.owner == player_id {
                        Some(*entity_id)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        for entity_id in remove_ids {
            self.remove_entity(entity_id);
        }
    }

    fn add_entity(&mut self, entity: comn::Entity) {
        let entity_id = self.next_entity_id;
        self.next_entity_id = comn::EntityId(self.next_entity_id.0 + 1);
        assert!(!self.state.entities.contains_key(&entity_id));

        self.state.entities.insert(entity_id, entity);
    }

    fn remove_entity(&mut self, entity_id: comn::EntityId) {
        debug!("Removing entity {:?}", entity_id);
        self.state.entities.remove(&entity_id).unwrap();
    }
}
