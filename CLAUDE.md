# Ledgerly (working name)

Open-source, local-first Rust desktop accounting + GST app for India. Spec:
`docs/specs/2026-11-20-ledgerly-design.md`. Mock: `design/mock/index.html`. Research: `docs/research/`.

## Rules (full text: `docs/engineering-rules.md`; read it before writing code)

- **Never a wrong number.** Value, `Unsupported { reason }`, or a typed error. No `_ =>` arms over
  domain enums. Money is `Paise` (i64); no floats.
- **Expected values need provenance:** `observed` (TallyPrime release + date, or GST portal/offline
  tool output + date) or `derived` (hand derivation from the Act/rule/notification, written in the
  test). Never copy our engine's output. No provenance means `pending`.
- **Never delete, ignore or loosen a test to get green.** Bug fixes start with a failing test.
- Library crates: no `unwrap`/`expect`/`panic!`/`todo!`, no `unsafe`, `thiserror` errors, documented
  public items. Headless crates never depend on UI crates.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`,
  `cargo test --workspace`, `cargo deny check`.
- AI proposes, engine computes, human confirms.
- Clean room: no Tally name, logo, look, TDL or decompiled code.
