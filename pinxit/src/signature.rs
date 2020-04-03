use crate::Error;
use hex::FromHex;
use std::{fmt, str::FromStr};

const SIGNATURE_LEN: usize = ed25519_dalek::SIGNATURE_LENGTH;

/// The cryptographic signature of some `Signable` data.
#[derive(Clone)]
pub struct Signature(pub(crate) ed25519_dalek::Signature);

impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write_hex!(f, &self.0.to_bytes(), SIGNATURE_LEN)
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

// custom serde implementation
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
                    formatter.write_str("an array of length 64, that contains a valid signature")
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
            deserializer.deserialize_tuple_struct(SIGNATURE_NAME, SIGNATURE_LEN, SignatureVisitor)
        }
    }
};
