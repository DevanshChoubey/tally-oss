# Ledgerly — design spec (v1)

Working name: **Ledgerly** (rename before any public release; never a name containing "Tally").
Date: 2026-11-20. Status: draft for review.

Inputs: research files `docs/research/01..10-*.md` (cited below as R1..R10) and the mock at
`design/mock/index.html`.

## 1. What we are building

A clean-room, open-source, local-first desktop accounting app for Indian businesses and their CAs,
able to replace TallyPrime for day-to-day books and GST. Keyboard-first (accountants keep their
muscle memory), modern UI, native on Windows, macOS and Linux, with AI that drafts and never
computes.

**The one promise: never a wrong number.** Every figure the app shows comes from the deterministic
engine. Where the engine cannot compute something correctly it says `Unsupported { reason }`; it
never guesses, defaults or silently "fixes".

### Why people will switch (R9)

- No TSS-style annual fee; e-invoice, e-way bill, multi-user and remote access are not paywalled.
- High-fidelity Tally import of full history with a **reconciliation report** proving our totals
  match Tally's, period by period. This is the flagship feature: migration distrust is the #1
  blocker and competitors' imports are shallow.
- Native macOS and Linux; transactional store (no data-corruption repairs); always-on tamper-evident
  audit trail; Indian-language UI.
- Keyboard flows compatible with what CAs already know.

### Non-goals for v1

Payroll, TDS/TCS returns, full manufacturing (BOM/job work), POS, multi-currency revaluation,
direct GSP filing, cloud sync, mobile app, Schedule III statements, TDL compatibility. See §12.

## 2. Users

1. **Accountant / data-entry operator** — enters hundreds of vouchers a day, never touches the
   mouse. Needs speed, Enter-driven forms, type-ahead, the familiar keys.
2. **CA / tax consultant** — manages many client companies, files GST, reconciles 2B/IMS, audits.
   Needs multi-company switching, exception buckets, auditor views, Tally import/export.
3. **Business owner** — wants dashboards, outstandings, GST liability, and to not lose data.

## 3. Decisions

| Topic | Decision | Source |
|---|---|---|
| Language | Rust, stable toolchain, edition 2024 | — |
| UI | egui + eframe with the wgpu renderer; our own design language (no Tally trade dress) | user, R2, R10 |
| Store | SQLite (one file per company), WAL mode, append-only event log as source of truth | R1, R7 |
| Money | `i64` paise (`Paise`) for INR amounts; `rust_decimal::Decimal` for quantities, rates and unit costs; never `f32/f64` | R1, R3, R6 |
| Licence | App and binaries: AGPL-3.0-or-later. `ledgerly-core` and the format/interop crates (`ledgerly-tally-xml`, `ledgerly-gst-formats`, `ledgerly-bankfmt`): MIT OR Apache-2.0 | user, R10 |
| Contributions | DCO sign-off (decide CLA vs DCO before first external PR; default DCO) | R10 |
| AI | Proposals only (typed structs), engine recomputes, human confirms; local by default, cloud BYOK opt-in | R10 |
| Platforms | Windows 10+, macOS 13+, Linux (X11 + Wayland); CI on all three | R9 |
| Interop | Tally via its documented HTTP/XML interface (port 9000) and user-exported XML files only; never parse `.900/.1800` company files | R8, R10 |

## 4. Architecture

