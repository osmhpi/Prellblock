use err_derive::Error;
use pinxit::PeerId;

/// An error of the `praftbft` consensus.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// The signature could not be verified.
    #[error(display = "{}", 0)]
    InvalidSignature(#[error(from)] pinxit::Error),

    /// The `Follower`'s leader_term is not equal to the received leader_term.
    #[error(display = "Follower is not in the correct Leader term.")]
    WrongLeaderTerm,

    /// The leader proposing the block is not the one the `Follower` saved (maybe there is no leader).
    #[error(display = "There is no leader.")]
    NoLeader,

    /// The Block Hash has changed between Phases.
    #[error(display = "The Block Hash has changed.")]
    ChangedBlockHash,

    /// The Block Hash is wrong.
    #[error(display = "The Block Hash is wrong.")]
    WrongBlockHash,

    /// The leader proposing the block is not the one the `Follower` saved (maybe there is no leader).
    #[error(display = "The RPU {} is not the current leader.", 0)]
    WrongLeader(PeerId),

    /// The current sequence number is already higher.
    #[error(display = "Sequence number is too low.")]
    SequenceNumberTooSmall,

    /// The current sequence number is different from the expected one.
    #[error(display = "Sequence number is wrong.")]
    WrongSequenceNumber,

    /// This peer is not allowed to take part in the consensus.
    #[error(
        display = "The RPU {} is not allowed to take part in the consensus.",
        0
    )]
    InvalidPeer(PeerId),

    #[error(display = "Not enough signatures.")]
    NotEnoughSignatures,
}
