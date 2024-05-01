mod bitfield;
mod handshake;
mod request;
use std::{fmt::Display, time::Duration};

use anyhow::Result;
pub use bitfield::Bitfield;
pub use handshake::HandShake;
pub use request::*;
use tokio::{io::AsyncReadExt, net::TcpStream, time::timeout};

#[derive(Debug, Clone)]
pub enum Message {
    Choke,
    UnChoke,
    Interested,
    NotInterested,
    #[allow(unused)]
    Have(u8),
    Bitfield(Bitfield),
    Request(Request),
    Piece(Piece),
    #[allow(unused)]
    Cancel(Cancel),
    KeepAlive,
    HandShake(HandShake),
}

pub enum MessageError {
    Timeout,
    ReadError,
    HandShakeError,
}

impl Display for MessageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout => f.write_str("timeout"),
            Self::ReadError => f.write_str("read error"),
            Self::HandShakeError => f.write_str("handshake error"),
        }
    }
}

impl Message {
    pub async fn from_stream(stream: &mut TcpStream) -> Result<Self, MessageError> {
        let dw = timeout(Duration::from_secs(3), stream.read_u32())
            .await
            .map_err(|_| MessageError::Timeout)?
            .map_err(|_| MessageError::ReadError)?;
        let mut buf = dw.to_be_bytes().to_vec();
        let msg = if buf[0] == 19 && buf[1..4] == *b"Bit" {
            let mut other = vec![0; 64];
            timeout(Duration::from_secs(3), stream.read_exact(&mut other))
                .await
                .map_err(|_| MessageError::Timeout)?
                .map_err(|_| MessageError::HandShakeError)?; // exit if
            buf.extend(other);
            Self::HandShake(HandShake::from_bytes(&buf))
        } else {
            let length = dw as usize;
            let mut other = vec![0; length];
            timeout(Duration::from_secs(3), stream.read_exact(&mut other))
                .await
                .map_err(|_| MessageError::Timeout)?
                .map_err(|_| MessageError::ReadError)?;
            buf.extend(other);
            Self::from(&buf[4..4 + length])
        };
        Ok(msg)
    }

    fn from(buf: &[u8]) -> Self {
        if buf.is_empty() {
            return Self::KeepAlive;
        }
        match buf[0] {
            0 => Self::Choke,
            1 => Self::UnChoke,
            2 => Self::Interested,
            3 => Self::NotInterested,
            4 => Self::Have(buf[1]),
            5 => Self::Bitfield(Bitfield::from(&buf[1..])),
            6 => Self::Request(Request::from_bytes(&buf[1..])),
            7 => Self::Piece(Piece::from_bytes(&buf[1..])),
            8 => Self::Cancel(Cancel::from_bytes(&buf[1..])),
            _ => unreachable!(),
        }
    }

    fn as_u8(&self) -> u8 {
        match self {
            Self::Choke => 0,
            Self::UnChoke => 1,
            Self::Interested => 2,
            Self::NotInterested => 3,
            Self::Have(_) => 4,
            Self::Bitfield(_) => 5,
            Self::Request(_) => 6,
            Self::Piece(_) => 7,
            Self::Cancel(_) => 8,
            _ => unreachable!(),
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        match self {
            Self::HandShake(handshake) => handshake.as_bytes(),
            Self::KeepAlive => vec![],
            Self::Choke | Self::UnChoke | Self::Interested | Self::NotInterested => {
                no_body_message(self.as_u8())
            }
            Self::Request(request) => request.as_bytes(),
            _ => todo!(),
        }
    }
}

fn no_body_message(code: u8) -> Vec<u8> {
    let mut bytes = 1_u32.to_be_bytes().to_vec();
    bytes.push(code);
    bytes
}
