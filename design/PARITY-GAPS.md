# Known gaps between the app and the mock

The current list of differences between the app's captures (`design/app-captures/`) and the mock's
(`design/mock-v2/captures/states/`), audited in round s14 against the code and the captures.
Remove an entry only in the commit that closes it. History lives in git, not here.

Classes:

- **FIX**: still true, the app is wrong or missing something it can do; waits for a round.
- **NOT BUILT**: waits on a feature that does not exist (the reason says which).
- **DELIBERATE**: we differ on purpose; the citation is the help page (`help.tallysolutions.com`,
  read date), the mock line or the rule that decides it. Kept so nobody "fixes" it back.

## Home (`home.*`, `start.recent`)

- **NOT BUILT** GST cash payable tile: the mock shows 39,498.00 after accepted ITC; the engine has
  no set-off, so the tile says "Not supported yet" (`screens/home/kpi.rs` `GST_REASON`).
- **NOT BUILT** IMS nudge ("3 2B records have no IMS action"): the count is not wired yet.
- **DELIBERATE** the GSTR-1 nudge uses the monthly due date, the 11th (CBIC Notification
  83/2020-Central Tax); QRMP dates are not modelled and no filing record exists.
- **NOT BUILT** compare dialog has no "Adjust rule" or "Not a duplicate": the engine cannot record a
  pair as not a duplicate; it adds a "Bank date" row in place of the mock's statement line number.
- **NOT BUILT** review rows have no age ("2 min ago"): `ReviewItem` carries no created time.
- **DELIBERATE** review decisions: Y and Ctrl+A accept the row, N and Alt+D hide it for the session
  only, because the engine stores no rejection; feedback is a note, not a toast.
- **DELIBERATE** the entry bar says "not connected in this build" where the mock says "runs on this
  computer"; claiming a model that is not there would be a false statement.
- **NOT BUILT** a recent GSTR-1 row has no live detail ("3 sections ready, 1 needs correction").
- **NOT BUILT** the palette's Ask rows ("Ask: q", "Or" Ask or Draft; index.html:1127, 1129) while AI is off.
- **DELIBERATE** Search vouchers detail says "Number, party, amount or date", which is what the
  search matches (`MatchedOn`); the mock's "Narrations and party names" would be wrong.
- **DELIBERATE** `home.loading` and `balance-sheet.loading` are captured with preset books and a
  held worker, so the shell's open-company answers do not queue behind the hold.
- **DELIBERATE** `start.recent`: the recent list is read from the user's config folder; the AI chip
  reads "AI: not set up" where the mock says "AI on this computer", and the sidebar has no review
  count, because no company is open to ask (a chip that claimed a tier would be false).

## Palette

- **NOT BUILT** the mock's Read a bill photo, Import bank statement, Import 2B / IMS JSON and Back
  up now commands (no bill reader, no palette route to the importers, no backup schedule).
- **DELIBERATE** Ctrl+E exports any open report, as the help page "How to Export Data"
  (help.tallysolutions.com/export-data-in-tally/, read 2026-11-23) says, though the mock's day book
  key bar has no Ctrl+E; the key bar leaves it out as the mock does.

## Balance sheet and drills (`balance-sheet.*`)

- **NOT BUILT** Export has no PDF (the mock offers "PDF, XLSX or CSV", index.html:1624); the print
  preview's Save as PDF writes it, the export dialog does not offer it.
- **DELIBERATE** `screens/audit.rs` (the old auditor window with `TableBuilder`) is not routed by the
  shell: the Audit screen people see is `report_view` with the mock's 34-point rows, so no capture
  uses it. It stays for its own tests.
- **DELIBERATE** F2 says "Period" and opens a from-to editor; the mock says "F2 Date".
- **DELIBERATE** the as-at date is the last voucher date, 20 Nov, where the mock reads its own clock,
  21 Nov (help page "Trial Balance", help.tallysolutions.com/tally-prime/accounting-financial-reports/trial-balance-tally/,
  read 2026-11-21; `navigate::default_period`).
