# Interface requests

Builders may not change Wave 0 interfaces, `Cargo.toml`, `Cargo.lock`, `ledgerly-core` or
`ledgerly-model`. Append a section here instead; the wave reviewer decides.

Template:

```
## <card id>: <short title>
- Crate/file:
- Current interface:
- Requested change:
- Why (test or spec section that needs it):
```

## U1: mock colour tokens fail WCAG 1.4.3 (owner note, mock update)
- Crate/file: design/mock/index.html (light theme tokens); shipped values live in crates/ledgerly-app/src/theme.rs
- Current interface: mock ink_3 gives 3.82:1 on the sheet, mock haldi on haldi_soft 4.41:1, mock leaf on leaf_soft 4.42:1 (all below 4.5:1)
- Requested change: update the mock to the corrected light tokens ink_3 #5D6A82, haldi #8F5C0C, leaf #286F45, as already shipped and pinned by tests/u1_theme.rs
- Why (test or spec section that needs it): spec section 7 accessibility; the mock and the app currently disagree
- Also for the Wave 2 wiring card: route Esc, Ctrl+T and Ctrl+W through the keymap (currently hard-coded in shell/mod.rs), and build split view and scalable fonts (spec 7), which U1 does not provide.
