#[derive(Debug, Clone)]
pub struct Bitfield(Vec<u8>);

impl Bitfield {
    pub fn new(size: u32) -> Self {
        Self(vec![0u8; size as usize])
    }

    pub fn from(buf: &[u8]) -> Self {
        Self(buf.to_vec())
    }

    pub fn has_piece(&self, index: u32) -> bool {
        let byte_index = index / 8;
        let offset = index % 8;
        self.0[byte_index as usize] >> (7 - offset) & 1 != 0
    }

    #[allow(unused)]
    pub fn set_piece(&mut self, index: u32) {
        let byte_index = index / 8;
        let offset = index % 8;
        self.0[byte_index as usize] |= 1 << (7 - offset);
    }

    pub fn len(&self) -> u32 {
        self.0.len() as u32
    }
}
