use std::{
    collections::{BTreeMap, VecDeque},
    time::Duration,
};

use instant::Instant;
use log::{debug, info, warn};

use comn::util::{diff::Diff, stats, GameTimeEstimation, LossEstimation, PingEstimation};

use crate::{prediction::Prediction, webrtc};

pub struct ReceivedState {
    pub game: comn::Game,
    pub my_last_input: Option<comn::TickNum>,
}

/// Statistics for debugging.
#[derive(Default)]
pub struct Stats {
    pub time_lag_ms: stats::Var,
    pub time_lag_deviation_ms: stats::Var,
    pub time_warp_factor: stats::Var,
    pub tick_interp: stats::Var,
    pub input_delay: stats::Var,
    pub received_ticks: stats::Var,
    pub recv_rate: f32,
    pub send_rate: f32,
    pub recv_delay_std_dev: f32,
    pub loss: LossEstimation,
    pub skip_loss: LossEstimation,
}

const MAX_TICKS_PER_UPDATE: usize = 5;
const KEEP_STATES_BUFFER: u32 = 5;

pub struct Game {
    settings: comn::Settings,
    my_token: comn::PlayerToken,
    my_player_id: comn::PlayerId,

    webrtc_client: webrtc::Client,

    last_inputs: VecDeque<(comn::TickNum, comn::Input)>,

    received_states: BTreeMap<comn::TickNum, ReceivedState>,
    received_events: BTreeMap<comn::TickNum, Vec<comn::Event>>,
    prediction: Option<Prediction>,

    interp_game_time: comn::GameTime,
    next_tick_num: Option<comn::TickNum>,

    start_time: Instant,

    recv_tick_time: GameTimeEstimation,
    next_time_warp_factor: f32,

    ping: PingEstimation,
    stats: Stats,
}

impl Game {
    pub fn new(join: comn::JoinSuccess, webrtc_client: webrtc::Client) -> Self {
        let prediction = Some(Prediction::new(join.your_player_id));
        let recv_tick_time = GameTimeEstimation::new(join.game_settings.tick_period());

        Self {
            settings: join.game_settings,
            my_token: join.your_token,
            my_player_id: join.your_player_id,
            webrtc_client,
            last_inputs: VecDeque::new(),
            received_states: BTreeMap::new(),
            received_events: BTreeMap::new(),
            prediction,
            interp_game_time: 0.0,
            next_tick_num: None,
            start_time: Instant::now(),
            recv_tick_time,
            next_time_warp_factor: 1.0,
            ping: PingEstimation::default(),
            stats: Stats::default(),
        }
    }

    pub fn my_player_id(&self) -> comn::PlayerId {
        self.my_player_id
    }

    pub fn is_good(&self) -> bool {
        self.webrtc_client.status() == webrtc::Status::Open
    }

    pub fn settings(&self) -> &comn::Settings {
        &self.settings
    }

    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    pub fn ping(&self) -> &PingEstimation {
        &self.ping
    }

    pub fn interp_game_time(&self) -> comn::GameTime {
        self.interp_game_time
    }

    fn target_time_lag(&self) -> comn::GameTime {
        self.settings.tick_period() * 1.5
    }

    fn tick_num(&self) -> comn::TickNum {
        comn::TickNum((self.interp_game_time / self.settings.tick_period()) as u32)
    }

