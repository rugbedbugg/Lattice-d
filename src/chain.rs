use crate::block::Block;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Blockchain {
    pub blocks: Vec<Block>,
}

impl Blockchain {
    pub fn new() -> Self {
        let genesis = Block::new(0, "genesis".to_string(), "0".repeat(64));
        Blockchain { blocks: vec![genesis] }
    }

    pub fn append(
            &mut self, 
            data: String
        ) {
        let prev        = self.blocks.last().unwrap();
        let prev_hash   = prev.hash.clone();
        let block       = Block::new(prev.index+1, data, prev_hash);
        
        self.blocks.push(block);
    }

    pub fn is_valid(&self) -> bool {
        for i in 1..self.blocks.len() {
            let current     = &self.blocks[i];
            let previous    = &self.blocks[i-1];
            if !current.is_valid()                 { return false; }
            if current.prev_hash != previous.hash  { return false; }
        }
        true
    }
}
