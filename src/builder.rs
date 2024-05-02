use std::{path::Path, sync::Arc};

use anyhow::Result;
use crossbeam::queue::ArrayQueue;
use indicatif::{ProgressBar, ProgressStyle};

use crate::{bencode::BencodeTorrent, message::Bitfield, torrent::TorrentClient};

#[derive(Debug, Default)]
pub struct TorrentClientBuilder {
    announce: Option<String>,
    info_hash: Option<[u8; 20]>,
    piece_hashes: Option<Vec<[u8; 20]>>,
    piece_length: Option<u32>,
    length: Option<u32>,
    name: Option<String>,
    id: Option<[u8; 20]>,
    port: Option<u16>,
}

impl TorrentClientBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn add_torrent_path<T>(self, path: T) -> Result<Self>
    where
        T: AsRef<Path>,
    {
        let bytes = std::fs::read(path.as_ref())?;
        self.add_torrent_bytes(&bytes)
    }

    pub async fn add_torrent_url(self, url: &str) -> Result<Self> {
        let client = reqwest::ClientBuilder::new()
            .user_agent("rbittorrent/v0.1.3")
            .build()?;
        let bytes = client.get(url).send().await?.bytes().await?;
        self.add_torrent_bytes(&bytes)
    }

    pub fn add_torrent_bytes(mut self, bytes: &[u8]) -> Result<Self> {
        let torrent: BencodeTorrent = serde_bencode::from_bytes(bytes)?;
        let info_hash = torrent.info.hash();
        let piece_hashes = {
            torrent
                .info
                .pieces
                .chunks(20)
                .map(|chunk| chunk[..20].try_into().unwrap())
                .collect()
        };
        self.announce = Some(torrent.announce);
        self.name = Some(torrent.info.name);
        self.length = Some(torrent.info.length);
        self.piece_length = Some(torrent.info.piece_length);
        self.info_hash = Some(info_hash);
        self.piece_hashes = Some(piece_hashes);

        Ok(self)
    }

    #[allow(unused)]
    pub fn set_peer_id(mut self, id: [u8; 20]) -> Self {
        self.id = Some(id);
        self
    }

    #[allow(unused)]
    pub fn set_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    fn piece_num(&self) -> u32 {
        self.length.unwrap() / self.piece_length.unwrap()
    }

    pub fn build(self) -> TorrentClient {
        let piece_num = self.piece_num();
        let task_queue = ArrayQueue::new(piece_num as usize);
        let pb = {
            let pb = ProgressBar::new(self.length.unwrap() as _);
            pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        .unwrap());
            pb
        };
        TorrentClient {
            announce: self.announce.unwrap(),
            info_hash: self.info_hash.unwrap(),
            piece_hashes: self.piece_hashes.unwrap(),
            piece_length: self.piece_length.unwrap(),
            name: Arc::new(self.name.unwrap()),
            length: self.length.unwrap(),
            id: self.id.unwrap_or(*b"-RT0001-123456012345"),
            port: self.port.unwrap_or(6881),
            task_queue: Arc::new(task_queue),
            bitfield: Bitfield::new(piece_num),
            pb,
        }
    }
}
