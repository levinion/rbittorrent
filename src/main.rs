use std::path::PathBuf;

use builder::TorrentClientBuilder;

mod bencode;
mod builder;
mod message;
mod peer;
mod task;
mod torrent;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();
    let path = PathBuf::from("./tests/debian-12.5.0-amd64-netinst.iso.torrent");
    let client = TorrentClientBuilder::new().add_path(path)?.build();
    client.send_request().await?;
    Ok(())
}
