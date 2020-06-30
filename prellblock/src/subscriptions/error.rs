//! Errors for `thingsboard` exporting.

use err_derive::Error;

/// An error of the `praftbft` consensus.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// The environment variable for the thingsboard tenant id was not set.
    #[error(display = "The environment variable for the thingsboard tenant id was not set.")]
    ThingsboardTenantIdNotSet,

    /// An error occurred while parsing to json.
    #[error(display = "{}", 0)]
    SerdeJson(#[error(from)] serde_json::error::Error),

    /// An error occurred while sending an HTTP request.
    #[error(display = "{}", 0)]
    Reqwest(#[error(from)] reqwest::Error),
}