- **DELIBERATE** section and group names are the engine's (decision 2 of the 2026-11-22 parity report).
- **DELIBERATE** the line under the statement keeps the engine's notes; the mock's "Net profit ... Equals
  'Profit to date' on the balance sheet, checked by the engine" and "Difference ..." lines are not drawn:
  the engine has no such check (`ledgerly-reports` computes the P&L net per period and the balance sheet's
  Profit and Loss A/c cumulatively, so the two are equal only for a period from the books' start), and the
  screen must not compute one. Needs an engine check with that scope; until then the line would be a claim
  nothing verified.
- **DELIBERATE** the provisional reason names the item, quantity and voucher the engine gives.
- **NOT BUILT** the voucher view's Audit card shows the engine's trail only: no attachment, no bank rule.
- **DELIBERATE** the capture company (Fictional Traders) is small, so its statement is shorter.

## Profit and loss and report tables (`profit-and-loss.*`, `trial-balance.*`, `outstanding.*`, `stock-summary.*`, `audit.*`)

- **DELIBERATE** the two-sided trading account and profit and loss account follow "How to View Profit
  and Loss Statement in TallyPrime" (help.tallysolutions.com/profit-and-loss-statement-tally/, read
  2026-11-25); the mock draws Income and Expenses and must change to the two-sided form.
- **DELIBERATE** reports open at the last voucher's date, 20 Nov (the Trial Balance page above).
- **DELIBERATE** `audit.unsupported` is a table here: the engine returns an `EditsAfterLock` table;
  capturing the mock's claim would be a wrong picture.
- **DELIBERATE** the audit edit log lists the engine's audit events (Seq, Date, Time, User, Action,
  Object, Reason, Field, Before, After), not the mock's vouchers.
- **DELIBERATE** the integrity certificate is the mock's card (title, one fact per line, no column
  headings, the head hash wrapped in its column) but the facts are the engine's (Result, Events
  checked, Head, First broken event, Checkpoints), not the mock's voucher counts.
- **DELIBERATE** outstanding bills: Overdue names the engine's ageing bucket ("31-60 days"), not the
  mock's day count (the engine gives buckets; the UI never works out a figure); a bucket past the
  due date is red ink as the mock's, and a bill due on the as-at date reads "Due today". The lines
  are grouped by party in the engine's party order, not the mock's most-overdue-first: the bill-party
  report documents party order and a bill sort inside it (help.tallysolutions.com/docs/te9rel53/Reports/MIS_Reports/BillParty_wise_Outstanding_Report.htm,
  read 2026-11-26).
- **NOT BUILT** the outstanding total line "7 open bills": the engine returns no count.
- **DELIBERATE** the stock summary's subtitle says "quantity and value from the stock engine" since
  the engine values by each item's own method.
- **DELIBERATE** the trial balance names groups as the engine does ("Capital Account", "Duties & Taxes").
- **DELIBERATE** a nil balance is blank, not "0.00 Dr"; a figure the engine could not give says why.

## Day book (`day-book.*`)

- **NOT BUILT** Origin shows Typed, AI draft, accepted or Imported only: no attachment, bank-rule or
  draft kind is stored ("Image attached", "Bank rule" in the mock).
- **NOT BUILT** Ctrl+I Origin: the original proposal is not opened from the day book.
- **DELIBERATE** a non-regular status is a badge in the Origin cell; the mock has no Status column.
- **DELIBERATE** the month view keeps the cursor on the voucher it was on (`replace_report`).
- **DELIBERATE** F2 offers the shown day and Alt+F2 changes the period: the TallyPrime page
  (help.tallysolutions.com/track-purchases/, read 2026-11-26) says F2 changes the date and Alt+F2 the
  period, and says nothing of a day after the current date.
- **DELIBERATE** the empty state's four buttons sit centred in the card; the mock's sit at the left.

## GST returns (`gst.*`)

- **NOT BUILT** GSTR-3B rows from IMS and set-off (index.html:1683-1686, "Payable in cash"): set-off
  (engine item E-1) does not exist.
- **NOT BUILT** the GSTR-3B key bar (index.html:1721): no rows on that tab, and only GSTR-1 exports.
- **NOT BUILT** the GSTR-2B JSON shape is `pending observed` (rule A5): no public GSTN schema was
  found; the parser keeps its `items[]` shape.
- **NOT BUILT** the 2B tab leaves out deemed acceptance and the amendment line: no public source found.
- **NOT BUILT** Re-apply masters and Export IMS action file: the shell takes neither request, so the
  buttons are not drawn.
