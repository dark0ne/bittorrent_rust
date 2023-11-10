use serde::{Deserialize, Deserializer, Serialize};
use std::{
    collections::btree_map,
    net::{Ipv4Addr, SocketAddrV4},
};

#[derive(Debug, Serialize)]
pub struct TrackerRequest {
    //info_hash: SingleHash,
    pub peer_id: String,
    pub port: u32,
    pub uploaded: usize,
    pub downloaded: usize,
    pub left: usize,
    pub compact: u8,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum TrackerResponse {
    Error {
        #[serde(rename = "failure reason")]
        failure_reason: String,
    },
    Peers {
        interval: usize,
        #[serde(rename = "min interval")]
        min_interval: Option<usize>,
        #[serde(rename = "tracker id")]
        tracker_id: String,
        complete: usize,
        incomplete: usize,
        #[serde(deserialize_with = "deser_socket_addr")]
        peers: Vec<SocketAddrV4>,
    },
}

fn deser_socket_addr<'de, D>(deserializer: D) -> Result<Vec<SocketAddrV4>, D::Error>
where
    D: Deserializer<'de>,
{
    let bytes: Vec<u8> = serde_bytes::deserialize(deserializer)?;
    let res = bytes
        .chunks_exact(6)
        .map(|buf| {
            SocketAddrV4::new(
                Ipv4Addr::new(buf[0], buf[1], buf[2], buf[3]),
                u16::from_be_bytes([buf[4], buf[5]]),
            )
        })
        .collect();
    Ok(res)
}
