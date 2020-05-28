use std::collections::BTreeMap;

use log::{debug, info, warn};

use comn::game::RunContext;

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
        server_state: Option<&comn::Tick>,
    ) -> Vec<comn::Event> {
        if self.max_logged_tick_num().unwrap_or(comn::TickNum(0)) > tick_num {
            info!("Predicting tick that we already predicted; resetting log");
            self.log = Default::default();
        }

        info!("at {:?} {:?}", tick_num, server_state);

        if let Some((server_state, my_last_input)) = server_state.and_then(|tick| {
            tick.your_last_input
                .as_ref()
                .map(|my_last_input| (tick, my_last_input))
        }) {
            info!("got {:?} at {:?}", my_last_input, tick_num);

            if let Some(record) = self.log.get_mut(my_last_input) {
                info!("applying correction at {:?}", my_last_input);
                Self::correct_prediction(&mut record.state, &server_state.state);
            }

            self.log = self
                .log
                .clone()
                .into_iter()
                .filter(|&(tick_num, _)| tick_num >= *my_last_input)
                .collect();
        }

        let last_state = if let Some((first_num, first_record)) = self.log.iter().next() {
            info!("replaying from {:?}", first_num);
            let mut last_state = first_record.state.clone();
            for record in self.log.values_mut().skip(1) {
                Self::run_tick(&mut last_state, self.my_player_id, &record.my_last_input);
                record.state = last_state.clone();
            }
            Some(last_state)
        } else if let Some(server_state) = server_state {
            Some(server_state.state.clone())
        } else {
            None
        };

        if let Some(mut state) = last_state {
            let events = Self::run_tick(&mut state, self.my_player_id, &my_input);
            //info!("inserting {:?}", state);
            self.log.insert(
                tick_num,
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
        false
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
        if let Err(e) = state.run_player_input(my_player_id, &my_input, &mut context) {
            // TODO: Simulation error handling on client side
            warn!("Simulation error: {:?}", e);
        }

        context.events
    }

    fn correct_prediction(predicted: &mut comn::Game, server: &comn::Game) {
        *predicted = server.clone();
    }
}
