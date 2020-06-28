use std::collections::{BTreeMap, VecDeque};

use log::{debug, info};
use rand::seq::SliceRandom;

use comn::{game::RunContext, Entity, PlayerState};

use crate::bot::Bot;

pub const FIRST_SPAWN_DURATION: comn::GameTime = 0.5;
pub const RESPAWN_DURATION: comn::GameTime = 2.0;
pub const KEEP_PREV_STATES_DURATION: comn::GameTime = 1.0;

pub struct PlayerMeta {
    pub last_input_num: Option<comn::TickNum>,
    pub bot: Option<Bot>,
}

pub struct Game {
    pub state: comn::Game,
    pub next_entity_id: comn::EntityId,

    pub players_meta: BTreeMap<comn::PlayerId, PlayerMeta>,

    /// Previous states, used for reconciliation.
    pub prev_states: VecDeque<comn::Game>,

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
            players_meta: BTreeMap::new(),
            prev_states: VecDeque::new(),
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

    pub fn join(&mut self, player_name: String, bot: bool) -> comn::PlayerId {
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

        let spawn_time = self.state.game_time() + FIRST_SPAWN_DURATION;
        let player = comn::Player {
            name: player_name,
            state: PlayerState::Respawning {
                respawn_time: spawn_time,
            },
            food: 0,
        };
        let player_meta = PlayerMeta {
            last_input_num: None,
            bot: if bot { Some(Bot::default()) } else { None },
        };
        info!(
            "New player {:?} with id {:?} joined game",
            player, player_id
        );

        self.state.players.insert(player_id, player);
        self.players_meta.insert(player_id, player_meta);

        player_id
    }

    pub fn run_tick(&mut self, inputs: &[(comn::PlayerId, comn::TickNum, comn::Input)]) {
        //debug!("tick with {} inputs", inputs.len());
        let current_time = self.state.game_time();
        let mut context = RunContext::default();

        self.state.run_tick(&mut context).unwrap();

        // TODO: Sort player input by tick num
        for (player_id, input_tick_num, input) in inputs {
            let input_state = self
                .prev_states
                .iter()
                .find(|state| state.tick_num == *input_tick_num);

            self.state
                .run_player_input(*player_id, input, input_state, &mut context)
                .unwrap();
            self.players_meta
                .get_mut(&player_id)
                .unwrap()
                .last_input_num = Some(*input_tick_num);
        }

        for (player_id, player_meta) in self.players_meta.iter_mut() {
            if let Some(bot) = player_meta.bot.as_mut() {
                let input = bot.get_next_input(&self.state);

                self.state
                    .run_player_input(*player_id, &input, None, &mut context)
                    .unwrap();
            }
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

                    // TODO: Random
                    let spawn_pos = self
                        .state
                        .settings
                        .spawn_points
                        .choose(&mut rand::thread_rng())
                        .unwrap();

                    context
                        .new_entities
                        .push(Entity::Player(comn::PlayerEntity::new(
                            *player_id, *spawn_pos,
                        )));

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

        self.state.tick_num = self.state.tick_num.next();

        self.last_events = context.events;

        self.prev_states.push_back(self.state.clone());

        let max_num_states =
            (KEEP_PREV_STATES_DURATION * self.state.settings.ticks_per_second as f32) as usize;
        while self.prev_states.len() > max_num_states {
            self.prev_states.pop_front();
        }
    }

    pub fn remove_player(&mut self, player_id: comn::PlayerId) {
        debug!("Removing player {:?}", player_id);
        self.state.players.remove(&player_id).unwrap();
        self.players_meta.remove(&player_id).unwrap();

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

    pub fn correct_time_for_player(&self, observer_id: comn::PlayerId, state: &mut comn::Game) {
        let state_time = state.game_time();
        for entity in state.entities.values_mut() {
            match entity {
                comn::Entity::Player(player) if player.owner != observer_id => {
                    if let Some(last_input_num) = self.players_meta[&player.owner].last_input_num {
                        let time_lag = state_time - self.state.tick_game_time(last_input_num);
                        if let Some((time, _)) = &mut player.last_dash {
                            *time += time_lag;
                        }
                        if let Some(hook) = &mut player.hook {
                            match &mut hook.state {
                                comn::HookState::Shooting { start_time, .. } => {
                                    *start_time += time_lag;
                                }
                                comn::HookState::Attached { start_time, .. } => {
                                    *start_time += time_lag;
                                }
                                comn::HookState::Contracting { start_time, .. } => {
                                    *start_time += time_lag;
                                }
                            }
                        }
                    }
                }
                _ => (),
            }
        }
    }

    fn add_entity(&mut self, entity: comn::Entity) {
        let entity_id = self.next_entity_id;
        self.next_entity_id = comn::EntityId(self.next_entity_id.0 + 1);

        // Sanity checks
        assert!(!self.state.entities.contains_key(&entity_id));
        if let comn::Entity::Player(entity) = &entity {
            assert!(self.state.get_player_entity(entity.owner).is_none());
        }

        self.state.entities.insert(entity_id, entity);
    }

    fn remove_entity(&mut self, entity_id: comn::EntityId) {
        debug!("Removing entity {:?}", entity_id);
        self.state.entities.remove(&entity_id);
    }

    fn kill_player(&mut self, player_id: comn::PlayerId) {
        let player = self.state.players.get_mut(&player_id).unwrap();
        debug!(
            "Killing player {:?} (in state {:?})",
            player_id, player.state
        );

        player.state = PlayerState::Dead;

        if let Some((player_entity_id, _)) = self.state.get_player_entity(player_id) {
            self.remove_entity(player_entity_id);
        }
    }
}