```
┌──────────────────────────── ledgerly-app (egui shell) ─────────────────────────────┐
│ screens: home · voucher entry · reports · masters · GST workspace · bank recon ·   │
│ import wizard · audit views · AI panel · command palette · keymap                  │
└───────────────┬──────────────────────────────────────────────┬─────────────────────┘
                │ commands / queries (ledgerly-api)             │ proposals
┌───────────────▼────────────┐  ┌────────────────────┐  ┌──────▼─────────────┐
│ ledgerly-engine            │  │ ledgerly-reports   │  │ ledgerly-assist    │
│ posting, validation,       │  │ 3 report engines + │  │ AI proposer (no DB │
│ balances, bills, numbering │  │ report params      │  │ writes, no numbers)│
└──────┬──────────┬──────────┘  └─────────┬──────────┘  └────────────────────┘
       │          │                        │
┌──────▼───┐ ┌────▼──────────┐ ┌───────────▼──────┐ ┌──────────────┐ ┌──────────────┐
│ ledgerly │ │ ledgerly-gst  │ │ ledgerly-stock   │ │ ledgerly-bank│ │ ledgerly-    │
│ -store   │ │ rules engine  │ │ movements +      │ │ statements,  │ │ tally-bridge │
│ events,  │ │ (dated packs) │ │ valuation        │ │ matching     │ │ import/sync  │
│ projections, audit chain  │ └──────────────────┘ └──────────────┘ └──────────────┘
└──────┬───┘
┌──────▼──────────────────────────────────────────────────────────────────────────┐
│ ledgerly-core: ids, Paise, dates/FY, Gstin, StateCode, errors, Unsupported       │
└──────────────────────────────────────────────────────────────────────────────────┘
format crates (MIT/Apache): ledgerly-tally-xml · ledgerly-gst-formats · ledgerly-bankfmt
```

Rules: headless crates never depend on UI crates; the app is the only egui dependent.
One query path for figures: every number on screen comes from `ledgerly-reports` or
`ledgerly-engine` queries.

### Crates

| Crate | Responsibility |
|---|---|
| `ledgerly-core` | Newtype IDs (`LedgerId`, `GroupId`, `VoucherId`, …), `Paise`, `Qty`, `Rate`, `Date`, `FinancialYear`, `Period`, `Gstin` (with checksum), `StateCode`, `Hsn`, `Unsupported`, shared error kinds. No I/O. |
| `ledgerly-model` | Master and voucher types (§5) as plain data + invariant checks that need no store. |
| `ledgerly-store` | SQLite schema, migrations, event log with hash chain and daily checkpoints, projections (balances, bills, stock positions), atomic save, backup. |
| `ledgerly-engine` | Commands: create/alter/cancel/delete masters and vouchers; numbering allocator; period locks; balance/bill/cost-centre queries; scenarios (optional/post-dated). |
| `ledgerly-gst` | Effective-dated GST rule packs (rates by HSN, place of supply, RCM, ITC eligibility, rounding), per-voucher tax computation, return builders (GSTR-1, 1A, 3B), 2B/IMS reconciliation. |
| `ledgerly-stock` | Units, items, godowns, batches, stock movements, valuation methods (§6.4). v1 = basic inventory. |
| `ledgerly-reports` | Report parameter model + three engines: roll-up (BS, P&L, TB, group/ledger summaries), voucher list (day book, registers, ledger vouchers), outstandings (bill-wise, ageing). Drill-down paths. Export (CSV, XLSX, PDF, JSON). |
| `ledgerly-bank` | Bank accounts, statement import (immutable facts), balance check, match model, rules, BRS. UPI QR payload. |
| `ledgerly-tally-xml` (MIT/Apache) | Tally XML envelope types, parser (lenient on invalid chars, strict on structure), writer, request builders. No network. |
| `ledgerly-tally-bridge` | HTTP client to Tally :9000, company discovery, batched master/voucher pull, staging by Tally GUID/AlterID, mapping to our model, reconciliation report, export of Tally-importable XML. Headless + CLI. |
| `ledgerly-gst-formats` (MIT/Apache) | GSTR-1/1A offline-tool Excel/JSON writers, GSTR-3B JSON, GSTR-2B/IMS JSON readers, e-invoice INV-01 JSON + e-way bill JSON writers. Built only from official templates and public specs. |
| `ledgerly-bankfmt` (MIT/Apache) | Data-driven bank statement format registry (TOML), CSV/XLSX/MT940 parsers, NEFT/RTGS bulk file writers. |
| `ledgerly-assist` | AI proposer: OCR + LLM behind one `Proposer` trait; outputs `VoucherDraft`, `MatchSuggestion`, `ReportQuery`, `MappingSuggestion`. The validator (in assist) recomputes through the engine and GST crates. |
| `ledgerly-print` | Declarative invoice/voucher print templates (TOML + layout), PDF rendering. |
| `ledgerly-api` | The command/query façade the UI and the MCP server both use. |
| `ledgerly-mcp` | MCP server exposing read queries and `propose_*` tools (no posting). |
| `ledgerly-app` | egui shell and screens. |
| `ledgerly-cli` | Headless: import, verify audit chain, export returns, run conformance. |
| `ledgerly-conformance` | Golden cases with provenance; ratchet pass-list. |

