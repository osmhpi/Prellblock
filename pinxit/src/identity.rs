use crate::{Error, PeerId, Signable, Signature};
use ed25519_dalek::{ExpandedSecretKey, SecretKey};
use std::{fmt, str};

const SECRET_LEN: usize = ed25519_dalek::SECRET_KEY_LENGTH;

/// A cryptographic identity contains a public and private key to sign messages.
pub struct Identity {
    id: PeerId,
    secret: SecretKey,
}

impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Identity").field("id", &self.id).finish()
    }
}

impl Clone for Identity {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            secret: SecretKey::from_bytes(self.secret.as_bytes()).unwrap(),
        }
    }
}

hexutil::impl_hex!(
    Identity,
    SECRET_LEN,
    |&self| self.secret.as_bytes(),
    |data| {
        SecretKey::from_bytes(&data)
            .map(Self::from_secret_key)
            .map_err(|_| hexutil::FromHexError::InvalidValue)
    }
);

impl Identity {
    pub(crate) fn from_secret_key(secret: SecretKey) -> Self {
        let id = PeerId((&secret).into());
        Self { id, secret }
    }

    /// Generate a new random identity.
    #[must_use]
    pub fn generate() -> Self {
        let secret = SecretKey::generate(&mut rand::rngs::OsRng {});
        Self::from_secret_key(secret)
    }

    /// Get the id of the identity.
    #[must_use]
    pub const fn id(&self) -> &PeerId {
        &self.id
    }

    /// Create a signature of a `message` that implements `Signable`.
    pub fn sign<S>(&self, message: S) -> Result<Signature, Error>
    where
        S: Signable,
    {
        let expanded = ExpandedSecretKey::from(&self.secret);
        let data = message.signable_data().map_err(Error::signable_error)?;
        let signature = expanded.sign(data.as_ref(), &self.id.0);
        Ok(Signature(signature))
    }
}
