use serde::{de::Visitor, ser::SerializeSeq, Deserialize, Deserializer, Serialize, Serializer};

use crate::Hashes;

struct HashesVisitor;

impl<'de> Visitor<'de> for HashesVisitor {
    type Value = Hashes;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("vector of bytes with a length multiple of 20.")
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if v.len() % 20 != 0 {
            return Err(E::custom(format!(
                "length of the byte vector ({}) is not multiple of 20.",
                v.len()
            )));
        }

        Ok(Hashes {
            data: v.chunks_exact(20).map(|x| x.try_into().unwrap()).collect(),
        })
    }
}

impl<'de> Deserialize<'de> for Hashes {
    fn deserialize<D>(deserializer: D) -> Result<Hashes, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_bytes(HashesVisitor)
    }
}

impl Serialize for Hashes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut res = Vec::with_capacity(20 * self.data.len());
        for e in &self.data {
            res.extend_from_slice(e);
        }

        serializer.serialize_bytes(res.as_slice())
    }
}
