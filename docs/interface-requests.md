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
## Q1: books_small() contains an expect
- Crate/file: crates/ledgerly-conformance/src/fixtures.rs
- Current interface: `pub fn books_small() -> Books`, frozen; implemented as `try_books_small().expect(..)` under a reasoned `#[expect(clippy::expect_used)]`. Data is constants; a test guards that it builds.
- Requested change: controller decides whether to accept this documented exception, or change the signature to `Result<Books, FixtureError>` in a coordinated change across consumers.
- Why (test or spec section that needs it): rule B2 (no panic paths in library crates). Not replaced with a silent default Books, which would risk a wrong number.
## U1: mock colour tokens fail WCAG 1.4.3 (owner note, mock update)
- Crate/file: design/mock/index.html (light theme tokens); shipped values live in crates/ledgerly-app/src/theme.rs
- Current interface: mock ink_3 gives 3.82:1 on the sheet, mock haldi on haldi_soft 4.41:1, mock leaf on leaf_soft 4.42:1 (all below 4.5:1)
- Requested change: update the mock to the corrected light tokens ink_3 #5D6A82, haldi #8F5C0C, leaf #286F45, as already shipped and pinned by tests/u1_theme.rs
- Why (test or spec section that needs it): spec section 7 accessibility; the mock and the app currently disagree
- Also for the Wave 2 wiring card: route Esc, Ctrl+T and Ctrl+W through the keymap (currently hard-coded in shell/mod.rs), and build split view and scalable fonts (spec 7), which U1 does not provide.
## U4: split `voucher_entry.rs` into a module directory
- Crate/file: `ledgerly-app/src/screens/mod.rs` (frozen), `screens/voucher_entry.rs` (about 1900 lines, over the 800-line limit of rule C6; it is not a table of cases).
- Current interface: `pub mod voucher_entry;` as a single file.
- Requested change: allow `screens/voucher_entry/{mod,arith,draft,picker,screen}.rs` behind the same `pub mod voucher_entry;` line (no change to `mod.rs` text is needed, since `voucher_entry/mod.rs` satisfies it), re-exporting every public item at its present path. Wave reviewer to approve the file move; the split is mechanical (arithmetic, draft, picker, screen) and is not done in this card because it would touch the whole diff again under review.
- Why: rule C6.

## U4: screen outbox on `AppCtx`
- Crate/file: `ledgerly-app/src/screen.rs` (`AppCtx`, frozen).
- Current interface: `AppCtx { books }` only.
- Requested change: a way for a screen to hand the host a voucher to post (`VoucherEntryScreen::take_save_request`) and ledgers to create (`take_master_requests`), and for the host to answer (`confirm_saved`, `save_failed`, `resolve_created_ledger`, `reject_created_ledger`). For example an `outbox: Vec<Command>` and an `inbox: Vec<Reply>` on `AppCtx`, or a `Screen::poll(&mut self, ctx)` hook.
- Why: spec 7 (Alt+C creates a ledger inline, a fresh voucher after save) and rule D3 (save is one transaction in the engine). Until then the screen exposes these as methods and nothing calls them from the shell.

## U4: keymap bindings
- Crate/file: `ledgerly-app/src/keymap.rs` (card U2).
- Current interface: none merged; U4 hard-codes its keys in one table (`bindings` in `voucher_entry.rs`): Ctrl+A and Ctrl+S save, Alt+C create ledger, Alt+S switch Dr/Cr, Ctrl+D delete line.
- Requested change: actions for "save", "create master", "switch side" and "delete line" in the classic preset and a lookup the screen can call each frame. Ctrl+Enter is not consumed by U4 (spec 7: alter master). Ctrl+D and Alt+S have no source in spec 7; the owner to choose the bindings.
- Why: spec 7 says the keys are fully rebindable.

## U4: shared helpers to adopt when C1, C2 and C3 merge
- Crate/file: `ledgerly-core` (`format::format_inr`, `money_ops::sum`, `date_entry::parse_date_entry`).
- Current interface: stubs only (frozen signatures), so U4 keeps private copies: `format_amount`/`group_indian`, the sums in `VoucherDraft::balance` and `build_voucher`, and `parse_entry_date`.
- Requested change: when the cards merge, replace the copies with the shared functions (`parse_date_entry` takes a `today` argument, so the screen passes the date it was opened with) and drop the duplicated `Unbalanced` pre-check in `build_voucher` in favour of `validate_voucher` and the engine, which then needs the existing `an_unbalanced_draft_reports_both_totals` case moved to where the engine reports it, with its provenance kept.
- Why: rules C4 and A1 (one query path for figures; one copy of each rule).
