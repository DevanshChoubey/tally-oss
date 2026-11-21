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
- Current interface: none merged; U4 hard-codes its keys in one table (`bindings` in `voucher_entry.rs`): Ctrl+A and Ctrl+S save, Alt+C create ledger, Ctrl+D delete line. Switch Dr/Cr is the on-screen button only (round 3: the Alt+S binding was removed, see below).
- Requested change: actions for "save", "create master", "switch side" and "delete line" in the classic preset and a lookup the screen can call each frame. Ctrl+Enter is not consumed by U4 (spec 7: alter master). Ctrl+D has no source in spec 7; the owner to choose it. Alt+S is removed (round 3, 2026-11-21): Tally's shortcut page gives Alt+S to Stock Query and lists no key that switches a line's Dr/Cr side, so that keyboard binding is pending.
- Why: spec 7 says the keys are fully rebindable.

## U4: shared helpers to adopt when C1, C2 and C3 merge
- Crate/file: `ledgerly-core` (`format::format_inr`, `money_ops::sum`, `date_entry::parse_date_entry`).
- Current interface: stubs only (frozen signatures), so U4 keeps private copies: `format_amount`/`group_indian`, the sums in `VoucherDraft::balance` and `build_voucher`, and `parse_entry_date`.
- Requested change: when the cards merge, replace the copies with the shared functions (`parse_date_entry` takes a `today` argument, so the screen passes the date it was opened with) and drop the duplicated `Unbalanced` pre-check in `build_voucher` in favour of `validate_voucher` and the engine, which then needs the existing `an_unbalanced_draft_reports_both_totals` case moved to where the engine reports it, with its provenance kept.
- Why: rules C4 and A1 (one query path for figures; one copy of each rule).

## Second-review fix round (2026-11-21)

