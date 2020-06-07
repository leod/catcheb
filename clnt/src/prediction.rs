use std::collections::BTreeMap;

use log::{info, warn};

use comn::game::RunContext;

use crate::game::ReceivedState;

#[derive(Debug, Clone)]
struct Record {
    state: comn::Game,
    my_last_input: comn::Input,
    //new_entities: Vec<comn::EntityId>,
}

pub struct Prediction {
    my_player_id: comn::PlayerId,
    log: BTreeMap<comn::TickNum, Record>,
}

impl Prediction {
    pub fn new(my_player_id: comn::PlayerId) -> Self {
        Self {
            my_player_id,
            log: BTreeMap::new(),
        }
    }

    pub fn record_tick_input(
        &mut self,
        tick_num: comn::TickNum,
        my_input: comn::Input,
        server_state: Option<&ReceivedState>,
    ) -> Vec<comn::Event> {
        // Let's make as few assumptions as possible regarding consistency
        // in calls to `record_tick_input`.
        if let Some(max_logged) = self.max_logged_tick_num() {
            if max_logged > tick_num {
                info!(
                    "Predicting tick that is in our past ({:?} vs {:?}); resetting log",
                    tick_num, max_logged,
                );
                self.log = Default::default();
            } else if max_logged != tick_num {
                // TODO: Do we really need to harshly re-initialize prediction
                // here?
                info!(
                    "Skipped ticks in prediction ({:?} vs {:?}); resetting log",
                    tick_num, max_logged,
                );
                self.log = Default::default();
            }
        }

        // If we have a server state for the tick, apply corrections for our
        // previous prediction.
        if let Some((server_state, my_last_input)) = server_state.and_then(|server_state| {
            server_state
                .my_last_input
                .map(|input| (server_state, input))
        }) {
            info!("got {:?} at {:?}", my_last_input, tick_num);

            if let Some(record) = self.log.get_mut(&my_last_input.next()) {
                Self::correct_prediction(&mut record.state, &server_state.game);

                // TODO: We should probably remove the redundant tick_num
                // state within game state.
                record.state.tick_num = my_last_input.next();
            }

            // We can now forget about any older predictions in the log.
            // TODO: Needless clone.
            self.log = self
                .log
                .clone()
                .into_iter()
                .filter(|&(tick_num, _)| tick_num > my_last_input)
                .collect();
        }

        let last_state = if let Some(first_record) = self.log.values().next() {
            // Starting at the oldest state in our log, re-apply the inputs
            // that we had.
            let mut last_state = first_record.state.clone();
            for record in self.log.values_mut().skip(1) {
                Self::run_tick(&mut last_state, self.my_player_id, &record.my_last_input);
                record.state = last_state.clone();
            }

            assert!(last_state.tick_num == tick_num);

            Some(last_state)
        } else if let Some(server_state) = server_state {
            // Our prediction log is empty, but we have a server state that we
            // can use to start prediction.
            Some(server_state.game.clone())
        } else {
            // We have no state from which we can start prediction.
            None
        };

        if let Some(mut state) = last_state {
            //info!("running at {:?} in {:?}", tick_num, state.tick_num);
            let events = Self::run_tick(&mut state, self.my_player_id, &my_input);

            assert!(tick_num.next() == state.tick_num);

            self.log.insert(
                tick_num.next(),
                Record {
                    state,
                    my_last_input: my_input.clone(),
                },
            );
            events
        } else {
            Vec::new()
        }
    }

    pub fn predicted_state(&self, tick_num: comn::TickNum) -> Option<&comn::Game> {
        self.log.get(&tick_num).map(|record| &record.state)
    }

    pub fn is_predicted(&self, entity: &comn::Entity) -> bool {
        match entity {
            comn::Entity::Player(entity) => entity.owner == self.my_player_id,
            comn::Entity::Bullet(entity) => entity.owner == Some(self.my_player_id),
            _ => false,
        }
    }

    fn max_logged_tick_num(&self) -> Option<comn::TickNum> {
        self.log.keys().next_back().copied()
    }

    fn run_tick(
        state: &mut comn::Game,
        my_player_id: comn::PlayerId,
        my_input: &comn::Input,
    ) -> Vec<comn::Event> {
        let mut context = RunContext::default();
        if let Err(e) = state.run_player_input(my_player_id, &my_input, None, &mut context) {
            // TODO: Simulation error handling on client side
            warn!("Simulation error: {:?}", e);
        }

        for entity in context.new_entities {
            Self::add_predicted_entity(state, entity);
        }

        // TODO: We should probably remove the redundant tick_num
        // state within game state.
        state.tick_num = state.tick_num.next();

        context.events
    }

    fn correct_prediction(predicted: &mut comn::Game, server: &comn::Game) {
        // TODO: Smooth correction of positions
        *predicted = server.clone();
    }

    fn add_predicted_entity(state: &mut comn::Game, entity: comn::Entity) {
        // TODO: Some scheme for entity IDs of predicted entities
        let entity_id = state
            .entities
            .keys()
            .next_back()
            .copied()
            .unwrap_or(comn::EntityId(0))
            .next();

        // Sanity checks
        assert!(!state.entities.contains_key(&entity_id));

        state.entities.insert(entity_id, entity);
    }
}
