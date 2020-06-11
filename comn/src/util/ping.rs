use std::collections::VecDeque;
use std::time::Duration;

use instant::Instant;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Serialize, Deserialize)]
pub struct SequenceNum(pub usize);

const INITIAL_ESTIMATE_MS: u64 = 100;
const PING_PERIOD_MS: u64 = 500;
const TIMEOUT_MS: u64 = 30_000;
const NUM_KEEP_DURATIONS: usize = 100;

#[derive(Debug, Clone)]
pub enum ReceivedPongError {
    InvalidSequenceNum,
}

#[derive(Debug, Clone)]
pub struct PingEstimation {
    next_sequence_num: SequenceNum,
    waiting_pings: Vec<(SequenceNum, Instant)>,
    last_send_time: Option<Instant>,
    last_received_pong_time: Instant,
    last_rtts: VecDeque<Duration>,
    estimate: Duration,
}

impl Default for PingEstimation {
    fn default() -> Self {
        Self {
            next_sequence_num: SequenceNum(0),
            waiting_pings: Vec::new(),
            last_send_time: None,
            last_received_pong_time: Instant::now(),
            last_rtts: VecDeque::new(),
            estimate: Duration::from_millis(INITIAL_ESTIMATE_MS),
        }
    }
}

impl PingEstimation {
    pub fn estimate(&self) -> Duration {
        self.estimate
    }

    pub fn next_ping_sequence_num(&mut self, now: Instant) -> Option<SequenceNum> {
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

    pub fn record_pong(
        &mut self,
        recv_time: Instant,
        num: SequenceNum,
    ) -> Result<(), ReceivedPongError> {
        if let Some((_, send_time)) = self
            .waiting_pings
            .iter()
            .find(|(send_num, _)| num == *send_num)
        {
            assert!(recv_time >= *send_time);

            self.last_received_pong_time = recv_time;

            self.last_rtts.push_back(recv_time - *send_time);
            while self.last_rtts.len() > NUM_KEEP_DURATIONS {
                self.last_rtts.pop_front();
            }
            self.estimate = self.calculate_estimate();

            // Due to the unreliable connection, it is possible that earlier
            // waiting pings have not been answered.
            self.waiting_pings.retain(|(send_num, _)| *send_num > num);

            Ok(())
        } else {
            Err(ReceivedPongError::InvalidSequenceNum)
        }
    }

    pub fn is_timeout(&self, now: Instant) -> bool {
        now - self.last_received_pong_time >= Duration::from_millis(TIMEOUT_MS)
    }

    fn calculate_estimate(&self) -> Duration {
        // TODO: Do some statistical thingy other than average for estimating
        // ping
        if self.last_rtts.is_empty() {
            Duration::from_millis(INITIAL_ESTIMATE_MS)
        } else {
            let sum: f32 = self.last_rtts.iter().map(Duration::as_secs_f32).sum();
            Duration::from_secs_f32(sum / self.last_rtts.len() as f32)
        }
    }
}
