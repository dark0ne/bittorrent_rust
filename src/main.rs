use anyhow::Error;
use hex;
use reqwest;
use sha1::{Digest, Sha1};
use std::{env, fs, path::Path};

mod hashes;
mod tracker;

use hashes::{Hashes, SingleHash};

#[derive(Debug, serde::Deserialize)]
struct Torrent {
    announce: String,

    info: Info,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
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

        //        println!("{}", unsafe {
        //            String::from_utf8_unchecked(info_ser.clone())
        //        });

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
            peer_id: "00112233445566778899".to_string(),
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

        println!("url: {}", full_url);

        let response = reqwest::blocking::get(full_url)
            .expect("GET for peers failed")
            .text()
            .unwrap();
        println!("response: {}", response);
        let response: tracker::TrackerResponse = serde_bencode::from_bytes(response.as_bytes())?;
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

// Available if you need it!
// use serde_bencode
/*
fn decode_bencoded_value(encoded_value: &str) -> (serde_json::Value, &str) {
    let mut input_string = encoded_value;
    if !input_string.is_empty() {
        match input_string.chars().next().unwrap() {
            '0'..='9' =>
            // Strings are encoded as <length>:<contents>.
            // Example: "5:hello" -> "hello"
            {
                if let Some((len, rest)) = input_string.split_once(':') {
                    if let Ok(len) = len
                        .parse::<usize>()
                        .context("Failed to parse string length")
                    {
                        let (string, rest) = rest.split_at(len);
                        return (string.into(), rest);
                    }
                }
            }
            'i' =>
            // Integers are encoded as i<number>e
            // Example: "i-5e" -> -5
            {
                if let Some((numb, rest)) = input_string[1..].split_once('e') {
                    if let Ok(numb) = numb.parse::<i64>().context("Failed to parse integer") {
                        return (numb.into(), rest);
                    }
                }
            }
            'l' =>
            // Lists are encoded as l<bencoded_elements>e.
            // Example: "l5:helloi52ee" -> ["hello", 52]
            {
                input_string = &input_string[1..];
                let mut list = Vec::new();
                while input_string.chars().next().unwrap() != 'e' {
                    let (list_value, rest) = decode_bencoded_value(input_string);
                    list.push(list_value);
                    input_string = rest;
                }
                return (list.into(), &input_string[1..]);
            }
            'd' =>
            //Dictionary is encoded as d<key1><value1>...<keyN><valueN>e.
            // <key1>, <value1> etc. correspond to the bencoded keys & values.
            // The keys are sorted in lexicographical order and must be strings
            // Example: "d3:foo3:bar5:helloi52ee" -> {"hello": 52, "foo":"bar"}
            {
                input_string = &input_string[1..];
                let mut dict = serde_json::Map::new();
                while input_string.chars().next().unwrap() != 'e' {
                    let (key, rest) = decode_bencoded_value(input_string);
                    let (value, rest) = decode_bencoded_value(rest);
                    if let Some(s) = key.as_str() {
                        dict.insert(s.into(), value);
                    }
                    input_string = rest;
                }
                return (dict.into(), &input_string[1..]);
            }

            _ => {}
        }
    }
    panic!("Unhandled encoded value: {}", encoded_value)
}
 */

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