## 5. Data model (summary; full field lists in R1 §Data-model, R3 §Data-model, R6 §Data-model)

- **Company**: name, addresses, state, GSTIN registrations (one company may hold many GSTINs),
  `fy_start`, `books_begin`, base currency INR, `entity_kind` (Company | LLP | Partnership |
  Proprietorship | Other) — Company ⇒ audit trail can never be disabled (it is always on for all).
  Feature flags are a typed struct with dependency rules; changing a flag with data behind it is a
  migration.
- **Group** tree with 28 reserved groups keyed by `ReservedGroup` enum (never by name). Nature
  (Asset/Liability/Income/Expense) only on primaries, inherited. `affects_gross_profit`.
- **Ledger**: group, opening (Paise, Dr/Cr), bill-wise flag + default credit days, cost-centre
  flag, party details (GSTIN, registration type, state, address), bank details, tax details
  (GST type/rate for duties ledgers). Reserved ledgers: Cash, Profit & Loss.
- **VoucherType**: user-defined types inheriting a closed `BaseVoucherKind` enum (Contra, Payment,
  Receipt, Journal, Sales, Purchase, CreditNote, DebitNote, Memorandum, ReversingJournal,
  ReceiptNote, DeliveryNote, RejectionIn, RejectionOut, StockJournal, PhysicalStock, SalesOrder,
  PurchaseOrder; others are v2). Numbering config with series. **Sales-side and GST-document series
  never renumber.**
- **Voucher**: type, number, date, status (Regular | Optional | PostDated | Cancelled), mode
  (AsVoucher | AccountingInvoice | ItemInvoice), party, place of supply, narration, ledger lines,
  inventory lines, GST block (computed, stored with the rule-pack version used).
- **LedgerLine**: ledger, Dr/Cr, amount, bill allocations (New | Against | Advance | OnAccount),
  cost allocations, bank allocations.
- **Bills** and **stock positions** and **balances** are projections, never edited directly.
- Every imported object keeps `tally_guid`, `tally_alterid` and unknown UDF fields as an opaque blob.

### Invariants (each a typed error, never a silent fix)

1. Posting voucher: Σ Dr = Σ Cr exactly in paise.
2. Bill allocations on a bill-wise line sum to the line amount; Against refs must exist, be open,
   be on the same ledger, and not over-settle.
3. Optional, memorandum, reversing and future post-dated vouchers never affect base balances; only
   a named scenario includes them, and reports declare their scenario.
4. Opening trial balance may be unbalanced; the difference is always shown as an explicit line.
5. Reserved groups/ledgers cannot be deleted; renaming is harmless.
6. `fy_start <= books_begin < fy_start + 1 year`; voucher date ≥ books_begin.
7. Number uniqueness per (type, series, numbering period); atomic allocator.
8. Period lock: no create/alter/delete dated on or before the lock date (alter of lock date is itself
   an audited event).
9. GST tax on a voucher is recomputed by the engine from masters + rule pack; user overrides are
   allowed only on flagged fields and are logged.

## 6. Engines

### 6.1 Posting & balances
Vouchers are events; balances are projections over the event log, recomputable from scratch (a CLI
command rebuilds and compares). Multi-year continuous data with no closing entries: prior-year P&L
folds into the Profit & Loss ledger at query time. Period locks replace "split company".

### 6.2 Reports (R7)
Three engines and one parameter model: `ReportParams { period, as_of, scenario, basis, detail
(Condensed|Detailed), columns (comparison periods, monthly), filters, exceptions_only, view }`.
- Roll-up: TB, BS, P&L (trading form: `GP = Sales + Closing stock − Opening stock − Purchases −
  Direct expenses`), group/ledger summaries, cash/bank books, cost-centre summaries.
- Voucher list: day book, ledger vouchers, sales/purchase/journal registers, exception lists
  (optional, post-dated, cancelled, memorandum, negative balances).
