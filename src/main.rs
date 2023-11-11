use anyhow::Error;
use hex;
use reqwest;
use sha1::{Digest, Sha1};
use std::{
    env, fs,
    io::{Read, Write},
    mem::size_of,
    net::{SocketAddrV4, TcpStream},
    path::Path,
};

mod hashes;
mod peer;
mod tracker;

use hashes::Hashes;

#[derive(Debug, serde::Deserialize)]
struct Torrent {
    announce: String,

    info: Info,
}
use serde::{Deserialize, Deserializer, Serialize};

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

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        let encoded_value = &args[2];
        //let decoded_value = decode_bencoded_value(encoded_value);
        let decoded_value =
            serde_bencode::from_str(&encoded_value).expect("cannot decode bencoded string");
        println!("{}", bencode_to_serde(decoded_value).to_string());
    } else if command == "info" {
        let torrent: Torrent = read_torrent(&args[2]);

        let info_hash = torrent.info.calc_hash();

        println!("Tracker URL: {}", torrent.announce);
        println!("Length: {}", torrent.info.length);
        println!("Info Hash: {}", hex::encode(info_hash));
        println!("Piece Length: {}", torrent.info.piece_length);
        println!("Piece Hashes:");
        for h in torrent.info.pieces.data {
            println!("{}", hex::encode(h));
        }
    } else if command == "peers" {
        let torrent: Torrent = read_torrent(&args[2]);
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
    } else if command == "handshake" {
        let torrent: Torrent = read_torrent(&args[2]);
        let addr: SocketAddrV4 = args[3].parse()?;
        let my_handshake = peer::Handshake::new(
            torrent.info.calc_hash(),
            MY_PEER_ID.as_bytes().to_owned().try_into().unwrap(),
        );
        let mut stream = TcpStream::connect(addr)?;
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
    } else {
        println!("unknown command: {}", args[1])
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
