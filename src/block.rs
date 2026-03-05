use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};
use chrono::Utc;


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub index: u64,
    pub timestamp: i64,
    pub data: String,
    pub prev_hash: String,
    pub hash: String,
}

impl Block {
    pub fn new(
            index: u64,
            data: String,
            prev_hash: String
        )-> Self {
        let timestamp = Utc::now().timestamp();
        let hash = Self::compute_hash(index, timestamp, &data, &prev_hash);
        
        Block { index, timestamp, data, prev_hash, hash }
    }

    pub fn compute_hash(
            index: u64,
            timestamp: i64,
            data: &str,
            prev_hash: &str
        )-> String {
        let input = format!("{}{}{}{}", index, timestamp, data, prev_hash);
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        
        hex::encode(hasher.finalize())
    }

    pub fn is_valid(&self) -> bool {
        let recomputed = Self::compute_hash(
            self.index,
            self.timestamp,
            &self.data,
            &self.prev_hash);

        recomputed == self.hash
    }
}
