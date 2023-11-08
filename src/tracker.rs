#[derive(Debug, serde::Serialize)]
pub struct TrackerRequest {
    //info_hash: SingleHash,
    pub peer_id: String,
    pub port: u32,
    pub uploaded: usize,
    pub downloaded: usize,
    pub left: usize,
    pub compact: u8,
}

pub struct TrackerResponse {}
