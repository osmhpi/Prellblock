#![allow(clippy::pub_enum_variant_names)]

use err_derive::Error;
use std::error::Error as StdError;

type BoxError = Box<dyn StdError + Send + Sync + 'static>;

/// An error of the `pinxit` crate.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// An invalid signature was used.
    #[error(display = "Invalid signature: {}", 0)]
    SignatureError(#[error(from)] ed25519_dalek::SignatureError),

    /// A `Signable` failed to create a message.
    #[error(display = "Unable to create signable message: {}", 0)]
    SignableError(BoxError),
}

impl Error {
    pub(crate) fn signable_error(err: impl StdError + Send + Sync + 'static) -> Self {
        Self::SignableError(err.into())
    }
}
