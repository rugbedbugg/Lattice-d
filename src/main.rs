mod block;
mod chain;
mod watcher;
mod storage;
mod sign;

use chain::Blockchain;
use storage::Storage;
use ed25519_dalek::SigningKey;
use sign::{PUB_FILE, KEY_FILE};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use clap::{Parser, Subcommand};

const CHECKPOINT_INTERVAL_SECS: u64 = 60;


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
    /// Verify chain integrity and signed checkpoints
    Verify,
    /// Generate an Ed25519 signing keypair for chain checkpoints
    Keygen,
}

fn main() {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Start) {
        Commands::Start => start(),
        Commands::Verify => verify(),
        Commands::Keygen => keygen(),
    }
}


fn keygen() {
    let store = Storage::new();
    let pub_path = store.path(PUB_FILE);
    if pub_path.exists() {
        println!("[Lattice-d] Keypair already exists at {:?}", store.path(KEY_FILE));
        println!("[Lattice-d] Delete it first if you want to regenerate.");
        std::process::exit(1);
    }

    sign::generate_keypair(&store.dir);

    println!();
    println!("[Lattice-d] IMPORTANT: keep the secret key OFF this machine.");
    println!("[Lattice-d]   1. Copy {:?} to external media or another host.", store.path(KEY_FILE));
    println!("[Lattice-d]   2. DELETE {:?} from this machine.", store.path(KEY_FILE));
    println!("[Lattice-d] Only {} must remain locally (verify reads it).", PUB_FILE);
}

fn load_signing_key_if_present(store: &Storage) -> Option<SigningKey> {
    let key_path = store.path(KEY_FILE);
    if !key_path.exists() {
        return None;
    }
    Some(sign::load_signing_key(&key_path))
}

fn start() {
    println!("[Lattice-d] starting...");

    let store = Storage::new();

    // Load existing chain or start fresh
    let blockchain = match store.last_block() {
        Some(last) => {
            println!("[Lattice-d] Loaded existing chain ({} blocks)", last.index);
            let mut c = Blockchain::new();
            c.blocks[0] = last;
            c
        }
        None => {
            println!("[Lattice-d] No existing chain found, starting fresh");
            Blockchain::new()
        }
    };

    let chain = Arc::new(Mutex::new(blockchain));
    let store = Arc::new(Mutex::new(store));

    //----------------------//
    //--- Checkpoint keys ---//
    //----------------------//
    let signing_key_guard = {
        let s = store.lock().unwrap();
        load_signing_key_if_present(&s)
    };
    match &signing_key_guard {
        Some(_) => println!("[Lattice-d] Signed checkpoints enabled (every {}s)", CHECKPOINT_INTERVAL_SECS),
        None => println!(
            "[Lattice-d] WARNING: no signing.key found --> running WITHOUT signed checkpoints.\
             \n[Lattice-d] A root attacker could rewrite the entire chain undetected. Run `latticed keygen`."
        ),
    }

    //----------------------//
    //--- Signal Handler ---//
    //----------------------//
    let store_signal = Arc::clone(&store);
    let chain_signal = Arc::clone(&chain);
    let sk_signal = signing_key_guard.clone();
    ctrlc::set_handler(move || {
        println!("\n[Lattice-d] Shutdown signal received, flushing...");
        let mut s = store_signal.lock().unwrap();

        // final signed checkpoint so nothing after the last interval is unanchored
        if let Some(sk) = &sk_signal {
            let c = chain_signal.lock().unwrap();
            let head = c.blocks.last().unwrap();
            let cp = sign::create_checkpoint(head.index, &head.hash, sk);
            s.append_checkpoint(&cp);
            println!("[Lattice-d] Final checkpoint written at height {}", cp.height);
        }

        s.flush();
        println!("[Lattice-d] Flush complete. Goodbye.");
        std::process::exit(0);
    }).expect("[Lattice-d] Failed to set signal handler");

    //---------------------------//
    //--- Checkpoint thread  ---//
    //---------------------------//
    if let Some(sk) = signing_key_guard {
        let chain_cp = Arc::clone(&chain);
        let store_cp = Arc::clone(&store);
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(CHECKPOINT_INTERVAL_SECS));
            let c = chain_cp.lock().unwrap();
            let s = store_cp.lock().unwrap();
            let head = c.blocks.last().unwrap();
            let cp = sign::create_checkpoint(head.index, &head.hash, &sk);
            s.append_checkpoint(&cp);
            println!("[Lattice-d] Checkpoint signed at height {}", cp.height);
        });
    }

    let watched_paths = vec!["/etc", "/var/log", "/bin", "/usr/bin"];

    watcher::watch(watched_paths, |event| {
        let mut c = chain.lock().unwrap();
        let mut s = store.lock().unwrap();

        c.append(event.clone());

        let latest = c.blocks.last().unwrap().clone();
        let log_entry = format!("[Lattice-d] Block #{} | {}", latest.index, latest.hash);

        println!("{}", log_entry);
        s.append_log(&log_entry);
        s.push(latest);
    });
}

fn verify() {
    use crate::block::Block;

    println!("[Lattice-d] Verifying chain integrity...");

    let store = Storage::new();
    let p = store.path(storage::CHAIN_FILE);
    if !p.exists() {
        println!("[Lattice-d] No chain file found at {:?}", p);
        std::process::exit(1);
    }

    // walk rotated backups oldest -> newest so the
    // whole history is verified, not only the current segment
    let blocks: Vec<Block> = store.read_chain_blocks();

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

    //-----------------------------//
    //--- checkpoint validation ---//
    //-----------------------------//
    let pub_path = store.path(sign::PUB_FILE);
    if !pub_path.exists() {
        println!(
            "[Lattice-d] WARNING: no public key found --> checkpoint verification skipped.\
             \n[Lattice-d] A full-chain rewrite would go undetected. Run `latticed keygen`."
        );
    } else {
        let vk = sign::load_verifying_key(&pub_path);
        let checkpoints = store.read_checkpoints();

        if checkpoints.is_empty() {
            println!("[Lattice-d] No checkpoints found --> checkpoint verification skipped");
        } else {
            let mut last_height: Option<u64> = None;
            for cp in &checkpoints {
                if !sign::verify_checkpoint(cp, &vk) {
                    println!(
                        "[Lattice-d] TAMPER DETECTED at checkpoint #{} --> invalid signature",
                        cp.height
                    );
                    ok = false;
                }
                if last_height.is_some_and(|h| cp.height <= h) {
                    println!(
                        "[Lattice-d] TAMPER DETECTED at checkpoint #{} --> height rollback",
                        cp.height
                    );
                    ok = false;
                }
                last_height = Some(cp.height);
            }

            // latest signed head must match the actual block in the local chain.
            // catches a full regeneration of chain.jsonl even if hashes are internally consistent
            if let Some(latest) = checkpoints.last() {
                let anchored = blocks.iter().any(|b| b.index == latest.height && b.hash == latest.head_hash);
                if !anchored {
                    println!(
                        "[Lattice-d] TAMPER DETECTED at checkpoint #{} --> chain head mismatch (chain rewritten?)",
                        latest.height
                    );
                    ok = false;
                }
            }

            println!(
                "[Lattice-d] Checked {} checkpoint(s) against public key",
                checkpoints.len()
            );
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