Filed from the r2/* fix branches. Each line names the card and the request; the wave reviewer accepts or rejects it.

- **E2** (engine): Add EngineError::InUseByMaster { what: String } to crates/ledgerly-engine/src/error.rs (with a #[error] message such as "{what} is used by another master; change that first"). E2 would then return it from the master-on-master delete paths in masters_stock.rs (delete_unit for a unit used by a stock item or compound unit, and the same for stock item, godown, stock group, cost category and cost centre) instead of Invalid. Adding a variant to this public enum breaks exhaustive matches in other crates, so the wave reviewer must check them. Why: should-fix finding in _reports/review2/fix-engine.json; a UI cannot tell a dependency refusal from bad input.
- **E6** (engine): Accept the changed signature of crates/ledgerly-engine/src/scenario.rs effective_postings: fn(&Books, &Scenario, Date) -> Result<Outcome<Vec<&Posting>>, EngineError> (was Vec<&Posting>). Reports RP3 and RP5 (ledgerly-reports balance_sheet.rs and ledger.rs) should then call it and drop their own inclusion copy, so one rule applies everywhere. Why: rule A1 and the cross-card finding that the two inclusion rules must not differ.
- **S4 and S5** (core-store): Crate ledgerly-store, lib.rs StoreError: add Unsupported { reason: String } (and optionally Replay { seq, message }). Use it in stock_load.rs for Effect::Unsupported and the Optional/PostDated status rejection, and in load.rs replay. Currently these are Encoding, which misreports them.
- **S5** (core-store): Crate ledgerly-model, inventory.rs StockMove: add a status field (VoucherStatus) so Optional and PostDated inventory can be projected under a scenario. Until then load_books errors for those vouchers.
- **S5** (core-store): Crate ledgerly-store, stock_load.rs: accept the new public fn moves_from_vouchers(&Masters, &BTreeMap<VoucherId, Voucher>) -> Result<Vec<StockMove>, StoreError>, now called from load.rs.
- **S6** (core-store): Crate ledgerly-store, lib.rs StoreError: a typed variant for a restore head mismatch (for example RestoreMismatch { backup_head, restored_head }). restore_from currently returns Encoding.
- **M2** (core-store): Crate ledgerly-model, seed_voucher_types.rs: accept or remove the public fn affects_books. It is outside the frozen interface and has no engine caller. Its doc was narrowed to spec wording.
- **U11 (Wave 2 wiring)** (app-b): Build SuggestionView.check (new public field) from the engine for every bank match and mapping: Ok, Blocked, Unsupported or NotChecked. Any SuggestionView literal outside the tests must set it.
- **U12 (Wave 2 wiring)** (app-b): Change AuditScreen::set_table to take the report kind (or have the host echo it) so that a late answer for an earlier report is refused rather than shown under the current heading.
- **U2 / U11 (keymap)** (app-b): Owner to choose the Accept key for the assistant panel, since Ctrl+Enter is also the U2 alter-master key. Resolved in round 3 (2026-11-21): the panel accepts on Ctrl+A, which Tally's shortcut page lists as accept or save a screen.
- **U9 / U10 / U11 (wiring)** (app-b): New public error variants to handle where errors are mapped for the user: BankScreenError::NegativeLineAmount, NegativeVoucherAmount, UnknownSuggestionVoucher; ImportError::WrongPeriod. Any exhaustive match in wiring must cover them.
- **U5** (app-a): File in docs/interface-requests.md (I could not edit it): an engine or api seam that returns the posted line amount (quantity x rate less discount, with the rounding rule named) together with the GST block, so the screen shows that figure and does not compute it (rule C4). Until then check_block refuses any mismatch.
- **U4 (voucher_entry.rs, eval_amount) with U5** (app-a): File in docs/interface-requests.md: eval_amount should return an error or Unsupported when the exact result has more than two decimals, instead of HalfUpToPaise rounding, or the rounding needs provenance. Affects typed amounts in voucher_entry and item_invoice (draft.rs and nav.rs call it through typed_amount).
- **U7** (app-a): File the U7 entry that model.rs already cites in docs/interface-requests.md. Either accept ReportSource's synchronous refresh and drill callbacks as an explicit exception to rule D1, or change to queued requests (take_request and set_report) as U8 to U12 do. Decision for the controller.
- **U2 (keymap.rs) with U3, U5, U6** (app-a): One shared exact-modifier helper. consume_exact is now copied in home.rs, masters.rs and item_invoice/keys.rs, and the same match rule exists in keymap.rs consume_chord. Proposed: a pub(crate) helper in keymap.rs, or Keymap::consume_action wiring in Wave 2, which would replace all three copies.
- **K4** (bank): bounce_proposal needs a journal voucher type for the draft and an id assigned on acceptance. Either add a journal_type: VoucherTypeId parameter or have the caller set it. The frozen signature has no books or type argument. Not used by other crates today.
- **BF4** (bank): verify_balances is public and outside the frozen BF4 interface, and duplicates K1 check_statement. Remove it, or accept it for the wave reviewer and file it in docs/interface-requests.md.
- **K3 / BF2 / BF3 / BF4** (bank): Carry the statement's opening and closing balance dates (:60F:/:62F:, or the sheet labels) through RawStatement and BankStatement, so K3 can refuse an as_of beyond the coverage. This changes the public RawStatement.
- **BF3 / K1** (bank): An end-to-end test importing a registry XLSX through check_statement needs ledgerly-bank to dev-depend on rust_xlsxwriter (a Cargo.toml change), or the test to live in K1's crate.
- **BF1** (bank): TableSpec gained a public field balance_labels (BalanceLabels). It is outside the frozen BF1 list, so the wave reviewer should note it. No struct-literal construction exists outside registry.rs.
- **Bank lane (all)** (bank): docs/interface-requests.md has no entries for K2, K3, K4, K5, BF1, BF3 or BF4. The items above should be filed there by the wave reviewer.
- **P1** (assist-print): serve(api, ...) needs the API facade (Wave 2): a facade Api type and a ToolBackend impl for it, so the frozen serve(api, ...) signature in ledgerly-mcp server.rs can be added. P1 cannot be completed inside its own files until the facade lands.
- **X2** (tally): Add a StockCategory variant to TallyMaster.
- **X4** (tally): Make write_masters and write_vouchers fallible, or require callers to use the try_ pair.
- **X5** (tally): Add parse_report(&Envelope) and keep child order in Envelope.
- **X1** (tally): Keep Envelope child order so reports can be parsed in document order.
- **B4** (tally): Add a cancel token to reconcile.
- **B2** (tally): Change the pull return type to BridgeError.
- **B3** (tally): Define the reserved identity field.
- **G7** (gst): A use flag per purchase line (further supply, passenger or goods transport, driving training, obligation under law, and so on) so s.17(5) blocked categories can be decided instead of Unsupported. docs/interface-requests.md was not edited, per the task rules.
- **G6** (gst): Registration details on every party ledger used for sales, so classification is never Unsupported for a registered customer. Data-entry requirement only; no interface change.

## Round 3 (2026-11-21)

- **S5** (model): `InventoryLine` needs a typed side (source or destination) so a stock journal can
  be projected per the documented rule (source reduces, destination increases). Use
  `#[serde(default)]` so old events decode. Until then stock journals are `StoreError::Unsupported`.
- **S5** (store): accepted in round 3: `StockMove.status`, `StoreError::Unsupported { voucher,
  reason }`, `stock_moves()` returning carried (Regular and Optional) moves. Any valuation that
  reads `stock_moves()` must filter by status. `Books.stock_moves` holds Regular moves only.
- **E2** (engine): accepted in round 3: `EngineError::InUseByMaster { what, by }`.
- **K3** (bank): accepted in round 3: `Brs.statement_balance: Outcome<Paise>`,
  `Brs.balance_as_per_bank: Paise`, `Brs.difference: Outcome<Paise>`, `Brs::is_reconciled() ->
  Outcome<bool>`.
- **G7** (model + gst): accepted in round 3: `ineligible_itc: Option<bool>` on Ledger, Group,
  StockItem, StockGroup and LedgerLine, and `Company::ineligible_itc_default`.
  `itc_eligibility(v, line, company, masters, rules)`, plus the advisory `blocked_category_for`.
- **G7** (tally-bridge): map Tally's ineligible-for-input-credit setting on import once an
  observed XML capture shows the tag.

## Wave 2 W5 (bank statements, 2026-11-21)

- **K3** (model, bank): a statement payload in the store. `Event::StatementImported(BankStatementId)`
  carries only the id, so the parsed `BankStatement` is held in memory by `Api` for the session
  (`crates/ledgerly-api/src/bank.rs`). Statements are not in the company file: after a reopen the
  session does not know them, `Api::brs` gives no statement side, and a statement must be imported
  again (`ApiError::UnknownBankStatement`). Duplicate-file detection is also per session. Requested:
  an event that carries the parsed lines and file hash, so statements survive reopen.
- **K3** (bankfmt, CSV): the CSV parser leaves `RawStatement.opening` and `closing` unset for the
  generic split layout (`csv.rs`), so `check_statement` refuses every CSV file with
  `StatementError::MissingBalance`. Requested: read the opening and closing balance from the file
  (a balance column, or a header or footer line) so CSV statements can be verified.
- **K3** (bank, BRS): `BankStatement` has no end date for the statement, so the statement side is
  covered only for `as_of` strictly before the last line date. Requested: store the statement's
  stated end date (`:62F:` date for MT940) so the statement side can cover the last day too.
- **Facade** (W1, facade.rs): W5 added a `pub(crate) bank: Statements` field to `Api` and removed the
  `allow(dead_code)` lines on `loaded_books`, `cached_books`, `store`, `commit_as` and `actor`. The
  brief allowed removing the allow lines only; the field addition is outside that and is flagged here.

## W6 (Wave 2 wiring): the app cannot read the books or assign master ids

- Crate/file: `ledgerly-api/src/facade.rs` (`Api`, owned by W1; W6 may not edit it).
- Current interface: `Api` has no public read of the books. `cached_books(&self) -> Result<Outcome<&Books>, ApiError>` exists but is `pub(crate)`. The app's `AppCtx::books` therefore stays `None` once a company is opened through the API, and the voucher, item invoice and masters screens draw nothing (they show "The company is open, but this screen cannot read its masters yet.").
- Requested change: promote `cached_books` to `pub` (or add `pub fn books(&self) -> Result<Outcome<Books>, ApiError>` returning an owned copy, which the worker can send back to the UI thread after each commit). `Books` would also need a `pub use` from `ledgerly-api/src/lib.rs`, or the app would need `ledgerly-model` types only, which it already depends on.
- Why: the screens need `Masters` for pickers and `Books` to build a voucher. Rule C4 (one query path) and D1 (the UI thread never blocks: the copy is taken on the worker, not on the UI thread).

## W6: new master ids and new inline ledgers need an id from the company file

- Crate/file: `ledgerly-api` (`command.rs`, `Command::CreateGroup` and the other `Create*` variants) and `ledgerly-engine` (`masters_accounts.rs`, `masters_stock.rs`).
- Current interface: the masters draft carries id `0` (`UNASSIGNED_ID`) on a create, and the engine stores the id it is given. The comment in `screens/masters/draft.rs` says "the store assigns the real id on commit", but no code assigns it. Sending `Command::Create*` with id 0 would store a master under the reserved id 0.
- Requested change: either the engine or the API assigns the next id (for example `Command::CreateGroup` taking `Option<id>`, or the API returning the new id in `CommandResult`), or the screens get the next id from the books once the books read above exists.
- Until then W6 refuses creates in the app with "a new group needs an id from the company file, which this build cannot assign yet" (`screens/masters/command.rs`), and voucher entry refuses inline ledger creation (Alt+C) the same way. Alter and delete use the ids the books already hold, so they go through.

## W6: item invoice needs a tax computation the API can run without posting

- Crate/file: `ledgerly-api` (a new public method, or a `Command`-free query) over `ledgerly-gst`.
- Current interface: `ItemInvoiceScreen::set_gst` takes a `GstResult`, and nothing in the app computes one (rule C4: the UI must not compute tax). So Accept is refused with "Tax has not been computed for this invoice yet." The only frozen W4 name is `Api::gstr1_json`, which is a return, not a per-invoice computation.
- Requested change: a public `Api` method that runs the GST engine on a draft `Voucher` and returns the `GstBlock` (the ledger lines, the stored GST and the invoice total), or `Unsupported` with the reason. W4 owns the name; W6 then calls it from `ItemInvoiceScreen` and passes the result to `set_gst`.
- Why: spec 6.3 and 7 (the GST block shows engine output).

## W6: the save path for item invoices and masters, and the welcome screen's file dialog

- The app opens and creates company files by typed path (`shell/welcome.rs`). A file-picker dialog (`rfd`) would block the UI thread while open, so it is not used. Pending: a non-blocking dialog, if the owner wants one.

## C6: projections and rebuild check (Wave 3)

- Added `SqliteStore::enable_projections(&self) -> Result<bool, StoreError>`,
  `SqliteStore::projections_enabled(&self) -> Result<bool, StoreError>` and
  `Api::enable_projections(&self) -> Result<bool, ApiError>`. Once on (flag `projections` in
  `meta`), `Store::commit` writes the `masters`, `vouchers` and `postings` rows in the same
  transaction and refuses an event that does not fit the stored state.
- Not switched on automatically. The retained tests `ledgerly-api/tests/w2_maintenance.rs`
  (`rebuild_check_is_not_a_match_when_projections_are_not_maintained`), `ledgerly-cli/tests/w2_cli.rs`
  and `ledgerly-store/tests/s6_backup.rs` (`unmaintained_projection_tables_are_reported_not_matched`,
  and the tests that hand-insert projection rows) require `NotMaintained` or empty tables for files
  written through `create` plus `commit`. To make `Api::create` switch it on (so `rebuild_check`
  matches on every new file), `facade.rs` needs `store.enable_projections()?` after the seed commit,
  and those retained tests need an owner decision.
## Wave 3 C3 (bank statements persisted, 2026-11-21)

- **Done**: the three Wave 2 K3 requests. `Event::BankStatementStored { .. }` (store `event.rs`) holds the
  verified statement (lines, balances, end date, file hash); `Api::import_bank_statement` commits it instead
  of `StatementImported`. The CSV parser reads the opening and closing balance from the balance column
  (opening = first line's balance taken back over its own amounts; closing = last line's balance).
  `BankStatement::end_date` is the MT940 `:62F:` date and the BRS covers it.
- **Store, shared edit (coordinator)**: `crates/ledgerly-store/src/lib.rs` gains
  `pub use event::StoredStatementLine` (the line type the new event carries). Every exhaustive `match` on
  `Event` outside C3's files needs an arm for `BankStatementStored`: `ledgerly-store/src/load.rs` (no effect on
  books, like `StatementImported`), `ledgerly-tally-bridge/src/import.rs` and its `tests/w3_import.rs`.
- **Facade (C1/C2-owned `facade.rs`)**: `Api::open` does not read the statements. `bank.rs` reads them from
  the audit log on first use (lazy), because `facade.rs` is not C3's. If the owner prefers an eager read,
  `Api::open` can call a `Statements::load` after `reload`. Not requested, only an option.
- **Files written before C3**: they hold `StatementImported(id)` only. Those statements have no rows, so they
  are not shown after a reopen (as before); their ids are not reused.
- **Pending**: whether Tally covers the day of the statement's last line when the file states an end date is
  not on Tally's Bank Reconciliation help page; the end-date rule is ours.
- **Bank screen**: `BankScreen::worker(name, ledger, as_of)` runs `brs`, `bank_suggestions` and
  `confirm_bank_match` through the worker. The shell must construct it (C1/C5 own the shell); nothing in
  `ledgerly-app/src/shell` calls it yet. Manual matches and rule suggestions are not sent to the API.
## C5: palette wiring, report menu and the voucher drill

- Crate/file: `ledgerly-app/src/shell/mod.rs` (card C1 owns it), `ledgerly-app/src/screens/report_view/voucher_view.rs`.
- Done in C5: `shell::navigate` (report menu, palette pick to screen), `ScreenStack::open_choice` and `open_report` (and the same on `Tabs`), `PaletteSearch` (the palette's `Api::search` through the worker), and the read-only `VoucherViewScreen` for a voucher drill. The only edit to `shell/mod.rs` is the line `pub mod navigate;`.
- Requested (host, `Shell`): `Shell` has no palette yet. It needs a `Palette`, a `PaletteSearch`, and per frame: open on `Action::OpenPalette` / `Action::SwitchPalette` (`Palette::open_for`), `palette.set_entries(navigate::host_entries(&keymap))` once, `search.tick(&mut palette, self.app_ctx.api.as_ref())`, draw `palette.ui(ui)`, and on `Some(choice)` call `tabs.open_choice(&choice, &self.app_ctx)`. `Err(NavError::NotNavigation)` means the choice is a `Command` or `AiIntent`, which the shell runs itself. Any other `Err` is shown to the user as text (never swallowed).
- Pending (alteration): Tally opens a voucher from the last level of a report with Enter, and "for display" with Ctrl+Enter (<https://help.tallysolutions.com/keyboard-shortcuts-tally-prime/>, read 2026-11-21). Ledgerly has no screen that alters an existing voucher, so Enter opens a display-only screen. Requested: an alteration screen (an entry screen that takes an existing `Voucher` and sends `Command::AlterVoucher`), then Enter opens it and Ctrl+Enter opens the display screen.
- Pending: the voucher screen reads `AppCtx::books` (set by C1). It shows no stock lines or stored GST yet and says so on screen.
- Pending: the default report period follows the Trial Balance page of Tally (last voucher date, condensed). The other reports use the same default without a source of their own.
