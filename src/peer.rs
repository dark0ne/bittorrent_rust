use std::mem::size_of;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Default)]
pub struct Handshake {
    pub protocol_len: u8,
    pub protocol_string: [u8; 19],
    pub reserved: [u8; 8],
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
}

impl Handshake {
    pub fn new(info_hash: [u8; 20], peer_id: [u8; 20]) -> Self {
        Self {
            protocol_len: 19,
            protocol_string: *b"BitTorrent protocol",
            reserved: [0; 8],
            info_hash,
            peer_id,
        }
    }

    pub fn from_bytes(mut buf: &[u8]) -> Option<Self> {
        let mut s = Self::default();
        s.protocol_len = buf[0];
        buf = &buf[1..];
        s.protocol_string.copy_from_slice(&buf[..19]);
        buf = &buf[19..];
        s.reserved.copy_from_slice(&buf[..8]);
        buf = &buf[8..];
        s.info_hash.copy_from_slice(&buf[..20]);
        buf = &buf[20..];
        s.peer_id.copy_from_slice(&buf[..20]);
        buf = &buf[20..];
        if buf.is_empty() {
            Some(s)
        } else {
            None
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Handshake>());
        buf.push(self.protocol_len);
        buf.extend_from_slice(&self.protocol_string);
        buf.extend_from_slice(&self.reserved);
        buf.extend_from_slice(&self.info_hash);
        buf.extend_from_slice(&self.peer_id);
        buf
    }
}
