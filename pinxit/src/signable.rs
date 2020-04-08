use crate::{Error, PeerId, Signature};
use std::error::Error as StdError;

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
///     type Message = Vec<u8>;
///     type Error = io::Error;
///     fn message(&self) -> Result<Self::Message, Self::Error> {
///         Ok(vec![self.0, self.1])
///     }
/// }
///
/// // ---------------- Sign by owning a reference ----------------
/// struct SignStr<'a>(&'a str);
///
/// impl<'a> Signable for SignStr<'a> {
///     type Message = &'a [u8];
///     type Error = io::Error;
///     fn message(&self) -> Result<Self::Message, Self::Error> {
///         Ok(self.0.as_bytes())
///     }
/// }
///
/// // ---------------- Sign by returning a reference ----------------
/// struct SignString(String);
///
/// impl<'a> Signable for &'a SignString {
///     type Message = &'a str;
///     type Error = io::Error;
///     fn message(&self) -> Result<Self::Message, Self::Error> {
///         Ok(&self.0)
///     }
/// }
///
/// // ---------------- Test signable implementations ----------------
/// fn test_signable(message: impl Signable, expected: impl AsRef<[u8]>) {
///     assert_eq!(message.message().unwrap().as_ref(), expected.as_ref());
/// }
///
/// test_signable(SignCreateVec(4, 2), [4, 2]);
/// test_signable(SignStr("42"), [b'4', b'2']);
/// test_signable(&SignString("42".to_string()), [b'4', b'2']);
/// ```
pub trait Signable {
    /// The type of the signable message.
    type Message: AsRef<[u8]>;

    /// The type of error that can occur while creating the message.
    type Error: StdError + Send + Sync + 'static;

    /// Create a message from self.
    fn message(&self) -> Result<Self::Message, Self::Error>;

    /// Verify a `signature` of a `message` that implements `Signable`.
    fn verify(&self, peer_id: &PeerId, signature: &Signature) -> Result<(), Error>
    where
        Self: Sized,
    {
        peer_id.verify(self, signature)
    }
}

impl<'a, S> Signable for &'a S
where
    S: Signable,
{
    type Message = S::Message;
    type Error = S::Error;
    fn message(&self) -> Result<Self::Message, Self::Error> {
        S::message(self)
    }
}