- Outstandings: receivables/payables bill-wise, ageing buckets, party ledgers.
- Every figure drills down: group → ledger → voucher. Esc returns with selection preserved.
- Ratio analysis and cash/funds flow are v1.1.

### 6.3 GST (R4)
- **Rule packs** are versioned data (TOML) with `effective_from/effective_to`, selected by **time of
  supply**, not entry date. Packs ship with the app and can be updated without a code release.
  Initial packs: GST 2.0 slabs (0/5/18/40, from 22 Sep 2025), cess changes (1 Feb 2026), s.13(8)(b)
  removal (30 Mar 2026), credit-note/discount changes (1 Nov 2026). Every pack entry carries its
  legal reference (Act section / notification number) or is marked `pending` and treated as
  unsupported.
- **Place of supply** algorithm (R4 §2.4) returns `Intra(Sgst|Utgst) | Inter | ZeroRatedExport{…} |
  ZeroRatedSez{…} | DeemedExport | Unsupported{reason}`. Overrides are logged.
- RCM, exempt/nil/non-GST, SEZ/exports, advances, ITC eligibility incl. s.17(5) blocked credits.
  Rule 42/43 reversals and 180-day reversal are v1.1.
- Rounding: store unrounded line tax, the rounding policy, and an explicit round-off line; payable
  amounts round to the rupee (s.170).
- **Exception buckets** on every return: Ready · Not relevant · Mismatch · Needs correction; drill
  to fix; bulk "re-apply masters".
- Returns v1: GSTR-1 and GSTR-1A (offline-tool Excel + JSON), GSTR-3B working (books vs portal
  comparison, since Tables 3.1/3.2 are portal-locked), HSN summary, document summary.
- 2B/IMS: import downloaded GSTR-2B/IMS JSON, match against purchases (exact → fuzzy invoice number
  normalisation → amount/date tolerance), suggest Accept/Reject/Pending, **user confirms every
  action**, export the action file. Credit notes may stay pending one period.
- e-Invoice: build INV-01 JSON + validate locally; IRN/QR via IRP API is v1.1 (offline JSON export
  in v1). e-Way bill JSON export in v1.

### 6.4 Inventory (R3) — basic in v1
Units (simple + compound), stock groups/categories, items (HSN, GST history, base/alt units),
godowns, batches with mfg/expiry, item-invoice lines with godown/batch allocation, receipt/delivery
notes, stock journal, physical stock. Valuation methods v1: **Average Cost (periodic, documented)**,
FIFO (annual), Last Purchase Cost, Standard Cost (no silent fallback). LIFO is not offered
(not permitted under AS 2 / Ind AS 2 / ICDS-II). Closing stock is computed lazily per as-of date
from the movement ledger; valuation over a period with negative stock is returned as
`Provisional { reason: NegativeStock }`. Negative-stock policy per company/voucher type:
Block | Warn | Allow. Orders, BOM, price lists, FEFO: v2.

### 6.5 Banking (R6)
Statements are immutable imported facts (file hash, raw rows); import is rejected unless opening +
Σ lines = closing and running balances agree. Formats v1: CSV, XLSX, MT940 via the data-driven
registry (top 10 banks first). Many-to-many matches; kinds Exact | Rule | AiSuggested | Manual;
only user-confirmed matches set the bank date. BRS report. Cheque books/leaves, post-dated
cheques, bounced-cheque guided action. UPI QR (`upi://pay`) on printed invoices. NEFT/RTGS bulk
files, gateway links and Account Aggregator: v2.

### 6.6 Audit trail (R7, R10)
- Every create/alter/cancel/delete of masters, vouchers **and settings** is an append-only event:
  id, object id, version, actor (user + machine, or import job / assistant on behalf of user),
  UTC time + offset, monotonic sequence, before/after snapshot, reason, app version.
- Hash chain (`prev_hash`) plus daily checkpoint hashes; `ledgerly verify` prints a pass/fail
  certificate; an auditor can run a standalone verifier.
- Always on for every entity; no off switch; schema migrations preserve events and hashes;
  8-year retention; archive years are read-only and verifiable; reading old data never needs a
  licence or network.
