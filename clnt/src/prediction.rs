use std::collections::BTreeMap;

#[derive(Debug, Clone)]
struct Record {
    state: comn::Game,
    last_input: comn::Input,
    new_entities: Vec<comn::EntityId>,
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
    ) {
        if let Some((server_state, my_last_input)) = server_state.and_then(|tick| {
            tick.your_last_input
                .as_ref()
                .map(|my_last_input| (tick, my_last_input))
        }) {
            if let Some(record) = self.log.get_mut(my_last_input) {
                Self::correct_prediction(&mut record.state.entities, &server_state.state.entities);
            }

            self.log.insert(
                tick_num,
                Record {
                    state: server_state.state.clone(),
                    last_input: my_input.clone(),
                    new_entities: Vec::new(),
                },
            );

            self.log = self
                .log
                .clone()
                .into_iter()
                .filter(|&(tick_num, _)| tick_num > *my_last_input)
                .collect();
        }
    }

    pub fn predicted_state(&self, tick_num: comn::TickNum) -> Option<&comn::Game> {
        self.log.get(&tick_num).map(|record| &record.state)
    }

    pub fn is_predicted(&self, entity: &comn::Entity) -> bool {
        false
    }

    fn correct_prediction(predicted_state: &mut comn::EntityMap, server_state: &comn::EntityMap) {}
}
