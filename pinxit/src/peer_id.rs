use crate::{Error, Signable, Signature};
use ed25519_dalek::PublicKey;
use hex::FromHex;
use lazy_static::lazy_static;
use std::{
    collections::HashMap,
    fmt,
    hash::{Hash, Hasher},
    str::FromStr,
    sync::RwLock,
};

const PUBLIC_LEN: usize = ed25519_dalek::PUBLIC_KEY_LENGTH;

/// The unique identifier of a peer.
#[derive(Clone, Eq)]
pub struct PeerId(pub(crate) PublicKey);

impl Hash for PeerId {
    fn hash<H>(&self, h: &mut H)
    where
        H: Hasher,
    {
        self.0.as_bytes().hash(h)
    }
}

impl PartialEq for PeerId {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
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

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write_hex!(f, self.0.as_bytes(), PUBLIC_LEN)
    }
}

impl FromStr for PeerId {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

lazy_static! {
    static ref PEER_NAMES: RwLock<HashMap<PeerId, String>> = RwLock::new(HashMap::new());
}

impl PeerId {
    /// Create a peer id from it's hexadecimal representation.
    pub fn from_hex(hex: &str) -> Result<Self, Error> {
        let public: [u8; PUBLIC_LEN] = FromHex::from_hex(hex)?;
        let public = PublicKey::from_bytes(&public)?;
        Ok(Self(public))
    }

    /// Create a hexadecimal representation.
    #[must_use]
    pub fn hex(&self) -> String {
        hex::encode(&self.0.as_bytes())
    }

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
        let message = message.message().map_err(Error::signable_error)?;
        Ok(self.0.verify(message.as_ref(), &signature.0)?)
    }
}

const _: () = {
    use serde::{
        de::{Error, Unexpected},
        Deserialize, Deserializer, Serialize, Serializer,
    };

    impl Serialize for super::PeerId {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            self.0.as_bytes().serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for PeerId {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let peer_id: [u8; PUBLIC_LEN] = Deserialize::deserialize(deserializer)?;
            let peer_id = PublicKey::from_bytes(&peer_id).map_err(|_| {
                Error::invalid_value(Unexpected::Bytes(&peer_id), &"valid ed25519 public key")
            })?;
            Ok(Self(peer_id))
        }
    }
};
