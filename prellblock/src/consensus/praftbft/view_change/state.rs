use super::RingBuffer;
use crate::{
    consensus::{LeaderTerm, SignatureList},
    if_monitoring,
};
use pinxit::{PeerId, Signature};
use std::{
    collections::HashMap,
    time::{Instant, SystemTime},
};

if_monitoring! {
    use lazy_static::lazy_static;
    use prometheus::{register_int_gauge, register_gauge, IntGauge, Gauge};
    lazy_static! {
        /// Measure the current LeaderTerm..
        static ref LEADER_TERM: IntGauge = register_int_gauge!(
            "praftbft_leader_term",
            "The current LeaderTerm."
        )
        .unwrap();

        /// Measure the time of the last change of the LeaderTerm.
        static ref LAST_LEADER_TERM_CHANGE: Gauge = register_gauge!(
            "praftbft_leader_term_change",
            "The time of the last change of the LeaderTerm."
        )
        .unwrap();
    }
}

#[derive(Debug)]
pub struct State {
    pub leader_term: LeaderTerm,
    pub new_view_time: Option<Instant>,
    pub current_signatures: Option<SignatureList>,
    pub future_signatures: RingBuffer<LeaderTerm, HashMap<PeerId, Signature>>,
}

impl State {
    pub fn new(size: usize) -> Self {
        if_monitoring!({
            LEADER_TERM.set(0);
        });
        Self {
            leader_term: LeaderTerm::default(),
            new_view_time: None,
            current_signatures: None,
            future_signatures: RingBuffer::new(HashMap::new(), size, LeaderTerm::default()),
        }
    }

    pub fn did_reach_supermajority(&mut self, new_leader_term: LeaderTerm) {
        assert!(new_leader_term > self.leader_term);

        self.future_signatures
            .increment_to(new_leader_term, HashMap::new());

        self.leader_term = new_leader_term;
        if_monitoring!({
            #[allow(clippy::cast_possible_wrap)]
            LEADER_TERM.set(u64::from(self.leader_term) as i64);
            println!("nlt: {}", new_leader_term);
            match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
                Ok(duration) => LAST_LEADER_TERM_CHANGE.set(duration.as_secs_f64()),
                Err(err) => log::warn!("Error getting time of last ViewChange: {}", err),
            }
        });
        self.new_view_time = Some(Instant::now());
        self.current_signatures = Some(
            self.future_signatures
                .increment(HashMap::new())
                .into_iter()
                .collect(),
        );
    }
}
