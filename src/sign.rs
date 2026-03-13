use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use std::fs;
use std::path::Path;


const KEY_FILE: &str = "signing.key";
const PUB_FILE: &str = "signing.pub";
pub const ANCHOR_FILE: &str = "checkpoints.jsonl";   // jsonl = one checkpoint per line


//-------------------------------------//
//--- signed chain-head checkpoints ---//
//-------------------------------------//
// A Checkpoint binds the chain's head hash + height to an Ed25519
// signature made by a key held OUTSIDE the watched machine. Even if
// root on the watched host rewrites the entire local chain, it cannot
// forge a checkpoint matching the anchored head.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Checkpoint {
    pub height: u64,
    pub head_hash: String,
    pub timestamp: i64,
    pub signature: String,
}

impl Checkpoint {
    fn message(&self) -> Vec<u8> {
        format!("{}|{}|{}", self.height, self.head_hash, self.timestamp).into_bytes()
    }
}


//-----------------------------//
//--- keypair management  ---//
//-----------------------------//
pub fn generate_keypair(dir: &Path) {
    let mut csprng = rand_core::OsRng;
    let sk = SigningKey::generate(&mut csprng);

    fs::write(dir.join(KEY_FILE), hex::encode(sk.to_bytes()))
        .expect("[Lattice-d] Failed to write signing key");
    fs::write(dir.join(PUB_FILE), hex::encode(sk.verifying_key().to_bytes()))
        .expect("[Lattice-d] Failed to write public key");

    println!("[Lattice-d] Generated keypair:");
    println!("  secret: {}", dir.join(KEY_FILE).display());
    println!("  public: {}", dir.join(PUB_FILE).display());
}

pub fn load_signing_key(path: &Path) -> SigningKey {
    let hex_key = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("[Lattice-d] Failed to read signing key {:?}: {}", path, e));
    let bytes: [u8; 32] = hex::decode(hex_key.trim())
        .expect("[Lattice-d] Invalid hex in signing key")
        .try_into()
        .expect("[Lattice-d] Signing key must be 32 bytes");
    SigningKey::from_bytes(&bytes)
}

pub fn load_verifying_key(path: &Path) -> VerifyingKey {
    let hex_key = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("[Lattice-d] Failed to read public key {:?}: {}", path, e));
    let bytes: [u8; 32] = hex::decode(hex_key.trim())
        .expect("[Lattice-d] Invalid hex in public key")
        .try_into()
        .expect("[Lattice-d] Public key must be 32 bytes");
    VerifyingKey::from_bytes(&bytes)
        .expect("[Lattice-d] Invalid public key bytes")
}


//-------------------------------//
//--- checkpoint sign / verify ---//
//-------------------------------//
pub fn create_checkpoint(height: u64, head_hash: &str, key: &SigningKey) -> Checkpoint {
    let mut cp = Checkpoint {
        height,
        head_hash: head_hash.to_string(),
        timestamp: Utc::now().timestamp(),
        signature: String::new(),
    };
    let sig = key.sign(&cp.message());
    cp.signature = hex::encode(sig.to_bytes());
    cp
}

pub fn verify_checkpoint(cp: &Checkpoint, vk: &VerifyingKey) -> bool {
    hex::decode(&cp.signature).ok()
        .and_then(|b| <[u8; 64]>::try_from(b).ok())
        .map(|b| vk.verify(&cp.message(), &Signature::from_bytes(&b)).is_ok())
        .unwrap_or(false)
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_keypair_files_written() {
        let tmp = tempdir().unwrap();
        generate_keypair(tmp.path());
        assert!(tmp.path().join(KEY_FILE).exists());
        assert!(tmp.path().join(PUB_FILE).exists());
    }

    #[test]
    fn test_checkpoint_roundtrip_verifies() {
        let tmp = tempdir().unwrap();
        generate_keypair(tmp.path());
        let sk = load_signing_key(&tmp.path().join(KEY_FILE));

        let cp = create_checkpoint(42, "ab".repeat(32).as_str(), &sk);
        assert_eq!(cp.height, 42);

        let vk = load_verifying_key(&tmp.path().join(PUB_FILE));
        assert!(verify_checkpoint(&cp, &vk));
    }

    #[test]
    fn test_tampered_height_fails_verification() {
        let tmp = tempdir().unwrap();
        generate_keypair(tmp.path());
        let sk = load_signing_key(&tmp.path().join(KEY_FILE));
        let vk = load_verifying_key(&tmp.path().join(PUB_FILE));

        let mut cp = create_checkpoint(42, "ab".repeat(32).as_str(), &sk);
        cp.height = 43; // forged rollback attempt
        assert!(!verify_checkpoint(&cp, &vk));
    }

    #[test]
    fn test_tampered_head_fails_verification() {
        let tmp = tempdir().unwrap();
        generate_keypair(tmp.path());
        let sk = load_signing_key(&tmp.path().join(KEY_FILE));
        let vk = load_verifying_key(&tmp.path().join(PUB_FILE));

        let mut cp = create_checkpoint(42, "ab".repeat(32).as_str(), &sk);
        cp.head_hash = "cd".repeat(32); // fabricated chain head
        assert!(!verify_checkpoint(&cp, &vk));
    }

    #[test]
    fn test_wrong_public_key_fails_verification() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        generate_keypair(dir1.path());
        generate_keypair(dir2.path());

        let sk = load_signing_key(&dir1.path().join(KEY_FILE));
        let wrong_vk = load_verifying_key(&dir2.path().join(PUB_FILE));

        let cp = create_checkpoint(7, "ab".repeat(32).as_str(), &sk);
        assert!(!verify_checkpoint(&cp, &wrong_vk));
    }

    #[test]
    fn test_garbage_signature_fails_cleanly() {
        let tmp = tempdir().unwrap();
        generate_keypair(tmp.path());
        let sk = load_signing_key(&tmp.path().join(KEY_FILE));
        let vk = load_verifying_key(&tmp.path().join(PUB_FILE));

        let mut cp = create_checkpoint(1, "ab".repeat(32).as_str(), &sk);
        cp.signature = "zz".repeat(64); // invalid hex length/content
        assert!(!verify_checkpoint(&cp, &vk));
    }
}
