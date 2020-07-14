use std::{convert::TryFrom, fmt};

const SIGNATURE_LEN: usize = ed25519_dalek::SIGNATURE_LENGTH;

/// The cryptographic signature of some `Signable` data.
#[derive(Clone, Eq, PartialEq)]
pub struct Signature(pub(crate) ed25519_dalek::Signature);

impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

hexutil::impl_hex!(Signature, SIGNATURE_LEN, |self| self.0.to_bytes(), |data| {
    ed25519_dalek::Signature::try_from(&data[..])
        .map(Self)
        .map_err(|_| hexutil::FromHexError::InvalidValue)
});
