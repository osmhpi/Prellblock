use super::RingBuffer;
use crate::consensus::{LeaderTerm, SignatureList};
use pinxit::{PeerId, Signature};
use std::{collections::HashMap, time::Instant};

#[derive(Debug)]
pub struct State {
    pub leader_term: LeaderTerm,
    pub new_view_time: Option<Instant>,
    pub current_signatures: Option<SignatureList>,
    pub future_signatures: RingBuffer<LeaderTerm, HashMap<PeerId, Signature>>,
}

impl State {
    pub fn new(size: usize) -> Self {
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
        self.new_view_time = Some(Instant::now());
        self.current_signatures = Some(
            self.future_signatures
                .increment(HashMap::new())
                .into_iter()
                .collect(),
        );
    }
}
