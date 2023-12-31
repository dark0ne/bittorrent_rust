use anyhow::Error;
use clap::Parser;
use futures::{sink::SinkExt, stream::StreamExt};
use hex;
use rand::{seq::SliceRandom, thread_rng};
use reqwest;
use sha1::{Digest, Sha1};
use std::{fs, mem::size_of, path::Path};
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

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = args::Args::parse();
    match args.command {
        args::Commands::Download {
            output_file,
            torrent,
        } => {
            let index = 0;
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
            let peer_address = peers
                .choose(&mut thread_rng())
                .expect("List of peers should not be empty.");
            let my_peer_id = MY_PEER_ID.as_bytes().to_owned().try_into().unwrap();
            let info_hash = torrent.info.calc_hash();

            let mut streams = futures::stream::iter(peers.iter())
                .map(|address| async move {
                    let my_handshake = peer::Handshake::new(info_hash, my_peer_id);
                    println!("Connecting to the peer. Address = {}", address);
                    let mut stream = tokio::net::TcpStream::connect(address).await?;
                    println!("Sending handshake. Address = {}", address);
                    {
                        let my_handshake_bytes = my_handshake.to_bytes();
                        stream.write_all(&my_handshake_bytes.as_slice()).await?;
                    }

                    println!("Receiving handshake. Address = {}", address);
                    let peer_handshake = {
                        let mut peer_handshake_bytes = vec![0; size_of::<peer::Handshake>()];
                        stream
                            .read_exact(peer_handshake_bytes.as_mut_slice())
                            .await?;
                        peer::Handshake::from_bytes(peer_handshake_bytes.as_slice())
                            .ok_or(Error::msg("Invalid size for handshake"))?
                    };
                    if my_handshake.info_hash != peer_handshake.info_hash {
                        return Err(Error::msg("info_hash from the peer does not match."));
                    }
                    Ok(stream)
                })
                .buffer_unordered(5)
                .collect::<Vec<Result<tokio::net::TcpStream, Error>>>()
                .await;

            let stream = streams[0].as_mut().unwrap();

            /*
            let mut stream = tokio_util::codec::Framed::new(stream, peer::MessageFramer);

            // 1. step: wait for Bitfield message
            println!("Waiting for Bitfield message.");
            let next_msg = stream
                .next()
                .await
                .expect("expecting messages")
                .expect("should be a valid message");
            println!("Received message is {:?}", next_msg);

            match next_msg {
                peer::Message::Bitfield(_) => {}
                _ => {
                    return Err(Error::msg(format!(
                        "Unexpected message type: Expected Bitfield, got {:?}",
                        next_msg
                    )))
                }
            }
            // 2. step: send Interested message.
            println!("Sending Interested message.");
            stream.send(peer::Message::Interested).await?;
            // 3. step: wait for the Unchoke message.
            println!("Waiting for Unchoke message.");
            let next_msg = stream
                .next()
                .await
                .expect("expecting messages")
                .expect("should be a valid message");
            println!("Received message is {:?}", next_msg);

            match next_msg {
                peer::Message::Unchoke => {}
                _ => {
                    return Err(Error::msg(format!(
                        "Unexpected message type: Expected Unchoke, got {:?}",
                        next_msg
                    )))
                }
            }

            // 4. step: send request for all blocks
            let requested_piece_size = if index == torrent.info.pieces.data.len() - 1 {
                torrent.info.length % torrent.info.piece_length
            } else {
                torrent.info.piece_length
            };

            println!("requested piece size = {}", requested_piece_size);

            let nr_of_blocks = requested_piece_size.div_ceil(BLOCK_SIZE);
            assert!(nr_of_blocks > 1);

            let mut piece_data: Vec<Vec<u8>> = vec![Vec::new(); nr_of_blocks];

            for block_nr in 0..nr_of_blocks {
                let cur_block_size = if block_nr != nr_of_blocks - 1 {
                    // not last block, always of block size.
                    BLOCK_SIZE
                } else if requested_piece_size % BLOCK_SIZE == 0 {
                    BLOCK_SIZE
                } else {
                    // last block and piece if not multiple of block size.
                    requested_piece_size % BLOCK_SIZE
                };
                println!("Sending Request message: index: {}", index);
                println!("                         block: {}", block_nr);
                println!("                         len:   {}", cur_block_size);
                let request = peer::Message::Request {
                    index: index as u32,
                    begin: (block_nr * BLOCK_SIZE) as u32,
                    length: cur_block_size as u32,
                };

                stream.send(request).await?;

                // 4. step: receive piece message
                let block = stream
                    .next()
                    .await
                    .expect("expecting piece message")
                    .expect("should be a valid message");
                let (received_index, begin, data) = match block {
                    peer::Message::Piece {
                        index,
                        begin,
                        block,
                    } => (index, begin, block),
                    _ => {
                        return Err(Error::msg(format!(
                            "Unexpected message type: Expected Piece, got {:?}",
                            next_msg
                        )))
                    }
                };
                assert!(index == received_index as usize);
                assert!(begin as usize % BLOCK_SIZE == 0);
                assert!(data.len() == cur_block_size);
                let block_index = begin as usize / BLOCK_SIZE;
                println!("Block received: index: {}", index);
                println!("                block: {}", block_index);
                piece_data[block_index] = data;
            }
            let mut hasher = Sha1::new();
            for block in piece_data {
                hasher.update(block);
            }
            let piece_hash = hasher.finalize();
            println!(
                "torrent hash:  {}",
                hex::encode(torrent.info.pieces.data[index])
            );
            println!("received hash: {}", hex::encode(piece_hash));

            */
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