- **DELIBERATE** the 2B tab's "Mark GSTR-3B filed" button and stacked IMS cell: the mock has no way
  to record a filing.
- **DELIBERATE** GSTR-1 by section with a side card, a folded voucher table and the agreement line
  (index.html:1673-1676; offline tool worksheets, help.tallysolutions.com/file-gstr-1-using-offline-tool/,
  read 2026-11-25).
- **DELIBERATE** a Cess column on GSTR-3B, our own words under "What happens next", the period month
  picker (Alt+F2), the bucket filter on the tiles, and Export JSON disabled off GSTR-1: the engine
  works out cess and a figure is never dropped; the mock's sentences claim things the app does not do.
- **NOT BUILT** "monthly" in the subtitle comes from the shell, which always opens one calendar month
  (`gst_return_period`); telling a QRMP (quarterly) filer apart needs a filing-frequency field on
  the registration, which the company model does not have.
- **DELIBERATE** the capture company's GSTIN and figures differ from the mock's Godavari Drip; the
  sample's GST ledgers are one per head and its GSTINs use the fake PAN pattern `ZZZZZ####Z`.

## Masters (`masters.*`)

- **DELIBERATE** the create ledger form has one row more than the mock's compact one: a "More
  details: opening balance, party, bank and tax" link that opens the rest of the ledger master the
  help page "Ledgers" describes (help.tallysolutions.com/ledgers-in-tally/, read 2026-11-21); the
  State field is a list that names "(from GSTIN)" in its text where the mock draws a badge.
- **DELIBERATE** Balance sits after Bill-wise and Credit days, State is a list, GSTIN shows with Party
  details on, Credit days with Bill-wise on, Bill-wise is a checkbox and there is no City field
  (help page "Ledgers", help.tallysolutions.com/ledgers-in-tally/; index.html:1804-1812).
- **DELIBERATE** Alt+D Delete and Alt+Left / Right Change kind stay in the key bar; "Create {kind}"
  follows the tab; the filter is on every tab; ledgers are in name order; no Delete button under
  the list; Enter opens a ledger's vouchers; tabs are dimmed while a form is open.
- **DELIBERATE** open amounts carry their side ("1,18,000.00 Dr"); a bill with no due date shows a
  "No due date" badge; a credit or provisional stock value is marked.
- **DELIBERATE** `masters.empty` and `masters.items-empty` keep the filter box and More master kinds
  above the empty card; hiding them would strand a user on an empty kind.
- **NOT BUILT** Alt+C on the item invoice's item field (index.html:390, 1253): re-read 2026-11-26,
  help.tallysolutions.com/tally-prime/inventory/manage-stock-item-tally/ documents Alt+C for a stock
  group or a unit inside the stock item form, and no India page says Alt+C on a voucher's Name of Item
  creates a stock item (only a Brazil-edition page does), so the key does nothing there.
- **NOT BUILT** `masters.suggested-ledger` (needs the assistant); the "Not supported yet" notice has no
  mock state because the sample engine never refuses a masters action.
- **DELIBERATE** `masters.create-item` is captured from the sample company: a company without
  inventory is not offered stock items (help page "Inventory Information",
  help.tallysolutions.com/docs/te9rel65/Creating_Masters/Inventory_Info/Inventory_Information.htm,
  read 2026-11-25).

## Bank reconciliation (`bank.*`)

- **NOT BUILT** Export of the reconciliation statement (index.html:1784): the bank screen has no export.
- **NOT BUILT** the statement lists the engine's totals, not each line (index.html:1741-1745): the
  engine's `Brs` carries no lines.
