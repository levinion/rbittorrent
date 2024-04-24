use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use crossbeam::{queue::ArrayQueue, sync::WaitGroup};
use log::error;

use crate::{message::Bitfield, peer::Peers, task::Task};

#[derive(Debug)]
pub struct TorrentClient {
    pub announce: String,
    pub info_hash: [u8; 20],
    pub piece_hashes: Vec<[u8; 20]>,
    pub piece_length: u32,
    pub length: u32,
    pub name: Arc<String>,
    pub id: [u8; 20],
    pub port: u16,
    pub task_queue: Arc<ArrayQueue<Task>>,
    pub bitfield: Bitfield,
}

impl TorrentClient {
    pub async fn look_for_peers(&self, peer_id: [u8; 20], port: u16) -> Result<Peers> {
        let info_hash_query = format!(
            "info_hash={}",
            url::form_urlencoded::byte_serialize(&self.info_hash[..]).collect::<String>()
        );
        let peer_id_query = format!(
            "peer_id={}",
            url::form_urlencoded::byte_serialize(&peer_id[..]).collect::<String>()
        );

        let mut url = url::Url::parse(&self.announce)?;
        url.set_query(Some(&format!("{}&{}", info_hash_query, peer_id_query)));

        let res = reqwest::ClientBuilder::new()
            .user_agent("rbittorrent/0.1.0")
            .build()?
            .get(url)
            .query(&[
                ("port", &port.to_string()),
                ("uploaded", &"0".to_string()),
                ("downloaded", &"0".to_string()),
                ("compact", &"1".to_string()),
                ("left", &self.length.to_string()),
            ])
            .send()
            .await
            .unwrap();

        Peers::new(
            &res.bytes().await?,
            self.task_queue.clone(),
            self.name.clone(),
        )
    }

    fn assign_tasks(&self) -> Result<()> {
        for index in 0..self.length / self.piece_length {
            if !self.bitfield.has_piece(index) {
                let task = Task::new(index, self.piece_length, self.piece_hashes[index as usize]);
                self.task_queue.push(task).unwrap();
            }
        }
        Ok(())
    }

    pub async fn send_request(&self) -> Result<()> {
        self.assign_tasks()?;
        let peers = self.look_for_peers(self.info_hash, self.port).await?;
        let wg = WaitGroup::new();
        for peer in peers.into_iter() {
            tokio::spawn({
                let wg = wg.clone();
                let info_hash = self.info_hash;
                let peer_id = self.id;
                async move {
                    if let Err(err) = peer.try_download(&info_hash, &peer_id).await {
                        error!("{}", err);
                    }
                    drop(wg)
                }
            });
        }
        wg.wait();
        self.concat_cache()?;
        Ok(())
    }

    #[inline]
    fn piece_num(&self) -> u32 {
        self.length / self.piece_length
    }

    fn concat_cache(&self) -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&*self.name)?;
        (0..self.piece_num())
            .map(|index| format!("{}-cache-{}", &self.name, index))
            .map(PathBuf::from)
            .filter(|path| path.is_file())
            .flat_map(|path| std::fs::OpenOptions::new().read(true).open(path))
            .try_for_each(|mut cache| {
                std::io::copy(&mut cache, &mut file)?;
                Ok::<(), anyhow::Error>(())
            })?;
        Ok(())
    }
}
