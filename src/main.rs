use anyhow::Context;
use serde_json;
use std::env;

// Available if you need it!
// use serde_bencode

#[allow(dead_code)]
fn decode_bencoded_value(encoded_value: &str) -> (serde_json::Value, &str) {
    if !encoded_value.is_empty() {
        match encoded_value.chars().next().unwrap() {
            '0'..='9' =>
            // Strings are encoded as <length>:<contents>.
            // Example: "5:hello" -> "hello"
            {
                if let Some((len, rest)) = encoded_value.split_once(':') {
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
                if let Some((numb, rest)) = encoded_value[1..].split_once('e') {
                    if let Ok(numb) = numb.parse::<i64>().context("Failed to parse integer") {
                        return (numb.into(), rest);
                    }
                }
            }
            _ => {}
        }
    }
    panic!("Unhandled encoded value: {}", encoded_value)
}

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        println!("{}", decoded_value.0.to_string());
    } else {
        println!("unknown command: {}", args[1])
    }
}
