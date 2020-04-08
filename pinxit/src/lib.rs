#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]

//! Cryptographic types for identities and signatures.
//!
//! ```
//! use pinxit::{Identity, Signable, Signed, Verified};
//!
//! // define example struct
//! struct TestData<'a>(&'a str);
//!
//! // make the struct signable
//! impl<'a> Signable for TestData<'a> {
//!     type SignableData = &'a str;
//!     type Error = std::io::Error; // never used
//!     fn signable_data(&self) -> Result<Self::SignableData, Self::Error> {
//!         Ok(self.0)
//!     }
//! }
//!
//! // create an identity
//! let identity = Identity::generate();
//!
//! // create signable test data
//! let test_data = TestData("Lorem ipsum");
//!
//! // create a signed version of test data
//! // you cannot access the data until it is verified
//! let signed: Signed<TestData> = test_data.sign(&identity).unwrap();
//!
//! // get peer id of identity
//! let peer_id = identity.id();
//!
//! // verify the signature
//! let verified: Verified<TestData> = signed.verify(peer_id).unwrap();
//!
//! // access the data
//! println!("{}", verified.0)
//! ```

#[macro_use]
mod macros;

mod error;
mod identity;
mod peer_id;
mod signable;
mod signature;

pub use error::Error;
pub use identity::Identity;
pub use peer_id::PeerId;
pub use signable::{Signable, Signed, Verified};
pub use signature::Signature;
