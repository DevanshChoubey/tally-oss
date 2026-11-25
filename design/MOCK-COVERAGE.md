# Mock coverage: app screens and states against the mock

Round S12 (2026-11-25). The mock (`design/mock-v2/index.html`) is the source of truth. This file
lists every screen and state the app reaches, and the mock state that shows it, or MISSING.
Sources read: `crates/ledgerly-app/src/screens/*`, `shell/navigate.rs` (screen keys and report
kinds), `shell/welcome.rs`, `screens/import/*`, `screens/audit.rs`, `screens/print_job.rs`,
`screens/gst/draw.rs`, `screens/voucher_entry/*`, `screens/item_invoice/*`,
`examples/ui_snapshots.rs` (the `cold.*` and state captures), `design/app-captures/*`.

Status: **covered** = a mock state exists (new in S12 where marked); **MISSING** = the app has the
state and the mock does not; the reason is in the last column.

## Covered before S12

| App screen or state | Mock state | Status |
|---|---|---|
| Home (default, empty, loading, AI off, nudge open) | home.default, home.empty, home.loading, home.ai-off, home.nudge-open | covered |
| Command palette (default, search, voucher, ask, entry, commands, no results) | palette.* (7) | covered |
| Ask your books (result, loading, ambiguous, unsupported, AI off) | ask.* (5) | covered |
| Item invoice, sales (default, B2C, inter-state, item list, validation, locked, save confirm, saved) | sales-invoice.* (8) | covered |
| Bill photo purchase (reading, proposal, blank field, total mismatch, duplicate, cloud consent, unreadable, accepted, AI off) | purchase-bill.* (9) | covered |
| Plain-words entry (proposal, Hinglish, unknown name, drafting, rejected, AI off) | quick-entry.* (6) | covered |
| Payment (default, bill-wise, overdraft) and receipt | payment.* (3), receipt.default | covered |
| Balance sheet (condensed, detailed, group, ledger, voucher, computing, provisional) | balance-sheet.* (7) | covered. The ledger vouchers report is balance-sheet.ledger |
| Day book (day, November, empty) | day-book.* (3) | covered. The app has no separate filters; Alt+F2 period is day-book.month |
| GST returns: GSTR-1, correction, GSTR-3B, 2B/IMS recon, recon empty, import error, period locked, recon explain | gst.* (8) | covered for those states |
| Bank reconciliation (matching, AI suggestions, new ledger, empty, import error, AI off, statement) | bank.* (7) | covered |
| Masters (ledgers, party detail, items, create ledger, GSTIN error, suggested ledger) | masters.* (7) | covered |
| Review queue (default, empty) | review.* (2) | covered |
| Keyboard map (classic, standard) | keys.* (2) | covered |
| Company setup (welcome, company, GSTIN error, features, keyboard, AI, backup, done). Also the first-run start with no recent companies | setup.* (8) | covered. setup.welcome is the no-recent start |
| AI and privacy (local, cloud, off) | ai-settings.* (3) | covered |
| Balance sheet loading, provisional | balance-sheet.loading, balance-sheet.provisional | covered |

## Added in S12 (25 states)

