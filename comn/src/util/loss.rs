use std::collections::BTreeSet;

const NUM_KEEP_DURATION: usize = 1000;

#[derive(Default, Debug, Clone)]
pub struct LossEstimation {
    received: BTreeSet<usize>,
}

impl LossEstimation {
    pub fn record_received(&mut self, sequence_num: usize) {
        self.received = self
            .received
            .clone()
            .into_iter()
            .filter(|other_num| other_num + NUM_KEEP_DURATION >= sequence_num)
            .collect();

        self.received.insert(sequence_num);
    }

    pub fn estimate(&self) -> Option<f32> {
        let first = self.received.iter().next();
        let last = self.received.iter().next_back();

        if let (Some(&first), Some(&last)) = (first, last) {
            assert!(last >= first);
            if last - first > 20 {
                // If we include fisrt/last in the range, we will underestimate loss.
                let num_received = self
                    .received
                    .iter()
                    .filter(|&&num| num >= first + 10 && num + 10 <= last)
                    .count();
                let duration = last - first - 19;

                Some(1.0 - num_received as f32 / duration as f32)
            } else {
                None
            }
        } else {
            None
        }
    }
}
