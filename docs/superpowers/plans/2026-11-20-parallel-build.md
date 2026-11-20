# Ledgerly parallel build plan: about 110 Sonnet agents

> **For agentic workers:** this is an orchestration plan. Wave 0 is done in the controlling
> session. Each builder card in Waves 1–2 is that builder's whole brief together with
> "Every builder's brief" below. Every agent follows `CLAUDE.md` and `docs/engineering-rules.md`.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** build as much of Ledgerly v1 (Books + GST + Tally import, basic inventory, banking, AI
drafting) as one massively parallel run allows, without breaking the engineering rules.

**Architecture:** Wave 0 freezes every crate, every shared type and one module file per builder,
so 93 builders can work in separate git worktrees without touching the same file. Domain logic is
written as **pure functions over model types** (`Books` in, results out), so almost no builder
needs another builder's code to run its tests. Wave 2 wires the pure pieces to the store, API and
UI.

**Tech Stack:** Rust stable (edition 2024), egui/eframe + wgpu, rusqlite (bundled), rust_decimal,
time, thiserror, serde, quick-xml, calamine + rust_xlsxwriter, insta, proptest, cargo-fuzz,
criterion, tracing.

**Spec:** `docs/specs/2026-11-20-ledgerly-design.md` (cited as S§n). Research: `docs/research/`
(cited as R1..R10). Mock: `design/mock/index.html`.

## Global Constraints

- Money is `Paise(i64)`; quantities/rates/unit costs are `rust_decimal::Decimal`; no `f32/f64` outside `ledgerly-app` layout and `ledgerly-assist` confidences.
- Every computation returns `Ok(Outcome::Value(_))`, `Ok(Outcome::Unsupported(_))` or `Err(typed)`.
- No `_ =>` arms over the enums listed in engineering rule A2.
- Every expected test value is tagged `// provenance: derived — <reference>; <arithmetic>` or `// provenance: observed — <tool, version, date>`; otherwise the case is `pending` and asserts `Unsupported`.
- Licences: `ledgerly-core`, `ledgerly-tally-xml`, `ledgerly-gst-formats`, `ledgerly-bankfmt` are MIT OR Apache-2.0 (the format crates use `Paise`, so core must be permissive); every other crate AGPL-3.0-or-later.
- Return data shapes (`Gstr1`, `Gstr1a`, `Gstr3b`, `Gstr2b`, `Ims`, `EInvoice`, `EwayBill`) live in `ledgerly-gst-formats` (Wave 0); `ledgerly-gst` builds them, the format crate serialises/parses them. Dependency direction: gst → gst-formats → core.
- Headless crates never depend on egui/eframe/wgpu.
- Indian FY starts 1 April; display grouping `12,34,567.00`.
- Test data is fictional; GSTINs carry a valid check character (algorithm below).
- No "Tally" in product names or UI copy except factual interop labels ("Import from Tally").
- Builders never edit `Cargo.toml`, `Cargo.lock`, `ledgerly-core`, `ledgerly-model`, or another builder's files. Interface needs go to `docs/interface-requests.md` (append one section per request).

## Review Focus

Inputs the spec implies that no single builder's happy-path tests cover. Each line names the card
whose tests must pin it.

1. **Paise overflow and negative zero** — a ₹92 lakh crore sum or `-0` must error / normalise, never wrap (card C2: `checked_add_overflow_errors`, `neg_zero_displays_as_zero`).
2. **Voucher dated before books-begin or inside a locked period** — must be rejected with the date and the lock named, including on alter and delete (cards E3, E5).
3. **Supply, invoice and payment on different sides of the 22 Sep 2025 rate change** — the rate must follow CGST s.14 (which can depend on the payment date), and an unknown payment date must give `Unsupported`, not the voucher-date rate (cards G12 and G1: `pack_selected_by_time_of_supply_not_entry_date`).
4. **Tally XML with invalid characters, `&#4;` entities, UTF-16 BOM, or a missing `<ENVELOPE>` close** — parser must report row/byte position and keep what it parsed, never panic (cards X1, X2, H1).
5. **A bank statement whose running balance breaks mid-file** — import rejected with the first bad row; nothing half-imported (cards K1, BF2).

---

## What one run can and cannot produce

**Can produce:** every crate building with gates green on Linux; core types, model, SQLite event
store with hash chain and verifier; posting engine and invariants; the main reports; GST rule-pack
machinery, place of supply, voucher tax, GSTR-1/1A/3B builders and JSON/Excel writers; 2B/IMS
matcher; stock valuation; bank statement parsers and matcher; Tally XML parser/writer and bridge
with recorded fixtures; AI proposal types and validator; an egui shell with the main screens
driven by the real API.

**Cannot be production grade in one run:**
- **Observed numbers.** No TallyPrime or GST portal outputs have been captured yet. GST and
  accounting cases can be `derived` from the law; Tally-behaviour cases (valuation method choice,
  rounding of rates, report layouts) stay `pending` until observed. Fix: install **TallyPrime
  Educational mode** (free; dates limited to the 1st, 2nd and last day of a month) and capture
  fixtures — see Prerequisite 4.
- **Rule-pack legal references.** Many GST facts in R4 are tagged `[K]`. Each pack entry needs a
  notification number checked by a person before it leaves `pending`.
- Fuzz campaigns, cross-platform CI, coverage, mutation testing, performance tuning, installers.
- Integration: merge order and fixes are sequential (Wave 3).

## Prerequisites

1. **git:** `tally-oss/` is not a repo yet. `git init`, commit the docs on `main`, build on branch
   `build/wave-0` then `build/wave-1-…` per worktree.
2. **Toolchain:** Rust 1.100.0 is installed (used by Tessera); pin it in `rust-toolchain.toml`.
   Install `cargo-deny`, `cargo-nextest` (present), `cargo-fuzz`, `cargo-llvm-cov`.
3. **Agent limit:** this session's workflow size guideline is "medium" (under 10 agents). 110 needs
   "Dynamic workflow size" raised in the app settings. All builders, reviewers and integrators run
   on **Sonnet**; Wave 0 and the final whole-branch review run in the controlling session.
4. **Observed fixtures (owner, any time):** in TallyPrime Educational mode create the fixture
   company in `crates/ledgerly-conformance/fixtures/README.md` (written in Wave 0), post its
   vouchers, export Day Book / TB / BS / P&L / Stock Summary / GSTR-1 as XML and Excel, and record
   the release and date. Each file promotes `pending` cases to `observed`.
5. **Budget:** 113 agents, plus each card's review and up to 2 fix rounds (fixes go back to the
   same builder by SendMessage, but they still cost tokens). Estimate **30–60M tokens** for the full
   run. The 30-agent pilot (end of document) measures the real per-card cost first.
