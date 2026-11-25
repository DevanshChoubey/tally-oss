# Mock coverage: app screens and states against the mock

Rounds S12 and S12b (2026-11-25). The mock (`design/mock-v2/index.html`) is the source of truth. This file
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
| Purchase, typed by hand (F9 without the bill reader), default | purchase.default | Supplier Nilgiri Steel (29), place of supply 27 Maharashtra, IGST, PB/0145, total 51,141.00 (hand check in the code) |
| Purchase, validation errors | purchase.validation-error | Missing quantity and unknown item, from the app's wording |
| Credit note, default | credit-note.default | CN/0001 against SI/0412, CGST and SGST, total 5,959.00 |
| Credit note, validation error | credit-note.validation-error | Line with an item and no quantity (the app's refusal); tax Not computed |
| Debit note, default | debit-note.default | DN/0001, 2 HDPE coils back to Rupa Polymers against RP/5398 (PB/0125), IGST 738.00, total 4,838.00 |
| Debit note, validation error | debit-note.validation-error | Line with an item and no quantity (the app's refusal) |
| Journal, default | journal.default | JV/0033, 1,840.00 both sides |
| Journal, does not balance | journal.validation-error | Difference 360.00 shown, Save blocked |
| Contra, default | contra.default | CT/0022, 30,000.00 cash to bank |
| Contra, validation errors | contra.validation-error | No account picked, zero amount (app wording) |
| Bank account chooser | bank-chooser.default | Two accounts, balances from the engine |
| Start screen with recent companies (cold.start-open-selected) | start.recent | |
| Profit and loss, default and detailed | profit-and-loss.default, profit-and-loss.detailed | Trading account then profit and loss, groups (ledgers in detailed, Alt+F1); nett profit 16,96,569.70 equals the balance sheet's profit to date |
| Profit and loss, provisional stock | profit-and-loss.provisional | Pickaxe below zero on 19 Nov, back to 70; closing stock marked Provisional |
| Print preview (Ctrl+P) | profit-and-loss.print-preview | Save as PDF only; paper printing is not built |
| PDF saved | profit-and-loss.print-saved | |
| PDF not saved (file open elsewhere) | profit-and-loss.print-failed | |
| Trial balance | trial-balance.default | The sample's ledgers (CGST, SGST, IGST as one ledger each), engine group names, total 1,25,33,742.30 |
| Outstanding receivables | outstanding.receivables | Due from credit days; overdue from the engine |
| Outstanding payables | outstanding.payables | |
| Stock summary | stock-summary.default | Total equals closing stock on the balance sheet |
| Stock summary, provisional item | stock-summary.provisional | Quantity 70, value 32,900.00 kept and marked Provisional |
| Audit, edit log | audit.default | The engine's shape: Seq, Date, Time, User, Action, Object, Reason, Field, Before, After; 14 posted vouchers, 1 to 20 Nov |
| Audit, integrity certificate | audit.integrity | |
| Audit, edits after lock | audit.unsupported | State id kept; the engine returns a table, empty here (the lock was recorded after every voucher) |

## Added in S12b (30 states)

Round 2 of S12 (same day). Each row names the app code it follows. Sample figures and sample
engine reasons are marked as such; the wording follows the app.

| App screen or state | Mock state (new in S12b) | Source in the app |
|---|---|---|
| Home, GST cash payable tile not readable | home.gst-unsupported, home.new-tab | `screens/home/kpi.rs` gst_tile: "Needs GSTR-2B", "not readable yet" |
| Home, new tab with a company open (Ctrl+T) | home.new-tab | `shell/mod.rs` NewTab: a Gateway (Home) tab; the strip shows "Home" and "Home 2" |
| Profit and loss, computing | profit-and-loss.loading | `report_view/mod.rs` draw_loading: "Computing from all vouchers…", Esc cancels |
| Profit and loss, preparing the PDF (Ctrl+P) | profit-and-loss.print-busy | `print_job.rs` is_busy: the app shows no text, so the line is a proposal (PARITY-GAPS) |
| Profit and loss, not printed | profit-and-loss.print-error | `print_job.rs` print_notice: "{title} was not printed: {reason}" |
| Trial balance, computing | trial-balance.loading | draw_loading (shared by every report) |
| Outstanding receivables, computing | outstanding.loading | draw_loading |
| Stock summary, computing | stock-summary.loading | draw_loading |
| Audit, reading the log | audit.loading | `audit.rs` body_ui: "Reading the audit log..." |
| Audit, log could not be read | audit.failed | `audit.rs` AuditState::Failed, set_failed, shown in the error colour |
| GSTR-1 not loaded for the period | gst.gstr1-not-loaded | `gst/draw.rs` draw_not_ready Feed::NotLoaded: "Not loaded for this period yet." |
| GSTR-3B not loaded for the period | gst.gstr3b-not-loaded | same |
| GSTR-1 refused by the engine | gst.gstr1-unsupported | Feed::Unsupported: "Not supported yet: what (reason)" (`wire.rs` unsupported_message) |
| GSTR-3B refused by the engine | gst.gstr3b-unsupported | same |
| GSTR-1 sections could not be built | gst.gstr1-failed | `gst/worker.rs` failed(): "{what} could not be built", with the error as the reason |
| Masters, no ledgers yet | masters.empty | `masters.rs` draw_no_rows: "No ledgers yet", "Create the first ledger with Alt+C." |
| Masters, no stock items yet | masters.items-empty | same, for stock items |
| Masters, new stock item form | masters.create-item | `masters/form_ui.rs` stock item fields: Name, Alias, Under, Unit, Valuation, HSN/SAC, Track batches |
| Masters, balances loading | masters.loading | `masters/balances.rs`: dashes until the engine answers; the heading hint is hover text only |
| Masters, filter matches nothing | masters.no-match | `masters.rs` draw_no_rows: "No ledgers matches 'zzqx'." (the app's wording) |
| Import, one-step, idle | import.default | `import/view.rs` draw_direct; the step-by-step panel shows as not connected |
| Import, no company open | import.no-company | `import/view.rs` Step::Detect with no company: note and "Back to setup" |
| Import, company name missing | import.validation-error | `import/direct.rs`: "Error: enter the name of the company in the other app" |
| Import, port not in range | import.port-error | `import/direct.rs`: "\"99999\" is not a port number (1 to 65535)" |
| Import, running | import.running | `import/direct.rs` DirectState::Running: "Importing..." |
| Import, finished | import.imported | `ledgerly-api/src/tally_import.rs` ImportSummary Display (sample counts from the sample company) |
| Import, refused as not supported | import.refused | DirectState::Refused, unsupported_text (sample voucher type) |
| Import, failed | import.failed | DirectState::Failed: "could not import ... : {error}" (sample error) |
| Cold start, new tab (Ctrl+T, no company) | setup.new-tab | `shell/mod.rs` NewTab opens SetupScreen; `shell/chrome.rs` tab strip; titles "Company setup", "Company setup 2" |
| Cold start, sidebar collapsed (Ctrl+\) | setup.rail | `shell/sidebar.rs` 60 px rail; labels hidden |

Notes on the S12b rows:

- The rail resets on navigation in the mock (`railOn` is set from every hash). The app keeps the
  rail until it is toggled again. Recorded in PARITY-GAPS.
- Import and the report states above use the sample company. The import counts are the sample's
  own (14 vouchers, 43 ledgers), not a real import.

## Still MISSING (after S12b)

Each row below is in the app code but the app cannot reach it from the UI, or it is reached only
with a sample that the mock does not hold. The reason is exact.

| App screen or state | Why it has no mock state |
|---|---|
| Import, choose company, pull (running, cancelling, cancelled, failed), map (not started, working, unmapped, ready), reconcile (committing, commit failed, reconciling, failed, ready) | Not reachable in this build. `screens/import/view.rs:198` sets `STEP_BY_STEP_CONNECTED = false`, so the step-by-step controls are disabled and the screen says so (`import.default`). The steps are built and tested in `tests/u10_import_wizard.rs` but no UI path opens them. Designing them now would design screens users cannot open. |
| Open a company file, "Working on the company file..." (`shell/welcome.rs`) | `WelcomeScreen` is created only by `Shell::with_welcome`, which the app never calls; it is used by tests (`tests/u1_shell.rs`, `tests/s7_zoom.rs`). The cold start is `SetupScreen` (`Shell::cold_start`), which the mock covers. |
| Masters: "Not supported yet" notice (`masters.rs` line 2068) | Shown only after a masters action returns Unsupported. The sample engine does not refuse a masters action, so no reachable mock state shows it. The same wording is shown on the GST screen (gst.gstr1-unsupported). |

Former MISSING rows, now covered: cold new tab and cold rail (setup.new-tab, setup.rail), import's
entry card (setup.welcome links to import), profit and loss, trial balance, outstanding and stock
summary loading, print busy and its error, audit loading and failed, GSTR-1 and GSTR-3B not
loaded and unsupported, masters empty and loading variants.

## Open items for the designs above

- Rule 53(1A) of the CGST Rules: the credit note footer cites it from the taxguru commentary
  (read 2026-11-25), not the CBIC text. The debit note footer no longer says anything about
  rule 53 or input tax reversal: the page read says nothing about a buyer's reversal.
- Debit notes: s.34(3) puts a GST debit note with the supplier. The purchase-return booking in
  debit-note.default (a note to the supplier at the bill's rate, IGST by the engine) needs a GST
  adviser's check before the app's GST treatment is final.
- Voucher numbers shown in the mock are the sample's next numbers: PB/0145, CN/0001, DN/0001,
  JV/0033, CT/0022 (the engine counts from the highest number of each type).
- Profit and loss layout: the page https://help.tallysolutions.com/profit-and-loss-statement-tally/
  (read 2026-11-25) documents the horizontal form, "Show with Gross Profit" and a detailed view;
  the group order inside each side follows the owner's brief of 2026-11-25.
- import.imported uses neutral wording ("the other program's company"), no product name.
- Tally voucher fields (credit note reference, journal and contra rules) were found only as search
  snippets, not full pages. The layouts here follow the app's own forms and the GST rule, not a
  field-by-field copy of any other product.
- Tally Journal (`journal.*`) and Contra (`contra.*`): the Journal page was read (te9rel62
  release). The Contra page URL returned a "lost your way" page, so contra rules are not checked
  against Tally.
- Tally print preview and the edit log: no page URL was reachable from the pages read, so the
  print and audit states are not checked against Tally's help. They follow the app's own behaviour.
- Print busy (`profit-and-loss.print-busy`): the line "Preparing the PDF." is a proposal. The app
  shows no text while a PDF is prepared.
- Audit: the integrity certificate's zero broken links is the mock's sample state, not a computed
  result.

## Sources (read 2026-11-25)

Statute and rules (CBIC text, read as PDF text with pdftotext; not commentary):

- CGST Act 2017, updated to 31.08.2021: https://cbic-gst.gov.in/pdf/CGST-Act-Updated-31082021.pdf
  (PDF p.63): s.34(1) credit note, s.34(2) declaration in the return, s.34(3) debit note, s.34(4)
  declaration of the debit note in the return.
- CGST Rules 2017, Part A: https://cbic-gst.gov.in/pdf/24092021-CGST-Rules-2017-Part-A-Rules.pdf
  (PDF pp.60-61, printed "Page 60 of 169"): rule 53(1A)(a)-(i), the particulars of a credit or
  debit note. (g) is the serial number and date of the corresponding tax invoice or bill of supply.
  Rule 53 (original, not 1A) sets the particulars of a revised invoice and is not the note rule.
  Note: the 2026-11-25 research of this round first used a commentary (taxguru.in); the text above
  replaces it.

TallyPrime help (help.tallysolutions.com, pages fetched and body text extracted 2026-11-25):

- Recording Credit Note: https://help.tallysolutions.com/docs/te9rel65/Voucher_Entry/Accounting_Vouchers/Credit_Note_Entry.htm
  (voucher and invoice mode, F11 features, Ctrl+F8, Ctrl+V; sample: goods sold returned by a customer)
- Recording Debit Note: https://help.tallysolutions.com/docs/te9rel65/Voucher_Entry/Accounting_Vouchers/Debit_Note_Entry.htm
  (Ctrl+F9, Ctrl+V; sample: goods purchased returned to a supplier)
- Recording a Journal Entry: https://help.tallysolutions.com/docs/te9rel62/Voucher_Entry/Accounting_Vouchers/Creating_a_Journal_Entry.htm
  (F7; narration keys Alt+R, Ctrl+R; the page covers the interest example, not a balance check)
- Display Receivables (bills receivable, invoice by invoice, effective date, due-on, Shift+Enter
  breakup, F1 Detailed): https://help.tallysolutions.com/docs/te9rel65/Reports/MIS_Reports/display_receivables.htm
- Ledger Outstandings Report: https://help.tallysolutions.com/docs/te9rel65/Reports/MIS_Reports/Ledger_Outstandings_Report.htm
- Ageing Analysis Report (ageing by bill date or by due date, age periods set by the user):
  https://help.tallysolutions.com/docs/te9rel65/Reports/MIS_Reports/Ageing_Analysis_Report.htm
- Not readable: Contra Entry (URL returned a "lost your way" page); Print and Edit Log pages (no
  URL found in the pages read).

Primary sources for the app's own behaviour: `crates/ledgerly-app/src/**` (the files named in
each row above) and `crates/ledgerly-api/src/tally_import.rs`.
