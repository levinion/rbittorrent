use std::{
    fs::create_dir_all,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddrV4},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use anyhow::{anyhow, Result};
use bytes::Bytes;
use crossbeam::queue::ArrayQueue;
use indicatif::ProgressBar;
use log::info;
use serde::{Deserialize, Serialize};
use sha1::Digest;
use tokio::{io::AsyncWriteExt, net::TcpStream, time::timeout};

use crate::{
    message::{Bitfield, HandShake, Message, Piece, Request},
    task::Task,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerState {
    Preparing,
    Busy,
}

#[derive(Debug)]
pub struct Peer {
    pub ip: Ipv4Addr,
    pub port: u16,
    pub state: PeerState,
    pub id: Option<[u8; 20]>,
    pub stream: Option<TcpStream>,
    pub bitfield: Option<Bitfield>,
    pub task_queue: Arc<ArrayQueue<Task>>,
    pub current_task: Option<Task>,
    pub name: Arc<String>,
    pub pb: ProgressBar,
    pub received_pieces: Vec<Piece>,
}

#[derive(Serialize, Deserialize)]
pub struct TrackerReport {
    interval: i64,
    peers: Bytes,
}

#[derive(Debug)]
pub struct Peers(Vec<Peer>);

impl Peers {
    pub fn new(
        buf: &[u8],
        task_queue: Arc<ArrayQueue<Task>>,
        name: Arc<String>,
        pb: ProgressBar,
    ) -> Result<Self> {
        let tracker_report: TrackerReport = serde_bencode::from_bytes(buf)?;
        let buf = tracker_report.peers;

        assert!(buf.len() % 6 == 0);
        let peers: Vec<_> = (0..buf.len() / 6)
            .map(|i| {
                let offset = 6 * i;
                let ip_bits = u32::from_be_bytes(buf[offset..offset + 4].try_into().unwrap());
                let port = u16::from_be_bytes(buf[offset + 4..offset + 6].try_into().unwrap());
                Peer {
                    ip: Ipv4Addr::from(ip_bits),
                    port,
                    state: PeerState::Preparing,
                    id: None,
                    stream: None,
                    bitfield: None,
                    task_queue: task_queue.clone(),
                    current_task: None,
                    name: name.clone(),
                    pb: pb.clone(),
                    received_pieces: vec![],
                }
            })
            .collect();
        Ok(Self(peers))
    }

    #[allow(unused)]
    pub fn iter(&self) -> impl Iterator<Item = &Peer> {
        self.0.iter()
    }

    #[allow(unused)]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Peer> {
        self.0.iter_mut()
    }
}

impl IntoIterator for Peers {
    type Item = Peer;
    type IntoIter = <Vec<Peer> as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

enum PeerEvent {
    Continue,
    Exit,
}

impl Peer {
    const BLOCK_SIZE: u32 = 2_u32.pow(14);

    fn has_piece(&self, index: u32) -> bool {
        self.bitfield.as_ref().unwrap().has_piece(index)
    }

    fn is_current_task_done(&self) -> Option<bool> {
        if self.current_task.is_some() {
            Some(
                self.received_pieces.len() as u32 * Self::BLOCK_SIZE
                    >= self.current_task.as_ref().unwrap().piece_length,
            )
        } else {
            None
        }
    }

    /// if current task is done or none, fetch task from queue
    async fn try_fetch_task(&mut self) -> Result<PeerEvent> {
        match self.is_current_task_done() {
            // task exists and done
            Some(true) => {
                self.save_pieces()?;
                if let Err(err) = self.check_sum() {
                    info!("{}", err);
                    self.put_task_back();
                } else {
                    info!(
                        "piece #{} downloaded successfully",
                        self.current_task.as_ref().unwrap().index
                    );
                }
                self.fetch_task()?;
                self.request_piece().await?;
                Ok(PeerEvent::Continue)
            }
            // task exists and not done
            Some(false) => Ok(PeerEvent::Continue),
            // task not exists
            None => self.fetch_task(),
        }
    }

    fn fetch_task(&mut self) -> Result<PeerEvent> {
        let task = match self.task_queue.pop() {
            Some(task) => task,
            None => return Ok(PeerEvent::Exit),
        };
        self.current_task = Some(task);
        Ok(PeerEvent::Continue)
    }

    fn put_task_back(&mut self) {
        self.task_queue
            .push(self.current_task.take().unwrap())
            .unwrap();
    }

    async fn try_connect(&mut self) -> Result<()> {
        let stream = timeout(
            Duration::from_secs(3),
            TcpStream::connect(SocketAddrV4::new(self.ip, self.port)),
        )
        .await??;
        self.stream = Some(stream);
        info!("peer connected: {}", self.ip);
        Ok(())
    }

    async fn send_message(&mut self, msg: Message) -> Result<()> {
        self.stream
            .as_mut()
            .unwrap()
            .write_all(&msg.as_bytes())
            .await?;
        Ok(())
    }

