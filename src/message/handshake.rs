use bytes::{BufMut, BytesMut};

#[derive(Debug, Clone, Copy)]
pub struct HandShake {
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
}

impl HandShake {
    pub fn new(info_hash: &[u8], peer_id: &[u8]) -> Self {
        Self {
            info_hash: info_hash.try_into().unwrap(),
            peer_id: peer_id.try_into().unwrap(),
        }
    }

    pub fn from_bytes(buf: &[u8]) -> Self {
        assert!(buf.len() >= 68);
        assert!(buf[0] == 19);
        assert!(buf[1..20] == *b"BitTorrent protocol");
        let info_hash = buf[28..48].try_into().unwrap();
        let peer_id = buf[48..68].try_into().unwrap();
        Self { info_hash, peer_id }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut buf = BytesMut::with_capacity(68);
        buf.put_u8(19); // len of pstr
        buf.put_slice(b"BitTorrent protocol");
        buf.put_slice(&[0u8; 8]);
        buf.put_slice(&self.info_hash);
        buf.put_slice(&self.peer_id);
        buf.to_vec()
    }
}
