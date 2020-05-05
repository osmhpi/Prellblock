//! Module to check permissions of transactions.

use err_derive::Error;

/// An error of the `block_storage` module.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// The `Block` could not be stored correctly.
    #[error(display = "{}", 0)]
    Sled(#[error(from)] sled::Error),

    /// The `Block` hash does not match the previous block hash.
    #[error(display = "Block hash does not match the previous block hash.")]
    BlockHashDoesNotMatch,

    /// The `Block` height does not fit the previous block height.
    #[error(display = "Block height does not fit the previous block height.")]
    BlockHeightDoesNotFit,

    /// The `Block` could not be encoded correctly.
    #[error(display = "{}", 0)]
    Encoding(#[error(from)] postcard::Error),
}