- **DELIBERATE** the card beside the statement says Reconciled or Not yet reconciled from the engine's
  Difference line being nil, in its own words ("The imported statement agrees with the balance as per
  bank"); the mock's "once the listed lines are posted" and "Review suggestions" button are not
  drawn: the statement lists the bank-side items the help page's reconciliation statement lists, and
  suggestions wait for the assistant.
- **DELIBERATE** "Reconciliation statement as of 20 Nov 2026" stays over the card: the as-of date is
  the engine's, and the subtitle's month alone would name the wrong day.
- **NOT BUILT** the lines table lacks Exact, Rule and "AI suggested" badges and a confidence meter.
- **DELIBERATE** the key that changes account is C, not F4, which is Contra in this app.
- **NOT BUILT** `bank.suggested-ledger` and `bank.ai-suggestions` need the in-app assistant.
- **DELIBERATE** `bank.default` shows the empty state because the fixture has no statement.

## Payment, receipt, contra, journal (`payment.*`, `receipt.*`, `contra.*`, `journal.*`)

- **DELIBERATE** `payment.bill-wise` shows an empty narration behind the popup: the popup opens on the
  amount's Enter and takes the keyboard, so the narration can only be typed after it; the mock's
  "NEFT, against VL/861" is a filled narration the app does not write for the user.
- Fixed in s14: the `payment.default` capture ends mid-entry with the keyboard back on row 1, as the mock.
- **DELIBERATE** `payment.overdraft` shows the same state as the mock (narration typed, the warning that
  Mula Valley Co-op Bank goes below zero, shortfall 36,000.00); the warning's words differ: the mock
  names a per-company policy (Block, Warn, Allow) and says "no overdraft limit in its master", but the
  company has no such policy yet, so ours says so (a claim of a setting that does not exist would be false).
- **DELIBERATE** the mid-entry lilac fill is not drawn for a row the keyboard is not on: it would mislead.
- **DELIBERATE** the Mode is bare text once set but a box with a list while empty or focused; the
  mock's "NEFT / UPI" names two instruments the engine stores as one each.
- **DELIBERATE** group names keep the sample's capitalisation ("Direct Expenses").
- **DELIBERATE** a prefilled amount is selected when the keyboard arrives; a literal finer than a
  paisa is refused.
- **DELIBERATE** the purchase's input credit line is the engine's credit "before the 2B match": the
  s.16(2) conditions (bill in 2B, receipt, payment) cannot be known on a draft. A ledger without an HSN/SAC
  shows "Not computed" and the engine's reason, never a guessed figure; a reverse-charge purchase does too.
  A blocked credit is shown as "0.00 (n not claimable)".
- **DELIBERATE** the balance line can say "balance not shown": the API answers Unsupported, never zero.
- **DELIBERATE** the note's "Invoiced on SI/0412, 14 Nov 2026: 67 set of ..." line lists only the
  quantity invoiced (the pages read ask for the original invoice number and date and say nothing about
  showing a quantity; it is our choice). It does not subtract earlier notes, so it does not stop a
  second return of the same goods, and it shows nothing while the invoice is not found (a half-typed
  number is not an error).
- **DELIBERATE** a debit note goes back to the supplier, as a purchase return is a Debit Note against
  the supplier in the accounting product; a note is titled "Credit Note" or "Debit Note".

## Sales invoice and item invoices (`sales-invoice.*`)

- **DELIBERATE** a company without inventory gets an accounting invoice
  (help.tallysolutions.com/sales-of-goods-services/ and
  help.tallysolutions.com/tally-prime/sales-process/faq-sales/, both read 2026-11-23).
- **DELIBERATE** As voucher mode is Ctrl+Shift+H, not a choice in Ctrl+H's list
  (help.tallysolutions.com/sales-of-goods-services/, read 2026-11-26, says Ctrl+H then select the mode):
  Ctrl+H keeps toggling item and accounting mode. It is for sales and purchase only; a note keeps two
  modes. Accounting and as voucher share their ledger lines.
- **DELIBERATE** As voucher shows Dr and Cr, not By and To (the pages offer both), asks the party in
  the header and lists only the lines the user typed; the engine's party line, tax amounts and round-off
  show in the tax box and are posted, not listed as rows as the mock's `sales-invoice.as-voucher` does.
- **DELIBERATE** the pages say nothing on the GST ledger lines of an as-voucher entry, so the rule is
  our choice, built on the GST sales and GST-not-calculated pages (read 2026-11-26): the user picks the
  GST ledgers, they must be exactly the heads of the supply (CGST and SGST, CGST and UTGST, or IGST),
  an amount typed on one is replaced, and the engine fills the party line. A stock item line, a
  cess ledger, no GST ledger or the wrong heads show "Not supported yet" with the reason.
- **DELIBERATE** an as-voucher entry is saved as an accounting invoice (the GST returns read that
  mode); it opens again as one.
- **NOT BUILT** the e-Invoice cell says "Not built yet": nothing maps a voucher to an `EInvoice`.
- **DELIBERATE** the sample GSTIN is synthetic and the rule pack reads "2026.1-slabs".
- **NOT BUILT** the toast has no Undo Ctrl+Z: no undo exists.
- **DELIBERATE** the empty tax box shows dashes, not 0.00, until the engine has answered (never a
  wrong number); the rate shows as typed because rate parsing stays strict.
- **DELIBERATE** the party list offers every ledger, recents first: the page
  (help.tallysolutions.com/tally-prime/gst-regular-sales/gst-sales-tally/, read 2026-11-26) says a party
  is a Sundry Debtor or Creditor ledger and that a Cash or Bank ledger can be picked too, and gives no
  other rule, so the list is not narrowed.
- **DELIBERATE** the place of supply is not filled from the party: the same page has the user set it
  on the Party Details screen (it is prefilled only for SEZ sales, which are not built).
- **NOT BUILT** the item list shows name and unit only; the free-text item row of
  `sales-invoice.validation-error` is not offered.
- **DELIBERATE** the period-locked banner reads "lock date"; the date and Sales ledger are bare
  inputs; Disc % and Typed amount hide by default; there is no Discard button (Esc asks).
- **DELIBERATE** the focus ring is 2 points outside in the accent (index.html:65); hc widens it.

## Review queue, AI and privacy, Ask, quick entry, bill photo

- **NOT BUILT** the queue lists bank matches and Rule D-02 duplicates only; bill drafts, plain-words
  drafts, new-ledger suggestions and 2B explanations need a draft store; the sidebar count stays withheld.
- **NOT BUILT** Home rows do not open their proposal; the empty state's "Read a bill" (Alt+O).
- **DELIBERATE** no optional-voucher list (help page "Display Optional Vouchers",
  help.tallysolutions.com/docs/te9rel66/Reports/MIS_Reports/Display_Optional_Vouchers.htm).
- **NOT BUILT** `ai-settings.cloud`, the local model rows, per-feature switches and AI event rows:
  no keychain store, model runner or feature switch exists.
- **DELIBERATE** no tier is chosen by default; the log headers are the company file's columns; the
  "Current" badge is a border only; the report list shows for every tier.
- **NOT BUILT** `ask.result`, `ask.loading`, `ask.low-confidence`, `ask.unsupported`, `quick-entry.*`
  (proposal, hinglish, unknown-ledger, loading, rejected), `purchase-bill.reading`, `.proposal`,
  `.low-confidence`, `.mismatch`, `.duplicate`, `.cloud-consent`, `.error`, `.accepted` and
  `gst.recon-explain`: no model runs; a screen would show a look with nothing behind it.
- **NOT BUILT** the AI-off note's AI and privacy button is a secondary button; the mock's is a ghost
  (the kit has no ghost `ButtonKind`).
- **DELIBERATE** Ask's box holds a hint and is disabled and the key bar lists only the name, because
  nothing would answer a typed question.
- **NOT BUILT** `purchase-bill.offline-ai` has no form or image beside it and its head lacks "Read
  another" (Alt+O); the key bar is named "Bill photo".

## Setup (`setup.*`)

- **NOT BUILT** the Keyboard step's preset and the cloud AI choice are shown but not stored (no
  store, no keychain); the backup folder is not saved and no backup runs on a schedule.
