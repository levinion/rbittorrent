use rbittorrent::TorrentClientBuilder;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let path = PathBuf::from("./tests/debian-12.5.0-amd64-netinst.iso.torrent");
    let client = TorrentClientBuilder::new().add_torrent_path(path)?.build();
    client.send_request().await?;
    Ok(())
}
