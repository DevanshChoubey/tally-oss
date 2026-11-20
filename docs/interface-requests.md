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
