use serde::{
    de::{Error, SeqAccess, Visitor},
    ser::SerializeTupleStruct,
    Deserializer, Serialize, Serializer,
};
use std::fmt;

#[derive(Copy, Clone)]
pub struct ByteArrayHelper(pub &'static str, pub usize);

impl ByteArrayHelper {
    pub fn serialize<S>(self, serializer: S, data: &[u8]) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ByteArraySerializer(self.0, self.1, data).serialize(serializer)
    }

    pub fn deserialize<'de, D>(self, deserializer: D, data: &mut [u8]) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_tuple_struct(self.0, self.1, ByteArrayVisitor(data))
    }
}

////////////////////////////////////////////////////////////////////////////////

struct ByteArraySerializer<'a>(&'static str, usize, &'a [u8]);

impl<'a> Serialize for ByteArraySerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut tup = serializer.serialize_tuple_struct(self.0, self.1)?;
        for b in self.2 {
            tup.serialize_field(b)?;
        }
        tup.end()
    }
}

////////////////////////////////////////////////////////////////////////////////

struct ByteArrayVisitor<'a>(&'a mut [u8]);

impl<'de, 'a> Visitor<'de> for ByteArrayVisitor<'a> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "an array of {} bytes", self.0.len())
    }

    #[inline]
    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        for (i, b) in self.0.iter_mut().enumerate() {
            match seq.next_element()? {
                Some(val) => *b = val,
                None => return Err(Error::invalid_length(i, &self)),
            }
        }
        Ok(())
    }
}
