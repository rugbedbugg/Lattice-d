# Contributing to Lattice-d

Lattice-d is a security-sensitive Rust daemon for chained event records and signed checkpoints. Treat integrity, key handling, and verifiability as primary requirements.

## Build and test

The checked-in toolchain file selects Rust 1.98 with rustfmt and Clippy:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
```

Use `cargo run -- --help` and `cargo run -- verify` for command-level smoke checks with disposable data.

## Change guidelines

- Keep ingestion, chain storage, checkpoint signing, verification, and configuration concerns in their existing modules.
- Add tests for tampering, truncation, restart recovery, serialization, and invalid configuration.
- Never commit signing keys, production paths, event logs, or real checkpoint material.
- Document any compatibility change to the chain or checkpoint format and include migration/verification behavior.
- Avoid weakening the threat-model guarantees for convenience.

## Pull requests

Include formatting, Clippy, and test results. Describe the security boundary affected and how failure cases were exercised.
