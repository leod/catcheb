use std::{collections::VecDeque, fmt, time::Duration};

use instant::Instant;

#[derive(Debug, Clone)]
pub struct Var {
    sample_duration: Duration,
    records: VecDeque<(Instant, f32)>,
}

impl Default for Var {
    fn default() -> Self {
        Var::new(Duration::from_secs(1))
    }
}

impl fmt::Display for Var {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:>7.2} {:>7.2} {:>7.2} {:>7.2}",
            self.mean().unwrap_or(0.0 / 0.0),
            self.std_dev().unwrap_or(0.0 / 0.0),
            self.min().unwrap_or(0.0 / 0.0),
            self.max().unwrap_or(0.0 / 0.0),
        )
    }
}

impl Var {
    pub fn new(sample_duration: Duration) -> Self {
        Self {
            sample_duration,
            records: VecDeque::new(),
        }
    }

    pub fn record(&mut self, value: f32) {
        let now = Instant::now();

        self.records.push_back((now, value));

        while let Some((time, _)) = self.records.front() {
            if now.duration_since(*time) > self.sample_duration {
                self.records.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn recent_values(&self) -> impl Iterator<Item = f32> + '_ {
        self.records.iter().map(|(_, value)| *value)
    }

    pub fn mean(&self) -> Option<f32> {
        if self.records.is_empty() {
            None
        } else {
            Some(mean(self.recent_values()))
        }
    }

    pub fn std_dev(&self) -> Option<f32> {
        if self.records.is_empty() {
            None
        } else {
            Some(std_dev(self.recent_values()))
        }
    }

    pub fn min(&self) -> Option<f32> {
        if self.records.is_empty() {
            None
        } else {
            Some(self.recent_values().fold(0.0 / 0.0, f32::min))
        }
    }

    pub fn max(&self) -> Option<f32> {
        if self.records.is_empty() {
            None
        } else {
            Some(self.recent_values().fold(0.0 / 0.0, f32::max))
        }
    }

    pub fn sum_per_sec(&self) -> Option<f32> {
        if let Some((first_time, _)) = self.records.front() {
            let sum = self.recent_values().sum::<f32>();
            Some(sum / Instant::now().duration_since(*first_time).as_secs_f32())
        } else {
            None
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
