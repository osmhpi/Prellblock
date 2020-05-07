use super::state::Phase;
use crate::{
    block_storage,
    consensus::{BlockHash, BlockNumber, LeaderTerm},
    transaction_checker::PermissionError,
};
use err_derive::Error;
use pinxit::PeerId;

/// An error of the `praftbft` consensus.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// A Message was received that was not expected.
    #[error(display = "An unexpected message was received.")]
    UnexpectedMessage,

    /// A Response was received that was not expected.
    #[error(display = "An unexpected response was received.")]
    UnexpectedResponse,

    // ----------------------------------------------------------------
    // Errors from underlying components.
    // ----------------------------------------------------------------
    /// An Error occured while sending data over the network.
    #[error(display = "{}", 0)]
    Balise(#[error(from)] balise::Error),

    /// The Block could not be written to the BlockStorage.
    #[error(display = "{}", 0)]
    BlockStorage(#[error(from)] block_storage::Error),

    /// The Client does not have the correct permissions.
    #[error(display = "{}", 0)]
    Permission(#[error(from)] PermissionError),

    // ----------------------------------------------------------------
    // Errors with signatures.
    // ----------------------------------------------------------------
    /// The messegae does not contain enough signatures.
    #[error(display = "Not enough signatures.")]
    NotEnoughSignatures,

    /// There where duplicates of the same signature found.
    #[error(display = "A Signature is duplicated.")]
    DuplicateSignatures,

    /// The signature could not be verified.
    #[error(display = "{}", 0)]
    InvalidSignature(#[error(from)] pinxit::Error),

    // ----------------------------------------------------------------
    // Errors with the leader / consensus RPUs.
    // ----------------------------------------------------------------
    /// The `Follower`'s leader_term is not equal to the received leader_term.
    #[error(display = "Follower is not in the correct Leader term.")]
    WrongLeaderTerm,

    /// The leader proposing the block is not the one the `Follower` saved (maybe there is no leader).
    #[error(display = "The RPU {} is not the current leader.", 0)]
    WrongLeader(PeerId),

    /// The leader proposing the block is not the one the `Follower` saved (maybe there is no leader).
    #[error(display = "There is no leader.")]
    NoLeader,

    /// This peer is not allowed to take part in the consensus.
    #[error(
        display = "The RPU {} is not allowed to take part in the consensus.",
        0
    )]
    InvalidPeer(PeerId),

    // ----------------------------------------------------------------
    // Errors with wrong message content.
    // ----------------------------------------------------------------
    /// The Leader tried to propose an empty block.
    #[error(display = "The proposed Block is empty.")]
    EmptyBlock,

    // ----------------------------------------------------------------
    // Errors with the block hash.
    // ----------------------------------------------------------------
    /// The Block Hash has changed between Phases.
    #[error(display = "The Block Hash has changed.")]
    ChangedBlockHash,

    /// The Block Hash is wrong.
    #[error(display = "The sent BlockHash does not match the hash of the block.")]
    BlockNotMatchingHash,

    /// The `BlockHash` does not match the expected `BlockHash`.
    #[error(
        display = "The BlockHash {} does not match the expected previous BlockHash {}.",
        0,
        1
    )]
    PrevBlockHashDoesNotMatch(BlockHash, BlockHash),

    // ----------------------------------------------------------------
    // Errors with the block number.
    // ----------------------------------------------------------------
    /// The current block number was already higher.
    #[error(display = "Block number {} is too low.", 0)]
    BlockNumberTooSmall(BlockNumber),

    /// The request for the block number could not be processed
    /// because it is too far in the future.
    #[error(display = "Block number {} is too big.", 0)]
    BlockNumberTooBig(BlockNumber),

    /// The `BlockNumber` does not match the expected `BlockNumber` (previous + 1).
    #[error(
        display = "The BlockNumber {} does not match the expected BlockNumber {}.",
        0,
        1
    )]
    PrevBlockNumberDoesNotMatch(BlockNumber, BlockNumber),

    // ----------------------------------------------------------------
    // Errors with the internal state
    // ----------------------------------------------------------------
    /// The current leader term was already higher.
    #[error(display = "Request ViewChange to term {} failed: term too low.", 0)]
    LeaderTermTooSmall(LeaderTerm),

    /// The request for the leader term could not be processed
    /// because it is too far in the future.
    #[error(display = "Request ViewChange to term {} failed: term too high.", 0)]
    LeaderTermTooBig(LeaderTerm),

    /// The state for the round with the sent block number was in a false phase.
    #[error(
        display = "Expected to be in {:?} phase but was in {:?} phase.",
        expected,
        current
    )]
    WrongPhase {
        current: PhaseName,
        expected: PhaseName,
    },
}

#[derive(Debug)]
pub enum PhaseName {
    Waiting,
    Prepare,
    Append,
    Commited,
}

impl Phase {
    /// Convert a phase to the corresponding `PhaseName`.
    pub(super) fn to_phase_name(&self) -> PhaseName {
        match self {
            Self::Waiting => PhaseName::Waiting,
            Self::Prepare(..) => PhaseName::Prepare,
            Self::Append(..) => PhaseName::Append,
            Self::Committed(..) => PhaseName::Commited,
        }
    }
}
