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
pub use signable::Signable;
pub use signature::Signature;
