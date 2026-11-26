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

- **DELIBERATE** the GST cash tile and the IMS nudge read the month of the as-at date from the
  engine (`Api::home_gst`). Without a single regular GSTIN, a 2B file or a supported set-off they
  say why instead of a number; zero records gives no IMS card.
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
- **DELIBERATE** outstanding bills: Overdue names the engine's ageing band ("31-60 days", "Not yet due",
  "Due today"), not a day count (the engine gives bands; the UI never works out a figure); a band past the
  due date is red ink. The lines are grouped by party in the engine's party order, then by bill date, as the
  bill-party report documents (help.tallysolutions.com/docs/te9rel53/Reports/MIS_Reports/BillParty_wise_Outstanding_Report.htm,
  read 2026-11-26). The mock was corrected in round s17 to the same order, bands and footnote.
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

- **DELIBERATE** the GSTR-3B key bar stays "1 2 3 Tabs, Esc Back": the engine has no GSTR-3B JSON
  and the tab has no openable rows, so J/K, Enter and Ctrl+E would be dead keys (Ctrl+E is shown
  disabled).
- **NOT BUILT** set-off starts from the month's liability and the credit the user accepted in IMS;
  the electronic credit ledger's opening balance and earlier periods are not modelled, and credit
  that is only deemed accepted is treated as not used (rule `pending observed`: IMS no-action
  handling). Rule 86B (the 1% cash rule) returns Unsupported. Reverse-charge tax, the cess source
  and interest or late fee are not worked out.
- **DELIBERATE** the IGST-credit split between CGST and SGST uses a least-cash policy, which Circular
  98/17/2019-GST allows but does not require (the order across heads is law: CGST Act s.49(5), 49A,
  49B, Rule 88A).
- **DELIBERATE** the app shows a Cess column the mock lacks, and six notes beside the table where
  the mock has four.
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

- **DELIBERATE** Export of the reconciliation statement (Ctrl+E on the Statement tab) writes CSV or
  XLSX through the report exporter. The capture's save dialog is scripted to a folder of the
  capture run, so the notice names that path, not the mock's /home/asha/Documents.
- **DELIBERATE** a long line ("... cheque 004512") wraps inside its column and the row grows to fit
  (the mock lets it run on under the amount column). Nothing is clipped or overlapped.
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
- **DELIBERATE** Ctrl+H opens a list of the modes and a pick changes the mode, as the sales and
  purchase pages say ("Press Ctrl+H (Change Mode) > select" a mode; Item Invoice, Accounting Invoice,
  As Voucher in that order; help.tallysolutions.com/sales-of-goods-services/ and
  help.tallysolutions.com/tally-prime/purchase-process/purchase-goods-and-services-tally/, read
  2026-11-26). The pages do not describe the list, so its look is ours (`sales-invoice.mode-list`):
  a card under the mode button, opening on the current mode, Up and Down, Enter or a click. A note
  is offered Item and Accounting only (the engine taxes an as-voucher entry for sales and purchase
  invoices). Accounting and as voucher share their ledger lines.
- **DELIBERATE** in As voucher the party is typed on line 1 of the table, the Dr line of a sale (the Cr
  line of a purchase), as the page says ("In the Dr field, select the party ledger",
  help.tallysolutions.com/sales-of-goods-services/, read 2026-11-26); the head has no Party box. The line
  takes the party list, Alt+C creates a ledger there, Ctrl+D on it empties the party (the engine needs one,
  so the line cannot be removed) and a mode switch keeps the party. The Enter order stays Party, Place of
  supply, Reference, then the ledger lines: the engine needs the place before it can name the GST heads,
  and the product asks it on the Party Details step that follows the party (gst-sales-tally page, read
  2026-11-26). A purchase in As voucher loses the supplier's GSTIN badge the head showed (it sat next to
  the Supplier box); Party GSTIN still shows on a sale.
- **DELIBERATE** As voucher shows Dr and Cr, not By and To (the pages offer both). Its table lists the
  party (Dr), each typed ledger, and the engine's invoice round-off line when there is one, as the
  mock's `sales-invoice.as-voucher` does. Every figure of the party, a GST ledger and the round-off
  is the engine's, in Indian grouping, and empty until the engine has answered. A typed amount
  keeps its right edge a little left of the engine's figures (it is an input).
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
- **DELIBERATE** while a line is invalid Save is disabled and the error is a red box with an alert
  icon naming the line. The valid lines keep the engine's amount and rate (the engine is asked about
  them alone); the tax box shows dashes, not figures for the half-entered invoice (never a wrong number).
- **NOT BUILT** the item list shows name and unit only; the free-text item row of
  `sales-invoice.validation-error` is not offered.
- **DELIBERATE** the unknown-item error ends "create it in Masters", where the mock said "with Alt+C": the
  pages read document Alt+C on ledger fields only and the screen cannot make an item, so the mock's
  sentence would promise what the app does not do (mock corrected in round s17).
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

