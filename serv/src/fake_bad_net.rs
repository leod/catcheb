use std::time::Duration;

use log::warn;
use rand::Rng;
use rand_distr::Distribution;

use futures::{pin_mut, prelude::Stream, select, FutureExt};
use tokio::{stream::StreamExt, sync::mpsc, time::DelayQueue};

use crate::webrtc::{MessageIn, MessageOut};

pub trait AddFakeLag {
    fn add_fake_lag(&mut self, lag: Duration);
}

impl AddFakeLag for MessageIn {
    fn add_fake_lag(&mut self, lag: Duration) {
        self.recv_time += lag;
    }
}

impl AddFakeLag for MessageOut {
    fn add_fake_lag(&mut self, _: Duration) {}
}

#[derive(Debug, Clone)]
pub struct Config {
    pub lag_mean: Duration,
    pub lag_std_dev: f32,
    pub loss: f32,
}

pub struct FakeBadNet<S: Stream> {
    config: Config,
    lag_distribution: rand_distr::Normal<f32>,
    orig_rx: S,
    new_tx: mpsc::UnboundedSender<S::Item>,
    delay_queue: DelayQueue<S::Item>,
}

impl<S: Stream> FakeBadNet<S>
where
    S::Item: AddFakeLag,
{
    pub fn new(config: Config, orig_rx: S, new_tx: mpsc::UnboundedSender<S::Item>) -> Self {
        let lag_distribution =
            rand_distr::Normal::new(config.lag_mean.as_secs_f32() * 1000.0, config.lag_std_dev)
                .unwrap();

        Self {
            config,
            lag_distribution,
            orig_rx,
            new_tx,
            delay_queue: DelayQueue::new(),
        }
    }

    pub async fn run(mut self) {
        let orig_rx = self.orig_rx;
        pin_mut!(orig_rx);

        loop {
            select! {
                message = orig_rx.next().fuse() => {
                    let mut rng = rand::thread_rng();

                    match message {
                        Some(mut message) => {
                            if rng.gen::<f32>() >= self.config.loss {
                                let lag = Duration::from_secs_f32(self.lag_distribution.sample(&mut rng) / 1000.0);
                                message.add_fake_lag(lag);
                                self.delay_queue.insert(message, lag);
                            }
                        }
                        None => {
                            // TODO: Can this happen in a case other than
                            // closed channel?
                        }
                    }
                }
                delayed_message = self.delay_queue.next().fuse() => {
                    match delayed_message {
                        Some(Ok(delayed_message)) => {
                            //debug!("got delayed message");
                            if self.new_tx.send(delayed_message.into_inner()).is_err() {
                                warn!("new_tx closed, terminating");
                                return;
                            }
                        }
                        Some(Err(err)) => {
                            // Not sure when this happens.
                            warn!("Error when reading DelayQueue: {:?}", err);
                        }
                        None => {
                            // Ok to ignore I think, stream will resume.

                            // Unfortunately, DelayQueue immediately produces
                            // None if the queue is empty. Thus, the following
                            // yield is apparently very important, so that the
                            // task does not become blocking by
                            // short-circuiting into this path. If we don't
                            // have this, messages are not received in the
                            // other select arm.
                            tokio::task::yield_now().await;
                        }
                    }
                }
            }
        }
    }
}
