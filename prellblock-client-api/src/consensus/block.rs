use super::{BlockNumber, LeaderTerm, SignatureList};
use crate::Transaction;
use blake2::{
    digest::{generic_array::typenum::Unsigned, FixedOutput},
    Blake2b, Digest,
};
use pinxit::Signed;
use serde::{Deserialize, Serialize};
use std::{fmt, time::SystemTime};

/// A `Block` stores transactions verified by the blockchain.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    /// The `Body` of a block. (Everything that is signed)
    pub body: Body,
    /// The list of append signatures that accepted the body.
    pub signatures: SignatureList,
}

impl Block {
    /// Hash the `Block`. This will call `hash()` on it's `Body`.
    #[must_use]
    pub fn hash(&self) -> BlockHash {
        self.body.hash()
    }

    /// Return the `Block`s block number.
    #[must_use]
    pub const fn block_number(&self) -> BlockNumber {
        self.body.height
    }
}

/// The `Body` of a `Block` stores the Block number (height in chain), the Hash of the previous `Block`
/// and an Array of the actual `Transaction`s with their corresponding Signature in the `Block`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Body {
    /// The `LeaderTerm` of the `Block`.
    pub leader_term: LeaderTerm,

    /// The `BlockNumber` of the `Block`.
    pub height: BlockNumber,

    /// The `BlockHash` of the previous `Block`.
    pub prev_block_hash: BlockHash,

    /// The time, the leader proposed this block.
    pub timestamp: SystemTime,

    /// The actual data (`Signed<Transactions>`).
    pub transactions: Vec<Signed<Transaction>>,
}

impl Body {
    /// Calculate the hash of the blocks body.
    #[must_use]
    pub fn hash(&self) -> BlockHash {
        let val = postcard::to_stdvec(self).unwrap();

        let result = Blake2b::digest(&val);

        let mut body_hash = BlockHash([0; HASH_SIZE]);
        body_hash.0.copy_from_slice(&result);
        body_hash
    }
}

const HASH_SIZE: usize = <Blake2b as FixedOutput>::OutputSize::USIZE;

/// The datatype of hashes of blocks is `BlockHash`.
#[derive(Copy, Clone)]
#[allow(clippy::module_name_repetitions)]
pub struct BlockHash([u8; HASH_SIZE]);

impl fmt::Debug for BlockHash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Default for BlockHash {
    fn default() -> Self {
        Self([0; HASH_SIZE])
    }
}

impl PartialEq for BlockHash {
    fn eq(&self, other: &Self) -> bool {
        self.0[..] == other.0[..]
    }
}

impl Eq for BlockHash {}

hexutil::impl_hex!(BlockHash, HASH_SIZE, |&self| &self.0, |data| {
    Ok(Self(data))
});
