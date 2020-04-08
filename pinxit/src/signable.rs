use crate::{Error, Identity, PeerId, Signature};
use serde::{Deserialize, Serialize};
use std::{
    error::Error as StdError,
    ops::{Deref, DerefMut},
};

/// A `Signable` is something that can be signed.
///
/// ```
/// use pinxit::Signable;
/// use std::io;
///
/// // ---------------- Sign by creating a Vec ----------------
/// struct SignCreateVec(u8, u8);
///
/// impl Signable for SignCreateVec {
///     type SignableData = Vec<u8>;
///     type Error = io::Error;
///     fn signable_data(&self) -> Result<Self::SignableData, Self::Error> {
///         Ok(vec![self.0, self.1])
///     }
/// }
///
/// // ---------------- Sign by owning a reference ----------------
/// struct SignStr<'a>(&'a str);
///
/// impl<'a> Signable for SignStr<'a> {
///     type SignableData = &'a [u8];
///     type Error = io::Error;
///     fn signable_data(&self) -> Result<Self::SignableData, Self::Error> {
///         Ok(self.0.as_bytes())
///     }
/// }
///
/// // ---------------- Sign by returning a reference ----------------
/// struct SignString(String);
///
/// impl<'a> Signable for &'a SignString {
///     type SignableData = &'a str;
///     type Error = io::Error;
///     fn signable_data(&self) -> Result<Self::SignableData, Self::Error> {
///         Ok(&self.0)
///     }
/// }
///
/// // ---------------- Test signable implementations ----------------
/// fn test_signable(message: impl Signable, expected: impl AsRef<[u8]>) {
///     assert_eq!(message.signable_data().unwrap().as_ref(), expected.as_ref());
/// }
///
/// test_signable(SignCreateVec(4, 2), [4, 2]);
/// test_signable(SignStr("42"), [b'4', b'2']);
/// test_signable(&SignString("42".to_string()), [b'4', b'2']);
/// ```
pub trait Signable: Sized {
    /// The type for representing signable data.
    type SignableData: AsRef<[u8]>;

    /// The type of error that can occur while creating the signable data.
    type Error: StdError + Send + Sync + 'static;

    /// Create a signable representation from self.
    fn signable_data(&self) -> Result<Self::SignableData, Self::Error>;

    /// Sign a `Signable` message with an `identity`.
    fn sign(self, identity: &Identity) -> Result<Signed<Self>, Error> {
        let signature = identity.sign(&self)?;
        Ok(Signed {
            body: self,
            signature,
        })
    }
}

impl<'a, S> Signable for &'a S
where
    S: Signable,
{
    type SignableData = S::SignableData;
    type Error = S::Error;
    fn signable_data(&self) -> Result<Self::SignableData, Self::Error> {
        S::signable_data(self)
    }
}

/// Wraps a message with signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signed<T> {
    body: T,
    signature: Signature,
}

impl<T> Signed<T>
where
    T: Signable,
{
    /// Verify the signature of a signed message.
    pub fn verify(self, peer_id: &PeerId) -> Result<Verified<T>, Error> {
        peer_id.verify(&self.body, &self.signature)?;
        Ok(Verified(self))
    }
}

/// A verified signed message.
pub struct Verified<T>(Signed<T>);

impl<T> Verified<T> {
    /// Get the signature of the message.
    pub const fn signature(&self) -> &Signature {
        &self.0.signature
    }

    /// Extract the message.
    #[allow(clippy::missing_const_for_fn)] // stupid clippy :(
    pub fn into_inner(self) -> T {
        self.0.body
    }
}

impl<T> Deref for Verified<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0.body
    }
}

impl<T> DerefMut for Verified<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0.body
    }
}

impl<T> From<Verified<T>> for Signed<T> {
    fn from(v: Verified<T>) -> Self {
        v.0
    }
}
