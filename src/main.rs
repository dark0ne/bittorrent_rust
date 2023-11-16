use anyhow::Error;
use clap::Parser;
use futures::{sink::SinkExt, stream::StreamExt};
use hex;
use reqwest;
use sha1::{Digest, Sha1};
use std::{
    fs,
    io::{Read, Write},
    mem::size_of,
    net::TcpStream,
    path::Path,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

mod args;
mod hashes;
mod peer;
mod tracker;

use hashes::Hashes;

#[derive(Debug, serde::Deserialize)]
struct Torrent {
    announce: String,

    info: Info,
}
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct Info {
    length: usize,

    name: String,

    #[serde(rename = "piece length")]
    piece_length: usize,

    //#[serde(with = "serde_bytes")]
    pieces: Hashes,
}

impl Info {
    pub fn calc_hash(&self) -> [u8; 20] {
        let info_ser = serde_bencode::to_bytes(self).expect("Could not serialize");
        let mut hasher = Sha1::new();
        hasher.update(info_ser);
        let info_hash = hasher.finalize();
        info_hash.into()
    }
}

const MY_PEER_ID: &str = "00112233445566778899";
const BLOCK_SIZE: usize = 1 << 14;

// Usage: your_bittorrent.sh decode "<encoded_value>"
#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = args::Args::parse();
    match args.command {
        args::Commands::Decode { input } => {
            //let decoded_value = decode_bencoded_value(encoded_value);
            let decoded_value =
                serde_bencode::from_str(&input).expect("cannot decode bencoded string");
            println!("{}", bencode_to_serde(decoded_value).to_string());
        }
        args::Commands::Info { torrent } => {
            let torrent: Torrent = read_torrent(torrent);
            let info_hash = torrent.info.calc_hash();

            println!("Tracker URL: {}", torrent.announce);
            println!("Length: {}", torrent.info.length);
            println!("Info Hash: {}", hex::encode(info_hash));
            println!("Piece Length: {}", torrent.info.piece_length);
            println!("Piece Hashes:");
            for h in torrent.info.pieces.data {
                println!("{}", hex::encode(h));
            }
        }
        args::Commands::Peers { torrent } => {
            let torrent: Torrent = read_torrent(torrent);
            let request = tracker::TrackerRequest {
                //info_hash: SingleHash(torrent.info.calc_hash()),
                peer_id: MY_PEER_ID.to_string(),
                port: 6881,
                uploaded: 0,
                downloaded: 0,
                left: torrent.info.length,
                compact: 1,
            };
            let params = serde_urlencoded::to_string(request).expect("url encode failed");

            let full_url = format!(
                "{}?info_hash={}&{}",
                torrent.announce,
                urlencoding::encode_binary(&torrent.info.calc_hash()),
                params
            );

            let response = reqwest::blocking::get(full_url)
                .expect("GET for peers failed")
                .bytes()
                .unwrap();
            let response: tracker::TrackerResponse = serde_bencode::from_bytes(&*response)?;
            match response {
                tracker::TrackerResponse::Error { failure_reason } => {
                    println!("Peer request failed. Reason: {}", failure_reason)
                }
                tracker::TrackerResponse::Peers {
                    interval: _,
                    min_interval: _,
                    tracker_id: _,
                    complete: _,
                    incomplete: _,
                    peers,
                } => {
                    for addr in peers {
                        println!("{}:{}", addr.ip(), addr.port());
                    }
                }
            }
        }
        args::Commands::Handshake { torrent, address } => {
            let torrent: Torrent = read_torrent(torrent);
            //let addr: SocketAddrV4 = args[3].parse()?;
            let my_handshake = peer::Handshake::new(
                torrent.info.calc_hash(),
                MY_PEER_ID.as_bytes().to_owned().try_into().unwrap(),
            );
            let mut stream = TcpStream::connect(address)?;
            {
                let my_handshake_bytes = my_handshake.to_bytes();
                stream.write_all(&my_handshake_bytes.as_slice())?;
            }

            let peer_handshake = {
                let mut peer_handshake_bytes = vec![0; size_of::<peer::Handshake>()];
                stream.read_exact(peer_handshake_bytes.as_mut_slice())?;
                peer::Handshake::from_bytes(peer_handshake_bytes.as_slice())
                    .ok_or(Error::msg("invalid size for handshake"))?
            };
            if my_handshake.info_hash != peer_handshake.info_hash {
                return Err(Error::msg("info_has from the peer does not match."));
            }

            println!("Peer ID: {}", hex::encode(peer_handshake.peer_id));
        }
        args::Commands::DownloadPiece {
            output_file,
            torrent,
            index,
        } => {
            let torrent: Torrent = read_torrent(torrent);
            let request = tracker::TrackerRequest {
                //info_hash: SingleHash(torrent.info.calc_hash()),
                peer_id: MY_PEER_ID.to_string(),
                port: 6881,
                uploaded: 0,
                downloaded: 0,
                left: torrent.info.length,
                compact: 1,
            };
            let params = serde_urlencoded::to_string(request).expect("url encode failed");

            let full_url = format!(
                "{}?info_hash={}&{}",
                torrent.announce,
                urlencoding::encode_binary(&torrent.info.calc_hash()),
                params
            );

            let response = reqwest::get(full_url)
                .await
                .expect("GET for peers failed")
                .bytes()
                .await
                .unwrap();
            let response: tracker::TrackerResponse = serde_bencode::from_bytes(&*response)?;
            let peers = match response {
                tracker::TrackerResponse::Error { failure_reason } => Err(Error::msg(format!(
                    "Peer request failed. Reason: {}",
                    failure_reason
                ))),
                tracker::TrackerResponse::Peers {
                    interval: _,
                    min_interval: _,
                    tracker_id: _,
                    complete: _,
                    incomplete: _,
                    peers,
                } => Ok(peers),
            }?;
            let peer_address = peers[0];
            let my_handshake = peer::Handshake::new(
                torrent.info.calc_hash(),
                MY_PEER_ID.as_bytes().to_owned().try_into().unwrap(),
            );
            let mut stream = tokio::net::TcpStream::connect(peer_address).await?;
            {
                let my_handshake_bytes = my_handshake.to_bytes();
                stream.write_all(&my_handshake_bytes.as_slice()).await?;
            }

            let peer_handshake = {
                let mut peer_handshake_bytes = vec![0; size_of::<peer::Handshake>()];
                stream
                    .read_exact(peer_handshake_bytes.as_mut_slice())
                    .await?;
                peer::Handshake::from_bytes(peer_handshake_bytes.as_slice())
                    .ok_or(Error::msg("invalid size for handshake"))?
            };
            if my_handshake.info_hash != peer_handshake.info_hash {
                return Err(Error::msg("info_has from the peer does not match."));
            }

            let mut stream = tokio_util::codec::Framed::new(stream, peer::MessageFramer);

            // 1. step: wait for Bitfield message
            let next_msg = stream
                .next()
                .await
                .expect("expecting messages")
                .expect("should be a valid message");
            println!("Received message is {:?}", next_msg.tag);

            if next_msg.tag != peer::MessageTag::Bitfield {
                return Err(Error::msg(format!(
                    "Unexpected message type: Expected Bitfield, got {:?}",
                    next_msg.tag
                )));
            }
            // 2. step: send Interested message.
            stream
                .send(peer::RawMessage {
                    tag: peer::MessageTag::Interested,
                    payload: vec![],
                })
                .await?;
            // 3. step: wait for the Unchoke message.
            let next_msg = stream
                .next()
                .await
                .expect("expecting messages")
                .expect("should be a valid message");
            println!("Received message is {:?}", next_msg.tag);

            if next_msg.tag != peer::MessageTag::Unchoke {
                return Err(Error::msg(format!(
                    "Unexpected message type: Expected Unchoke, got {:?}",
                    next_msg.tag
                )));
            }

            let requested_piece_size = if index == torrent.info.pieces.data.len() - 1 {
                torrent.info.length % torrent.info.piece_length
            } else {
                torrent.info.piece_length
            };

            let nr_of_blocks = requested_piece_size.div_ceil(BLOCK_SIZE);
            assert!(nr_of_blocks > 1);

            // todo download all blocks. Now only the first.
            let block_nr = 0;
        }
    }
    Ok(())
}

fn read_torrent<P>(path: P) -> Torrent
where
    P: AsRef<Path>,
{
    let contents = fs::read(path).expect("Could not read file");
    serde_bencode::from_bytes(contents.as_slice()).expect("Could not deserialize")
}

fn bencode_to_serde(value: serde_bencode::value::Value) -> serde_json::Value {
    match value {
        serde_bencode::value::Value::Bytes(bytes) => {
            serde_json::Value::String(String::from_utf8_lossy(bytes.as_slice()).to_string())
        }
        serde_bencode::value::Value::Int(int) => {
            serde_json::Value::Number(serde_json::value::Number::from(int))
        }
        serde_bencode::value::Value::List(list) => {
            serde_json::Value::Array(list.into_iter().map(|el| bencode_to_serde(el)).collect())
        }
        serde_bencode::value::Value::Dict(dict) => serde_json::Value::Object(
            dict.into_iter()
                .map(|el| {
                    (
                        String::from_utf8_lossy(el.0.as_slice()).to_string(),
                        bencode_to_serde(el.1),
                    )
                })
                .collect(),
        ),
    }
}