## Print (`*.print-preview`, `*.print-config`, `*.print-range-error`, `*.print-busy`, `*.print-saved`, `*.print-failed`)

Ctrl+P (or the Print preview button) prepares the PDF on the worker, shows the pages the print
engine laid out with the mock's card (paper, pages, destination, Save as PDF, Close), and Save writes
the same bytes. Flow per "How to Print Invoices/Reports in TallyPrime"
(help.tallysolutions.com/print-invoices-reports/, read 2026-11-25): print, preview, then print.
Printed reports use Indian digit grouping and the screens' Dr/Cr rule; profit and loss prints as the
two-sided account, the balance sheet as one vertical statement, the trial balance as Debit and
Credit (`ledgerly_reports::print_text` places the engine's figures only).

- **MATCHES** print configuration as the help page documents it
  (help.tallysolutions.com/configure-for-print-export-share/, read 2026-11-26): the card holds
  Paper (A4 or A5), Copies, First page number and Pages to print (`3`, `2-4`, `1, 3-5`; blank is
  all); Update preview (Enter) prints again with them and the preview shows the new pages. A
  range that selects no page of the document, or a setting that is not a number, is said under the
  field and the preview keeps its last good pages (`profit-and-loss.print-range-error`). Pages are
  numbered from the first page number and a range selects by the printed number.
- **DELIBERATE** the settings the engine cannot produce are not drawn: printer choice, margins,
  Type of Copy and Show Page Number (the page number always prints) and "Set Preview as Default"
  (there is no stored preference to set). The help page lists them; they come with printing to a
  device.
- **DELIBERATE** Copies repeats the kept pages in the one PDF, in order; the card says "1, in 2
  copies". The help page's copies go to the printer; there is no printer.
- **MATCHES** the day book and the payment, receipt, journal, contra and purchase vouchers print
  through the page renderer (`day-book.print-preview`, `journal.print-preview`). The help pages
  give no layout for them, so they follow the screen: the day book prints the shown period with
  Particulars, Date, Voucher type, Voucher no., Amount and Narration (the screen's Entered by and
  Origin are audit facts and are not printed; Status and Narration are dropped when empty); a
  voucher prints "<Type> Voucher" with its ledger lines as Particulars, Debit and Credit, an items
  band for item lines, the total, and Reference and Narration. The mock has one state for each
  family; `journal.print-preview` stands for payment, receipt, contra and purchase too.
- **NOT BUILT** printing to a device; a UPI QR (no UPI address is held); a voucher of any other
  kind than sales, credit note, debit note, payment, receipt, journal, contra and purchase
  (memorandum, optional and cancelled vouchers answer Unsupported with the reason).
- **DELIBERATE** an invoice with several HSN codes prints one tax-summary row for each code and rate:
  the screen's engine answer groups the lines by the item's HSN and rate, sums the unrounded heads
  and rounds once, and the print refuses (Unsupported) when an item has no code or the grouped rows
  do not add up to the stored GST. `sales-invoice.print-preview` shows SI/0412, as the mock's state
  does again.
- **DELIBERATE** the preview has no zoom; it scrolls. The app's paper is the full A4 or A5 page
  at the width of the pane, so a short document sits at the top of a tall white page; the mock's
  paper is cut to its content. The voucher preview's page head reads "Voucher SI/0412" and its
  crumbs "Day book" (the screen the voucher was opened from), where the mock reads "Sales invoice
  SI/0412" under "Sales invoice".
- **DELIBERATE** the printed invoice, day book and voucher pages are the engine's own layout
  (company address block, tax summary by HSN code, Particulars/Date/Voucher type/Voucher no./
  Amount/Narration); the mock's paper was written to the same content and wording.
- **DELIBERATE** the preview sets each text in the app's font inside the box the engine gave it
  (shrunk if wider): the PDF uses Helvetica, which the app does not embed.
- **DELIBERATE** the printed page has horizontal rules and bold only: the mock's vertical rule
  between the two sides and tinted section bands are not drawn; the balance sheet's rupee-sign
  heading prints "Amount" because the print font is ASCII-only (the print crate refuses other
  characters).
- **DELIBERATE** Ctrl+P is not on the key bars, as the mock's (index.html:1634).
- **DELIBERATE** the preview draws the whole sheet as the print engine laid it out (A4 or A5 with its margins,
  scaled to the pane), not the mock's card cut to the content: the page is what will print, and a preview that
  hid the margins or the empty foot would not show it. The mock's text is larger because its card is not a
  scaled sheet.
- **DELIBERATE** a voucher's preview reads as the entry screen names it ("Sales invoice SI/0412", crumb
  "Sales invoice", subtitle "Saved · party · date", as the mock). The app reaches it from the voucher view,
  which a drill from the Day book opens, so the crumb is Home, Day book, Sales invoice; the mock's is Home,
  Sales invoice because it has no such drill. The invoice entry screen has no Ctrl+P on a saved invoice.

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
- **gap: no in-app print preview or configuration.** TallyPrime's flow is Ctrl+P (Current),
  then C (configure), I (preview), P (print), as read on "How to Print Invoices/Reports in
  TallyPrime" (help.tallysolutions.com/print-invoices-reports/, read 2026-11-24, last updated
  2026-11-20). The mock has no print state. Ours saves a PDF to a file chosen in a save dialog,
  and an "Open PDF" button then opens it in the system's PDF viewer (xdg-open, open or start).
  That viewer is the preview until an in-app preview exists.
