use std::collections::BTreeMap;

use log::{info, warn};

use comn::{game::RunContext, util::join};

use crate::game::ReceivedState;

const MIN_PREDICTION_ERROR_FOR_REPLAY: f32 = 0.001;

#[derive(Debug, Clone)]
struct Record {
    entities: comn::EntityMap,
    my_last_input: comn::Input,
}

pub struct Prediction {
    my_player_id: comn::PlayerId,
    log: BTreeMap<comn::TickNum, Record>,
    last_server_state_scratch: Option<comn::Game>,
}

impl Prediction {
    pub fn new(my_player_id: comn::PlayerId) -> Self {
        Self {
            my_player_id,
            log: BTreeMap::new(),
            last_server_state_scratch: None,
        }
    }

    pub fn record_tick_input(
        &mut self,
        tick_num: comn::TickNum,
        my_input: comn::Input,
        server_state: Option<&ReceivedState>,
    ) -> Vec<comn::Event> {
        // We predict the state for `predict_tick_num`, given the state of
        // `tick_num`.
        let predict_tick_num = tick_num.next();

        // Let's make as few assumptions as possible regarding consistency
        // in calls to `record_tick_input`.
        if let Some(max_logged) = self.max_logged_tick_num() {
            if max_logged != tick_num {
                info!(
                    "Skipped ticks in prediction ({:?} vs {:?}); resetting log",
                    tick_num, max_logged,
                );
                self.log = Default::default();
                self.last_server_state_scratch = None;
            }
        }

        // If we have a server state for the tick, apply corrections for our
        // previous prediction.
        let server_state_and_my_last_input_num =
            server_state.and_then(|state| state.my_last_input_num.map(|input| (state, input)));

        if let Some((server_state, my_last_input_num)) = server_state_and_my_last_input_num {
            let mut last_state = server_state.game.clone();

            let prediction_error = if let Some(record) = self.log.get_mut(&my_last_input_num.next())
            {
                Self::correct_prediction(
                    self.my_player_id,
                    &mut record.entities,
                    &server_state.game.entities,
                )
            } else {
                0.0
            };

            if prediction_error > 0.0 {
                info!("error: {}", prediction_error);
            }

            // We can now forget about any older predictions in the log.
            self.log = std::mem::replace(&mut self.log, BTreeMap::new())
                .into_iter()
                .filter(|&(tick_num, _)| tick_num > my_last_input_num)
                .collect();

            // Check if we need to replay our inputs following the corrected state.
            if prediction_error >= MIN_PREDICTION_ERROR_FOR_REPLAY {
                // Starting at the second-oldest state in our log (the oldest
                // one just got corrected), re-apply the inputs that we had
                // for those ticks.
                let last_entities = self.log.values().next().unwrap().entities.clone();
                Self::load_entities(&mut last_state, &last_entities);

                for (tick_num, record) in self.log.iter_mut().skip(1) {
                    last_state.tick_num = *tick_num;
                    Self::run_player_input(
                        &mut last_state,
                        self.my_player_id,
                        &record.my_last_input,
                    );
                    record.entities =
                        Self::extract_predicted_entities(&last_state, self.my_player_id);
                }
            }

            self.last_server_state_scratch = Some(last_state);
        }

        // Run prediction for the new given input.
        if let Some(last_state) = self.last_server_state_scratch.as_mut() {
            // Prepare state to run prediction in.
            if let Some(last_record) = self.log.values().next_back() {
                Self::load_entities(last_state, &last_record.entities);
            }
            last_state.tick_num = tick_num;

            let events = Self::run_player_input(last_state, self.my_player_id, &my_input);

            let mut entities = Self::extract_predicted_entities(last_state, self.my_player_id);
            if let Some(server_state) = server_state {
                // Some properties of predicted entities cannot be predicted,
                // i.e. those that are affected by events that only the server
                // controls (the motivating case was players taking food). We
                // overwrite these by the server state. Note that there will be
                // some time shift in when the properties are updated, but such
                // is life.
                Self::write_unpredicted_properties(&mut entities, &server_state.game.entities);
            }

            self.log.insert(
                predict_tick_num,
                Record {
                    entities,
                    my_last_input: my_input,
                },
            );

            events
        } else {
            // We have not received any authorative state yet at all, so we
            // cannot run prediction.
            Vec::new()
        }
    }

    pub fn predicted_entities(&self, tick_num: comn::TickNum) -> Option<&comn::EntityMap> {
        self.log.get(&tick_num).map(|record| &record.entities)
    }

    fn is_predicted(my_player_id: comn::PlayerId, entity: &comn::Entity) -> bool {
        match entity {
            comn::Entity::Player(entity) => entity.owner == my_player_id,
            comn::Entity::Bullet(entity) => entity.owner == Some(my_player_id),
            _ => false,
        }
    }

    fn extract_predicted_entities(
        state: &comn::Game,
        my_player_id: comn::PlayerId,
    ) -> comn::EntityMap {
        state
            .entities
            .iter()
            .filter(|(_, entity)| Self::is_predicted(my_player_id, entity))
            .map(|(entity_id, entity)| (*entity_id, entity.clone()))
            .collect()
    }

    fn max_logged_tick_num(&self) -> Option<comn::TickNum> {
        self.log.keys().next_back().copied()
    }

    fn load_entities(state: &mut comn::Game, entities: &comn::EntityMap) {
        // TODO: To be safe, we should remove all predicted entities from the
        // state before adding the new entities.
        state.entities.extend(
            entities
                .iter()
                .map(|(entity_id, entity)| (*entity_id, entity.clone())),
        )
    }

