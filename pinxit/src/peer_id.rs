use crate::{Error, Signable, Signature};
use ed25519_dalek::{PublicKey, Verifier};
use lazy_static::lazy_static;
use std::{
    collections::HashMap,
    fmt,
    hash::{Hash, Hasher},
    sync::RwLock,
};

const PUBLIC_LEN: usize = ed25519_dalek::PUBLIC_KEY_LENGTH;

/// The unique identifier of a peer.
#[derive(Clone, PartialEq, Eq)]
pub struct PeerId(pub(crate) PublicKey);

#[allow(clippy::derive_hash_xor_eq)]
impl Hash for PeerId {
    fn hash<H>(&self, h: &mut H)
    where
        H: Hasher,
    {
        self.0.as_bytes().hash(h)
    }
}

impl fmt::Debug for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match PEER_NAMES
            .read()
            .ok()
            .as_ref()
            .and_then(|names| names.get(self))
        {
            Some(name) => f.write_str(name),
            None => fmt::Display::fmt(self, f),
        }
    }
}

hexutil::impl_hex!(PeerId, PUBLIC_LEN, |&self| self.0.as_bytes(), |data| {
    PublicKey::from_bytes(&data)
        .map(Self)
        .map_err(|_| hexutil::FromHexError::InvalidValue)
});

lazy_static! {
    static ref PEER_NAMES: RwLock<HashMap<PeerId, String>> = RwLock::new(HashMap::new());
}

impl PeerId {
    /// Set an alias `name` for this `PeerId`.
    ///
    /// This `name` will be used when this peer id is printed with `std::fmt::Debug`.
    pub fn set_name(self, name: &impl ToString) {
        PEER_NAMES.write().unwrap().insert(self, name.to_string());
    }

    /// Verify a `signature` of a `message` that implements `Signable`.
    pub fn verify<S>(&self, message: S, signature: &Signature) -> Result<(), Error>
    where
        S: Signable,
    {
        let data = message.signable_data().map_err(Error::signable_error)?;
        Ok(self.0.verify(data.as_ref(), &signature.0)?)
    }

    /// Get a reference to a binary representation.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}
