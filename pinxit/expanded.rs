#![feature(prelude_import)]
#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]
//! Cryptographic types for identities and signatures.
//!
//! ```
//! use pinxit::{Identity, Signable};
//!
//! // define example struct
//! struct TestData(String);
//!
//! // make the struct signable
//! impl<'a> Signable for &'a TestData {
//!     type Message = &'a str;
//!     type Error = std::io::Error; // never used
//!     fn message(&self) -> Result<Self::Message, Self::Error> {
//!         Ok(&self.0)
//!     }
//! }
//!
//! // create an identity
//! let identity = Identity::generate();
//!
//! // create signable test data
//! let test_data = TestData("Lorem ipsum".to_string());
//!
//! // create a signature
//! let signature = identity.sign(&test_data).unwrap();
//!
//! // get peer id of identity
//! let peer_id = identity.id();
//!
//! // verify the signature
//! peer_id.verify(&test_data, &signature).unwrap();
//! ```
#[prelude_import]
use std::prelude::v1::*;
#[macro_use]
extern crate std;
#[macro_use]
mod macros {}
mod error {
    #![allow(clippy::pub_enum_variant_names)]
    use err_derive::Error;
    /// An error of the `pinxit` crate.
    #[non_exhaustive]
    pub enum Error {
        /// An invalid hexadecimal value vas used.
        #[error(display = "invalid hex")]
        HexError(#[error(source)] hex::FromHexError),
        /// An invalid signature was used.
        #[error(display = "invalid signature")]
        SignatureError(#[error(source)] ed25519_dalek::SignatureError),
        /// A `Signable` failed to create a message.
        #[error(display = "unable to create signable message")]
        SignableError(#[error(source, no_from)] BoxError),
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for Error {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match (&*self,) {
                (&Error::HexError(ref __self_0),) => {
                    let mut debug_trait_builder = f.debug_tuple("HexError");
                    let _ = debug_trait_builder.field(&&(*__self_0));
                    debug_trait_builder.finish()
                }
                (&Error::SignatureError(ref __self_0),) => {
                    let mut debug_trait_builder = f.debug_tuple("SignatureError");
                    let _ = debug_trait_builder.field(&&(*__self_0));
                    debug_trait_builder.finish()
                }
                (&Error::SignableError(ref __self_0),) => {
                    let mut debug_trait_builder = f.debug_tuple("SignableError");
                    let _ = debug_trait_builder.field(&&(*__self_0));
                    debug_trait_builder.finish()
                }
            }
        }
    }
    #[allow(non_upper_case_globals)]
    #[doc(hidden)]
    const _DERIVE_std_error_Error_FOR_Error: () = {
        impl ::std::error::Error for Error {
            fn description(&self) -> &str {
                "description() is deprecated; use Display"
            }
            #[allow(unreachable_code)]
            fn cause(&self) -> ::std::option::Option<&::std::error::Error> {
                match *self {
                    Error::HexError(ref __binding_0) => {
                        return Some(__binding_0 as &::std::error::Error)
                    }
                    Error::SignatureError(ref __binding_0) => {
                        return Some(__binding_0 as &::std::error::Error)
                    }
                    Error::SignableError(ref __binding_0) => {
                        return Some(__binding_0 as &::std::error::Error)
                    }
                }
                None
            }
            #[allow(unreachable_code)]
            fn source(&self) -> ::std::option::Option<&(::std::error::Error + 'static)> {
                match *self {
                    Error::HexError(ref __binding_0) => {
                        return Some(__binding_0 as &::std::error::Error)
                    }
                    Error::SignatureError(ref __binding_0) => {
                        return Some(__binding_0 as &::std::error::Error)
                    }
                    Error::SignableError(ref __binding_0) => {
                        return Some(__binding_0 as &::std::error::Error)
                    }
                }
                None
            }
        }
    };
    #[allow(non_upper_case_globals)]
    #[doc(hidden)]
    const _DERIVE_core_fmt_Display_FOR_Error: () = {
        impl ::core::fmt::Display for Error {
            #[allow(unreachable_code)]
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                match *self {
                    Error::HexError(ref __binding_0) => {
                        return f.write_fmt(::core::fmt::Arguments::new_v1(
                            &["invalid hex"],
                            &match () {
                                () => [],
                            },
                        ))
                    }
                    Error::SignatureError(ref __binding_0) => {
                        return f.write_fmt(::core::fmt::Arguments::new_v1(
                            &["invalid signature"],
                            &match () {
                                () => [],
                            },
                        ))
                    }
                    Error::SignableError(ref __binding_0) => {
                        return f.write_fmt(::core::fmt::Arguments::new_v1(
                            &["unable to create signable message"],
                            &match () {
                                () => [],
                            },
                        ))
                    }
                }
                f.write_fmt(::core::fmt::Arguments::new_v1(
                    &["An error has occurred."],
                    &match () {
                        () => [],
                    },
                ))
            }
        }
    };
    #[allow(non_upper_case_globals)]
    #[doc(hidden)]
    const _DERIVE_core_convert_From_hex_FromHexError_FOR_Error: () = {
        impl ::core::convert::From<hex::FromHexError> for Error {
            fn from(from: hex::FromHexError) -> Self {
                Error::HexError(from)
            }
        }
    };
    #[allow(non_upper_case_globals)]
    #[doc(hidden)]
    const _DERIVE_core_convert_From_ed25519_dalek_SignatureError_FOR_Error: () = {
        impl ::core::convert::From<ed25519_dalek::SignatureError> for Error {
            fn from(from: ed25519_dalek::SignatureError) -> Self {
                Error::SignatureError(from)
            }
        }
    };
    impl Error {
        pub(crate) fn signable_error<E>(err: E) -> Self
        where
            E: std::error::Error + Send + Sync + 'static,
        {
            Self::SignableError(err.into())
        }
    }
    pub type BoxError = Box<dyn std::error::Error>;
}
mod identity {
    use crate::{Error, PeerId, Signable, Signature};
    use ed25519_dalek::{ExpandedSecretKey, SecretKey};
    use hex::FromHex;
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
    impl Identity {
        pub(crate) fn from_secret_key(secret: SecretKey) -> Self {
            let id = PeerId((&secret).into());
            Self { id, secret }
        }
        /// Create an identity from it's hexadecimal representation.
        pub fn from_hex(hex: &str) -> Result<Self, Error> {
            let bytes: [u8; SECRET_LEN] = FromHex::from_hex(hex)?;
            let secret = SecretKey::from_bytes(&bytes).unwrap();
            Ok(Self::from_secret_key(secret))
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
        /// Create a hexadecimal representation.
        #[must_use]
        pub fn hex(&self) -> String {
            hex::encode(self.secret.as_bytes())
        }
        /// Create a signature of a `message` that implements `Signable`.
        pub fn sign<S>(&self, message: S) -> Result<Signature, Error>
        where
            S: Signable,
            S::Error: Send + Sync + 'static,
        {
            let expanded = ExpandedSecretKey::from(&self.secret);
            let message = message.message().map_err(Error::signable_error)?;
            let signature = expanded.sign(message.as_ref(), &self.id.0);
            Ok(Signature(signature))
        }
    }
}
mod peer_id {
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
    pub struct PeerId(pub(crate) PublicKey);
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::clone::Clone for PeerId {
        #[inline]
        fn clone(&self) -> PeerId {
            match *self {
                PeerId(ref __self_0_0) => PeerId(::core::clone::Clone::clone(&(*__self_0_0))),
            }
        }
    }
    impl ::core::marker::StructuralEq for PeerId {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::Eq for PeerId {
        #[inline]
        #[doc(hidden)]
        fn assert_receiver_is_total_eq(&self) -> () {
            {
                let _: ::core::cmp::AssertParamIsEq<PublicKey>;
            }
        }
    }
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
            {
                let data: &[u8; 32] = self.0.as_bytes();
                let data: &[u8] = data;
                let out = &mut [0; 32 * 2];
                ::hex::encode_to_slice(data, out).unwrap();
                f.write_str(::std::str::from_utf8(out).unwrap())
            }
        }
    }
    impl FromStr for PeerId {
        type Err = Error;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            Self::from_hex(s)
        }
    }
    #[allow(missing_copy_implementations)]
    #[allow(non_camel_case_types)]
    #[allow(dead_code)]
    struct PEER_NAMES {
        __private_field: (),
    }
    #[doc(hidden)]
    static PEER_NAMES: PEER_NAMES = PEER_NAMES {
        __private_field: (),
    };
    impl ::lazy_static::__Deref for PEER_NAMES {
        type Target = RwLock<HashMap<PeerId, String>>;
        fn deref(&self) -> &RwLock<HashMap<PeerId, String>> {
            #[inline(always)]
            fn __static_ref_initialize() -> RwLock<HashMap<PeerId, String>> {
                RwLock::new(HashMap::new())
            }
            #[inline(always)]
            fn __stability() -> &'static RwLock<HashMap<PeerId, String>> {
                static LAZY: ::lazy_static::lazy::Lazy<RwLock<HashMap<PeerId, String>>> =
                    ::lazy_static::lazy::Lazy::INIT;
                LAZY.get(__static_ref_initialize)
            }
            __stability()
        }
    }
    impl ::lazy_static::LazyStatic for PEER_NAMES {
        fn initialize(lazy: &Self) {
            let _ = &**lazy;
        }
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
            S::Error: Send + Sync + 'static,
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
}
mod signable {
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
        type Error: StdError;
        /// Create a message from self.
        fn message(&self) -> Result<Self::Message, Self::Error>;
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
}
mod signature {
    use crate::Error;
    use hex::FromHex;
    use std::{fmt, str::FromStr};
    const SIGNATURE_LEN: usize = ed25519_dalek::SIGNATURE_LENGTH;
    /// The cryptographic signature of some `Signable` data.
    pub struct Signature(pub(crate) ed25519_dalek::Signature);
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::clone::Clone for Signature {
        #[inline]
        fn clone(&self) -> Signature {
            match *self {
                Signature(ref __self_0_0) => Signature(::core::clone::Clone::clone(&(*__self_0_0))),
            }
        }
    }
    impl fmt::Debug for Signature {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            fmt::Display::fmt(self, f)
        }
    }
    impl fmt::Display for Signature {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            {
                let data: &[u8; SIGNATURE_LEN] = &self.0.to_bytes();
                let data: &[u8] = data;
                let out = &mut [0; SIGNATURE_LEN * 2];
                ::hex::encode_to_slice(data, out).unwrap();
                f.write_str(::std::str::from_utf8(out).unwrap())
            }
        }
    }
    impl FromStr for Signature {
        type Err = Error;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            Self::from_hex(s)
        }
    }
    impl Signature {
        /// Create a signature from it's hexadecimal representation.
        pub fn from_hex(hex: &str) -> Result<Self, Error> {
            let signature: [u8; SIGNATURE_LEN] = FromHex::from_hex(hex)?;
            let signature = ed25519_dalek::Signature::from_bytes(&signature)?;
            Ok(Self(signature))
        }
        /// Create a hexadecimal representation.
        #[must_use]
        pub fn hex(&self) -> String {
            hex::encode(&self.0.to_bytes()[..])
        }
    }
    const _: () = {
        use serde::{
            de::{Error, SeqAccess, Unexpected, Visitor},
            ser::SerializeTupleStruct,
            Deserialize, Deserializer, Serialize, Serializer,
        };
        const SIGNATURE_NAME: &str = "Signature";
        impl Serialize for Signature {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                let mut tup = serializer.serialize_tuple_struct(SIGNATURE_NAME, SIGNATURE_LEN)?;
                for b in self.0.to_bytes().iter() {
                    tup.serialize_field(b)?;
                }
                tup.end()
            }
        }
        impl<'de> Deserialize<'de> for Signature {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct SignatureVisitor;
                impl<'de> Visitor<'de> for SignatureVisitor {
                    type Value = Signature;
                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter
                            .write_str("an array of length 64, that contains a valid signature")
                    }
                    #[inline]
                    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
                    where
                        A: SeqAccess<'de>,
                    {
                        let mut bytes = [0; SIGNATURE_LEN];
                        for (i, b) in bytes.iter_mut().enumerate() {
                            match seq.next_element()? {
                                Some(val) => *b = val,
                                None => return Err(Error::invalid_length(i, &self)),
                            }
                        }
                        match ed25519_dalek::Signature::from_bytes(&bytes) {
                            Ok(sig) => Ok(Signature(sig)),
                            Err(err) => Err(Error::invalid_value(
                                Unexpected::Bytes(&bytes),
                                &err.to_string().as_ref(),
                            )),
                        }
                    }
                }
                deserializer.deserialize_tuple_struct(
                    SIGNATURE_NAME,
                    SIGNATURE_LEN,
                    SignatureVisitor,
                )
            }
        }
    };
}
pub use error::Error;
pub use identity::Identity;
pub use peer_id::PeerId;
pub use signable::Signable;
pub use signature::Signature;
