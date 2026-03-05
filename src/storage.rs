use std::fs::{self, OpenOptions};
use std::io::{Write, BufWriter};
use std::path::{Path, PathBuf};
use crate::block::Block;


const STORAGE_DIR: &str   = "/var/lib/latticed";
pub const CHAIN_FILE: &str    = "chain.jsonl";      // jsonl = one block per line
pub const LOG_FILE: &str      = "latticed.log";
pub const MAX_SIZE_BYTES: u64 = 1_000_000;          // 1MB
pub const FLUSH_EVERY: usize  = 50;                 // blocks per flush
const MAX_BACKUPS: u32    = 3;


pub struct Storage {
    pub buffer: Vec<Block>,
    pub dir: PathBuf,
}

impl Storage {
    pub fn new() -> Self {
        Self::with_dir(Path::new(STORAGE_DIR))
    }

    pub fn with_dir(dir: &Path) -> Self {
        fs::create_dir_all(dir)
            .expect("[Lattice-d] Failed to create storage dir");
        Storage { 
            buffer: Vec::with_capacity(FLUSH_EVERY),
            dir: dir.to_path_buf(),
        }
    }

    pub fn path(&self, filename: &str) -> PathBuf {
        self.dir.join(filename)
    }

    //----------------//
    //--- rotation ---//
    //----------------//
    fn rotate(&self, filename: &str) {
        let base = self.path(filename);
        if !base.exists() { return; }
        let meta = fs::metadata(&base).unwrap();
        if meta.len() < MAX_SIZE_BYTES { return; }

        // delete oldest backup if at limit
        let oldest = self.path(&format!("{}.bak.{}", filename, MAX_BACKUPS));
        if oldest.exists() { fs::remove_file(&oldest).unwrap(); }

        // shift existing backups up
        for i in (1..MAX_BACKUPS).rev() {
            let from = self.path(&format!("{}.bak.{}", filename, i));
            let to   = self.path(&format!("{}.bak.{}", filename, i + 1));
            if from.exists() { fs::rename(&from, &to).unwrap(); }
        }

        // current becomes .bak.1
        fs::rename(&base, self.path(&format!("{}.bak.1", filename))).unwrap();
    }


    //---------------------------------------//
    //--- chain persistence (append-only) ---//
    //---------------------------------------//
    //--- one JSON per line
    pub fn push(&mut self, block: Block) {
        self.buffer.push(block);
        if self.buffer.len() >= FLUSH_EVERY {
            self.flush();
        }
    }

    pub fn flush(&mut self) {
        if self.buffer.is_empty() { return; }
        self.rotate(CHAIN_FILE);

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.path(CHAIN_FILE))
            .expect("[Lattice-d] Failed to open chain file");

        let mut writer = BufWriter::new(file);
        for block in &self.buffer {
            let line = serde_json::to_string(block)
                .expect("[Lattice-d] Failed to serialize block");
            writeln!(writer, "{}", line)
                .expect("[Lattice-d] Failed to write block");
        }
        writer.flush().expect("[Lattice-d] Failed to flush writer");
        self.buffer.clear();
    }


    //--------------------------//
    //--- human-readable log ---//
    //--------------------------//
    pub fn append_log(&self, entry: &str) {
        self.rotate(LOG_FILE);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.path(LOG_FILE))
            .expect("[Lattice-d] Failed to open log file");
        let mut writer = BufWriter::new(file);
        writeln!(writer, "{}", entry)
            .expect("[Lattice-d] Failed to write log entry");
    }


    //-------------------------------------//
    //--- Load existing chain for reuse ---//
    //-------------------------------------//
    pub fn last_block(&self) -> Option<Block> {
        let p = self.path(CHAIN_FILE);
        if !p.exists() { return None; }
        let contents = fs::read_to_string(&p)
            .expect("[Lattice-d] Failed to read chain file");
        contents.lines()
            .last()
            .and_then(|line| serde_json::from_str(line).ok())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::Block;
    use tempfile::tempdir;

    fn dummy_block(index: u64, prev_hash: &str) -> Block {
        Block::new(index, format!("test event {}", index), prev_hash.to_string())
    }

    #[test]
    fn test_buffer_accumulates_without_flush() {
        let tmp = tempdir().unwrap();
        let mut s = Storage::with_dir(tmp.path());
        let initial_len = s.buffer.len();
        s.buffer.push(dummy_block(1, &"0".repeat(64)));
        assert_eq!(s.buffer.len(), initial_len + 1);
    }

    #[test]
    fn test_manual_flush_clears_buffer() {
        let tmp = tempdir().unwrap();
        let mut s = Storage::with_dir(tmp.path());
        s.buffer.push(dummy_block(1, &"0".repeat(64)));
        s.buffer.push(dummy_block(2, &"0".repeat(64)));
        s.flush();
        assert_eq!(s.buffer.len(), 0);
    }

    #[test]
    fn test_flush_writes_to_disk() {
        let tmp = tempdir().unwrap();
        let mut s = Storage::with_dir(tmp.path());
        let block = dummy_block(99, &"0".repeat(64));
        s.buffer.push(block.clone());
        s.flush();

        let contents = std::fs::read_to_string(
            s.path(CHAIN_FILE)
        ).unwrap();

        assert!(contents.contains("\"index\":99"));
    }

    #[test]
    fn test_last_block_resumes_correctly() {
        let tmp = tempdir().unwrap();
        let mut s = Storage::with_dir(tmp.path());
        let block = dummy_block(42, &"0".repeat(64));
        s.buffer.push(block.clone());
        s.flush();

        let last = s.last_block().unwrap();
        assert_eq!(last.index, 42);
    }

    #[test]
    fn test_auto_flush_at_threshold() {
        let tmp = tempdir().unwrap();
        let mut s = Storage::with_dir(tmp.path());
        let prev = "0".repeat(64);

        for i in 0..FLUSH_EVERY {
            s.push(dummy_block(i as u64, &prev));
        }

        // buffer should have been auto-flushed and cleared
        assert_eq!(s.buffer.len(), 0);
    }

    #[test]
    fn test_rotation_renames_at_size_limit() {
        // write a file that exceeds MAX_SIZE_BYTES
        let tmp = tempdir().unwrap();
        let s = Storage::with_dir(tmp.path());
        let p = s.path(CHAIN_FILE);
        let big_data = "x".repeat((MAX_SIZE_BYTES + 1) as usize);
        std::fs::write(&p, big_data).unwrap();

        s.rotate(CHAIN_FILE);

        let bak = s.path(&format!("{}.bak.1", CHAIN_FILE));
        assert!(bak.exists(), "bak.1 should exist after rotation");
        assert!(!p.exists(), "original should be gone after rotation");
    }
}