| App screen or state | Mock state (new) | Notes |
|---|---|---|
| Purchase, typed by hand (F9 without the bill reader), default | purchase.default | Supplier Nilgiri Steel, IGST, total 51,141.00 (hand check in the code) |
| Purchase, validation errors | purchase.validation-error | Missing quantity and unknown item, from the app's wording |
| Credit note, default | credit-note.default | CN against SI/0412, CGST and SGST, total 5,959.00 |
| Credit note, validation error | credit-note.validation-error | 70 returned against 67 invoiced |
| Debit note, default | debit-note.default | Purchase return against NST/1145, total 2,360.00 |
| Debit note, validation error | debit-note.validation-error | Return with no matching bill line |
| Journal, default | journal.default | JV/0033, 1,840.00 both sides |
| Journal, does not balance | journal.validation-error | Difference 360.00 shown, Save blocked |
| Contra, default | contra.default | CT/0022, 30,000.00 cash to bank |
| Contra, validation errors | contra.validation-error | No account picked, zero amount (app wording) |
| Bank account chooser | bank-chooser.default | Two accounts, balances from the engine |
| Start screen with recent companies (cold.start-open-selected) | start.recent | Not built (no recent-companies store); PARITY-GAPS, Reports |
| Profit and loss, default | profit-and-loss.default | Shows the engine's refusal for the sample (opening balance on an expense ledger); PARITY-GAPS |
| Profit and loss, provisional stock | profit-and-loss.provisional | Steel pickaxe -30 nos |
| Print preview (Ctrl+P) | profit-and-loss.print-preview | Not captured: no in-app preview, and the sample's profit and loss is refused |
| PDF saved | profit-and-loss.print-saved | Not captured, as above |
| PDF not saved (file open elsewhere) | profit-and-loss.print-failed | Not captured, as above |
| Trial balance | trial-balance.default | |
| Outstanding receivables | outstanding.receivables | Due from credit days; overdue from the engine |
| Outstanding payables | outstanding.payables | |
| Stock summary | stock-summary.default | Total equals closing stock on the balance sheet |
| Stock summary, provisional item | stock-summary.provisional | |
| Audit, edit log | audit.default | |
| Audit, integrity certificate | audit.integrity | |
| Audit, edits after lock (Unsupported) | audit.unsupported | Not captured: the engine returns a table for this report, the mock says Unsupported; PARITY-GAPS |

## Still MISSING

| App screen or state | Why not in S12 |
|---|---|
| Cold start, new tab (Ctrl+T, cold.new-tab) | Not read in the app code this round; behaviour unknown, so no design |
| Cold start, sidebar collapsed (cold.rail) | The mock has the Ctrl+\ command but no capture state; add with the shell round |
| Import from another app, five steps: detect, choose company, pull with progress and cancel, map, reconcile report; and the error | Large flow with no mock design; needs its own round. The entry card exists on setup.welcome |
| Profit and loss, trial balance, outstanding and stock summary loading states | The app's report loading card exists (report_view draw_loading); the mock has loading only for balance sheet. Add with the reports round |
| Print job running (busy) and the print job's other error texts | Not designed |
| Audit, loading and failed states | Not designed (the app has set_failed) |
| GSTR-3B cash payable unsupported ("Needs GSTR-2B: not readable yet") and GSTR-1 unsupported feed | Needs an edit to the GST screen; deferred |
| GSTR-1 and 3B not loaded ("Not loaded for this period yet") | Not designed |
| Masters empty and loading variants | Not checked against the app this round |

## Open items for the designs above

- Rule 53(1A) of the CGST Rules: the research of 2026-11-25 read the particulars list from a
  commentary; the official CBIC text was not reachable. The credit and debit note footers say
  "rule 53" and need checking against the CBIC text before the wording is final.
- Debit notes: s.34(3) puts the debit note with the supplier. The purchase-return booking in
  debit-note.default follows the mock's own rule (Dr supplier, Cr purchases and input tax). Its
  GST treatment for the buyer needs a GST adviser's check before the app is built to match.
- Tally voucher fields (credit note reference, journal and contra rules) were found only as search
  snippets, not full pages. The layouts here follow the app's own forms and the GST rule, not a
  field-by-field copy of any other product.
- Audit: the integrity certificate's zero broken links is the mock's sample state, not a computed
  result.

## Sources (read 2026-11-25)

- CGST Act 2017 s.34 (credit and debit notes) and CGST Rules 2017 r.53, via
  https://taxguru.in/goods-and-service-tax/credit-note-debit-note-section-34-rule-53-cgst-act-2017.html
  (commentary, not the statute; the statute and CBIC text were not reachable)
- Profit and loss, trial balance and reports overview: https://help.tallysolutions.com/accounting-financial-reports-tally/ (search snippet and page)
- Outstanding receivables and ageing: TallyPrime help search snippets only (no page text read)
- Print preview and the edit log: TallyPrime help search snippets only (https://help.tallysolutions.com/?p=108972 for the edit log)
- Import: the general steps only (XML or Excel, company selection, error log); no mapping or reconciliation steps were found
