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

## S4: replay failures need their own StoreError variant
- Crate/file: `crates/ledgerly-store/src/lib.rs` (`StoreError`), used by `load.rs`.
- Current interface: `StoreError::Encoding(String)`, documented for undecodable stored records.
- Requested change: add a variant such as `StoreError::Replay { seq: EventSeq, message: String }`
  (or `InconsistentLog`) and return it from `load_books` instead of `Encoding`.
- Why (test or spec section that needs it): `load_books` fails on a log that decodes but is
  semantically inconsistent (create of an existing id, alter or delete of a missing one, negative
  line amount, match naming a missing voucher). Reporting those as "encoding" errors misleads the
  user. The `s4_load.rs` error-path tests currently match `Encoding` plus the event number.
  Related: the `masters`/`vouchers`/`postings` projection tables are not read by S4 (nothing fills
  them yet); S6 decides whether to compare a rebuild against them.
