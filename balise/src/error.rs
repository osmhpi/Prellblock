#![allow(clippy::pub_enum_variant_names)]

use err_derive::Error;

/// An error of the `pinxit` crate.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// Timeout: Could not send request.
    #[error(display = "Timeout: Could not send request.")]
    Timeout,

    /// The message is too loong.
    #[error(display = "The message is too long.")]
    MessageTooLong,

    /// An IO error.
    #[error(display = "{}", 0)]
    IO(#[error(from)] std::io::Error),

    /// An encoding error.
    #[error(display = "{}", 0)]
    Encoding(#[error(from)] postcard::Error),

    /// A tls error.
    #[error(display = "{}", 0)]
    Tls(#[error(from)] native_tls::Error),

    /// A serverside error.
    #[error(display = "Server: {}", 0)]
    Server(#[error(from)] String),

    /// Any error :D.
    #[error(display = "{}", 0)]
    BoxError(#[error(from)] crate::BoxError),
}