6. **Machine (16 cores, 91 GB RAM, 195 GB free on this disk):** About 100 worktrees each with its own
   `target/` (bundled SQLite, egui/wgpu) would need several hundred GB and would thrash the CPU.
   So: install `sccache`; one shared `CARGO_TARGET_DIR` per lane (17, not one per worktree); builders build with
   `-p <crate>` only; **cap concurrent builders at 24** (the DAG keeps the queue full); abort if
   free space drops under 40 GB. Real concurrency is about 24 + reviewers, not 100.

## Every builder's brief (applies to every card)

Each builder gets: this section, its card, `CLAUDE.md`, `docs/engineering-rules.md`, the spec
sections its card cites, and the Wave 0 interfaces of the crates it consumes (read the code).

Procedure for every card — one card, one worktree, one branch `w1/<card-id>`:

- [ ] Read the files the card owns (Wave 0 created them with a doc comment and the frozen signatures) and the types it consumes.
- [ ] Write the card's listed tests first, with provenance comments. Run `cargo nextest run -p <crate> <test-name>`; expect FAIL (compile error or assertion).
- [ ] Implement the card's signatures in the owned files only.
- [ ] Run `cargo nextest run -p <crate>`; expect all PASS.
- [ ] Run `cargo fmt --check && cargo clippy -p <crate> --all-targets --all-features -- -D warnings`; expect clean.
- [ ] Commit: `git commit -m "<crate>: <card-id> <what and why>"` ending with the attribution line.
- [ ] Report: files changed, tests added, every case left `pending` or `Unsupported` and why, and any interface request filed.

A card's listed tests are the minimum. Add tests for the invariants the spec names in the cited
sections. Never weaken a Wave 0 signature; if it is wrong, file an interface request and
implement against it as declared.

## Wave 0: freeze the interfaces (controlling session, sequential)

Nothing parallel starts until this lands with all gates green.

### Task 0.1: workspace and gates

**Files:** `Cargo.toml` (workspace, `[workspace.dependencies]`, `[workspace.lints]`),
`rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `deny.toml`, `.github/workflows/ci.yml`,
`LICENSE-AGPL`, `LICENSE-MIT`, `LICENSE-APACHE`, `docs/interface-requests.md`.

- [ ] Create all 19 crates of S§4 with `[lints] workspace = true` and **every dependency each crate will need declared now** (so builders never touch `Cargo.toml`/`Cargo.lock`). At least: core `rust_decimal`, `time`, `serde`, `thiserror`; store `rusqlite` (bundled), `sha2`, `ciborium`; gst/packs `toml`; tally-xml `quick-xml`, `encoding_rs`; bridge `ureq`; formats `serde_json`, `rust_xlsxwriter`, `calamine`; bankfmt `csv`, `calamine`; print `printpdf`, `qrcode`; assist `keyring`, a llama.cpp binding (vendored); mcp an MCP server crate; app `eframe` (wgpu), `egui_extras`, `accesskit`; dev `insta`, `proptest`, `criterion`. Pick exact versions with `cargo add` and commit `Cargo.lock`.
- [ ] Lints: `unsafe_code = "forbid"`, `missing_docs = "warn"`, clippy `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `indexing_slicing`, `float_arithmetic` (allowed via `#[expect]` in app/assist only), `wildcard_enum_match_arm` = deny; `cognitive-complexity-threshold = 25`, `too-many-lines-threshold = 80`, `too-many-arguments-threshold = 7`.
- [ ] CI job `headless-check`: `cargo tree -p <each headless crate> -e normal | grep -E 'egui|eframe|wgpu'` must be empty.
- [ ] Gates green; commit.

### Task 0.2: `ledgerly-core` (complete, not just declared)

Wave 0 writes and tests these fully because every builder consumes them.

```rust
pub struct Paise(i64);                       // checked_add/sub/mul_qty/neg -> Result<Paise, MoneyError>
pub enum DrCr { Dr, Cr }
pub struct SignedAmount { pub amount: Paise, pub side: DrCr }
pub struct Qty(pub rust_decimal::Decimal);
pub struct Rate(pub rust_decimal::Decimal);  // per-unit price or percentage, documented per use
pub enum RoundingRule { HalfUpToPaise, HalfUpToRupee, Truncate, Up }
pub fn round_decimal_to_paise(v: Decimal, rule: RoundingRule) -> Result<Paise, MoneyError>;
pub struct FinancialYear { pub start: time::Date }   // 1 April by default
pub struct Period { pub from: time::Date, pub to: time::Date }   // inclusive; from <= to
pub enum Outcome<T> { Value(T), Unsupported(Unsupported) }
pub struct Unsupported { pub what: String, pub reason: String }
pub struct Gstin(String);   // parse() validates length 15, state code, PAN shape, check char
pub struct Pan(String);
pub struct StateCode(u8);   // GST state codes; the list is copied from the GSTN code table and the source is cited in the file
pub struct Hsn(String);     // 4, 6 or 8 digits; SAC 99xxxx
pub enum Clock { System, Fixed(time::OffsetDateTime) }
// IDs: one macro, newtypes over u64 allocated by the store:
// CompanyId, GroupId, LedgerId, VoucherTypeId, VoucherId, CostCentreId, CostCategoryId,
// UnitId, StockGroupId, StockItemId, GodownId, BatchId, BankStatementId, UserId, EventSeq
pub struct TallyRef { pub guid: String, pub alter_id: Option<u64> }
```

GSTIN check character (to be `derived` only after the Wave 0 author quotes the published algorithm
source in the test; the description here is from memory — Luhn mod-36: chars `0-9A-Z` → 0..35, scan the 14
chars right to left with factor 2,1,2,…, add `p/36 + p%36` per product, check = `(36 − sum%36)%36`).
Test vectors (fictional, computed with that algorithm, which also reproduces a widely published
sample GSTIN that is not used as test data): `29ABCDE1234F1ZW`, `07AAACL0000A1ZP`, `24AAACB1111C1ZT` are valid; changing
the last character of any makes `Gstin::parse` return `GstinError::CheckCharacter`.

- [ ] Tests: GSTIN vectors above; `Paise::checked_add(Paise(i64::MAX), Paise(1))` errors; `FinancialYear::containing(2026-03-31)` starts 2025-04-01; `Period` with `from > to` errors.
- [ ] Commit.

### Task 0.3: `ledgerly-model` types (complete data types, invariant fns declared)

Data types per S§5 with serde: `Company`, `Features`, `Group` (+ `ReservedGroup` enum of the 28
reserved groups from R1, `Nature`), `Ledger` (+ `PartyDetails`, `BankDetails`, `TaxDetails`,
`RegistrationType`), `VoucherType` (+ `BaseVoucherKind`, `NumberingConfig`), `Voucher`
(+ `VoucherStatus`, `VoucherMode`), `LedgerLine`, `BillAllocation` (+ `BillRefKind`),
`CostAllocation`, `InventoryLine` (+ `GodownAllocation`, `BatchAllocation`), `Unit`, `StockGroup`,
`StockItem` (+ `ValuationMethod`, `GstHistory`), `Godown`, `Batch`, `CostCategory`, `CostCentre`.

