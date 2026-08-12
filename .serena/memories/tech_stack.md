# Tech Stack

- Language: Rust edition 2024; minimum Rust version 1.88 (`Cargo.toml`, `rust-toolchain.toml`).
- TUI: `ratatui` 0.30 with `crossterm` 0.29.
- Error handling: `color-eyre`, `thiserror`.
- Serialization/utility deps include `serde_json`, `regex`, `scopeguard`.
- Tests use Rust built-in test runner plus `insta` snapshots, `proptest`, and `tempfile`.
- External runtime dependency: `jj` CLI in PATH; README currently requires Jujutsu >= 0.42.0.