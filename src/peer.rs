use bytes::{Buf, BytesMut};
use std::convert::{TryFrom, TryInto};
use std::mem::size_of;
use tokio_util::codec::Decoder;

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

#[repr(u8)]
pub enum MessageTag {
    Choke = 0,
    Unchoke,
    Interested,
    NotInterested,
    Have,
    Bitfield,
    Request,
    Piece,
    Cancel,
}
impl TryFrom<u8> for MessageTag {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            x if x == MessageTag::Choke as u8 => Ok(MessageTag::Choke),
            x if x == MessageTag::Unchoke as u8 => Ok(MessageTag::Unchoke),
            x if x == MessageTag::Interested as u8 => Ok(MessageTag::Interested),
            x if x == MessageTag::NotInterested as u8 => Ok(MessageTag::NotInterested),
            x if x == MessageTag::Have as u8 => Ok(MessageTag::Have),
            x if x == MessageTag::Bitfield as u8 => Ok(MessageTag::Bitfield),
            x if x == MessageTag::Request as u8 => Ok(MessageTag::Request),
            x if x == MessageTag::Piece as u8 => Ok(MessageTag::Piece),
            x if x == MessageTag::Cancel as u8 => Ok(MessageTag::Cancel),
            _ => Err(()),
        }
    }
}

pub enum Message {
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    /// The 'have' message's payload is a single number, the index which that downloader just completed and checked the hash of.
    Have {
        index: u32,
    },
    Bitfield(Vec<u8>),
    /// 'request' messages contain an index, begin, and length. The last two are byte offsets. Length is generally a power of
    ///  two unless it gets truncated by the end of the file. All current implementations use 2^14 (16 kiB), and close
    /// connections which request an amount greater than that.
    Request {
        index: u32,
        begin: u32,
        length: u32,
    },
    /// 'piece' messages contain an index, begin, and piece. Note that they are correlated with request messages implicitly.
    /// It's possible for an unexpected piece to arrive if choke and unchoke messages are sent in quick succession and/or transfer
    /// is going very slowly.
    Piece {
        index: u32,
        begin: u32,
        block: Vec<u8>,
    },
    /// 'cancel' messages have the same payload as request messages.They are generally only sent towards the end of a download,
    /// during what's called 'endgame mode'. When a download is almost complete, there's a tendency for the last few pieces
    /// to all be downloaded off a single hosed modem line, taking a very long time. To make sure the last few pieces come in
    /// quickly, once requests for all pieces a given downloader doesn't have yet are currently pending, it sends requests
    /// for everything to everyone it's downloading from. To keep this from becoming horribly inefficient, it sends cancels
    /// to everyone else every time a piece arrives.
    Cancel {
        index: u32,
        begin: u32,
        length: u32,
    },
}

struct MessageDecoder {}

impl Decoder for MessageDecoder {
    type Item = Message;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 4 {
            // Not enough data to read length marker.
            return Ok(None);
        }

        // Read length marker.
        let mut length_bytes = [0u8; 4];
        length_bytes.copy_from_slice(&src[..4]);
        let length = u32::from_be_bytes(length_bytes) as usize;

        if src.len() < 4 + length {
            // The full message has not yet arrived.
            //
            // We reserve more space in the buffer. This is not strictly
            // necessary, but is a good idea performance-wise.
            src.reserve(4 + length - src.len());

            // We inform the Framed that we need more bytes to form the next
            // frame.
            return Ok(None);
        }

        // Use advance to modify src such that it no longer contains
        // this frame.
        let data = src[4..4 + length].to_vec();
        src.advance(4 + length);

        match <u8 as TryInto<MessageTag>>::try_into(data[0]) {
            Ok(MessageTag::Choke) => todo!(),
            Ok(MessageTag::Unchoke) => todo!(),
            Ok(MessageTag::Interested) => todo!(),
            Ok(MessageTag::NotInterested) => todo!(),
            Ok(MessageTag::Have) => todo!(),
            Ok(MessageTag::Bitfield) => todo!(),
            Ok(MessageTag::Request) => todo!(),
            Ok(MessageTag::Piece) => todo!(),
            Ok(MessageTag::Cancel) => todo!(),
            Err(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Message tag {} is too large.", data[0]),
                ))
            }
        }
    }
}