    pub fn update(&mut self, now: Instant, dt: Duration, input: &comn::Input) -> Vec<comn::Event> {
        self.webrtc_client.set_now((Instant::now(), now));
        while let Some((recv_time, message)) = self.webrtc_client.take_message() {
            self.handle_message(recv_time, message);
        }

        if let Some(sequence_num) = self.ping.next_ping_sequence_num(now) {
            self.send(comn::ClientMessage::Ping(sequence_num));
        }

        // Determine new local game time, making sure to stay behind the receive
        // stream by our desired lag time. We do this so that we have ticks
        // between which we can interpolate.
        //
        // If we are off too far from our lag target, slow down or speed up
        // playback time.
        let new_interp_game_time =
            self.interp_game_time + dt.as_secs_f32() * self.next_time_warp_factor;

        // Don't let time run further than the ticks that we have received.
        // This is here so that we stop local time if the server drops or
        // starts lagging heavily.
        let max_tick_num = self
            .received_states
            .keys()
            .rev()
            .next()
            .copied()
            .unwrap_or(comn::TickNum(0))
            .max(self.tick_num())
            .max(self.next_tick_num.unwrap_or(comn::TickNum(0)));

        // Advance our playback time.
        let prev_tick_num = self.tick_num();
        self.interp_game_time =
            new_interp_game_time.min(self.settings.tick_game_time(max_tick_num));
        let new_tick_num = self.tick_num();

        // Determine the time warp factor to be used in the next update call.
        let time_since_start = now.duration_since(self.start_time).as_secs_f32();
        let recv_game_time = self.recv_tick_time.estimate(time_since_start);
        self.next_time_warp_factor = if let Some(recv_game_time) = recv_game_time {
            let current_time_lag = recv_game_time - self.interp_game_time;
            let time_lag_deviation = self.target_time_lag() - current_time_lag;

            self.stats
                .time_lag_deviation_ms
                .record(time_lag_deviation * 1000.0);

            /*let k = 0.5 + (2.0 - 0.5) / (1.0 + 2.0 * (time_lag_deviation / 0.05).exp());

            if time_lag_deviation > 0.0 {
                1.0 / k
            } else {
                k
            }*/

            0.5 + (2.0 - 0.5) / (1.0 + 2.0 * (time_lag_deviation / 0.05).exp())

        //0.5 * ((-time_lag_deviation).tanh() + 2.0)
        } else {
            0.0
        };

        // Look at all the intermediate ticks. We will have one of the
        // following cases:
        //
        // 1. In this update call, the tick number did not change, so
        //    `prev_tick_num == new_tick_num`.
        // 2. We crossed one tick, e.g. prev_tick_num is 7 and new_tick_num is
        //    8.
        // 3. We crossed more than one tick. This should happen only on lag
        //    spikes, be it local or in the network.
        let mut crossed_tick_nums: Vec<comn::TickNum> = (prev_tick_num.0 + 1..=new_tick_num.0)
            .map(|i| comn::TickNum(i))
            .collect();

        if crossed_tick_nums.len() > MAX_TICKS_PER_UPDATE {
            // It's possible that we have a large jump in ticks, e.g. due to a
            // lag spike, or because we are running in a background tab. In this
            // case, we don't want to overload ourselves by sending many input
            // packets and performing prediction over many ticks. Instead, we
            // just jump directly to the last couple of ticks.
            info!("Crossed {} ticks, will skip", crossed_tick_nums.len());

            // TODO: In order to nicely reinitialize prediction, we should take
            // those crossed ticks for which we actually received a server
            // state...
            crossed_tick_nums.drain(0..crossed_tick_nums.len() - MAX_TICKS_PER_UPDATE);
            assert!(crossed_tick_nums.len() == MAX_TICKS_PER_UPDATE);
        }

        // Iterate over all the ticks that we have crossed, also including
        // those for which we did not anything from the server.
        let mut events = Vec::new();

        for tick_num in crossed_tick_nums.iter() {
            // For debugging, keep track of how many ticks we do not
            // receive server data on time.
            if let Some(_) = self.received_states.get(tick_num) {
                self.stats.skip_loss.record_received(tick_num.0 as usize);
            }

            // Start server events of crossed ticks.
            if let Some(tick_events) = self.received_events.get(tick_num) {
                events.extend(tick_events.clone().into_iter());
                self.received_events.remove(tick_num);
            }

            // Send inputs for server ticks we cross.
            self.last_inputs.push_back((*tick_num, input.clone()));
            while self.last_inputs.len() > comn::MAX_INPUTS_PER_MESSAGE {
                self.last_inputs.pop_front();
            }

            self.send(comn::ClientMessage::Input(
                self.last_inputs.iter().cloned().collect(),
            ));

            // Predict effects of our own input locally.
            if let Some(prediction) = self.prediction.as_mut() {
                prediction.record_tick_input(
                    *tick_num,
                    input.clone(),
                    self.received_states.get(tick_num),
                );
            }
        }

        if self.next_tick_num <= Some(self.tick_num()) {
            // We have reached the tick that we were interpolating into, so
            // we'll need to look for the next interpolation target.
            self.next_tick_num = None;
        }

        // Do we have a tick to interpolate into ready?
        if self.next_tick_num.is_none() {
            let min_ready_num = self.received_states.keys().find(|tick_num| {
                **tick_num > self.tick_num() && tick_num.0 - self.tick_num().0 <= 3
            });

            if let Some(min_ready_num) = min_ready_num {
                self.next_tick_num = Some(*min_ready_num);
            }
        }

        // Remove events for older ticks, we will no longer need them. Note,
        // however, that the same cannot be said about the received states,
        // since we may still need them as the basis for delta decoding.
        {
            let remove_tick_nums: Vec<comn::TickNum> = self
                .received_events
                .keys()
                .copied()
                .filter(|tick_num| *tick_num < self.tick_num())
                .collect();

            for tick_num in remove_tick_nums {
                self.received_events.remove(&tick_num);
            }
        }

        // Keep some statistics for debugging...
        if let Some(recv_game_time) = recv_game_time {
            self.stats
                .time_lag_ms
                .record((recv_game_time - self.interp_game_time) * 1000.0);
        } else {
            // We cannot estimate the server time, so we probably disconnected
            // or just connected.
            self.stats.time_lag_ms = stats::Var::default();
        }

        self.stats
            .tick_interp
            .record(self.next_tick_num.map_or(0.0, |next_tick_num| {
                (next_tick_num.0 - self.tick_num().0) as f32
            }));
        self.stats
            .time_warp_factor
            .record(self.next_time_warp_factor);

        self.stats.send_rate = self.webrtc_client.send_rate();
        self.stats.recv_rate = self.webrtc_client.recv_rate();
        self.stats.recv_delay_std_dev = self.recv_tick_time.recv_delay_std_dev().unwrap_or(-1.0);

        events
    }

