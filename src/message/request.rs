use bytes::{BufMut, BytesMut};

#[derive(Debug, Clone, Copy)]
pub struct Request {
    pub index: u32,
    pub begin: u32,
    pub length: u32,
}

impl Request {
    pub fn new(index: u32, begin: u32, length: u32) -> Self {
        Self {
            index,
            begin,
            length,
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut buf = BytesMut::new();
        buf.put_u32(13);
        buf.put_u8(6);
        buf.put_u32(self.index);
        buf.put_u32(self.begin);
        buf.put_u32(self.length);
        buf.to_vec()
    }

    pub fn from_bytes(buf: &[u8]) -> Self {
        assert!(buf.len() == 12);
        let index = u32::from_be_bytes(buf[..4].try_into().unwrap());
        let begin = u32::from_be_bytes(buf[4..8].try_into().unwrap());
        let length = u32::from_be_bytes(buf[8..12].try_into().unwrap());
        Self {
            index,
            begin,
            length,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Piece {
    pub index: u32,
    #[allow(unused)]
    pub begin: u32,
    pub piece: Vec<u8>,
}

impl Piece {
    #[allow(unused)]
    pub fn new(index: u32, begin: u32, piece: &[u8]) -> Self {
        Self {
            index,
            begin,
            piece: piece.to_vec(),
        }
    }

    pub fn from_bytes(buf: &[u8]) -> Self {
        let index = u32::from_be_bytes(buf[..4].try_into().unwrap());
        let begin = u32::from_be_bytes(buf[4..8].try_into().unwrap());
        Self {
            index,
            begin,
            piece: buf[8..].to_vec(),
        }
    }
}

#[derive(Debug, Clone)]
#[allow(unused)]
pub struct Cancel {
    pub index: u32,
    pub begin: u32,
    pub length: u32,
}

#[allow(unused)]
impl Cancel {
    pub fn new(index: u32, begin: u32, length: u32) -> Self {
        Self {
            index,
            begin,
            length,
        }
    }

    pub fn from_bytes(buf: &[u8]) -> Self {
        assert!(buf.len() == 12);
        let index = u32::from_be_bytes(buf[..4].try_into().unwrap());
        let begin = u32::from_be_bytes(buf[4..8].try_into().unwrap());
        let length = u32::from_be_bytes(buf[8..12].try_into().unwrap());
        Self {
            index,
            begin,
            length,
        }
    }
}
