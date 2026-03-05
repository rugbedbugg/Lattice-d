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

# verify chain integrity
sudo latticed verify

# view live logs
sudo journalctl -u latticed -f
```

## Storage

| Path | Description |
|------|-------------|
| `/var/lib/latticed/chain.jsonl` | Append-only blockchain (one block per line) |
| `/var/lib/latticed/latticed.log` | Human-readable event log |

Log rotation triggers at 1MB, keeping up to 3 backups (`.bak.1`, `.bak.2`, `.bak.3`).

## Threat model

Lattice-d detects post-compromise log tampering. It does not prevent intrusions.
If an attacker gains root and modifies the chain file, `latticed verify` will
report the exact block where tampering occurred.

Note: `SIGKILL` cannot be intercepted. For guaranteed persistence use the
systemd service which handles `SIGTERM` cleanly.

## License

MIT