- **gap: no printing to a device.** Ours writes PDF files only. The Alt+P > Configuration
  printer and paper setup is not built.
- **gap: some reports cannot be printed.** Trial balance, balance sheet, profit and loss, day book
  and ledger vouchers print (`Api::print_report`, `ledgerly-print::pdf::render_report`). Other
  reports (group summary, registers, outstanding, stock summary, audit) say "not supported" with
  the reason; CSV and XLSX export remain.
- **gap: only sales invoices, credit notes and debit notes print.** Any other voucher, and a
  voucher with no GST block, says "not supported" with the reason. Reasons come from the print
  crate (`PrintError::Unsupported`), never a blank page.
- **gap: no UPI QR on printed invoices.** The books hold no UPI address to pay to, so the built-in
  template (`ledgerly-print/templates/tax-invoice-a4.toml`) has no `upi_qr` block.
  `render_invoice_with_upi` draws one when a payee is given; the company needs a UPI address
  field before it can.
- **by design: printed text is plain ASCII.** The print crate refuses other characters (built-in
  Helvetica); the refusal is shown as "not supported", not replaced.
- **by design: Ctrl+P is not on the key bars.** The mock's report key bars (index.html:1634)
  and the voucher screen have no Print key, so neither shows Ctrl+P. The key map lists
  "Print" under Reports (index.html:1839; `keymap_map::layout`), and Ctrl+P prints the open
  report or voucher.

## Hindi UI (card s18, 2026-11-26)

Hindi (`hi`) is set per user from the palette's Language row and applies at once. Every
user-visible string of every current screen and state goes through `assets/i18n/{en,hi}.toml`:
a keyed string (`t`, `tf`) or, for static labels, `lit("English")`, which finds the English phrase
in `en.toml` and returns the Hindi. Figures, dates and Indian digit grouping come from the
formatters as before; month and weekday words come from the `format.*` keys.

Guards (all in `crates/ledgerly-app/tests/`):

- `i18n_hindi_screens.rs` draws every screen the palette can open (reports, bank ledgers, GST,
  masters, import, review, ask, AI, setup, keyboard map) and walks the real shell (home, palette,
  F8/F9/F5/F6 vouchers, balance sheet, day book, print preview, GST, masters, create ledger) with the
  language set to Hindi. It fails on any Latin word that is not data, a kept abbreviation or a listed
  engine text.
- `i18n_lit_phrases.rs` fails when a `lit("...")` phrase has no `en.toml` entry, or a `t`/`tf` key
  does not exist.
- `i18n.rs` keeps the key parity of the two files.

Open, with the reason:

- **gap: Hindi terms are mostly unsourced.** NACIN's administrative glossary and the CBIC Hindi
  page labels settle about twenty words; the official Hindi CGST Act, the CBIC lexicon and the RBI
  shabdavali could not be read (403, a registration gate, a human-verification page) and Tally has no
  Hindi UI. The rest are marked `# unsourced` or `# s18: see glossary` in `hi.toml` and need a Hindi
  speaking accountant's review (the header lists what was read and when).
- **gap: PDF bodies are English.** The print crate's fonts are Latin only, so printed pages and the
  page the preview draws stay English; the preview's own controls are Hindi.
- **gap: Dr and Cr, GST, GSTIN, GSTR, ITC, IMS, PAN stay Latin** as the books and the portal write them.
- **gap: engine text stays English.** Engine and validation reasons, GSTR table names and engine
  notes under reports, state names (the core GST code list) and engine report column labels that the
  generic view cannot match by identity. Spec section 7: the engine's words are not translated here.
- **gap: user data is not translated.** Ledger, group and voucher type names the user types, bank
  narrations, units and party names. Standard names the product ships (Sales, Purchase, Credit note)
  are translated when they match `en.toml`.
- **gap: native dialog filter names stay English** (the file chooser is the system's).
- **gap: text computed once keeps its language.** Worker lines (GST agreement text, bank match
  reasons) and `return_label` are built when the worker answers; changing language while one is on
  screen shows the old language until the screen reloads.
- **gap: the mock translates whole labels only.** `design/mock-v2/hi.js` is generated from the app's
  own files (`gen-hi.py`) and replaces whole text nodes and dates of a `lang=hi` state; sentences the
  mock words differently from the app stay English in the mock. The `hi.*` mock states exist to
  compare Devanagari widths and wrapping, not copy.
- **gap: Hindi captures cover eight states** (home, sales invoice, its print preview, masters,
  balance sheet, GSTR-1, bank reconciliation, setup welcome). Other screens are covered by the
  guard test, not by captures.