- Daily automatic backup to a user-chosen location (warn companies that Rule 3(5) needs India).
- Auditor reports: per-object history with diff; edit log by type/user/period; deleted/altered
  vouchers; back-dated entries; edits after lock date; edits after e-invoice generation;
  privileged-user activity; integrity certificate; full trail export (CSV/JSON).
- Reproducible builds so a CA can confirm the binary matches the source.

### 6.7 Tally interop (R8) — flagship
1. Detect Tally on `localhost:9000`, list companies.
2. Pull masters in dependency order (groups → ledgers → units → stock groups/categories → items →
   godowns → cost categories/centres → voucher types), then vouchers in date batches (≤ 5,000 per
   request), capturing raw XML to disk first.
3. Stage keyed by Tally GUID; map to our model; unknown fields kept opaque.
4. **Reconciliation report**: TB, P&L, BS, stock summary, party outstandings and GST summaries
   for each period, ours vs Tally's own exported reports. Differences are listed, never hidden.
   Tally's exported numbers are valid `observed` expected values for conformance (with version+date).
5. Offline path: parse XML files the user exported from Tally (same parser).
6. Exit path: export our books as Tally-importable XML (CA hand-off).
7. Live two-way sync (poll AlterIDs, diff GUIDs for deletes, write via TALLYMESSAGE): v1.1.

### 6.8 AI (R10)
- `Proposer` trait; tiers per company: Off | Local (default when enabled: PaddleOCR/Tesseract +
  small instruct model via llama.cpp with grammar-constrained JSON) | Cloud BYOK (user's key, PII
  redaction).