The read model every pure function consumes:

```rust
pub struct Masters { /* BTreeMap<Id, T> per master type */ }
pub struct Posting { pub voucher: VoucherId, pub date: Date, pub ledger: LedgerId,
                     pub amount: SignedAmount, pub status: VoucherStatus, pub type_id: VoucherTypeId }
pub struct StockMove { pub voucher: VoucherId, pub date: Date, pub item: StockItemId,
                       pub godown: GodownId, pub batch: Option<BatchId>,
                       pub qty_in: Qty, pub qty_out: Qty, pub value: Paise }
pub struct Books { pub company: Company, pub masters: Masters, pub vouchers: BTreeMap<VoucherId, Voucher>,
                   pub postings: Vec<Posting>, pub stock_moves: Vec<StockMove> }   // sorted by (date, voucher)
pub enum Scenario { Actual, Including(Vec<VoucherStatus>) }
```

- [ ] Also here, because store events carry them: `BankMatch` and `ProposalRecord` (plain data; the bank and assist crates build them).
- [ ] Declared (implemented by cards M1–M4): `pub fn validate_voucher(v: &Voucher, m: &Masters, c: &Company) -> Result<(), VoucherError>` and seed functions `reserved_groups()`, `default_voucher_types()`.
- [ ] Commit.

### Task 0.4: every other crate's frozen surface + one module file per builder

For each card below, create the files it **Owns** with a module doc comment and the card's
**Produces** signatures with bodies returning `Ok(Outcome::Unsupported(..))` naming the card ID
(returning `Unsupported` is a correct answer, not a lying stub). Wire every module in `lib.rs`.
Seams (the only traits, rule C2):

```rust
// ledgerly-store
pub trait Store { fn commit(&mut self, events: &[Event], meta: &AuditMeta) -> Result<CommitReceipt, StoreError>;
                  fn load_books(&self, period: Option<Period>) -> Result<Books, StoreError>;
                  fn events(&self, from: EventSeq, limit: usize) -> Result<Vec<AuditRecord>, StoreError>; }
pub enum Event { MasterCreated(MasterSnapshot), MasterAltered{before: MasterSnapshot, after: MasterSnapshot},
                 MasterDeleted(MasterSnapshot), VoucherPosted(Voucher), VoucherAltered{before: Voucher, after: Voucher},
                 VoucherCancelled{before: Voucher}, VoucherDeleted{before: Voucher},
                 SettingChanged{key: String, before: String, after: String}, PeriodLocked{until: Date},
                 StatementImported(BankStatementId), MatchConfirmed(BankMatch), AiProposalRecorded(ProposalRecord) }
pub struct AuditMeta { pub actor: Actor, pub at: OffsetDateTime, pub reason: Option<String>, pub app_version: String }
pub enum Actor { User(UserId), Import{job: String, on_behalf_of: UserId}, Assistant{model: String, on_behalf_of: UserId} }
pub struct AuditRecord { pub seq: EventSeq, pub prev_hash: [u8; 32], pub hash: [u8; 32], pub meta: AuditMeta, pub event: Event }
// ledgerly-tally-bridge
pub trait TallyTransport { fn post(&self, xml_request: &str) -> Result<String, TransportError>; }
// ledgerly-assist
pub trait Proposer { fn propose(&self, input: &ProposalInput, cancel: &CancelToken) -> Result<Proposal, AssistError>; }
```

- [ ] `crates/ledgerly-conformance/fixtures/README.md`: the fixture company (2 GSTINs in states 27 and 29, 12 ledgers, 3 stock items, the R3 valuation item, 20 vouchers across FY 2025-26 including both sides of 22 Sep 2025), and the export checklist for Prerequisite 4.
- [ ] Gates green (fmt, clippy, test, deny, headless-check); commit on `main`; tag `wave-0`.

---

## Wave 1: builders (93 Sonnet agents, parallel, each in its own worktree)

Card format: **ID — title.** Owns · Depends on · Produces · Tests (minimum; values with provenance) · S§.
IDs: C core, M model, S store, E engine, RP reports, G gst, F gst-formats, T stock, K bank, BF bankfmt,
X tally-xml, B bridge, A assist, P mcp, PR print, Q conformance, H hardening, U app. (R1..R10 always
means a research file.)

**Scheduling (a DAG, not one batch).** A card with no `Depends on` starts at once (about 70 cards).
A card with `Depends on` starts when those cards are approved and merged into `integration`, and
branches from there. So Wave 1 runs as a rolling pipeline, not a fixed batch.
"Books fixture" = `ledgerly_conformance::fixtures::books_small()` (card Q1); until it lands, build
the `Books` you need inline in the test.

### Core extras (3)