    pub fn state(&self) -> Option<&comn::Game> {
        if let Some(prediction) = self.prediction.as_ref() {
            prediction.predicted_state(self.tick_num())
        } else {
            // TODO: If prediction is disabled, loss in tick packages leads to
            // obvious flickering, since states will be None.
            self.received_states
                .get(&self.tick_num())
                .map(|state| &state.game)
        }
    }

    pub fn next_entities(&self) -> BTreeMap<comn::EntityId, (comn::GameTime, comn::Entity)> {
        let mut entities = BTreeMap::new();

        if let Some((recv_tick_num, recv_state)) = self
            .next_tick_num
            .and_then(|key| self.received_states.get(&key).map(|value| (key, value)))
        {
            let recv_game_time = self.settings.tick_game_time(recv_tick_num);

            entities.extend(
                recv_state
                    .game
                    .entities
                    .clone()
                    .into_iter()
                    .map(|(entity_id, entity)| (entity_id, (recv_game_time, entity))),
            );

            if let Some((prediction, predicted_state)) = self.prediction.as_ref().and_then(|p| {
                p.predicted_state(self.tick_num().next())
                    .map(|state| (p, state))
            }) {
                entities.extend(
                    predicted_state
                        .entities
                        .clone()
                        .into_iter()
                        .filter(|(_, entity)| prediction.is_predicted(entity))
                        .map(|(entity_id, entity)| {
                            (
                                entity_id,
                                (self.settings.tick_game_time(self.tick_num().next()), entity),
                            )
                        }),
                );
            }
        }

        entities
    }

