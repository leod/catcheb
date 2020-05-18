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

pub fn mean(samples: impl Iterator<Item = f32>) -> f32 {
    let samples: Vec<f32> = samples.collect();

    samples.iter().sum::<f32>() / samples.len() as f32
}

pub fn std_dev(samples: impl Iterator<Item = f32>) -> f32 {
    let samples: Vec<f32> = samples.collect();

    let avg = mean(samples.iter().copied());
    let variance = mean(samples.into_iter().map(|x| (x - avg).powi(2)));

    variance.sqrt()
}

/// Simple linear regression:
///
///     y(x) = alpha + beta * x
///
/// where:
///
///     alpha = avg(y) - beta * avg_x
pub struct LinearRegression {
    pub alpha: f32,
    pub beta: f32,
}

impl LinearRegression {
    pub fn eval(&self, x: f32) -> f32 {
        self.alpha + self.beta * x
    }
}

pub fn linear_regression_with_beta(
    beta: f32,
    samples: impl Iterator<Item = (f32, f32)>,
) -> LinearRegression {
    let samples: Vec<(f32, f32)> = samples.collect();
    let avg_x = mean(samples.iter().map(|(x, _)| x).copied());
    let avg_y = mean(samples.iter().map(|(_, y)| y).copied());
    let alpha = avg_y - beta * avg_x;

    LinearRegression { alpha, beta }
}
