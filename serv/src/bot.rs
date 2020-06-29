use rand::Rng;

#[derive(Debug, Clone, Default)]
pub struct Bot {
    last_input: comn::Input,
}

impl Bot {
    pub fn get_next_input(&mut self, _state: &comn::Game) -> comn::Input {
        let mut rng = rand::thread_rng();

        for (p, b) in &mut [
            (0.02, &mut self.last_input.move_left),
            (0.02, &mut self.last_input.move_right),
            (0.02, &mut self.last_input.move_up),
            (0.02, &mut self.last_input.move_down),
            (0.002, &mut self.last_input.use_item),
            (0.002, &mut self.last_input.use_action),
        ]
        .iter_mut()
        {
            let x: f32 = rng.gen();

            if x < *p {
                **b = !**b;
            }
        }

        self.last_input.clone()
    }
}
