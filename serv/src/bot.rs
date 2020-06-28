use rand::Rng;

#[derive(Debug, Clone, Default)]
pub struct Bot {
    last_input: comn::Input,
}

impl Bot {
    pub fn get_next_input(&mut self, _state: &comn::Game) -> comn::Input {
        let mut rng = rand::thread_rng();

        for b in &mut [
            &mut self.last_input.move_left,
            &mut self.last_input.move_right,
            &mut self.last_input.move_up,
            &mut self.last_input.move_down,
            &mut self.last_input.use_item,
            &mut self.last_input.use_action,
        ]
        .iter_mut()
        {
            let x: f32 = rng.gen();

            if x < 0.02 {
                **b = !**b;
            }
        }

        self.last_input.clone()
    }
}
