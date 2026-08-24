# Lattice-d

A tamper-evident filesystem audit daemon for Linux. Every filesystem event on
watched paths is hashed and chained using SHA-256, making unauthorized
modifications to the audit trail detectable.

## How it works

Lattice-d watches critical system paths (`/etc`, `/var/log`, `/bin`, `/usr/bin`)
using inotify. Each event is recorded as a block containing the event data,
a timestamp, and a SHA-256 hash chained to the previous block. Any tampering
with the chain, including deletion or modification of log entries, is
detectable via the verify subcommand.

## Installation

### From AUR
```bash
yay -S latticed
```

### From source
```bash
cargo build --release
sudo cp target/release/latticed /usr/bin/latticed
sudo cp latticed.service /etc/systemd/system/
sudo systemctl enable --now latticed
```

## Usage
```bash
# start as systemd service (recommended)
sudo systemctl enable --now latticed

# start manually
sudo latticed start

# verify chain integrity + signed checkpoints
sudo latticed verify

# view live logs
sudo journalctl -u latticed -f
```

## Signed checkpoints (anti full-rewrite)

Hash chaining alone cannot detect an attacker who regenerates the entire
chain file. Lattice-d therefore signs periodic checkpoints binding the
chain height and head hash to an Ed25519 key held **outside** the watched
machine:

```bash
# generate keypair (one-time setup)
sudo latticed keygen

# then export the secret OFF this machine:
sudo cp /var/lib/latticed/signing.key /media/usb/
sudo shred -u /var/lib/latticed/signing.key
```

Only `signing.pub` stays on the host. The daemon signs a checkpoint every
60 seconds and once more on clean shutdown. `latticed verify` checks every
checkpoint signature, rejects checkpoint rollbacks, and confirms the latest
signed head matches the actual local chain: a root attacker can rewrite
`chain.jsonl`, but cannot forge a matching signature.

## Storage

| Path | Description |
|------|-------------|
| `/var/lib/latticed/chain.jsonl` | Append-only blockchain (one block per line) |
| `/var/lib/latticed/latticed.log` | Human-readable event log |
| `/var/lib/latticed/checkpoints.jsonl` | Signed chain-head checkpoints |
| `/var/lib/latticed/signing.pub` | Ed25519 public verification key |

Log rotation triggers at 1MB, keeping up to 3 backups (`.bak.1`, `.bak.2`, `.bak.3`).

## Threat model

Lattice-d detects post-compromise log tampering. It does not prevent intrusions.

- **Partial tampering** (edited/deleted lines): caught by SHA-256 hash chaining.
- **Full rewrite / rollback** of the chain by an attacker with root: caught by
  checkpoint signatures, provided the secret key was exported off-machine as
  described above. Without signing set up, only partial tampering is detectable.

Note: `SIGKILL` cannot be intercepted. For guaranteed persistence use the
systemd service which handles `SIGTERM` cleanly.

## License

[MIT](LICENSE)
