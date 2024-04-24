use bytes::Bytes;
use serde::{Deserialize, Serialize};
use sha1::Digest;

#[derive(Serialize, Deserialize, Debug)]
pub struct BencodeTorrent {
    pub announce: String,
    pub info: BencodeInfo,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BencodeInfo {
    pub pieces: Bytes,
    #[serde(rename = "piece length")]
    pub piece_length: u32,
    pub length: u32,
    pub name: String,
}

impl BencodeInfo {
    pub fn hash(&self) -> [u8; 20] {
        let buf = serde_bencode::to_bytes(self).unwrap();
        let mut hasher = sha1::Sha1::new();
        hasher.update(&buf);
        hasher.finalize().into()
    }
}
