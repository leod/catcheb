use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Serialize, Deserialize)]
pub struct SequenceNum(pub usize);

const INITIAL_ESTIMATE_MS: u64 = 100;
const PING_PERIOD_MS: u64 = 1_000;
const TIMEOUT_MS: u64 = 30_000;
const NUM_KEEP_DURATIONS: usize = 10;

#[derive(Debug, Clone)]
pub enum ReceivedPongError {
    InvalidSequenceNum,
}

pub struct PingEstimation {
    next_sequence_num: SequenceNum,
    waiting_pings: Vec<(SequenceNum, Instant)>,
    last_send_time: Option<Instant>,
    last_durations: VecDeque<Duration>,
    estimate: Duration,
}

impl Default for PingEstimation {
    fn default() -> Self {
        Self {
            next_sequence_num: SequenceNum(0),
            waiting_pings: Vec::new(),
            last_send_time: None,
            last_durations: VecDeque::new(),
            estimate: Duration::from_millis(INITIAL_ESTIMATE_MS),
        }
    }
}

impl PingEstimation {
    pub fn estimate(&self) -> Duration {
        self.estimate
    }

    pub fn next_ping_sequence_num(&mut self) -> Option<SequenceNum> {
        let now = Instant::now();

        if self.last_send_time.map_or(true, |last_time| {
            now - last_time > Duration::from_millis(PING_PERIOD_MS)
        }) {
            let sequence_num = self.next_sequence_num;
            self.last_send_time = Some(now);
            self.waiting_pings.push((sequence_num, now));

            self.next_sequence_num = SequenceNum(sequence_num.0 + 1);
            Some(sequence_num)
        } else {
            None
        }
    }

    pub fn received_pong(&mut self, num: SequenceNum) -> Result<(), ReceivedPongError> {
        if let Some((_, send_time)) = self
            .waiting_pings
            .iter()
            .find(|(send_num, _)| num == *send_num)
        {
            let now = Instant::now();
            assert!(now >= *send_time);

            self.last_durations.push_back(*send_time - now);
            while self.last_durations.len() > NUM_KEEP_DURATIONS {
                self.last_durations.pop_front();
            }

            // Due to the unreliable connection, it is possible that earlier
            // waiting pings have not been answered.
            self.waiting_pings.retain(|(send_num, _)| *send_num > num);

            Ok(())
        } else {
            Err(ReceivedPongError::InvalidSequenceNum)
        }
    }

    pub fn is_timeout(&self) -> bool {
        if let Some((_, send_time)) = self.waiting_pings.last() {
            (Instant::now() - *send_time) >= Duration::from_millis(TIMEOUT_MS)
        } else {
            // All our recent pings have been ponged, all good
            // (assuming that the user regularly calls next_ping_sequence_num)
            false
        }
    }

    fn calculate_estimate(&self) -> Duration {
        // TODO: Do some statistical thingy other than average for estimating
        // ping
        if self.last_durations.is_empty() {
            Duration::from_millis(INITIAL_ESTIMATE_MS)
        } else {
            let sum: f32 = self.last_durations.iter().map(Duration::as_secs_f32).sum();
            Duration::from_secs_f32(sum / self.last_durations.len() as f32)
        }
    }
}
