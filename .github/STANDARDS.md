# Repository standards baseline

Adapted from ReAgent's shared baseline for this Linux Rust daemon.

CI runs on main/CI branch pushes, pull requests, manual and reusable calls.
It uses read-only permissions, concurrency cancellation, a timeout, mise tasks,
rustup's existing Rust 1.98.0 declaration, and Cargo.lock with --locked. Rust
builds are cached; ShellCheck is pinned through mise. No version policy changes.

`mise run check` performs rustfmt, Clippy, ShellCheck, unit tests, a locked build
and an unprivileged integration test. The latter uses `--storage-dir` and
`start --watch` to isolate files, verifies signed shutdown and checks that
modifying recorded events is rejected. Defaults retain the system paths.
The tested debug binary is uploaded with a SHA-256 checksum; this is not a release.

No CD exists or is added. Future releases must require validation, reuse tested
artifacts, protect publication credentials and verify remote outputs.
README updates are deferred at the user's request. The later pass must cover
accurate installation, quick start, usage/configuration, development checks,
license and CI/release links using the shared structure.

Validate workflow syntax and require local checks and GitHub CI before integration.
