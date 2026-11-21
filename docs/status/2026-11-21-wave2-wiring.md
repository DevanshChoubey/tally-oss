# Wave 2 (wiring): status, 2026-11-21

Seven code-only builders (W2 to W8) worked in their own worktrees, starting from integration
`373601a`, which holds the W1 API facade and the app's API worker. The coordinator merged them
and ran every gate once per round.

**Gates at `0d92123` (after W7):**
- fmt, clippy (`-D warnings`), doc (`-D warnings`), doctest and deny are clean.
- nextest: 2369 of 2369 passed.

W7 is in only in part (see below).

## What is in

| Card | What it added | Main entry points |
|---|---|---|
| W2 | Maintenance on the API; the CLI; the MCP server | `Api::verify`, `backup_to`, `rebuild_check`, `check_voucher`, `verify_file`; `ledgerly import-tally / verify / export gstr1 / rebuild-check / conformance` (exit codes 0 pass, 1 check failed, 2 could not run); `ledgerly_mcp::server::serve` and `ToolBackend for Api` |
| W3 | Full Tally import into a new company file | `Api::import_tally_http`, `import_tally_with`, `ImportSummary`; `ledgerly_tally_bridge::import` |
| W4 | GST returns from the open file | `Api::gstr1_json`, `gstr1`, `gstr3b` (+ `_with_pack`) |
| W5 | Bank statements, matches, BRS | `Api::import_bank_statement`, `bank_statements`, `bank_suggestions`, `confirm_bank_match`, `brs`; `Command::ConfirmBankMatch` |
| W6 | App: welcome screen opens or creates a file; voucher entry, item invoice and masters submit through the worker | `shell::submit::{send, settle, CommandReply}`, `shell::company_from_form` |
| W8 | Headless end-to-end test: masters, 20 vouchers, TB/BS/P&L, GSTR-1, backup, chain check, reopen, identical reports | `crates/ledgerly-api/tests/w8_headless_e2e.rs` |

## Decided by Tally's documented behaviour this wave

Owner instruction (2026-11-21): take how Tally does it, and every test follows Tally.

- **Trial balance, mixed groups:** Tally's Trial Balance screenshot shows a group row with both a
  Debit and a Credit amount, and the Grand Total adds both columns. Source:
  <https://help.tallysolutions.com/article/Tally.ERP9/Reports/Display_Financial_Statements/Display_Trial_Balance.htm>,
  image `3._Trial_Balance-1.gif`. The detailed image shows the split is by ledger. The engine
  used to net each group, which understated both totals. It now carries debit-balance and
  credit-balance ledger totals per group. One-sided groups are unchanged. New test: rp2
  `a_group_with_debit_and_credit_ledgers_shows_both_sides_and_totals_are_gross`.
- **Balance sheet, one amount per group:** Tally's Balance Sheet screenshot
  (<https://help.tallysolutions.com/balance-sheet-tally/>, `View-Balance-sheet.gif`) shows each
  group as one figure. So a debit ledger inside Current Liabilities nets there. W8's expectation
  was re-derived to 507,250 on that basis, and the reason is written in the test.
- Not stated by either page, and left as it was: where the balance sheet shows a loss (the
  engine puts it on the asset side), and the ledger-level split, which is inferred from the
  detailed screenshot.

## Fixed during integration

- **Lint and format:** a wildcard arm over `Event` in the Tally import; collect-by-format;
  identical match arms; single-arm matches; doc lists; format pushes; a large enum variant
  (`CommandReply::Booked` boxed); a doc link to a private item.
- **Merge conflicts:** these were all additions (`ApiError` variants, `lib.rs` re-exports,
  interface-request sections), and both sides were kept.
- **First test run, 14 failures, each fixed by its owner:**
  - W6 had broken four existing u5/u6 tests. With no worker present, screens again hand their
    requests to the host. Fixed in code; no test changed.
  - W4's fixtures posted GST vouchers as plain Dr/Cr. They are now accounting invoices; no
    figure changed.
  - W2's tamper fixture changed a stale page copy. It now rewrites every copy.
  - W3's clash fixture had removed the voucher type its voucher needed.
  - W5's rule-suggestion test now compares the whole listing.
  - W6's state test used 99, which is a valid code (Centre Jurisdiction). It now uses 39.
  - W8's trial balance total was the Tally question above.

## Pending (interface requests filed in `docs/interface-requests.md`)

- **App cannot read the books:** there is no public read of the books on `Api`. Until there is,
  the app's voucher, invoice and masters screens cannot draw data (W6).
- **Id assignment for new masters and inline ledgers:** masters Create and Alt+C refuse at the
  submit step (W6).
- **GST on a single invoice:** there is no per-invoice GST computation the API can run before
  posting, so item-invoice Accept refuses (W6). `Command` does not set `Voucher::gst` and does not
  check hand-typed tax lines (spec invariant 9 gap, W4).
- **Bank statements:** they are held in memory per session, CSV statements are refused until the
  parser reads balances, the statement end date is not stored, and only exact matches can be
  confirmed (W5). The store has an `Event::StatementImported` variant that W5 did not use. Worth
  checking.
- **Rule pack gaps:** the bundled pack lacks the `itc.blocked.*` (s.17(5)),
  `gstr1.b2cl_threshold` and `gstr1.hsn_credit_notes` entries. So GSTR-3B refuses any period with
  a purchase, and B2C inter-state and credit notes need correction (W4, W8 use test-only
  fixtures).
- **Rebuild check:** `rebuild_check` cannot report a match until something maintains the
  projection tables (W2).
- **Tally import scope:** it refuses inventory lines and non-zero opening balances, and skips
  vouchers outside books-begin to the current year without a warning (W3).
- **GSTR-1 JSON:** key names and number format await the official GSTN sample (W4).

## W7 (in part)

The owner kept the original app tests (2026-11-21). W7's first attempt replaced the report
view's callbacks, which broke `u7_report_view.rs`, so it was reverted. The rebuild (W7b, `8d732b0`)
keeps the callback mode, which the u7 tests use unchanged, and adds a worker mode.

**Done: the report viewer in worker mode.**
- `ReportSource::worker(base)` and `ReportViewScreen::worker`, `tick` and `is_loading` let the viewer
  run reports on the API worker.
- It shows a loading line until the answer comes, and an error (never a figure) if the worker stops.
- A newer period or detail change drops the older answer.
- Group and ledger drill open a new worker-mode screen.
- New tests in `w7_report_worker.rs`:
  - a trial balance with a mixed group (Dr 1,05,000 and Cr 5,000 on one row, gross totals);
  - a dropped stale answer;
  - no company open.
- Shared helpers live in `screens::wire`.

**Not done (still to wire):**
- **Bank:** BRS, suggestions and confirm. The Tally bank reconciliation pages W7b fetched gave
  only navigation, so that behaviour has no source yet.
- **GST screen:** GSTR-1 and GSTR-3B, with Unsupported reasons.
- **Import:** a worker path for `import_tally_http`.
- **Home:** KPIs through `Api::report`.
- **Palette:** `Api::search` through the worker.

**Also open:**
- **Opening the screen:** nothing in the shell opens a worker-mode report yet, so the running app
  does not show one.
- **Voucher drill:** it only shows a notice.
- **Bill and stock-item drill:** in worker mode these say Unsupported, since there is no host to
  hand them to.
