use bytes::{Buf, BufMut, BytesMut};
use int_enum::IntEnum;
use std::io::{self, Cursor, Read};
use std::mem::size_of;
use tokio_util::codec::{Decoder, Encoder};

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

    pub fn from_bytes(buf: &[u8]) -> Option<Self> {
        let mut cur = Cursor::new(buf);
        if cur.remaining() != 1 + 19 + 8 + 20 + 20 {
            return None;
        }

        let mut s = Self::default();
        s.protocol_len = cur.get_u8();
        cur.copy_to_slice(&mut s.protocol_string);
        cur.copy_to_slice(&mut s.reserved);
        cur.copy_to_slice(&mut s.info_hash);
        cur.copy_to_slice(&mut s.peer_id);
        if cur.has_remaining() {
            None
        } else {
            Some(s)
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, IntEnum)]
pub enum MessageTag {
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
}

pub struct RawMessage {
    pub tag: MessageTag,
    pub payload: Vec<u8>,
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

impl TryFrom<RawMessage> for Message {
    type Error = std::io::Error;

    fn try_from(value: RawMessage) -> Result<Self, Self::Error> {
        let error_payload_not_empty = |tag: MessageTag| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{:?} message type does not suppose to have payload.",
                    value.tag
                ),
            )
        };
        let error_invalid_size = |tag: MessageTag, len: usize| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{:?} message type has invalid payload length {}.",
                    value.tag, len
                ),
            )
        };
        match value.tag {
            MessageTag::Choke
            | MessageTag::Unchoke
            | MessageTag::Interested
            | MessageTag::NotInterested
                if !value.payload.is_empty() =>
            {
                Err(error_payload_not_empty(value.tag))
            }
            MessageTag::Have if value.payload.len() != 4 => {
                Err(error_invalid_size(value.tag, value.payload.len()))
            }
            MessageTag::Bitfield if value.payload.len() == 0 => {
                Err(error_invalid_size(value.tag, value.payload.len()))
            }
            MessageTag::Request | MessageTag::Cancel if value.payload.len() != 12 => {
                Err(error_invalid_size(value.tag, value.payload.len()))
            }
            MessageTag::Piece if value.payload.len() < 9 => {
                Err(error_invalid_size(value.tag, value.payload.len()))
            }

            MessageTag::Choke => Ok(Message::Choke),
            MessageTag::Unchoke => Ok(Message::Unchoke),
            MessageTag::Interested => Ok(Message::Interested),
            MessageTag::NotInterested => Ok(Message::NotInterested),
            MessageTag::Have => {
                let mut cur = io::Cursor::new(value.payload);
                Ok(Message::Have {
                    index: cur.get_u32(),
                })
            }
            MessageTag::Bitfield => Ok(Message::Bitfield(value.payload)),
            MessageTag::Request => {
                let mut cur = io::Cursor::new(value.payload);
                Ok(Message::Request {
                    index: cur.get_u32(),
                    begin: cur.get_u32(),
                    length: cur.get_u32(),
                })
            }
            MessageTag::Piece => {
                let mut cur = io::Cursor::new(value.payload);
                Ok(Message::Piece {
                    index: cur.get_u32(),
                    begin: cur.get_u32(),
                    block: {
                        // take the rest of the buffer
                        let pos = cur.position() as usize;
                        let mut v = cur.into_inner();
                        v.drain(0..pos);
                        v
                    },
                })
            }
            MessageTag::Cancel => {
                let mut cur = io::Cursor::new(value.payload);
                Ok(Message::Cancel {
                    index: cur.get_u32(),
                    begin: cur.get_u32(),
                    length: cur.get_u32(),
                })
            }
        }
    }
}

impl Into<RawMessage> for Message {
    fn into(self) -> RawMessage {
        let mut payload: Vec<u8> = Vec::new();
        let tag = match self {
            Message::Choke => MessageTag::Choke,
            Message::Unchoke => MessageTag::Unchoke,
            Message::Interested => MessageTag::Interested,
            Message::NotInterested => MessageTag::NotInterested,
            Message::Have { index } => {
                payload.put_u32(index);
                MessageTag::Have
            }
            Message::Bitfield(b) => {
                payload = b;
                MessageTag::Bitfield
            }
            Message::Request {
                index,
                begin,
                length,
            } => {
                payload.put_u32(index);
                payload.put_u32(begin);
                payload.put_u32(length);
                MessageTag::Request
            }
            Message::Piece {
                index,
                begin,
                mut block,
            } => {
                payload.put_u32(index);
                payload.put_u32(begin);
                payload.append(&mut block);
                MessageTag::Piece
            }
            Message::Cancel {
                index,
                begin,
                length,
            } => {
                payload.put_u32(index);
                payload.put_u32(begin);
                payload.put_u32(length);
                MessageTag::Cancel
            }
        };
        RawMessage { tag, payload }
    }
}

pub struct MessageFramer;

const MAX_SIZE: usize = 1 << 15;

impl Encoder<RawMessage> for MessageFramer {
    type Error = std::io::Error;

    fn encode(&mut self, item: RawMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let full_size = item.payload.len() + 1;
        // Don't send a message if it is longer than the other end will
        // accept.
        if full_size > MAX_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Frame of length {} is too large.", full_size),
            ));
        }

        // Convert the length into a byte array.
        // The cast to u32 cannot overflow due to the length check above.
        let len_slice = u32::to_be_bytes(full_size as u32);

        // Reserve space in the buffer.
        dst.reserve(4 + full_size);

        // Write the length and string to the buffer.
        dst.extend_from_slice(&len_slice);
        dst.put_u8(item.tag as u8);
        dst.extend_from_slice(item.payload.as_slice());
        Ok(())
    }
}

impl Decoder for MessageFramer {
    type Item = RawMessage;
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

        // Check that the length is not too large to avoid a denial of
        // service attack where the server runs out of memory.
        if length > MAX_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Frame of length {} is too large.", length),
            ));
        }

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
        let tag = src[4];
        let data = src[5..5 + length - 1].to_vec();
        src.advance(4 + length);

        let tag = MessageTag::try_from(tag).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Message tag {} is invalid.", tag),
            )
        })?;
        Ok(Some(RawMessage { tag, payload: data }))
    }
}
