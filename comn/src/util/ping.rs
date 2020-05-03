use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Serialize, Deserialize)]
pub struct SequenceNum(pub usize);

const INITIAL_ESTIMATE_MS: u64 = 100;
const PING_PERIOD_MS: u64 = 1000;

#[derive(Debug, Clone)]
pub enum ReceivedPongError {
    InvalidSequenceNum,
}

pub struct PingEstimation {
    next_sequence_num: SequenceNum,
    waiting_pings: Vec<(SequenceNum, Instant)>,
    last_send_time: Option<Instant>,
    last_durations: Vec<Duration>,
    estimate: Duration,
}

impl Default for PingEstimation {
    fn default() -> Self {
        Self {
            next_sequence_num: SequenceNum(0),
            waiting_pings: Vec::new(),
            last_send_time: None,
            last_durations: Vec::new(),
            estimate: Duration::from_millis(INITIAL_ESTIMATE_MS),
        }
    }
}

impl PingEstimation {
    pub fn estimate(&self) -> Duration {
        self.estimate
    }

    pub fn next_ping_sequence_num_if_it_is_time(&mut self) -> Option<SequenceNum> {
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
            // Due to the unreliable connection, it is possible that earlier
            // waiting pings have not been answered.
            self.waiting_pings.retain(|(send_num, _)| *send_num > num);

            Ok(())
        } else {
            Err(ReceivedPongError::InvalidSequenceNum)
        }
    }

    pub fn is_timeout(&self) -> bool {
        false
    }
}