- v1 features: bill/invoice image or PDF → `VoucherDraft`; natural-language entry ("paid rent 25k
  to Sharma Properties by HDFC") → `VoucherDraft`; bank line → `MatchSuggestion`; Tally/Excel column
  mapping → `MappingSuggestion`; question → `ReportQuery` (engine runs it; answer shows the query).
- Extracted amounts are transcriptions with source (doc hash, page, bbox) and confidence; below
  threshold → blank. Engine recomputes all derived numbers; mismatches block Accept.
- AI can only call read queries and `propose_*`; never post, file or send. Every AI action is an
  audit event (model, version, prompt template version, input hash, output, human edits).
- Evaluation corpus with hand-labelled expected values; publish precision/recall; no feature ships
  below its threshold.

## 7. UX (R2, mock)

- **Classic keys preset on by default** (F4–F9 voucher types, F2 date/period, F3 company, F11
  features, F12 configure, Ctrl+A save, Esc back, Alt+C create master inline, Ctrl+Enter alter
  master, Alt+D delete, Alt+X cancel, Alt+2 duplicate, Alt+F1 detail toggle, Ctrl+P/E/M output),
  plus modern aliases (Ctrl+S, Ctrl+K palette, Ctrl+Z undo). Fully rebindable; exported as TOML.
  Our own labels, layout and visuals.
- Screen stack with visible breadcrumbs and open-screen tabs; "open on top" (resumable) vs
  "replace"; split view (report beside voucher).
- Command palette searches reports, masters (name/alias/GSTIN), vouchers (number/party/amount/date),
  settings and AI intents.
- Entry forms: Enter advances, auto-skip, type-ahead lists (fuzzy + alias, recents first, show
  balance/GSTIN/stock), "create new" option, sub-forms pop automatically, Ctrl+I more details,
  inline arithmetic (`1200*18%`), balancing line prefilled, fresh voucher after save.
- Reports reshape in place; Enter drills, Esc returns with selection preserved; saved views; charts.
- Indian digit grouping (12,34,567.00); light/dark/high-contrast themes; scalable fonts; screen
  reader labels (AccessKit); visible focus; no colour-only signals.
- UI languages v1: English. Hindi moved to v1.1 (decided 2026-11-24 to release v1 without it; branch
  `s7/hindi` holds the translation layer and most screens); others via community translation
  files. Transliteration input v2.
- The mock (`design/mock/index.html`) is the visual reference; deviations need a reason.

## 8. Performance targets (benchmarked in CI)

- Cold start to home screen < 1.5 s on a 4-core, 8 GB Windows laptop with a 1M-voucher company.
- Voucher save (validate + post + audit event) p95 < 30 ms.
- Trial balance / BS / P&L for a full year at 1M vouchers < 300 ms; drill-down < 100 ms.
- Type-ahead over 50k ledgers < 16 ms per keystroke.
- Tally import ≥ 2,000 vouchers/s after fetch (fetch speed is bounded by Tally).
- Memory < 500 MB for a 1M-voucher company.

## 9. Security & privacy

- Optional per-company encryption at rest (SQLCipher-compatible) with a **recovery key** (a Tally
  pain point: lost vault password = lost data).
- Users and roles (Owner, Accountant, Data-entry, Auditor read-only); role changes are audited.
- Opening a company touches no network. Credentials (IRP/GSP, AI keys) in the OS keychain only.
- Logs never contain amounts, party names or account numbers by default. Bank account numbers
  masked in UI and logs.
- DPDP obligations apply once sync/cloud AI exist (v2); v1 is local-only by default.

## 10. Testing & correctness

- Engineering rules: `docs/engineering-rules.md` (adapted from Tessera's).
- Expected values need provenance: `observed` (TallyPrime version + date, or GST portal output with
  date) or `derived` (hand derivation from the Act/rule/notification or accounting standard,
  written in the test). Never copied from our engine. No provenance → `pending` → feature reported
  unsupported.
- Conformance suite (`ledgerly-conformance`) with a ratchet pass-list: GST cases per rule pack
  entry, valuation worked examples (R3), report calculations (R7), Tally import reconciliations.
- Property tests: voucher balance, bill allocation sums, projection rebuild == incremental,
  XML round-trip, hash-chain verification detects any single-byte change.
- Fuzzing: Tally XML parser, GST JSON readers, bank statement parsers.

## 11. Milestones

1. **M0 Foundation** — workspace, CI gates, core types, model, store with event log + hash chain,
   engine posting + invariants, report param model, API façade, app shell skeleton.
2. **M1 Books** — masters, all v1 voucher types, numbering, period locks, roll-up/voucher-list/
   outstanding reports with drill-down, audit views, backup, CLI verify.
3. **M2 GST** — rule packs, place of supply, voucher tax, GSTR-1/1A/3B builders + exports, 2B/IMS
   reconciliation, e-invoice/e-way bill JSON.
4. **M3 Interop** — Tally XML parser/writer, bridge pull + staging + mapping, reconciliation
   report, offline XML import, export to Tally XML, Excel import with saved mappings.
5. **M4 Banking + inventory basic** — statements, matching, BRS, cheques, UPI QR; stock masters,
   item invoices, valuation.
6. **M5 AI + polish** — assist proposer, validator integration, AI panel, MCP server, print
   templates, accessibility, packaging/installers, reproducible builds. (Hindi UI moved to v1.1.)

## 12. Later (v1.1 / v2)

Live Tally two-way sync; IRP/EWB API and GSP adapter (pass-through per-call fees, ₹0.25–3 per
document by public price lists); Rule 42/43 and 180-day ITC reversals; ratio analysis, cash/funds
flow; Schedule III statements; TDS/TCS under Income-tax Act 2025 with dual-regime rule packs (R5);
payroll with dated statutory parameters (R5); MSME 45-day payment tracking (s.37(2)(g)) — move to
v1.1 if cheap; orders, BOM, price lists, FEFO, NRV write-down, barcodes/labels, POS (R3); NEFT/RTGS
files, gateway links, Account Aggregator via regulated partner (R6); LAN multi-user and E2E sync;
WASM plugin hooks and custom fields (R8); more Indian languages and transliteration; mobile
read-only companion.

## 13. Open questions

1. Average cost: periodic (chosen) vs moving — confirm against observed Tally output before M4.
2. Cost-centre partial allocation allowed? (R1) — decide in M1; default: must sum to line.
3. GSTR-3B Table 4 lock status — check GSTN advisories before M2.
4. Bills on foreign-currency ledgers (R1) — v2 with multi-currency.
5. Auditor acceptance of an open-source audit trail (R10) — talk to 2–3 CAs during M1.
6. Final product name and trademark search before public release.
7. CLA vs DCO before the first external contribution.
