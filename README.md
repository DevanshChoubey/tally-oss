# Ledgerly

A local-first, keyboard-driven desktop accounting app for Indian small businesses: books, GST
returns, bank reconciliation, basic inventory and printing, in one company file on your own
computer. Written in Rust with egui.

- Download: see the [releases](https://github.com/DevanshChoubey/opentally/releases) page (Linux, Windows,
  macOS). What is in each release and what is not yet: [RELEASE-NOTES.md](RELEASE-NOTES.md).
- Design spec: [docs/specs/2026-11-20-ledgerly-design.md](docs/specs/2026-11-20-ledgerly-design.md).
- Engineering rules: [docs/engineering-rules.md](docs/engineering-rules.md).
- Licence: [LICENSING.md](LICENSING.md) (AGPL-3.0-or-later for the app; MIT OR Apache-2.0 for the
  format crates).

## Build from source

```bash
cargo build --release -p ledgerly-app -p ledgerly-cli
```

The app is `target/release/ledgerly-app`; the command line tool is `target/release/ledgerly`
(`ledgerly --help`).

## Checks

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace
cargo deny check
```

Ledgerly is an independent project. It imports and exports Tally XML for compatibility, and is not
affiliated with or endorsed by Tally Solutions.