- **C1 — Indian number formatting and amount in words.** Owns `core/src/format.rs`. Produces `format_inr(Paise) -> String`, `amount_in_words(Paise) -> String`. Tests: `1_234_567_00` → `"12,34,567.00"`; `12_345_678_50` → `"Rupees One Crore Twenty Three Lakh Forty Five Thousand Six Hundred Seventy Eight and Fifty Paise Only"`; `-50_00` → `"-50.00"` (derived — Indian numbering lakh = 10^5, crore = 10^7). S§7.
- **C2 — Money arithmetic hardening.** Owns `core/src/money_ops.rs` (+ proptests). Produces `Paise::sum<I>(iter) -> Result<Paise, MoneyError>`, `percent_of(Paise, Rate, RoundingRule) -> Result<Paise, MoneyError>`, `split_evenly(Paise, n) -> Vec<Paise>` (remainder to first parts). Tests: `checked_add_overflow_errors`; `neg_zero_displays_as_zero`; `percent_of(10_000_00, 18%, HalfUpToPaise) == 1_800_00`; `split_evenly(1_00, 3) == [34, 33, 33]`; proptest Σ split == input. S§3.
- **C3 — Date entry parsing.** Owns `core/src/date_entry.rs`. Produces `parse_date_entry(input: &str, fy: FinancialYear, today: Date) -> Result<Date, DateEntryError>`. Tests (FY 2026-27): `"1-4"` → 2026-04-01, `"15/1"` → 2027-01-15 (month < April resolves into the FY's second calendar year), `"31-2"` errors, `"2026-05-07"` → 2026-05-07 (derived — Indian FY 1 Apr–31 Mar). S§7.

### Model (4)

- **M1 — Reserved groups seed.** Owns `model/src/seed_groups.rs`. Produces `reserved_groups() -> Vec<Group>`. Tests: 28 groups; 15 primary; every `ReservedGroup` variant appears exactly once; `Sales Accounts` nature Income with `affects_gross_profit = true`; `Duties & Taxes` under `Current Liabilities` (derived — R1 §Groups). S§5.
- **M2 — Default voucher types.** Owns `model/src/seed_voucher_types.rs`. Produces `default_voucher_types() -> Vec<VoucherType>`. Tests: one per v1 `BaseVoucherKind`; Sales and Credit Note numbering `Automatic` with `prevent_renumbering = true`; Memorandum `affects_books = false`. S§5.
- **M3 — Feature flags dependency rules.** Owns `model/src/features.rs`. Produces `Features::validate(&self) -> Result<(), FeatureError>`, `Features::requires(flag) -> Vec<Flag>`. Tests: batches without inventory errors; GST without a GSTIN on the company errors. S§5.
- **M4 — Voucher validation (store-free invariants).** Owns `model/src/validate.rs`. Produces `validate_voucher` (from 0.3). Tests: Σ Dr ≠ Σ Cr → `VoucherError::Unbalanced { dr, cr }`; bill allocations not summing to line → `BillAllocationMismatch`; date before `books_begin` → `BeforeBooksBegin`; Against ref on another ledger → `BillOnWrongLedger`; a Sales voucher with no party → `MissingParty`. S§5 invariants 1, 2, 6.

### Store (6)

- **S1 — Schema and migrations.** Owns `store/src/schema.rs`, `store/migrations/0001_init.sql`. Produces `SqliteStore::create(path, &Company, &Clock) -> Result<SqliteStore, StoreError>`, `SqliteStore::open(path)`. Tests: create then open round-trips `Company`; WAL mode on; opening a file with a newer schema version errors `SchemaTooNew`. S§4.
- **S2 — Event append and hash chain.** Owns `store/src/commit.rs`. Produces `impl Store::commit`. Tests: two commits → seq 1,2 and `rec2.prev_hash == rec1.hash`; hash = SHA-256 over canonical CBOR of `(seq, prev_hash, meta, event)`; a failing commit (constraint error mid-batch) leaves no rows. S§6.6.
- **S3 — Verifier and daily checkpoints.** Owns `store/src/verify.rs`. Depends on S1, S2. Produces `verify_chain(&SqliteStore) -> Result<VerifyReport, StoreError>`, `write_checkpoint(day)`. Tests: flipping one byte in any stored event → report names that seq; deleting a row → report names the gap. S§6.6.
- **S4 — Books loader (masters, vouchers, postings).** Owns `store/src/load.rs`. Depends on S1, S2. Produces `impl Store::load_books`. Tests: posting a voucher then `load_books` returns its postings sorted by (date, voucher); altered voucher shows only the new postings; cancelled voucher has zero postings but is listed. S§6.1.
- **S5 — Stock moves projection.** Owns `store/src/stock_load.rs`. Depends on S4. Produces `stock_moves` filled in `Books`. Tests: item invoice with 2 godown allocations → 2 moves; receipt note → `qty_in`. S§6.4.
- **S6 — Backup, restore, rebuild-compare.** Owns `store/src/backup.rs`. Depends on S1, S2, S4. Produces `backup_to(dir, &Clock) -> Result<PathBuf>`, `rebuild_projections_and_compare() -> Result<RebuildReport>`. Tests: backup file opens and verifies; restore into new path equals original `load_books`. S§6.6.

### Engine (7) — pure: `(command, &Books) -> Result<Outcome<Vec<Event>>, EngineError>`

- **E1 — Group and ledger commands.** Owns `engine/src/masters_accounts.rs`. Produces `create_group`, `alter_group`, `delete_group`, `create_ledger`, `alter_ledger`, `delete_ledger`. Tests: deleting a reserved group errors `ReservedGroup`; deleting a ledger with postings errors `InUse { vouchers: n }`; cycle in group parent errors. S§5 inv 5.
- **E2 — Inventory and cost masters commands.** Owns `engine/src/masters_stock.rs`. Same verbs for units, stock groups, items, godowns, batches, cost categories/centres. Tests: compound unit referencing itself errors; deleting a godown with stock errors. S§6.4.
- **E3 — Voucher post/alter/cancel/delete.** Owns `engine/src/vouchers.rs`. Depends on M4, E4, E5, E7. Produces `post_voucher`, `alter_voucher`, `cancel_voucher`, `delete_voucher`. Tests: valid payment → one `VoucherPosted`; altering a voucher inside a locked period errors `PeriodLocked { until }`; cancel keeps the number; deleting a sales voucher errors `SalesSeriesNoDelete` (cancel instead). S§5 inv 1, 7, 8.
- **E4 — Numbering allocator.** Owns `engine/src/numbering.rs`. Produces `next_number(&VoucherType, series, date, &Books) -> Result<String, EngineError>`. Tests: prefix `INV/26-27/` + width 4 → `INV/26-27/0001`; restart yearly; number used by a cancelled voucher is not reused; GST invoice number > 16 chars errors (derived — CGST Rule 46(b): ≤ 16 characters, unique per FY). S§5 inv 7.
- **E5 — Period locks.** Owns `engine/src/locks.rs`. Produces `lock_period(until, &Books) -> Result<Outcome<Vec<Event>>>`, `check_unlocked(date, &Books)`. Tests: post/alter/delete on lock date all error; lock event recorded with reason. S§5 inv 8.
- **E6 — Scenarios.** Owns `engine/src/scenario.rs`. Produces `effective_postings(&Books, &Scenario, as_of) -> Vec<&Posting>`. Tests: optional voucher excluded from `Actual`, included in `Including([Optional])`; post-dated voucher after `as_of` excluded; memorandum never in `Actual`. S§5 inv 3.
- **E7 — Bill-wise allocation.** Owns `engine/src/bills.rs`. Produces `open_bills(&Books, ledger, as_of) -> Vec<OpenBill>`, `validate_bill_refs(&Voucher, &Books)`. Tests: receipt against a ₹10,000 bill for ₹12,000 errors `OverSettled`; advance then invoice then Against → bill closes; credit days 30 sets due date. S§5 inv 2.

### Reports (10; RP10 is listed under App next to U12) — pure: `(&Books, &ReportParams) -> Result<Outcome<Report>, ReportError>`

- **RP1 — Params, columns, drill keys.** Owns `reports/src/params.rs`, `reports/src/drill.rs`. Produces `ReportParams`, `ColumnSpec::{Single, Comparative(Vec<Period>), Monthly}`, `DrillKey::{Group, Ledger, Voucher, Item, Bill}`, `drill_down(&Report, row) -> Option<ReportParams>`. Tests: Monthly over FY → 12 columns Apr..Mar; drill from group row yields ledger-level params with the same period and scenario. S§6.2.
- **RP2 — Trial balance.** Owns `reports/src/trial_balance.rs`. Tests: hand-built books (opening Capital Cr 1,00,000; Cash Dr 1,00,000; sale 11,800 incl. GST) → TB totals equal; unbalanced opening shows explicit `Difference in opening balances` row. S§6.2, inv 4.
- **RP3 — Balance sheet.** Owns `reports/src/balance_sheet.rs`. Tests: BS totals equal; P&L ledger carries current-year profit; condensed shows primary groups only. S§6.2.
- **RP4 — Profit & loss (trading form).** Owns `reports/src/profit_loss.rs`. Tests: GP = Sales + Closing − Opening − Purchases − Direct expenses on a hand set (derived — S§6.2 formula); closing stock from an injected `StockValuation` input. S§6.2.
- **RP5 — Group/ledger summaries and ledger vouchers.** Owns `reports/src/ledger.rs`. Tests: running balance per line; opening row; Dr/Cr side switch when balance crosses zero. S§6.2.
- **RP6 — Day book and registers.** Owns `reports/src/registers.rs`. Tests: day book ordered by date then entry seq; sales register monthly totals; cancelled vouchers listed with zero amount and flagged. S§6.2.
- **RP7 — Outstandings and ageing.** Owns `reports/src/outstanding.rs`. Tests: buckets 0–30/31–60/61–90/>90 by due date; on-account amounts shown separately. S§6.2.
- **RP8 — Exports.** Owns `reports/src/export.rs`. Produces `to_csv`, `to_xlsx`, `to_json`. Tests: `insta` snapshot of CSV for the RP2 fixture; XLSX opens with calamine and cells are numbers in rupees with 2 decimals (no float rounding: written from `Paise` via string). S§6.2.

### GST engine (12) — rule packs are data; every entry carries `legal_ref` or `pending = true`

- **G1 — Rule pack format, loader, effective dating.** Owns `gst/src/pack.rs`, `gst/packs/README.md`. Produces `RulePack::load(toml) -> Result<RulePack, PackError>`, `RuleSet::for_time_of_supply(date)`. Tests: `pack_selected_by_time_of_supply_not_entry_date`; overlapping effective ranges error; entry with neither `legal_ref` nor `pending` errors. S§6.3.
- **G12 — Time of supply.** Owns `gst/src/time_of_supply.rs`. Produces `time_of_supply(&SupplyDates) -> Outcome<Date>` where `SupplyDates { invoice: Option<Date>, supply: Option<Date>, payment: Option<Date>, kind: GoodsOrServices }`, and `rate_change_rule(&SupplyDates, change_date) -> Outcome<RateSide>` per CGST s.14. Tests: each s.14 case (supply/invoice/payment on either side of a change) with the section's text **quoted in the test** as the derivation; any case where a needed date is `None` → Unsupported. S§6.3.
- **G2 — Rate data packs.** Owns `gst/packs/rates-*.toml`. Content: slab structure from 22 Sep 2025 (0/5/18/40) and pre-change slabs, each entry with its notification reference **or `pending = true`** (R4 `[K]` facts stay pending). Tests: every entry has `legal_ref` xor `pending`; no HSN appears twice in one range. S§6.3.
- **G3 — Place of supply.** Owns `gst/src/pos.rs`. Produces `place_of_supply(&SupplyFacts) -> Outcome<PlaceOfSupply>`. Tests (derived — IGST Act s.10 goods, s.12 services): goods moved Maharashtra → Karnataka = Inter; goods not moved, supplier and location both in 27 = Intra; B2C service by supplier in 29 to unregistered recipient with no address = location of supplier (s.12(2)(b)(ii)); export = ZeroRatedExport; SEZ unit = ZeroRatedSez; anything else = Unsupported. S§6.3.
- **G4 — Voucher tax computation and rounding.** Owns `gst/src/compute.rs`. Depends on G1, G2. Produces `compute_voucher_tax(&Voucher, &Masters, &RuleSet) -> Result<Outcome<GstBreakup>, GstError>`. Tests: ₹10,000 @ 18% intra → CGST 900.00 + SGST 900.00; inter → IGST 1,800.00; 3 lines 333.33 each @ 5% → line tax unrounded, explicit round-off line; pack entry `pending` → Unsupported (derived — CGST s.9, IGST s.5, s.170 rounding). S§6.3.
- **G5 — Reverse charge.** Owns `gst/src/rcm.rs`. Tests: purchase from unregistered under notified RCM service → liability on recipient, ITC eligible after payment; non-notified → no RCM; pack-driven list. S§6.3.
- **G6 — Exempt / nil / non-GST / exports / SEZ / advances.** Owns `gst/src/classify.rs`. Produces `classify_line(..) -> Outcome<SupplyKind>`. Tests: each `SupplyKind` variant reachable; unknown combination → Unsupported (goes to "uncertain" bucket). S§6.3.
- **G7 — ITC eligibility incl. s.17(5).** Owns `gst/src/itc.rs`. Tests: motor vehicle (blocked, pack-driven), food and beverages blocked, office supplies eligible; ineligible ITC never appears in 3B 4(A). S§6.3.
- **G8 — GSTR-1 builder (incl. HSN and document summary).** Owns `gst/src/gstr1.rs`. Depends on G4, G6, G12. Produces `build_gstr1(&Books, gstin, period, &RuleSet) -> Result<Outcome<Gstr1>, GstError>` with exception buckets `Ready | NotRelevant | Mismatch | NeedsCorrection`. Tests: B2B invoice lands in 4A; B2CL threshold (pack value) vs B2CS; credit note in 9B; voucher missing party GSTIN → NeedsCorrection with voucher id. S§6.3.
- **G9 — GSTR-1A amendments.** Owns `gst/src/gstr1a.rs`. Depends on G8. Tests: altering a filed-period invoice produces a 1A entry, not an edit of GSTR-1. S§6.3.
- **G10 — GSTR-3B working.** Owns `gst/src/gstr3b.rs`. Depends on G4, G6, G7. Tests: 3.1(a) taxable value = Σ outward taxable; 4(A)(5) all other ITC; books-vs-portal comparison column populated from an injected portal value and differences listed. S§6.3.
- **G11 — 2B/IMS reconciliation matcher.** Owns `gst/src/recon2b.rs`. Depends on F3. Produces `match_2b(&Books, &Gstr2b, tolerance) -> ReconResult` with `Exact | Normalised | Tolerance | Unmatched` and suggested `Accept | Reject | Pending`. Tests: invoice `INV-001` vs `INV/001` → Normalised; ₹1 difference within tolerance → Tolerance, not Exact; credit note may stay Pending; nothing is auto-accepted (every suggestion has `confirmed = false`). S§6.3.

- **RP9 — Stock summary.** Owns `reports/src/stock_summary.rs`. Depends on T3, T4, T5. Tests: per item and godown: opening, inwards, outwards, closing qty and value for the R3 dataset; a `Provisional` valuation is shown with its flag, never as a plain number. S§6.2, §6.4.

### GST formats (5) — MIT/Apache, built from official templates only

- **F1 — GSTR-1 JSON writer.** Owns `gst-formats/src/gstr1_json.rs`. Tests: `insta` snapshot for a 4A + B2CS + HSN sample; amounts serialised as numbers with 2 decimals from `Paise` (no float path). S§6.3.
- **F2 — GSTR-1 offline tool Excel writer.** Owns `gst-formats/src/gstr1_xlsx.rs`. Tests: sheet names and header rows match the template file in `gst-formats/templates/` (owner supplies the official template; until then the test is `pending`). S§6.3.
- **F3 — GSTR-2B and IMS JSON readers.** Owns `gst-formats/src/gstr2b.rs`, `ims.rs`. Tests: parse sample; unknown fields preserved in `extra`; malformed JSON → error with path. S§6.3.
- **F4 — e-Invoice INV-01 JSON + local validation.** Owns `gst-formats/src/einvoice.rs`. Tests: required fields present; `DocDtls.No` ≤ 16 chars; totals equal Σ items; schema-invalid → error naming the field. S§6.3.
- **F5 — e-Way bill JSON.** Owns `gst-formats/src/ewb.rs`. Tests: snapshot; distance and vehicle number validation. S§6.3.

### Stock (5) — valuation over `&[StockMove]`

R3 dataset (all tests below): opening 100 @ 10 = 1,000; 10-Apr purchase 50 @ 13 = 650; 15-Apr sale
120; 20-May purchase 40 @ 15 = 600; 31-May sale 30; closing 40 (derived — R3 §Valuation worked
examples from the vendor's published formulas). **Average cost here is our documented periodic
definition** (spec open question 1); it is not a claim about Tally's output until observed.

- **T1 — Units and compound conversion.** Owns `stock/src/units.rs`. Tests: 1 box = 12 pcs, 2 box 5 pcs = 29 pcs; alternate unit conversion uses Decimal, no rounding until display. S§6.4.
- **T2 — Godown/batch allocation and positions.** Owns `stock/src/positions.rs`. Tests: position per (item, godown, batch) as of date; expiry recorded. S§6.4.
- **T3 — Average cost (periodic).** Owns `stock/src/avg.rs`. Tests: closing 473.68; consumption 1,776.32. S§6.4.
- **T4 — FIFO annual.** Owns `stock/src/fifo.rs`. Tests: closing 600.00; next-year opening rate = closing value ÷ qty. S§6.4.
- **T5 — Last purchase, standard cost, negative stock.** Owns `stock/src/other_methods.rs`, `stock/src/negative.rs`. Tests: last purchase → 40 × 15 = 600.00; standard cost with no dated standard → error, never fallback; any period where running qty < 0 → `Provisional { reason: NegativeStock { item, date } }`; policy Block rejects the voucher. S§6.4.

### Banking (5) and bank formats (4)

- **K1 — Statement model and balance check.** Owns `bank/src/statement.rs`. Tests: opening + Σ = closing else `BalanceMismatch`; running balance break → error naming the first bad row and nothing stored (all-or-nothing); duplicate file hash rejected. S§6.5.
- **K2 — Matcher (exact, rule).** Owns `bank/src/matcher.rs`. Tests: many-to-many (one deposit, three receipts); amount + date ± 3 days; rule `"NEFT-*-RENT"` → Rent ledger suggestion; matches are suggestions until confirmed. S§6.5.
- **K3 — BRS report.** Owns `bank/src/brs.rs`. Tests: balance as per books ± unreconciled = balance as per bank (derived — standard BRS identity). S§6.5.
- **K4 — Cheques, PDCs, bounce.** Owns `bank/src/cheques.rs`. Tests: leaf reuse errors; bounce produces a reversing journal proposal plus charges line (proposal, not posting). S§6.5.
- **K5 — UPI QR payload.** Owns `bank/src/upi.rs`. Tests: `upi://pay?pa=…&pn=…&am=1180.00&cu=INR&tn=INV%2F26-27%2F0001`; amount from `Paise`; percent-encoding. S§6.5.
- **BF1 — Format registry.** Owns `bankfmt/src/registry.rs`, `bankfmt/formats/*.toml`. Tests: TOML describing columns, date format, Dr/Cr convention loads; unknown key errors. S§6.5.
- **BF2 — CSV parser.** Owns `bankfmt/src/csv.rs`. Tests: Dr/Cr split columns and signed single column both handled; Indian-formatted amounts `1,23,456.78`. S§6.5.
- **BF3 — XLSX parser.** Owns `bankfmt/src/xlsx.rs`. Tests: header row detection skips bank letterhead rows. S§6.5.
- **BF4 — MT940 parser.** Owns `bankfmt/src/mt940.rs`. Tests: `:60F:`/`:61:`/`:86:`/`:62F:` parsed; balance check passes on the sample. S§6.5.

### Tally XML (5) and bridge (4)

- **X1 — Envelope types and request builders.** Owns `tally-xml/src/envelope.rs`, `request.rs`. Tests: export request for "List of Companies" and for vouchers in a date range snapshot-matches the documented envelope (derived — R8 §XML interface). S§6.7.
- **X2 — Master parser.** Owns `tally-xml/src/masters.rs`. Tests: GROUP, LEDGER, STOCKITEM, UNIT, GODOWN, VOUCHERTYPE parsed with GUID and ALTERID; invalid control chars and `&#4;` tolerated with a warning; UTF-16 LE with BOM decoded; truncated input (no `</ENVELOPE>`) → error with byte offset plus everything parsed so far; unknown tags kept in `extra`. S§6.7.
- **X3 — Voucher parser.** Owns `tally-xml/src/vouchers.rs`. Tests: ALLLEDGERENTRIES.LIST, BILLALLOCATIONS.LIST, INVENTORYENTRIES.LIST, BATCHALLOCATIONS.LIST; Tally amount sign convention (negative = Dr) mapped to `DrCr` (derived — R8); amounts parsed to `Paise` exactly. S§6.7.
- **X4 — Writer (Tally-importable XML).** Owns `tally-xml/src/writer.rs`. Tests: proptest parse(write(x)) == x for masters and vouchers. S§6.7.
- **X5 — Tally report exports.** Owns `tally-xml/src/reports.rs`. Produces request builders and parsers for Tally's exported Trial Balance, Balance Sheet, Profit & Loss, Stock Summary and Bills Outstanding into `TallyReports { tb, bs, pl, stock, outstanding }` (row label, Tally GUID where present, Dr/Cr `Paise`). Tests: recorded XML for each report parses; totals row kept separately from detail rows. Fixtures become `observed` once the owner captures them (Prerequisite 4); until then `pending`. S§6.7.
- **B1 — Transport and discovery.** Owns `tally-bridge/src/transport.rs`, `discover.rs`. Produces `HttpTransport` (port 9000), `list_companies(&dyn TallyTransport)`. Tests: recorded-fixture transport; connection refused → `TallyNotRunning` with the port. S§6.7.
- **B2 — Batched pull and raw capture.** Owns `tally-bridge/src/pull.rs`. Tests: date range split so each request ≤ 5,000 vouchers; raw responses written to the capture dir before parsing; cancellation between batches leaves capture consistent. S§6.7.
- **B3 — Staging and mapping.** Owns `tally-bridge/src/mapping.rs`. Depends on X2, X3. Produces `map_to_events(staged) -> Result<Vec<Event>, MapError>` in dependency order. Tests: ledger under a Tally reserved group maps to our `ReservedGroup` by Tally's reserved name; unknown parent → error; GUID kept in `TallyRef`. S§6.7.
- **B4 — Reconciliation report.** Owns `tally-bridge/src/reconcile.rs`. Depends on X5, RP2, RP3, RP4, RP9. Produces `reconcile(ours: &Books, theirs: &TallyReports) -> ReconReport`. Tests: equal TB → all green; one ledger differs by ₹1 → listed with both values, never hidden. S§6.7.

### Assist (4), MCP (1), print (2)

- **A1 — Proposal types and validator.** Owns `assist/src/proposal.rs`, `assist/src/validate.rs` (assist depends on engine and gst; never the reverse). Depends on G4, E3. Produces `VoucherDraft`, `MatchSuggestion`, `ReportQuery`, `MappingSuggestion`, `Transcribed<T> { value, source: SourceRef, confidence }`, `validate_draft(&VoucherDraft, &Books, &RuleSet) -> DraftCheck`. Tests: draft total ≠ engine-recomputed total → `Blocked { field }`; confidence below threshold → field blank. S§6.8.
- **A2 — Natural-language entry.** Owns `assist/src/nl_entry.rs`. Produces a `Proposer` impl over a prompt template + grammar-constrained JSON; test double returns recorded outputs. Tests: "paid rent 25k to Sharma Properties by HDFC" → Payment draft, ledger names resolved against masters (unresolved → `CreateLedger` suggestion, never invented id). S§6.8.
- **A3 — OCR pipeline.** Owns `assist/src/ocr.rs`. Tests: recorded OCR output for a fictional bill image → `Transcribed` fields with bbox; GSTIN failing checksum → field rejected. S§6.8.
- **A4 — Cloud BYOK client and redaction.** Owns `assist/src/cloud.rs`, `redact.rs`. Tests: PAN/GSTIN/account numbers redacted before send; key read from keychain abstraction; no network in tests. S§6.8, §9.
- **P1 — MCP server.** Owns `mcp/src/*.rs`. Tools: read queries and `propose_voucher`; no posting tool exists. Tests: tool list snapshot; calling a non-existent `post_voucher` errors. S§6.8.
- **PR1 — Print template format.** Owns `print/src/template.rs`. Tests: TOML template with header, item table, tax table, amount in words, UPI QR slot loads; unknown slot errors. S§4.
- **PR2 — PDF rendering.** Owns `print/src/pdf.rs`. Depends on C1, K5, PR1. Tests: tax invoice for the RP2 sale renders; text extraction contains "Tax Invoice", GSTIN, total `11,800.00`. S§4.

### Conformance and hardening (4)

- **Q1 — Conformance runner, fixtures, ratchet.** Owns `conformance/src/*.rs`, `pass-list.txt`. Produces `fixtures::books_small()`, case format with `provenance` field, runner, ratchet check. Tests: a case without provenance reports `pending`; a passing case removed from passing fails the ratchet. S§10.
- **H1 — Fuzz Tally XML.** Owns `fuzz/fuzz_targets/tally_xml.rs`. 10-minute campaign, no panics/timeouts. S§10.
- **H2 — Fuzz GST JSON + bank parsers.** Owns `fuzz/fuzz_targets/gst_json.rs`, `bank.rs`. S§10.
- **H3 — Benchmarks.** Owns `benches/*.rs`. Synthetic 1M-voucher `Books` generator; TB/BS/P&L and voucher validate benches against S§8 targets. S§8.

### App (12) — against `ledgerly-api` types; until Wave 2 wiring, screens take a `Books` fixture

- **U1 — Shell, theme, screen stack, breadcrumbs, tabs.** Owns `app/src/shell/*`, `app/src/theme.rs`. Visual reference: mock. Light/dark/high-contrast tokens. S§7.
- **Keys: the spec wins over the mock.** Save is Ctrl+A (and Ctrl+S); Ctrl+Enter alters the master under the cursor (the mock's Ctrl+Enter-to-save is changed in U4/U5). Mock-only keys (Ctrl+J AI, `?` help, J/K/A/R/P in IMS) are kept.
- **U2 — Keymap and command palette.** Owns `app/src/keymap.rs`, `palette.rs`, `assets/keymap-classic.toml`. Tests: classic preset binds F4–F9, F2, Ctrl+A, Esc, Alt+C, Ctrl+Enter, Alt+D, Alt+X, Alt+2; rebinding round-trips through TOML. S§7.
- **U3 — Home / dashboard.** Owns `app/src/screens/home.rs`. KPI tiles read from report outputs only. S§7, mock Home.
- **U4 — Accounting voucher entry.** Owns `app/src/screens/voucher_entry.rs`. Enter-driven, type-ahead with balance shown, inline arithmetic `1200*18%`, balancing line prefilled, Alt+C creates a ledger inline. S§7.
- **U5 — Item invoice entry.** Owns `app/src/screens/item_invoice.rs`. GST block shows engine output; amount in words from C1. S§7.
- **U6 — Masters screens.** Owns `app/src/screens/masters.rs`. S§7.
- **U7 — Report viewer and drill.** Owns `app/src/screens/report_view.rs`. Enter drills, Esc returns with selection preserved; Alt+F1 detail toggle; period F2. S§6.2, §7.
- **U8 — GST workspace.** Owns `app/src/screens/gst.rs`. Exception buckets, 2B/IMS table with Accept/Reject/Pending (J/K/A/R/P as in the mock). S§6.3.
- **U9 — Bank reconciliation screen.** Owns `app/src/screens/bank.rs`. S§6.5.
- **U10 — Tally import wizard.** Owns `app/src/screens/import.rs`. Steps: detect → choose company → pull (progress, cancel) → map → reconciliation report. S§6.7.
- **U11 — AI panel.** Owns `app/src/screens/assist.rs`. Labels "AI proposed" vs "engine computed"; Accept disabled while `DraftCheck` blocks. S§6.8.
- **U12 — Audit views.** Owns `app/src/screens/audit.rs`. Depends on RP10. Shows RP10's outputs only. S§6.6.
- **RP10 — Auditor reports (headless).** Owns `reports/src/audit.rs`. Depends on S3. Produces per-object history with field diff, edit log by type/user/period, deleted/altered vouchers, back-dated entries, edits after lock date, integrity certificate (from `verify_chain`). Tests: an alter shows before/after of the changed fields only; a back-dated entry (date < previous max date by > 0 days, entered later) is listed. S§6.6.

**Wave 1 builders: 93.** With the 10 reviewers: 103 agents in Wave 1, run at most 24 builders at a time (Prerequisite 6).

## Wave 1 reviewers (10 Sonnet agents, pipelined)

Each reviewer owns a lane group and reviews each card's diff **as soon as that card finishes**
(not at the end): reads the diff itself, then the card, then the rules. Checks in order: test diffs
and provenance first (rule E3, A4), wildcard arms, floats, panics, file ownership, then logic.
Verdict: approve, or a findings list sent back to that builder (max 2 fix rounds, then escalate to
the controlling session).

| Reviewer | Cards |
|---|---|
| RV1 | C1–C3, M1–M4 |
| RV2 | S1–S6 |
| RV3 | E1–E7 |
| RV4 | RP1–RP10 |
| RV5 | G1–G6 |
| RV6 | G7–G11, F1–F5 |
| RV7 | T1–T5, K1–K5, BF1–BF4 |
| RV8 | X1–X4, B1–B4 |
| RV9 | A1–A4, P1, PR1, PR2, Q1, H1–H3 |
| RV10 | U1–U12 |

## Wave 2: wiring (8 Sonnet builders, after Wave 1 merges)

- **W1 — `ledgerly-api` façade.** `Api::execute(Command) -> Result<Outcome<CommandResult>>` = engine pure fn → `Store::commit`; `Api::report(ReportParams)`; `Api::search(query)` (ledgers by name/alias/GSTIN, vouchers by number/party/amount/date, reports, settings). Tests: post → report reflects it; failed commit leaves report unchanged.
- **W2 — CLI.** `ledgerly import-tally`, `verify`, `export gstr1`, `rebuild-check`, `conformance`. Tests: `verify` exit code 1 on a tampered file.
- **W3 — Bridge to store.** Tally pull → staging → `map_to_events` → commit as `Actor::Import`; reconciliation report saved. Test with recorded fixtures.
- **W4 — GST end-to-end.** Books fixture → compute tax on post → GSTR-1 JSON + 3B working via the API.
- **W5 — Banking end-to-end.** Import statement → match → confirm → BRS.
- **W6 — App wiring (entry screens).** U4/U5/U6 to `Api` on a worker thread; UI thread never blocks.
- **W7 — App wiring (reports, GST, bank, import, audit, AI).** U3/U7–U12 to `Api`.
- **W8 — Headless end-to-end test.** Create company → masters → 20 fixture vouchers → TB/BS/P&L → GSTR-1 → backup → verify chain → reopen → identical reports.

## Wave 3: integration (2 Sonnet integrators + controlling session)

1. Merge Wave 1 branches in order: core/model → store → engine, stock, gst, bank, formats, tally-xml → reports, bridge, assist, print, mcp → conformance, fuzz, benches → app. Full workspace gates after each crate group; a breaking branch goes back to its builder.
2. Merge Wave 2 the same way; run W8.
3. Whole-branch review in the controlling session.
4. Output `docs/status/2026-11-20-wave-report.md`: each card done / partial / blocked; the `pending` provenance list (what the owner must observe in TallyPrime or the GST portal); open interface requests; review findings ranked.

**Agent count:** 93 builders + 10 reviewers + 8 wiring builders + 2 integrators = **113 agents**.

## How it runs

One Workflow script (written when the owner says go, after loading the workflow-authoring
skill): `pipeline(cards, card => builder(card) [Sonnet, worktree isolation] → reviewer(lane) →
fix loop ≤ 2)`, then Wave 2 in parallel, then integrators. Card text is passed verbatim from this
file; agents report in a fixed JSON schema (`{card, status, tests_added, pending: [], unsupported:
[], interface_requests: []}`).

## Not in this run (spec items with no card)

Users and roles; encryption at rest and recovery key; Excel/CSV master and voucher import with
saved mappings; Hindi UI and translation files; cost-centre allocation validation and reports;
voucher-type create/alter commands; live Tally sync; installers and reproducible builds. Each
becomes a card in the next run.

## Risks

| Risk | Mitigation |
|---|---|
| Builders invent shared types | Wave 0 freezes core/model and seams; builders cannot edit them |
| Merge conflicts | One owned file set per card; Cargo manifests written once in Wave 0 |
| Tests weakened to pass | Rule E3; reviewers read test diffs first |
| Plausible but wrong GST/Tally behaviour with confident tests | Provenance rule; `[K]` facts stay `pending`; owner verifies pack references |
| Pure-function `Books` | **Deliberate deviation from S§6.1 and S§8 for this run**: domain logic reads a fully loaded `Books`, which will miss the 30 ms save / 300 ms report targets at 1M vouchers. It buys parallelism now; H3 measures the gap and incremental projections behind `Store` replace the full load before v1 |
| Cost overrun | 30-agent pilot first |

## Recommended first run (30-agent pilot)

Wave 0, then 27 builders in DAG order: C1, M1, M4, S1, S2, S4, E4, E5, E7, E3, RP1, RP2, RP4,
G1, G2, G3, G4, T3, T4, K1, BF2, X2, X3, A1, Q1, U1, U4 — plus 3 reviewers (RV-ledger:
core/model/store/engine; RV-tax: reports/gst/stock/bank; RV-interop: tally/assist/app). Every
card's dependencies are inside the set. That proves a voucher can be posted, taxed, reported,
parsed from Tally XML, drafted by the AI and shown, and measures real cost per card. Then scale
to the full plan.

## Wave 0 status and deviations (2026-11-20)

Wave 0 is done on branch `build/wave-0`, tag `wave-0`. Gates: fmt, clippy `-D warnings`, nextest
(16 tests, all in `ledgerly-core`), rustdoc `-D warnings`, `cargo deny check`, headless check.

Deviations from the text above, for builders and reviewers:

- **Contract modules, not stubs.** Each card's owned file exists and holds a module doc naming the
  card and the frozen interface in a `text` block. Builders replace the body; they never edit
  another card's file. The exceptions are `ledgerly-store` `commit.rs` and `load.rs`, which return
  `StoreError::NotBuilt("S2"/"S4")` because `impl Store for SqliteStore` must compile. S2 and S4
  remove those variants' uses; the `NotBuilt` variant goes in W-wave cleanup.
- **`arithmetic_side_effects` is not a lint.** It was too noisy against `Decimal` and `u64` ids.
  Instead: `overflow-checks = true` in every profile, and money arithmetic only through `Paise`
  checked methods (there is no `Add` impl).
- **`Proposal::Voucher` holds `Box<VoucherDraft>`**, and `Event` carries
  `#[expect(clippy::large_enum_variant)]`.
- **GSTIN check character and the state-code table are recalled, not sourced.** `ledgerly-core`
  marks both. Tests use self-generated vectors only. Before release someone quotes the GSTN source;
  until then G-cards treat a wrong check character as a parse error, as now.
- **Fixture GSTIN for Maharashtra is unfilled** (`27ABCDE1234F1Z?`); Q1 computes it.
- **Fuzz crate** `fuzz/` is outside the workspace; targets are no-ops until H1/H2.
- Interface change requests go to `docs/interface-requests.md`.
