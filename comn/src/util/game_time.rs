use std::{collections::VecDeque, iter};

use crate::{util::stats, GameTime};

const SAMPLE_DURATION: f32 = 2.0;

#[derive(Debug, Clone)]
pub struct GameTimeEstimation {
    recv_period: GameTime,
    recv_times: VecDeque<(f32, GameTime)>,
}

impl GameTimeEstimation {
    pub fn new(recv_period: GameTime) -> Self {
        Self {
            recv_period,
            recv_times: VecDeque::new(),
        }
    }

    pub fn record_tick(&mut self, recv_time: f32, game_time: GameTime) {
        if let Some((_last_recv_time, last_game_time)) = self.recv_times.back() {
            if game_time < *last_game_time {
                // Received packages out of order, just ignore
                return;
            }

            // I think the following assumption can be broken by `fake_bad_net`.
            //assert!(recv_time >= *last_recv_time);

            /*if recv_time < *last_recv_time {
                return;
            }*/
        }

        self.recv_times.push_back((recv_time, game_time));

        while let Some((first_recv_time, _)) = self.recv_times.front() {
            if recv_time - first_recv_time >= SAMPLE_DURATION {
                self.recv_times.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn recv_delay_std_dev(&self) -> Option<f32> {
        /*self.shifted_recv_times().map(|samples| {
            let samples: Vec<(f32, f32)> = samples.collect();
            let line = stats::linear_regression_with_beta(1.0, samples.iter().copied());

            let recv_delay = samples
                .iter()
                .map(|(delta_time, delta_game_time)| line.eval(*delta_time) - delta_game_time);

            stats::std_dev(recv_delay)
        })*/

        if !self.recv_times.is_empty() {
            Some(stats::std_dev(
                self.recv_times
                    .iter()
                    .zip(self.recv_times.iter().skip(1))
                    .map(|((recv_a, _), (recv_b, _))| recv_b - recv_a),
            ))
        } else {
            None
        }
    }

    pub fn has_started(&self) -> bool {
        !self.recv_times.is_empty()
    }

    pub fn estimate(&self, now: f32) -> Option<GameTime> {
        let mut recv_times = self
            .recv_times
            .iter()
            .filter_map(move |(recv_time, game_time)| {
                if now - recv_time >= SAMPLE_DURATION {
                    None
                } else {
                    Some((recv_time, game_time))
                }
            });

        recv_times.next().map(|(first_recv_time, first_game_time)| {
            let shifted_recv_times = iter::once((first_recv_time, first_game_time))
                .chain(recv_times)
                .map(|(recv_time, game_time)| {
                    (recv_time - first_recv_time, game_time - first_game_time)
                });

            let line = stats::linear_regression_with_beta(1.0, shifted_recv_times);
            let delta_recv_time = now - first_recv_time;
            let delta_game_time = line.eval(delta_recv_time);
            first_game_time + delta_game_time
        })
    }
}
