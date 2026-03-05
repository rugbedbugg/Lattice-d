mod block;
mod chain;
mod watcher;
mod storage;

use chain::Blockchain;
use storage::Storage;
use std::sync::{Arc, Mutex};
use clap::{Parser, Subcommand};


#[derive(Parser)]
#[command(name = "latticed", about = "Tamper-evident filesystem audit daemon")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the daemon
    Start,
    /// Verify chain integrity
    Verify,
}

fn main() {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Start) {
        Commands::Start => start(),
        Commands::Verify => verify(),
    }
}


fn start() {
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


    //----------------------//
    //--- Signal Handler ---//
    //----------------------//
    let store_signal = Arc::clone(&store);
    ctrlc::set_handler(move || {
        println!("\n[Lattice-d] Shutdown signal received, flushing...");
        let mut s = store_signal.lock().unwrap();
        s.flush();
        println!("[Lattice-d] Flush complete. Goodbye.");
        std::process::exit(0);
    }).expect("[Lattice-d] Failed to set signal handler");
    
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

fn verify() {
    use std::fs;
    use crate::block::Block;

    println!("[Lattice-d] Verifying chain integrity...");

    let p = Storage::path(storage::CHAIN_FILE);
    if !p.exists() {
        println!("[Lattice-d] No chain file found at {:?}", p);
        std::process::exit(1);
    }

    let contents = fs::read_to_string(&p)
        .expect("[Lattice-d] Failed to read chain file");

    let blocks: Vec<Block> = contents
        .lines()
        .filter(|l| !l.is_empty())
        .enumerate()
        .map(|(i, line)| {
            serde_json::from_str(line)
                .unwrap_or_else(|_| panic!("[Lattice-d] Failed to parse block at line {}", i))
        })
        .collect();

    if blocks.is_empty() {
        println!("[Lattice-d] Chain is empty.");
        std::process::exit(1);
    }

    let mut ok = true;
    for i in 1..blocks.len() {
        let current  = &blocks[i];
        let previous = &blocks[i - 1];

        // recompute hash and compare
        let recomputed = Block::compute_hash(
            current.index,
            current.timestamp,
            &current.data,
            &current.prev_hash,
        );

        if recomputed != current.hash {
            println!(
                "[Lattice-d] TAMPER DETECTED at block #{} --> hash mismatch",
                current.index
            );
            ok = false;
        }

        if current.prev_hash != previous.hash {
            println!(
                "[Lattice-d] TAMPER DETECTED at block #{} --> broken chain link",
                current.index
            );
            ok = false;
        }
    }

    if ok {
        println!(
            "[Lattice-d] Chain OK --> {} blocks verified, integrity intact",
            blocks.len()
        );
        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
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
