use std::collections::VecDeque;

pub struct Var {
    max_num_samples: usize,
    recent_values: VecDeque<f32>,
}

impl Default for Var {
    fn default() -> Self {
        Var::new(100)
    }
}

impl Var {
    pub fn new(max_num_samples: usize) -> Self {
        Self {
            max_num_samples,
            recent_values: VecDeque::new(),
        }
    }

    pub fn record(&mut self, value: f32) {
        self.recent_values.push_back(value);

        if self.recent_values.len() > self.max_num_samples {
            self.recent_values.pop_front();
        }
    }

    pub fn mean(&self) -> Option<f32> {
        if self.recent_values.is_empty() {
            None
        } else {
            Some(self.recent_values.iter().sum::<f32>() / self.recent_values.len() as f32)
        }
    }
}