    /// read and process message
    async fn read_message(&mut self) -> Result<PeerEvent> {
        if let Ok(msg) = Message::from_stream(self.stream.as_mut().unwrap()).await {
            self.process_msg(msg).await
        } else {
            Ok(PeerEvent::Continue)
        }
    }

    /// send request to peer
    async fn request_piece(&mut self) -> Result<()> {
        info!("send request to peer: {}", self.ip);
        let mut offset = 0;
        while offset < self.current_task.as_ref().unwrap().piece_length / Self::BLOCK_SIZE {
            self.send_message(Message::Request(Request::new(
                self.current_task.as_ref().unwrap().index,
                offset * Self::BLOCK_SIZE,
                Self::BLOCK_SIZE,
            )))
            .await?;
            offset += 1;
        }
        Ok(())
    }

    async fn process_msg(&mut self, msg: Message) -> Result<PeerEvent> {
        match msg {
            Message::HandShake(handshake) => {
                self.id = Some(handshake.peer_id);
                info!("handshake success with peer: {}", self.ip);
                self.send_message(Message::UnChoke).await?;
            }
            Message::Bitfield(bitfield) => {
                info!(
                    "get bitfield, length={}, from peer: {}",
                    bitfield.len(),
                    self.ip
                );
                self.bitfield = Some(bitfield);
                if let Ok(PeerEvent::Exit) = self.try_fetch_task().await {
                    return Ok(PeerEvent::Exit);
                }
                loop {
                    let index = self.current_task.as_ref().unwrap().index;
                    if !self.has_piece(index) {
                        self.put_task_back();
                        self.fetch_task()?;
                    } else {
                        self.send_message(Message::Interested).await?;
                        break;
                    }
                }
            }
            Message::Piece(piece) => {
                info!(
                    "download #{} block of #{} piece from peer: {}",
                    piece.begin / Self::BLOCK_SIZE,
                    piece.index,
                    self.ip
                );
                self.pb.inc(piece.piece.len() as _);
                self.received_pieces.push(piece);
                if let Ok(PeerEvent::Exit) = self.try_fetch_task().await {
                    return Ok(PeerEvent::Exit);
                }
            }
            Message::UnChoke => {
                info!("peer is unchoked: {}", self.ip);
                if self.state == PeerState::Busy {
                    return Ok(PeerEvent::Continue);
                }
                self.request_piece().await?;
                self.state = PeerState::Busy;
            }
            _ => {}
        }
        Ok(PeerEvent::Continue)
    }

    pub async fn handshake(&mut self, info_hash: &[u8], peer_id: &[u8]) -> Result<()> {
        self.state = PeerState::Preparing;
        self.send_message(Message::HandShake(HandShake::new(info_hash, peer_id)))
            .await?;
        Ok(())
    }

    pub async fn try_download(mut self, info_hash: &[u8], peer_id: &[u8]) -> Result<()> {
        self.try_connect().await?;
        self.handshake(info_hash, peer_id).await?;
        loop {
            match self.read_message().await {
                Ok(event) => match event {
                    PeerEvent::Continue => {}
                    PeerEvent::Exit => {
                        info!("peer {} disconnect while all tasks are done", self.ip);
                        break;
                    }
                },
                Err(err) => {
                    info!("peer {} disconnect cause of fatal error: {}", self.ip, err);
                    break;
                }
            }
        }
        Ok(())
    }

    fn check_sum(&self) -> Result<()> {
        let dir_path = PathBuf::from(&*self.name);
        let task = self.current_task.as_ref().unwrap();
        let cache_path = dir_path.join(format!("{}-cache-{}", &self.name, task.index));
        let mut cache_file = std::fs::OpenOptions::new().read(true).open(cache_path)?;
        let mut buf = Vec::new();
        cache_file.read_to_end(&mut buf)?;
        let mut hasher = sha1::Sha1::new();
        hasher.update(&buf);
        let sum: [u8; 20] = hasher.finalize().into();
        if task.piece_hash != sum {
            Err(anyhow!(
                "piece #{} has a wrong hash, expected: {:x?}, found: {:x?}",
                task.index,
                &task.piece_hash,
                &sum
            ))
        } else {
            Ok(())
        }
    }

    fn save_pieces(&mut self) -> Result<()> {
        let dir_path = PathBuf::from(&*self.name);
        if !dir_path.is_dir() {
            create_dir_all(&dir_path)?;
        }
        let cache_path = dir_path.join(format!(
            "{}-cache-{}",
            &self.name,
            self.current_task.as_ref().unwrap().index
        ));
        let mut cache_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(cache_path)?;
        let mut pieces = self.received_pieces.drain(0..).collect::<Vec<_>>();
        pieces.sort_by_key(|piece| piece.begin);
        pieces.into_iter().try_for_each(|piece| {
            cache_file.write_all(&piece.piece)?;
            anyhow::Ok(())
        })?;
        Ok(())
    }
}
