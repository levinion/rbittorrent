#[derive(Debug, Clone, Copy)]
pub struct Task {
    pub index: u32,
    pub piece_length: u32,
    pub piece_hash: [u8; 20],
}

impl Task {
    pub fn new(index: u32, piece_length: u32, piece_hash: [u8; 20]) -> Self {
        Self {
            index,
            piece_length,
            piece_hash,
        }
    }
}
