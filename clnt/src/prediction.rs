use std::collections::BTreeMap;

struct Record {
    state: comn::EntityMap,
    my_input: comn::Input,
}

#[derive(Default)]
pub struct Prediction {
    log: BTreeMap<comn::TickNum, Record>,
}

impl Prediction {
    pub fn record_tick_input(
        &mut self,
        tick_num: comn::TickNum,
        my_state: &mut comn::Game,
        my_input: comn::Input,
        server_state: Option<&comn::Tick>,
    ) {
    }
}
