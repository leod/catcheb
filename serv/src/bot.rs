use rand::Rng;

#[derive(Debug, Clone)]
pub enum Bot {
    Random {
        last_input: comn::Input,
    },
    LeftRight {
        duration: comn::GameTime,
        time_left: comn::GameTime,
        right: bool,
    },
}

impl Bot {
    pub fn random() -> Self {
        Bot::Random {
            last_input: comn::Input::default(),
        }
    }

    pub fn left_right(duration: comn::GameTime) -> Self {
        Bot::LeftRight {
            duration,
            time_left: duration,
            right: true,
        }
    }

    pub fn get_next_input(&mut self, state: &comn::Game) -> comn::Input {
        use Bot::*;

        match self {
            Random { last_input } => {
                let mut rng = rand::thread_rng();

                for (p, b) in &mut [
                    (0.02, &mut last_input.move_left),
                    (0.02, &mut last_input.move_right),
                    (0.02, &mut last_input.move_up),
                    (0.02, &mut last_input.move_down),
                    (0.002, &mut last_input.dash),
                    (0.002, &mut last_input.use_action),
                    (0.002, &mut last_input.shoot),
                ]
                .iter_mut()
                {
                    let x: f32 = rng.gen();

                    if x < *p {
                        **b = !**b;
                    }
                }

                last_input.clone()
            }
            LeftRight {
                duration,
                time_left,
                right,
            } => {
                *time_left -= state.settings.tick_period();
                if *time_left < 0.0 {
                    *time_left = *duration;
                    *right = !*right;
                }

                let mut result = comn::Input::default();
                if *right {
                    result.move_right = true;
                } else {
                    result.move_left = true;
                }

                result
            }
        }
    }
}
