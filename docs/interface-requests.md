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
## T4: StockError::Unsupported for inputs a valuation method cannot value
- Crate/file: `crates/ledgerly-stock/src/valuation.rs` (`StockError`)
- Current interface: `StockError` has `Money`, `Unit`, `NoStandardCost`, `NegativeStockBlocked` and the generic `Invalid { field, reason }`.
- Requested change: add `Unsupported { reason: String }` (engineering rule A1: a value, `Unsupported`, or an error). `fifo` returns `Invalid` today for: an item not set to FIFO, a voucher that both receives and issues the item (godown transfer, stock journal), and a value-only movement. These are unsupported inputs, not corrupt data, and a UI cannot tell them apart now. Once added, T4 switches those cases to `Unsupported`.
- Why (test or spec section that needs it): `malformed_moves_are_errors` and `other_valuation_method_is_an_error_not_a_fifo_number` in `crates/ledgerly-stock/tests/t4_fifo.rs`; spec section 6.4.
## G4: RuleSet lookup by id prefix
- Crate/file: `crates/ledgerly-gst/src/pack.rs` (`RuleSet`)
- Current interface: `RuleSet::get(id)` only; there is no way to list the entries in force.
- Requested change: an iterator over the verified (non-pending) entries in force, or a prefix lookup
  such as `entries_with_prefix("slab.")`, returning entries in id order.
- Why (test or spec section that needs it): `compute.rs` finds a rate slab by probing a fixed list of
  ids (`SLAB_IDS`); a slab added to a pack under a new id is silently never found (the voucher is
  `Unsupported`, the safe direction). A test guards the list meanwhile.
## A1: place of supply on VoucherDraft
- Crate/file: `ledgerly-assist/src/proposal.rs` (`VoucherDraft`)
- Current interface: `VoucherDraft` has no place of supply; `validate_draft` (frozen) takes none, so every GST draft is `Unsupported` through it. A1 added `validate_draft_with_place(.., Option<StateCode>)` beside it.
- Requested change: add `place_of_supply: Option<Transcribed<StateCode>>` to `VoucherDraft` so the proposal carries it, and have `validate_draft` read it. The wave reviewer to accept `validate_draft_with_place` (public addition) meanwhile.
- Why (test or spec section that needs it): spec 6.8; every taxed case in `tests/a1_validate.rs`.

## A1: AssistError variants for engine and GST errors
- Crate/file: `ledgerly-assist/src/seam.rs` (`AssistError`)
- Current interface: only `Unsupported` is usable for the validator; GST and engine errors are reported as `DraftCheck::Blocked { field: "gst", .. }`.
- Requested change: add a variant carrying `ledgerly_gst::GstError` (and the engine's error type) so a failure of the engine itself is not shown as a problem with the draft.
- Why (test or spec section that needs it): rule A1 (typed errors, no silent defaults).

## A1: acceptance of two public additions
- Crate/file: `ledgerly-assist/src/proposal.rs`, `src/validate.rs`
- Requested change: reviewer to accept `Transcribed::is_confident` and `validate_draft_with_place` as public additions beyond the frozen interface. Integrator: once clippy has run, change `#[allow(clippy::large_enum_variant, ..)]` on `DraftCheck` to `#[expect(..)]`, or remove it if the lint does not fire (the allow rests on a size guess, nothing was compiled).
- Why (test or spec section that needs it): rule B5.
