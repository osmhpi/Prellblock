use serde::{Deserialize, Serialize};
use std::{
    fmt,
    ops::{Add, AddAssign},
};

/// Number indicating the current Leader.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LeaderTerm(u64);

impl fmt::Display for LeaderTerm {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Add<u64> for LeaderTerm {
    type Output = Self;
    fn add(self, other: u64) -> Self {
        Self(self.0 + other)
    }
}

impl AddAssign<u64> for LeaderTerm {
    fn add_assign(&mut self, other: u64) {
        self.0 += other
    }
}

impl From<LeaderTerm> for u64 {
    fn from(v: LeaderTerm) -> Self {
        v.0
    }
}