    fn handle_message(&mut self, recv_time: Instant, message: comn::ServerMessage) {
        match message {
            comn::ServerMessage::Ping(_) => {
                // Handled in on_message callback to get better ping
                // estimates.
            }
            comn::ServerMessage::Pong(sequence_num) => {
                if self.ping.record_pong(recv_time, sequence_num).is_err() {
                    debug!("Ignoring out-of-order pong {:?}", sequence_num);
                } else {
                    //debug!("Received pong -> estimation {:?}", self.ping.estimate());
                }
            }
            comn::ServerMessage::Tick(tick) => {
                let recv_tick_num = tick.diff.tick_num;
                let recv_game_time = self.settings.tick_game_time(recv_tick_num);

                // Keep some statistics for debugging...
                self.stats.loss.record_received(recv_tick_num.0 as usize);
                if let Some(my_last_input) = tick.your_last_input.as_ref() {
                    self.stats
                        .input_delay
                        .record((recv_tick_num.0 - my_last_input.0) as f32 - 1.0);
                }

                if recv_game_time < self.interp_game_time {
                    debug!(
                        "Ignoring old tick of time {} vs our interp_game_time={}",
                        recv_game_time, self.interp_game_time,
                    );
                    return;
                }

                if !self.recv_tick_time.has_started() {
                    // If this is the first tick we have recevied from the server, reset
                    // to the correct time
                    self.interp_game_time = recv_game_time;

                    // TODO: Run events?

                    info!("Starting tick stream at recv_game_time={}", recv_game_time);
                }

                let mut new_state = if let Some(diff_base_num) = tick.diff_base {
                    // This tick has been delta encoded w.r.t. some tick
                    // that we have acknowledged receiving.
                    let received_state = if let Some(received_state) =
                        self.received_states.get(&diff_base_num)
                    {
                        received_state.game.clone()
                    } else {
                        // This should only happen if packets are severely
                        // reordered and delayed.
                        warn!(
                            "Received state {:?} encoded w.r.t. tick num {:?}, which we do not have (our oldest is {:?})",
                            recv_tick_num,
                            diff_base_num,
                            self.received_states.keys().next(),
                        );
                        return;
                    };

                    // The fact that we received a tick encoded w.r.t. this
                    // base, it means that we can forgot any older ticks --
                    // the server will never again send a new tick encoded
                    // w.r.t. an older tick.
                    //
                    // However, there may still be some packets in transit
                    // that rely on those older tickets. Thus, we add some
                    // delta here to keep a few more states around.
                    let remove_state_nums: Vec<comn::TickNum> = self
                        .received_states
                        .keys()
                        .copied()
                        .filter(|tick_num| tick_num.0 + KEEP_STATES_BUFFER < diff_base_num.0)
                        .collect();

                    for tick_num in remove_state_nums {
                        self.received_states.remove(&tick_num);
                    }

                    received_state
                } else {
                    // The state is encoded from scratch.
                    comn::Game::new(self.settings.clone())
                };

                {
                    let cur_tick_num = self.tick_num();
                    self.received_events.extend(
                        tick.events
                            .into_iter()
                            .filter(|(tick_num, _)| *tick_num > cur_tick_num),
                    );
                }

                if let Err(e) = tick.diff.apply(&mut new_state) {
                    warn!(
                        "Failed to delta decode tick {:?}, ignoring: {:?}",
                        recv_tick_num, e
                    );
                } else {
                    // Statistics for debugging...
                    if !self.received_states.contains_key(&recv_tick_num) {
                        self.stats.received_ticks.record(1.0);
                    }

                    self.received_states.insert(
                        recv_tick_num,
                        ReceivedState {
                            game: new_state,
                            my_last_input: tick.your_last_input,
                        },
                    );

                    // Let the server know which ticks we actually received, so
                    // that this can be used as the basis for delta encoding.
                    self.send(comn::ClientMessage::AckTick(recv_tick_num));

                    // Keep updating our estimate for when we expect to receive
                    // ticks. This is an attempt to counter network jitter.
                    let time_since_start = recv_time.duration_since(self.start_time).as_secs_f32();
                    self.recv_tick_time
                        .record_tick(time_since_start, recv_game_time);
                }
            }
        }
    }

    fn send(&self, message: comn::ClientMessage) {
        let signed_message = comn::SignedClientMessage(self.my_token, message);

        let data = signed_message.serialize();

        if let Err(err) = self.webrtc_client.send(&data) {
            warn!("Failed to send message: {:?}", err);
        }
    }
}