    fn run_player_input(
        state: &mut comn::Game,
        my_player_id: comn::PlayerId,
        my_input: &comn::Input,
    ) -> Vec<comn::Event> {
        let mut context = RunContext::default();
        context.is_predicting = true;

        if let Err(e) = state.run_player_input(my_player_id, &my_input, None, &mut context) {
            warn!("Simulation error: {:?}", e);
        }

        for entity in context.new_entities {
            Self::add_predicted_entity(&mut state.entities, entity);
        }

        context.events
    }

    fn correct_prediction(
        my_player_id: comn::PlayerId,
        predicted: &mut comn::EntityMap,
        server: &comn::EntityMap,
    ) -> f32 {
        let mut error = 0.0;

        *predicted = join::full_join(predicted.iter(), server.iter())
            .filter_map(|item| Self::correct_entity(my_player_id, item, &mut error))
            .collect();

        error
    }

    fn correct_entity(
        my_player_id: comn::PlayerId,
        item: join::Item<&comn::EntityId, &comn::Entity, &comn::Entity>,
        error: &mut f32,
    ) -> Option<(comn::EntityId, comn::Entity)> {
        use comn::Entity::*;
        use join::Item::*;

        match item {
            Both(id, predicted, server) => match (predicted, server) {
                (Player(predicted), Player(server))
                    if Self::is_predicted(my_player_id, &Player(predicted.clone())) =>
                {
                    *error += match (predicted.dash.as_ref(), server.dash.as_ref()) {
                        (Some(d1), Some(d2)) => {
                            (d1.time_left - d2.time_left).abs() + (d1.dir - d2.dir).norm()
                        }
                        (None, None) => 0.0,
                        _ => MIN_PREDICTION_ERROR_FOR_REPLAY,
                    };

                    Some((
                        *id,
                        Player(comn::PlayerEntity {
                            pos: Self::correct_point(predicted.pos, server.pos, error),
                            hook: match (&predicted.hook, &server.hook) {
                                (Some(a), Some(b)) => Some(Self::correct_hook(a, b, error)),
                                _ => {
                                    *error += MIN_PREDICTION_ERROR_FOR_REPLAY;
                                    server.hook.clone()
                                }
                            },
                            ..server.clone()
                        }),
                    ))
                }
                _ => Some((*id, server.clone())),
            },
            Left(_, predicted) => {
                if Self::is_predicted(my_player_id, predicted) {
                    // An entity that we predicted (most likely the
                    // PlayerEntity) no longer exists in the authorative
                    // state. Make sure to replay.
                    *error += MIN_PREDICTION_ERROR_FOR_REPLAY;
                }
                None
            }
            Right(id, server) => {
                if Self::is_predicted(my_player_id, server) {
                    // Server has a new entity, make sure to replay
                    // prediction so that we include it. Might be that
                    // there is a better way to go about it, because
                    // this will replay prediction too often.
                    *error += MIN_PREDICTION_ERROR_FOR_REPLAY;
                    Some((*id, server.clone()))
                } else {
                    None
                }
            }
        }
    }

    fn correct_point(predicted: comn::Point, server: comn::Point, error: &mut f32) -> comn::Point {
        let delta = server - predicted;

        *error += delta.norm();

        if delta.norm() < 0.01 || delta.norm() > 200.0 {
            server
        } else {
            // Smoothly correct prediction over time
            predicted + delta * 0.2
        }
    }

    fn correct_vector(
        predicted: comn::Vector,
        server: comn::Vector,
        error: &mut f32,
    ) -> comn::Vector {
        let delta = server - predicted;

        *error += delta.norm();

        if delta.norm() < 0.01 || delta.norm() > 200.0 {
            server
        } else {
            // Smoothly correct prediction over time
            predicted + delta * 0.2
        }
    }

    fn correct_hook(predicted: &comn::Hook, server: &comn::Hook, error: &mut f32) -> comn::Hook {
        use comn::Hook::*;

        match (predicted, server) {
            (
                Shooting { pos: a, .. },
                Shooting {
                    pos: b,
                    vel,
                    time_left,
                },
            ) => Shooting {
                pos: Self::correct_point(*a, *b, error),
                vel: *vel,
                time_left: *time_left,
            },
            (
                Attached {
                    target: target_a,
                    offset: offset_a,
                },
                Attached {
                    target: target_b,
                    offset: offset_b,
                },
            ) => {
                if target_a != target_b {
                    *error += MIN_PREDICTION_ERROR_FOR_REPLAY;
                }

                Attached {
                    target: *target_b,
                    offset: Self::correct_vector(*offset_a, *offset_b, error),
                }
            }
            (Contracting { pos: a }, Contracting { pos: b }) => Contracting {
                pos: Self::correct_point(*a, *b, error),
            },
            _ => {
                *error += MIN_PREDICTION_ERROR_FOR_REPLAY;
                server.clone()
            }
        }
    }

    fn add_predicted_entity(entities: &mut comn::EntityMap, entity: comn::Entity) {
        // TODO: Some scheme for entity IDs of predicted entities
        let entity_id = entities
            .keys()
            .next_back()
            .copied()
            .unwrap_or(comn::EntityId(0))
            .next();

        // Sanity check
        assert!(!entities.contains_key(&entity_id));

        entities.insert(entity_id, entity);
    }

    fn write_unpredicted_properties(predicted: &mut comn::EntityMap, server: &comn::EntityMap) {
        for item in join::full_join(predicted.iter_mut(), server.iter()) {
            match item {
                join::Item::Both(
                    _,
                    comn::Entity::Player(predicted),
                    comn::Entity::Player(server),
                ) => {
                    predicted.size_bump = server.size_bump;
                }
                _ => (),
            }
        }
    }
}