- **DELIBERATE** the import card on Start opens the Import screen with no company open; that screen
  says to create or open one first and offers Back to setup, as the mock's `import.no-company` does,
  so nothing can finish wrongly.
- **NOT BUILT** the import screen's step-by-step path (the five-step wizard) is not connected.
- **NOT BUILT** F11 on the Features step: no feature screen exists; rows with no model switch say so.
- **DELIBERATE** the "Company file" field is ours (the wizard writes a file); City is kept as the
  first address line because `Address` has no city field.
- **DELIBERATE** the mock's sample is prefilled and the wizard starts blank; refusals are a toast.

## Print (`*.print-preview`, `*.print-busy`, `*.print-saved`, `*.print-failed`)

Ctrl+P (or the Print preview button) prepares the PDF on the worker, shows the pages the print
engine laid out with the mock's card (paper, pages, destination, Save as PDF, Close), and Save writes
the same bytes. Flow per "How to Print Invoices/Reports in TallyPrime"
(help.tallysolutions.com/print-invoices-reports/, read 2026-11-25): print, preview, then print.
Printed reports use Indian digit grouping and the screens' Dr/Cr rule; profit and loss prints as the
two-sided account, the balance sheet as one vertical statement, the trial balance as Debit and
Credit (`ledgerly_reports::print_text` places the engine's figures only).

- **NOT BUILT** print configuration: printer, paper size, Pages to Print (F10), margin, page start
  number, "Set Preview as Default" (help.tallysolutions.com/configure-for-print-export-share/, read
  2026-11-26). The engine writes one A4 PDF (A5 by template) and has no page-range output; the card
  states paper and destination as facts. The preview scrolls and has no zoom.
- **NOT BUILT** printing to a device; the day book; any voucher but sales invoices, credit notes and
  debit notes; a UPI QR (no UPI address is held).
- **NOT BUILT** the mock has no preview state for an invoice; the same card and key bar are used
  there and no capture exists (reported to the mock round).
- **DELIBERATE** the preview sets each text in the app's font inside the box the engine gave it
  (shrunk if wider): the PDF uses Helvetica, which the app does not embed.
- **DELIBERATE** the printed page has horizontal rules and bold only: the mock's vertical rule
  between the two sides and tinted section bands are not drawn; the balance sheet's rupee-sign
  heading prints "Amount" because the print font is ASCII-only (the print crate refuses other
  characters).
- **DELIBERATE** Ctrl+P is not on the key bars, as the mock's (index.html:1634).

## Accessibility, themes, keys

- **DELIBERATE** the divider border (the mock's own `--border` token) is below 3:1 in light and dark.
  A divider only separates rows; each row's text and the field edges (`border_strong`) meet 3:1, and
  `tests/s7_contrast.rs` pins the gap. Darkening it would change the mock's look.
- **DELIBERATE** table rows and keycaps do not grow by the extra hc rule pixel: `T_ROW_HEIGHT` is fixed
  so a row is the same height in every theme and the rule is drawn inside it (a 1 px difference in a
  34 px row; the keycap's 2 px hc border is drawn inside its box too).
- Fixed in s14: egui's own disabled widgets fade to 0.8 instead of 0.5 in hc (7:1 text kept), pinned by
  `tests/s14_hc_disabled.rs`.
- **DELIBERATE** the hc focus ring is 3 px (`theme.rs` `focus_width`); zoom is egui's, per session.
- **DELIBERATE** the keys note keeps the "Keys in use now" line, which is true of the app; the new-tab
  title is "Company setup 2"; the rail stays until it is toggled.

## Import (`import.*`)

- **DELIBERATE** `import.refused` and `import.failed` say the engine's own text, not the mock's
  invented "Memorandum" and "connection refused" examples; the summary counts are `ImportSummary`'s.

## Captures with no real cause

- **DELIBERATE** `gst.gstr1-unsupported`, `gst.gstr1-failed`, `audit.failed`, `profit-and-loss.print-error`
  are not captured: the sample books never make the engine refuse; inventing a failure would be a
  wrong picture.
- **DELIBERATE** the open-company busy line ("Working on the company file...", `shell/welcome.rs`) has
  no mock state; the masters balance hint is hover text because the mock shows dashes.

## Real launch path (cold start, `s15-cold-start-sheet.png`)

Seen in the release binary under a nested X server with an empty config and data directory.

- **DELIBERATE** the title bar shows today's date and "AI: not set up" on a cold start; the fixture
  captures pin the date to 21 Nov 2026 and show "AI on this computer". Both are the app's real state.
- **DELIBERATE** the key bar drops the hint that does not fit before it would overlap the right-hand
  hint ("Enter Open" on Home at 1280 points; the mock shows it). The mock's bar is wider than the
  app's text at that size; overlapping text would be worse than a missing hint.
- **FIXED** the first company is created in `~/Documents/Ledgerly`, which did not exist on a new
  machine (Finish failed with "No such file or directory"); the folder is now created first.
- **FIXED** the bill-wise popup on Payment did not take the keyboard when the open bills arrived after
  the popup opened.
