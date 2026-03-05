mod block;
mod chain;
mod watcher;
mod storage;

use chain::Blockchain;
use storage::Storage;
use std::sync::{Arc, Mutex};


fn main() {
    println!("[Lattice-d] starting...");

    let store = Storage::new();

    // Load existing chain or start fresh
    let blockchain = match Storage::last_block() {
        Some(last) => {
            println!("[Lattice-d] Loaded existing chain ({} blocks)", last.index);
            let mut c = Blockchain::new();
            c.blocks[0] = last;
            c
        }
        None => {
            println!("[Lattice-d] Not existing chain found, starting fresh");
            Blockchain::new()
        }
    };

    let chain = Arc::new(Mutex::new(blockchain));
    let store = Arc::new(Mutex::new(store));
    let watched_paths = vec!["/etc", "/var/log", "/bin", "/usr/bin"];

    watcher::watch(watched_paths, |event| {
        let mut c = chain.lock().unwrap();
        let mut s = store.lock().unwrap();

        c.append(event.clone());

        let latest = c.blocks.last().unwrap().clone();
        let log_entry = format!("[Latttice-d] Block #{} | {}", latest.index, latest.hash);

        println!("{}", log_entry);
        s.append_log(&log_entry);
        s.push(latest);
    });
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_block_exists() {
        let chain = Blockchain::new();
        assert_eq!(chain.blocks.len(), 1);
        assert_eq!(chain.blocks[0].data, "genesis");
    }

    #[test]
    fn test_chain_grows_on_append() {
        let mut chain = Blockchain::new();
        chain.append("event one".to_string());
        chain.append("event two".to_string());
        assert_eq!(chain.blocks.len(), 3);
    }

    #[test]
    fn test_valid_chain_passes() {
        let mut chain = Blockchain::new();
        chain.append("/etc/passwd accessed".to_string());
        assert!(chain.is_valid());
    }

    #[test]
    fn test_tampered_data_fails() {
        let mut chain = Blockchain::new();
        chain.append("legit event".to_string());
        chain.blocks[1].data = "tampered".to_string();
        assert!(!chain.is_valid());
    }

    #[test]
    fn test_tampered_hash_fails() {
        let mut chain = Blockchain::new();
        chain.append("legit event".to_string());
        chain.blocks[1].hash = "a".repeat(64);
        assert!(!chain.is_valid());
    }

    #[test]
    fn test_prev_hash_linkage() {
        let mut chain = Blockchain::new();
        chain.append("event".to_string());
        let b1_hash = chain.blocks[1].hash.clone();
        chain.append("event 2".to_string());
        assert_eq!(chain.blocks[2].prev_hash, b1_hash);
    }
}
