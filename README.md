# Lattice-d

![GitHub last commit](https://img.shields.io/github/last-commit/rugbedbugg/Lattice-d?style=for-the-badge&labelColor=000000)
![GitHub repo size](https://img.shields.io/github/repo-size/rugbedbugg/Lattice-d?style=for-the-badge&labelColor=000000)
![Stars](https://img.shields.io/github/stars/rugbedbugg/Lattice-d?style=for-the-badge&labelColor=000000)
![AUR version](https://img.shields.io/aur/version/latticed?style=for-the-badge&labelColor=000000)

A tamper-evident filesystem audit daemon for Linux. Every filesystem event on watched paths is hashed and chained using SHA-256, making unauthorized modifications to the audit trail detectable. Signed checkpoints (Ed25519) bind chain state to an off-host key, detecting even full-chain rewrites by root attackers.

## Status

**Active**

## Features

| Feature | Description |
|---------|-------------|
| Real-time monitoring | inotify on `/etc`, `/var/log`, `/bin`, `/usr/bin` by default |
| Hash chain | Append-only SHA-256; any deletion/modification breaks the chain |
| Signed checkpoints | Ed25519 every 60s + on clean shutdown, off-host key storage |
| Verification | `verify` subcommand validates full chain + all checkpoint signatures |
| Systemd integration | Graceful `SIGTERM` handling |
| Log rotation | 1MB trigger, 3 backups |

## Tech Stack

| Component | Library | Purpose |
|-----------|---------|---------|
| Language | Rust | Edition 2021, MSRV 1.70+ |
| Filesystem events | `notify` | inotify wrapper |
| Signatures | `ed25519-dalek` | Ed25519 checkpoint signing |
| Hashing | `sha2` | SHA-256 hash chaining |
| Config | `serde` + `toml` | TOML config parsing |

## Pipeline

### Event Ingestion → Chain Block

1. inotify delivers filesystem event (create/modify/delete/move)
2. Serialize event data + timestamp + previous block hash
3. Compute SHA-256 hash of block → new chain head
4. Append block to `chain.jsonl` (one JSON line per block)

### Checkpoint Signing

1. Every 60s (and on `SIGTERM`), daemon reads current chain height + head hash
2. Sign `(height, head_hash)` with Ed25519 private key (held off-host in production)
3. Append signed checkpoint to `checkpoints.jsonl`

### Verification

1. Replay `chain.jsonl`: recompute each block's hash, verify chain linkage
2. Replay `checkpoints.jsonl`: verify every Ed25519 signature against stored public key
3. Confirm latest signed checkpoint matches actual chain head
4. Reject rollbacks: checkpoint height must be strictly increasing

## Install

### Arch Linux (AUR)

```bash
yay -S latticed
# or
paru -S latticed
```

### From Source

Requires Rust 1.70+ (edition 2021).

```bash
git clone https://github.com/rugbedbugg/Lattice-d.git
cd Lattice-d
cargo build --release
sudo cp target/release/latticed /usr/bin/latticed
sudo cp latticed.service /etc/systemd/system/
sudo systemctl enable --now latticed
```

The binary is `target/release/latticed`. During development, run via `cargo run --`:

```bash
cargo run -- --help
cargo run -- verify
```

> Args after `--` go to the program, not to cargo.

## Commands

| Command | Description |
|---------|-------------|
| `sudo latticed start` | Start daemon (foreground) |
| `sudo latticed verify` | Verify chain integrity + signed checkpoints (exits 0 on success, non-zero on failure) |
| `sudo latticed keygen` | Generate Ed25519 keypair (writes to `/var/lib/latticed/`) |
| `sudo journalctl -u latticed -f` | View live logs |

### Keygen - export secret off-machine immediately

```bash
sudo cp /var/lib/latticed/signing.key /media/usb/
sudo shred -u /var/lib/latticed/signing.key
```

Only `signing.pub` stays on the host.

## Options

### Common (all commands)

| Flag | Description |
|------|-------------|
| `--config <path>` | Config file path. Default: `/etc/latticed/config.toml`. |
| `--chain-dir <path>` | Override chain/storage directory. Default: `/var/lib/latticed/`. |
| `--help` | Print help for the command. |

### Daemon (start)

| Flag | Default | Description |
|------|---------|-------------|
| `--interval <secs>` | `60` | Checkpoint signing interval in seconds. |
| `--no-sign` | `false` | Disable checkpoint signing (not recommended for production). |

### Verify

| Flag | Default | Description |
|------|---------|-------------|
| `--strict` | `true` | Require signed checkpoints; fail if none exist. |
| `--allow-unsigned` | `false` | Allow verification without checkpoints (partial tampering only). |

## Config

Default: `/etc/latticed/config.toml` (create from example):

```toml
[daemon]
watch_paths = ["/etc", "/var/log", "/bin", "/usr/bin"]
checkpoint_interval_secs = 60
sign_checkpoints = true

[storage]
chain_path = "/var/lib/latticed/chain.jsonl"
checkpoints_path = "/var/lib/latticed/checkpoints.jsonl"
log_path = "/var/lib/latticed/latticed.log"
max_log_size_mb = 1
max_log_backups = 3

[keys]
public_key_path = "/var/lib/latticed/signing.pub"
# private_key_path = "/var/lib/latticed/signing.key"  # kept off-host in production
```

## Storage

| Path | Description |
|------|-------------|
| `/var/lib/latticed/chain.jsonl` | Append-only blockchain (one block per line) |
| `/var/lib/latticed/latticed.log` | Human-readable event log |
| `/var/lib/latticed/checkpoints.jsonl` | Signed chain-head checkpoints |
| `/var/lib/latticed/signing.pub` | Ed25519 public verification key |

Log rotation triggers at 1MB, keeping up to 3 backups (`.bak.1`, `.bak.2`, `.bak.3`).

## Threat Model

Lattice-d detects **post-compromise log tampering**. It does not prevent intrusions.

| Attack | Detection |
|--------|-----------|
| Partial tampering (edited/deleted lines) | SHA-256 hash chaining |
| Full rewrite/rollback by root | Checkpoint signatures (requires off-host secret key) |
| No signing configured | Partial tampering only |

Note: `SIGKILL` cannot be intercepted. For guaranteed persistence use the systemd service which handles `SIGTERM` cleanly.

## Project Structure

```
Lattice-d/
├── src/
│   ├── main.rs           # CLI entry, subcommands
│   ├── daemon.rs         # inotify event loop, chain writing
│   ├── chain.rs          # Block struct, hash chaining, JSONL I/O
│   ├── verify.rs         # Chain replay + checkpoint verification
│   ├── checkpoint.rs     # Ed25519 signing, checkpoint format
│   ├── config.rs         # TOML config loading with defaults
│   └── keys.rs           # Keygen, key loading
├── latticed.service      # Systemd unit
├── config.example.toml   # Example configuration
└── README.md
```

## Testing

```bash
cargo test
```

Covers: chain hash chaining, checkpoint sign/verify, config parsing, CLI argument handling.

## Links

- **Repo:** https://github.com/rugbedbugg/Lattice-d
- **Issues:** https://github.com/rugbedbugg/Lattice-d/issues
- **Releases:** https://github.com/rugbedbugg/Lattice-d/releases
