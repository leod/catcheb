use log::{debug, info};

use comn::{game::run::RunContext, Entity, PlayerState};

pub const RESPAWN_DURATION: comn::GameTime = 2.0;

pub struct Game {
    pub state: comn::Game,
    pub next_entity_id: comn::EntityId,

    /// Events produced in the last update. We keep these around so that we
    /// can send them to the players in this game in `Runner`.
    pub last_events: Vec<comn::Event>,
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
            last_events: Vec::new(),
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

        let spawn_time = self.state.current_game_time() + RESPAWN_DURATION;
        let player = comn::Player {
            name: player_name,
            state: PlayerState::Respawning {
                respawn_time: spawn_time,
            },
        };
        info!(
            "New player {:?} with id {:?} joined game",
            player, player_id
        );

        self.state.players.insert(player_id, player);

        player_id
    }

    pub fn run_tick(&mut self, inputs: &[(comn::PlayerId, comn::TickNum, comn::Input)]) {
        //debug!("tick with {} inputs", inputs.len());
        let current_time = self.state.current_game_time();
        let mut context = RunContext::default();

        self.state.run_tick(&mut context).unwrap();

        // TODO: Sort player input by tick num
        for (player_id, _tick_num, input) in inputs {
            self.state
                .run_player_input(*player_id, input, &mut context)
                .unwrap();
        }

        for (player_id, player) in self.state.players.iter_mut() {
            match player.state.clone() {
                PlayerState::Alive => (),
                PlayerState::Dead => {
                    player.state = PlayerState::Respawning {
                        respawn_time: current_time + RESPAWN_DURATION,
                    };
                }
                PlayerState::Respawning { respawn_time } if current_time >= respawn_time => {
                    debug!("Respawning player {:?}", player_id);

                    context
                        .new_entities
                        .push(Entity::Player(comn::PlayerEntity {
                            owner: *player_id,
                            pos: comn::Point::new(350.0, 100.0),
                            angle: None,
                            last_shot_time: None,
                        }));

                    player.state = PlayerState::Alive;
                }
                PlayerState::Respawning { .. } => (),
            }
        }

        for entity in context.new_entities {
            self.add_entity(entity);
        }

        for entity_id in context.removed_entities {
            self.remove_entity(entity_id);
        }

        for (player_id, reason) in context.killed_players {
            self.kill_player(player_id);
            context
                .events
                .push(comn::Event::PlayerDied { player_id, reason });
        }

        self.state.tick_num = comn::TickNum(self.state.tick_num.0 + 1);
        self.last_events = context.events;
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

        // Sanity checks
        assert!(!self.state.entities.contains_key(&entity_id));
        if let comn::Entity::Player(entity) = &entity {
            assert!(self
                .state
                .get_player_entity(entity.owner)
                .unwrap()
                .is_none());
        }

        self.state.entities.insert(entity_id, entity);
    }

    fn remove_entity(&mut self, entity_id: comn::EntityId) {
        debug!("Removing entity {:?}", entity_id);
        self.state.entities.remove(&entity_id).unwrap();
    }

    fn kill_player(&mut self, player_id: comn::PlayerId) {
        let player = self.state.players.get_mut(&player_id).unwrap();
        debug!(
            "Killing player {:?} (in state {:?})",
            player_id, player.state
        );

        player.state = PlayerState::Dead;

        if let Some((player_entity_id, _)) = self.state.get_player_entity(player_id).unwrap() {
            self.remove_entity(player_entity_id);
        }
    }
}
