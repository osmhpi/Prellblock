#![allow(clippy::use_self)]

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
        let signer = identity.id().clone();
        let signature = identity.sign(&self)?;
        Ok(Signed {
            signer,
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
    signer: PeerId,
    body: T,
    signature: Signature,
}

impl<T> Signed<T> {
    /// Get the signer of the signature.
    pub const fn signer(&self) -> &PeerId {
        &self.signer
    }

    /// Get the signature of the message.
    pub const fn signature(&self) -> &Signature {
        &self.signature
    }
}

impl<T> Signed<T>
where
    T: Signable,
{
    /// Verify the signature of a signed message.
    pub fn verify(self) -> Result<Verified<T>, Error> {
        self.signer.verify(&self.body, &self.signature)?;
        Ok(Verified(self))
    }

    /// Verify the signature of a signed message.
    pub fn verify_ref(&self) -> Result<VerifiedRef<T>, Error> {
        self.signer.verify(&self.body, &self.signature)?;
        Ok(VerifiedRef(self))
    }

    /// Get the unverified body.
    pub fn unverified(self) -> T {
        self.body
    }

    /// Get the unverified body.
    pub fn unverified_ref(&self) -> &T {
        &self.body
    }
}

impl<T> Eq for Signed<T> {}

impl<T> PartialEq for Signed<T> {
    fn eq(&self, other: &Self) -> bool {
        // Comparing the signatures should be enough.
        self.signature == other.signature
    }
}

/// A verified signed message.
pub struct Verified<T>(Signed<T>);

impl<T> Verified<T> {
    /// Get the signer of the signature.
    pub const fn signer(&self) -> &PeerId {
        self.0.signer()
    }

    /// Get the signature of the message.
    pub const fn signature(&self) -> &Signature {
        &self.0.signature
    }

    /// Extract the message.
    #[allow(clippy::missing_const_for_fn)] // stupid clippy :(
    pub fn into_inner(self) -> T {
        self.0.body
    }

    /// Try to map the `body` to another type.
    pub fn try_map<U, E>(self, f: impl FnOnce(T) -> Result<U, E>) -> Result<Verified<U>, E> {
        Ok(Verified(Signed {
            signer: self.0.signer,
            body: f(self.0.body)?,
            signature: self.0.signature,
        }))
    }

    /// Immutably borrows from an owned value.
    #[must_use]
    pub const fn borrow(&self) -> VerifiedRef<T> {
        VerifiedRef(&self.0)
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

/// A verified signed message.
pub struct VerifiedRef<'a, T>(&'a Signed<T>);

impl<'a, T> Copy for VerifiedRef<'a, T> {}

impl<'a, T> Clone for VerifiedRef<'a, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T> VerifiedRef<'a, T> {
    /// Get the signer of the signature.
    #[must_use]
    pub const fn signer(self) -> &'a PeerId {
        self.0.signer()
    }

    /// Get the signature of the message.
    #[must_use]
    pub const fn signature(self) -> &'a Signature {
        &self.0.signature
    }

    /// Creates owned data from borrowed data, by cloning.
    #[must_use]
    pub fn to_owned(self) -> Verified<T>
    where
        T: Clone,
    {
        Verified(self.0.clone())
    }
}

impl<'a, T> Deref for VerifiedRef<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0.body
    }
}

impl<'a, T> From<VerifiedRef<'a, T>> for &'a Signed<T> {
    fn from(v: VerifiedRef<'a, T>) -> Self {
        v.0
    }
}

/// Verify an `Iterator` over `Signed<T>` items.
///
/// This returns an `Iterator` over `VerifiedRef<T>` if and only if
/// **all** signatures can be verified.
///
/// # Example
/// ```
/// use pinxit::{verify_signed_batch_iter, Identity, Signable, Signed, VerifiedRef};
///
/// // define example struct
/// struct TestData<'a>(&'a str);
///
/// // make the struct signable
/// impl<'a> Signable for TestData<'a> {
///     type SignableData = &'a str;
///     type Error = std::io::Error; // never used
///     fn signable_data(&self) -> Result<Self::SignableData, Self::Error> {
///         Ok(self.0)
///     }
/// }
///
/// // create an identity
/// let identity = Identity::generate();
///
/// let mut batch: Vec<Signed<TestData>> = Vec::new();
///
/// for i in 0..200 {
///     // create signable test data
///     let test_data = TestData("Lorem ipsum");
///
///     // create a signed version of test data
///     // you cannot access the data until it is verified
///     let signed: Signed<TestData> = test_data.sign(&identity).unwrap();
///     batch.push(signed);
/// }
///
/// let verified_batch = verify_signed_batch_iter(batch.iter()).unwrap();
///
/// for verified in verified_batch {
///     // access the data
///     println!("{}", verified.0);
/// }
/// ```
pub fn verify_signed_batch_iter<'a, I, T>(
    batch: I,
) -> Result<impl ExactSizeIterator<Item = VerifiedRef<'a, T>>, Error>
where
    I: ExactSizeIterator<Item = &'a Signed<T>> + Clone,
    T: Signable + 'a,
{
    verify_signed_batch_inner(batch.clone())?;
    Ok(batch.map(VerifiedRef))
}

/// Verify a batch of `Signed<T>`.
///
/// This returns an `Iterator` over `Verified<T>` if and only if
/// **all** signatures can be verified.
///
/// # Example
/// ```
/// use pinxit::{verify_signed_batch, Identity, Signable, Signed, VerifiedRef};
///
/// // define example struct
/// struct TestData<'a>(&'a str);
///
/// // make the struct signable
/// impl<'a> Signable for TestData<'a> {
///     type SignableData = &'a str;
///     type Error = std::io::Error; // never used
///     fn signable_data(&self) -> Result<Self::SignableData, Self::Error> {
///         Ok(self.0)
///     }
/// }
///
/// // create an identity
/// let identity = Identity::generate();
///
/// let mut batch: Vec<Signed<TestData>> = Vec::new();
///
/// for i in 0..200 {
///     // create signable test data
///     let test_data = TestData("Lorem ipsum");
///
///     // create a signed version of test data
///     // you cannot access the data until it is verified
///     let signed: Signed<TestData> = test_data.sign(&identity).unwrap();
///     batch.push(signed);
/// }
///
/// let verified_batch = verify_signed_batch(batch).unwrap();
///
/// for verified in verified_batch {
///     // access the data
///     println!("{}", verified.0);
/// }
/// ```
pub fn verify_signed_batch<T>(
    batch: Vec<Signed<T>>,
) -> Result<impl ExactSizeIterator<Item = Verified<T>>, Error>
where
    T: Signable,
{
    verify_signed_batch_inner(batch.iter())?;
    Ok(batch.into_iter().map(Verified))
}

fn verify_signed_batch_inner<'a, T: 'a>(
    batch: impl ExactSizeIterator<Item = &'a Signed<T>>,
) -> Result<(), Error>
where
    T: Signable,
{
    let batch_length = batch.len();
    let mut messages = Vec::with_capacity(batch_length);
    let mut signers = Vec::with_capacity(batch_length);
    let mut signatures = Vec::with_capacity(batch_length);
    for signed in batch {
        messages.push(
            signed
                .unverified_ref()
                .signable_data()
                .map_err(Error::signable_error)?,
        );
        signers.push(signed.signer().0);
        signatures.push(signed.signature().0);
    }
    let messages_refs: Vec<_> = messages.iter().map(AsRef::as_ref).collect();
    match ed25519_dalek::verify_batch(&messages_refs, &signatures, &signers) {
        Ok(()) => Ok(()),
        Err(err) => Err(err.into()),
    }
}
