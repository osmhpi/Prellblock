use serde::{Deserialize, Serialize};
use std::{
    convert::TryInto,
    fmt,
    ops::{Add, AddAssign, Sub, SubAssign},
};

/// Number of the Block in the Blockchain.
#[derive(
    Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct BlockNumber(u64);

impl BlockNumber {
    /// Return the stored integer as a byte array.
    #[must_use]
    pub fn to_be_bytes(self) -> impl AsRef<[u8]> {
        self.0.to_be_bytes()
    }

    /// Create a `BlockNumber` from a byte slice.
    #[must_use]
    pub fn from_be_bytes(bytes: impl AsRef<[u8]>) -> Option<Self> {
        let bytes = bytes.as_ref().try_into().ok()?;
        Some(Self(u64::from_be_bytes(bytes)))
    }
}

impl fmt::Display for BlockNumber {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Add<u64> for BlockNumber {
    type Output = Self;
    fn add(self, other: u64) -> Self {
        Self(self.0 + other)
    }
}

impl AddAssign<u64> for BlockNumber {
    fn add_assign(&mut self, other: u64) {
        self.0 += other;
    }
}

impl Sub<u64> for BlockNumber {
    type Output = Self;
    fn sub(self, other: u64) -> Self {
        Self(self.0 - other)
    }
}

impl SubAssign<u64> for BlockNumber {
    fn sub_assign(&mut self, other: u64) {
        self.0 -= other;
    }
}

impl From<BlockNumber> for u64 {
    fn from(v: BlockNumber) -> Self {
        v.0
    }
}
