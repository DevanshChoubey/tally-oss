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
- **DELIBERATE** the palette has no `?` and `+` prefixes, Ask or Draft rows, or "Read a bill photo"
  command in 1.0.0 (owner decision of 2026-11-26: the release carries no AI); the mock's
  `AI_IN_RELEASE=false` drops them the same way, and the legend and placeholder name no `?` or `+`.
- **DELIBERATE** Search vouchers detail says "Number, party, amount or date", which is what the
  search matches (`MatchedOn`); the mock's "Narrations and party names" would be wrong.
- **DELIBERATE** `home.loading` and `balance-sheet.loading` are captured with preset books and a
  held worker, so the shell's open-company answers do not queue behind the hold.
- **DELIBERATE** `start.recent`: the recent list is read from the user's config folder, and the
  sidebar has no review count, because no company is open to ask. The AI chip reads "AI off", as the
  mock's does in 1.0.0.

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

- **FIX (s21 round F)** a voucher posted by accepting an assistant draft showed "Typed" (the posting's
  actor is the person who accepted). The accept event now names the voucher and its feature, so the
  Origin cell is the mock's AI badge with the reader's words: "Plain-words draft, accepted", "Bill
  photo draft, accepted" (index.html day book, `v.ai + ', accepted'`); the voucher view says it came
  from an AI draft.
- **NOT BUILT** no attachment or bank rule is stored on a voucher ("Image attached", "Bank rule" in
  the mock). The Godavari capture fixture posts every voucher by hand, so `day-book.month` shows no
  AI badge where the mock has two (PY/0315, PB/0141); the badge is tested in
  `s21_ai_drafts::an_accepted_draft_shows_its_reader_as_the_day_book_origin` and the
  `day_book::origin_badge` unit test.
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
  disabled). Re-checked 2026-11-26: help.tallysolutions.com/tallyprime-json-file-gstr-3b/ and
  /file-gstr-3b-offline-tool/ describe the export and upload steps but not the file's schema, and no
  GSTN-published schema for the GSTR-3B offline-tool JSON was found (only third-party descriptions),
  so no writer is built (rule A5: no shape without a source).
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
  buttons are not drawn. Read 2026-11-26: help.tallysolutions.com/upload-ims-inward-invoices/ documents
  only an online upload (Alt+Z, All GST Options, Upload IMS Inward Invoices; Ctrl+B filters; S Send
  (Online)); no page read describes an offline IMS action file or a "Re-apply masters" action, and the
  mock has no such button, so nothing is built and nothing is drawn.
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
- **FIX (s21 round F)** the assistant's bank suggestions: a line the exact matcher leaves is asked
  about (`Api::bank_ai_suggestions`), with the open vouchers in its direction; the engine checks the
  row, the one voucher, that it was offered and that the amounts are equal, stores the suggestion as
  a draft (it lists in the review queue as a bank match, as the mock's q2), and accepting confirms
  it as AI suggested with the accept in the same commit (`Api::accept_bank_ai_match`). Skipping its
  line rejects it; a rejected or accepted line is not asked again. Every ask, refusal, accept and
  reject is in the AI activity log.
- **DELIBERATE** the side text of an AI suggestion is the engine's check ("Same amount as the voucher
  on the bank, N days apart"), never the provider's reason; the mock shows the model's words
  ("m.why"). A suggestion whose amounts differ is refused, not shown with a red check (the mock's
  "Amounts differ" line), so Accept is never live on a mismatch.
- **DELIBERATE** a suggestion is accepted or rejected one line at a time; nothing is accepted in bulk.
- **NOT BUILT** `bank.ai-suggestions` as a screen state: the "AI suggestions" tab, the proposal card
  with its source chips and check lines, Y/N/E keys (the app's Enter accepts and S skips) and a
  capture. The Godavari sample has no line the assistant could match to an existing voucher (every
  same-amount pair is within the exact matcher's 3 days), and its two mock suggestions that post a
  new voucher (ANNAPURNA CANTEEN, MEHTA AGRO) are not built: a voucher from a bank line is refused.
  The mock's KSK INDORE line is an exact match in the app (same amount, same day).
- **NOT BUILT** `bank.suggested-ledger` (a new ledger for unexplained lines).
- **NOT BUILT** the cloud prompt for bank lines: the cloud provider refuses (`match_bank_line`'s
  default), and with a cloud provider each unmatched line logs that refusal on every load.
- **DELIBERATE** `bank.default` shows the empty state because the fixture has no statement.

## Payment, receipt, contra, journal (`payment.*`, `receipt.*`, `contra.*`, `journal.*`)

- Fixed in s22: the Account of a payment or receipt, and every contra line, lists only Cash-in-Hand
  and Bank Accounts ledgers (and their sub-groups), as help.tallysolutions.com/payments-and-receipts-tally
  and /record-contra-entry (read 2026-11-27) describe; the help pages do not say where the list opens,
  so it opens under the field as every other list.
- Fixed in s22: the top bar fits the 800 point minimum window; chips that do not fit are left out
  and the search words are truncated, rather than drawn past the right edge.
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
- **DELIBERATE** the e-Invoice cell shows the API's answer: "JSON ready (INV-01)" only when the invoice
  mapped to the schema and passed the local check, "Not ready" with the API's reason (the mock's
  `sales-invoice.einvoice-not-ready`), "Not applicable: B2C" for an unregistered buyer, a dash until the
  engine has answered. Source: NIC schema page (einv-apisandbox.nic.in/version1.03/generate-irn.html,
  read 2026-11-26). The sample company and its registered parties have two-line addresses and PIN
  codes (synthetic street lines, PINs in each city's India Post range), so the default invoice says
  "JSON ready (INV-01)" as the mock does; the printed paper and the mock's paper both carry the street,
  city, state and PIN. The mock's not-ready state is a buyer with no PIN: the capture removes Shree
  Ganesh Traders' PIN from a copy of the sample (`Fixture::GodavariBuyerNoPin`), and the reason is the
  API's: "the buyer Shree Ganesh Traders has no PIN code (Pin is required)".
- **NOT BUILT** in accounting and as-voucher modes the app still draws the Buyer's order no. and
  e-Invoice cells (the mock's `sales-invoice.as-voucher` draws neither); the cell then says "Not ready:
  an accounting invoice has no items to list (ItemList needs HSN codes)", which is the API's true
  answer. Hiding the cells in those modes is a layout change left for the mock and app to agree on.
- **NOT BUILT** the mock's "upload: v1.1" note is shown only beside "JSON ready"; upload itself has no
  network and is the user's step on the IRP bulk-upload page (help.tallysolutions.com/generate-e-invoice-offline-tallyhelp/,
  read 2026-11-26). Credit and debit notes, accounting invoices, reverse charge, UTGST and cess invoices
  are `Unsupported` for e-invoice JSON with the reason; no document type other than INV is written.
- **DELIBERATE** Ctrl+E on a saved sales invoice's voucher view exports its e-invoice JSON to a chosen
  file and says where in a note (the mock's `balance-sheet.voucher-einvoice-exported`); the key bar keeps
  the mock's generic "Ctrl+E Export". The export is covered by headless tests
  (`tests/s18_einvoice_export.rs`, and the sample's SI/0405 in `ledgerly-sample/tests/s18_einvoice_pins.rs`).
- **NOT BUILT** the accounting product exports from a list of vouchers (Alt+Z, Send for e-Invoicing,
  Offline Export, files split under 2 MB; read 2026-11-26): this build exports one voucher at a time.
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
- **DELIBERATE** a typed name that is no item stays typed in its row, and Enter is refused ("No item
  matches; create it on the masters screen."): no page read documents a free-text item on a voucher line
  (re-read 2026-11-26: help.tallysolutions.com/tally-prime/purchase-process/purchase-goods-and-services-tally/
  and the sales pages document Alt+C creating a master on the party field only). So the quantity and rate of
  that row cannot be typed, and the mock's `sales-invoice.validation-error` row 3 now shows the name alone
  (was "Drip kit deluxe", 4, 2,200.00: unreachable). The app lists both reasons (2 things to fix) as the
  mock does. Left over: the refusal's amber note stays under the red box, the Save button is disabled while
  a line is wrong, and the tax box shows dashes where the mock shows the valid line's partial total.
- **DELIBERATE** the unknown-item error ends "create it in Masters", where the mock said "with Alt+C": the
  pages read document Alt+C on ledger fields only and the screen cannot make an item, so the mock's
  sentence would promise what the app does not do (mock corrected in round s17).
- **DELIBERATE** the period-locked banner reads "lock date"; the date and Sales ledger are bare
  inputs; Disc % and Typed amount hide by default; there is no Discard button (Esc asks).
- **DELIBERATE** the focus ring is 2 points outside in the accent (index.html:65); hc widens it.

## Review queue, AI and privacy, Ask, quick entry, bill photo

- **NOT BUILT** with AI out of the release (1.0.x) the queue lists bank matches and Rule D-02
  duplicates only; the sidebar count stays withheld. The later-release queue (`ai.review.default`,
  round s21, `release::set_ai_enabled(true)`) lists stored assistant drafts first, with Y accepting
  through the engine and N storing the rejection in the AI activity log; new-ledger suggestions and
  2B explanations are still not listed (no assistant produces them yet).
- **NOT BUILT** `ai.review.default`: the Rupa Polymers draft shows Amount "none" where the mock shows
  80,240.00. The engine blocks it: Rupa Polymers is bill-wise and the draft carries no bill
  reference ("the assistant does not propose bill references"). Settled with the bill photo round
  (a New Ref named by the supplier bill number, as the voucher form offers), not by the queue.
- **NOT BUILT** `ai.review.default`: Enter on a draft row says the draft's own screen is not built
  yet. Plain-words entry (round C) takes a new sentence, not a stored draft, so it is not opened
  from the queue; the bill photo screen is round D.
- **NOT BUILT** `ai.review.default`: the title bar's AI chip reads "AI: not set up" (the fixture
  company stores no tier); the mock's reads "AI on this computer" (AI settings round).
- **NOT BUILT** Home rows do not open their proposal; the empty state's "Read a bill" (Ctrl+Alt+O).
- **DELIBERATE** no optional-voucher list (help page "Display Optional Vouchers",
  help.tallysolutions.com/docs/te9rel66/Reports/MIS_Reports/Display_Optional_Vouchers.htm).
- **DELIBERATE** the mock's `ai-settings.local` and `ai-settings.cloud` states are removed for 1.0.0;
  the screen is `ai-settings.off` only, with the two cards disabled. Their layout returns with the AI.
- **NOT BUILT** (1.0.x) the cloud panel, the local model rows, per-feature switches and AI event
  rows are later-release only. With AI on (round s21 B, `ai.ai-settings.local`, `.cloud`,
  `s21-b-sheet-1.png`): the Model file row (Ctrl+O chooses a file; only its path and size are
  read), the provider rows from the company's audited settings with the key state read from the
  key vault (the OS keychain in the real window), clickable per-feature switches stored as
  settings, and the log's When/Feature/Input/Output/Your decision rows from `AiActivityRecorded`.
- **NOT BUILT** `ai.ai-settings.local`: the log holds the fixture's one draft where the mock shows
  five rows (one per feature); the others appear as rounds C-F record them. The Output column says
  "Draft purchase" without the mock's line count, and the Input column shows "(hash 9f3c…)" where
  the mock says "(image hash 9f3c…e1)".
- **FIX** (s21, `s21-k-sheet-1.png`) `ai.ai-settings.cloud-edit`: Ctrl+Enter on the cloud panel
  opens the provider/key form (mock state added first): request format, endpoint or base URL, a
  model per feature, and a masked key field that is never filled back from the vault. Ctrl+A saves
  (the provider as audited settings, the key to the vault only), Alt+D deletes the key, Esc closes
  the form. Keys as TallyPrime documents them (Ctrl+Enter "To alter a master", Ctrl+A "To accept
  or save a screen", Alt+D delete, Esc back; help.tallysolutions.com/keyboard-shortcuts-tally-prime/,
  read 2026-11-27). The API refuses a non-https endpoint, a format other than the two it speaks
  and an empty key; tests and captures use `MemoryVault` only, and nothing opens a connection.
- **DELIBERATE** (owner, 2026-11-27) the cloud provider is the Anthropic Messages API with the
  user's own key or an OpenAI-compatible chat endpoint given as a base URL plus key; no other.
  With Messages and no model named, bill reading uses `claude-sonnet-5-5` and plain words
  `claude-haiku-5-5`; the cloud panel shows the model each feature is sent to. The cloud panel's
  rows are now Format, Requests go to, Bill reading, Plain words, Other features (mock changed
  first); the fixture's chat endpoint is a base URL.
- **DELIBERATE** (owner, 2026-11-27) no local model for now: the reader and language rows stay
  "Not installed"; no runtime is linked and nothing is downloaded. Cloud is the only real provider.
- **NOT BUILT** the cloud assistant (`Api::cloud_assistant`) reads no bill image: an image cannot be
  redacted, and the per-document confirmation (`purchase-bill.cloud-consent`) is not built, so the
  `claude-sonnet-5-5` default is stored and shown but not yet used. Open question for the owner:
  should a cloud bill read send the image after a per-document confirmation, or the local OCR text
  (ocrs) redacted?
- **NOT BUILT** the app does not yet attach the cloud assistant when a company with the Cloud tier
  opens (the AI-in-release switch stays false); tests build it through the API with a recording
  transport.
- **NOT BUILT** the form's text fields are egui's single-line edits, shorter than the mock's
  `.fld input` boxes.
- **NOT BUILT** `ai.ai-settings.cloud`: the Removed before sending card keeps the 1.0 app's wording
  ("Bank account numbers and IFSC codes", "PAN, GSTIN and Aadhaar numbers"), which names what the
  redaction module removes; the mock's shorter rows omit IFSC and GSTIN. The mock's table switches
  have no width (a mock CSS gap: `.switch` is an inline span), so its On columns look empty.
- **DELIBERATE** no tier is chosen by default; the log headers are the company file's columns; the
  "Current" badge is a border only; the report list shows for every tier.
- **NOT BUILT** (1.0.x) plain-words entry drafts nothing. With AI on (round s21 C,
  `ai.quick-entry.proposal`, `.unknown-ledger`, `.loading`, `.rejected`, `.not-installed`,
  `s21-c-sheet-1..3.png`): the sentence goes through the API's one gate (tier, feature switch,
  provider status) to the provider and the real `nl_entry` reader; the draft is stored (it also waits
  in the review queue), checked by the engine with today offered as the date, and drawn from the
  engine's voucher. Ctrl+A posts through the engine, Alt+D records the rejection; both are in the AI
  activity log. With no provider (this build) the box is off and "No local model can run." offers F5/F6.
- **NOT BUILT** `ai.quick-entry.hinglish`: the reader drafts no account from "NEFT", no bill reference
  ("SI/0401 ke against") and no date from a bank line, so the state is not captured.
- **FIX** (s21 follow-up, `s21-qe-sheet-1.png`) `ai.quick-entry.proposal`: the account's balance box
  ("<ledger> now" and "After (engine)", both from the trial balance through `Api::balance_change`;
  the capture's Saptagiri figures, 7,27,524.50 and 7,02,524.50, are the engine's and equal the
  mock's), the narration in a field, the lock-date check line ("21 Nov 2026 is after the lock
  date."), the "Try" sentences (a click reads one) and "High confidence" after the bars (mock `conf`,
  index.html:962; the review queue's band reads the same way now).
- **DELIBERATE** `ai.quick-entry.proposal`: the source chip says "Test provider" where the mock says
  "Local model": the captures run the fixed-answers provider, never a model, and the chip names the
  provider that answered. The balance box names the whole ledger ("Saptagiri Commercial Bank A/c
  now"); the mock's "Saptagiri" is a short form no master stores. The narration field is read-only:
  edits go through Enter (the voucher form), so the draft and what was accepted stay one thing.
- **NOT BUILT** `ai.quick-entry.proposal`: no PY/0315 duplicate warning (no engine check finds an
  earlier voucher for the same rent; the assistant may not assert one) and no "Why Rent and not
  Sharma Properties?" line (the reader explains nothing); "Sharma Properties" is not marked in
  "Read as:" (the reader reports source ranges for the amount and ledger names only); the Prop
  footer keeps the kit's Edit, Reject, Accept order (Accept last keeps the disabled state of a
  blocked draft from greying the other two in a wrapping row).
- **NOT BUILT** `ai.quick-entry.unknown-ledger`: the Dr ledger is red text, not a picker; no Alt+C to
  create it; the check line is the engine's ("create these ledgers first: Ramesh") where the mock
  says "Dr ledger is missing, so the voucher cannot balance yet."; no "Cash balance after paying"
  line (DELIBERATE: the engine computes a balance only for a voucher it can check, and a draft with
  a missing ledger is none); the narration and the lock line show here as on the proposal, where the
  mock leaves them out; the confidence reads Medium (the recorded answer's 800 per mille) where the mock shows Low;
  the Cr amount is tagged "your words" since the engine made no voucher to balance.
- **NOT BUILT** `ai.quick-entry.rejected`: no reason buttons (Wrong ledger, amount, account); the
  rejection is stored without a reason. Accept stays on the screen with "Payment <number> posted."
  where the mock goes to the day book. Enter "Edit in the voucher form" says it is not built yet.
- **DELIBERATE** `ai.quick-entry.accepted` stays on the screen with the card marked Accepted and
  "Payment PY/0319 posted." (the mock was changed to match in round C); `.idle` and `.read-failed`
  were added to the mock in round C, the failure text being the reader's own words.
- **NOT BUILT** `ai.quick-entry.loading`: Esc does not cancel the read; the key bar lists the idle
  keys. The "Try" sentences of the `ai=on` mock are the two the reader drafts (the Mehta Agro one
  needs a bill reference, which the assistant never proposes; the mock was changed first). Hindi words for the new lines are unsourced.
- **FIX** (round E) `ai.ask.idle`, `.result`, `.unsupported` and `.failed` are built on Ask your
  books with the assistant (mock states added first; `s21-e-sheet-1/2`). `Api::ask_books` runs the
  question through the gate, checks the provider's `ReportQuery` against the question (dates and
  ledger names it states), resolves it to an engine report (a ledger's vouchers need exactly one
  ledger; a ledger named for a whole-books report is refused, never dropped) and runs it with
  `Api::report`. The card draws the engine's `Report` only: the chips are its title and dates,
  every figure a cell. Ask and the palette share one default-period rule
  (`ledgerly_api::default_report_params`). Enter opens the full report; Alt+D dismisses.
- **DELIBERATE** `ai.ask.result`: the mock's earlier result (receivables overdue more than 30
  days, sorted by days overdue) needs a filter and a sort a `ReportQuery` does not carry; the
  `ai=on` result is Bills receivable as the engine opens it, and a report wider than four columns
  shows its last column and says how many more the full report has.
- **NOT BUILT** `ai.ask.result` Ctrl+A "Save as report" (shown blocked with the reason), a drill
  from a row of the card, and an Alt+D that logs the reading as wrong (Alt+D only dismisses; no
  AI event kind records it). `ai.ask.loading` is not captured (it lasts one frame with the test
  provider) and Esc does not cancel a question in flight.
- **NOT BUILT** `ask.low-confidence`: a `ReportQuery` has no confidence and no second reading.
- **NOT BUILT** the lead words "Unsupported." and "Not read." are not bold in the app's note, and
  the provider chips in the card head are plain text, as on the plain-words card.
- **DELIBERATE** an ask the engine cannot run is logged as `ReadFailed` with a reason starting
  "Unsupported" rather than a new AI event kind, so the stored event format is unchanged.
- **NOT BUILT** `gst.recon-explain`: the mock's card is an explanation in words around figures
  bound to the engine, plus a draft fix. None of the four typed proposals (`VoucherDraft`,
  `MatchSuggestion`, `ReportQuery`, `MappingSuggestion`) carries an explanation, and an
  explanation the provider writes is prose, which may state a number the engine did not compute.
  Building it needs a new typed proposal (the cause chosen from a fixed list the engine checks,
  with every figure the engine's), which is a change to the frozen proposal set for the
  coordinator to decide. The review queue keeps no 2B row for it.
- **DELIBERATE** (owner, 2026-11-27) OCR is `ocrs` (MIT OR Apache-2.0; `default-features = false`
  with `onnx`, and `rten` with `onnx_format`; `cargo deny check` passes). `ledgerly_assist::ocr_models`
  names the two model files and their fixed URLs (from the ocrs 0.14.0 crate's
  `examples/download-models.sh`), says Not installed while they are missing or do not load, and
  downloads only on request, keeping a file only when its SHA-256 matches. Nothing was downloaded.
- **FIX** (owner allowed one fetch, 2026-11-27) the OCR model checksums are recorded: the SHA-256
  of the two files the coordinator fetched from the fixed URLs (2,499,479 and 9,713,177 bytes). A
  download keeps a file only when its hash is that one, and stops at the first that differs.
- **DELIBERATE** (coordinator, 2026-11-27) ocrs gives no confidence, so every value it reads
  carries the lowest confidence, "needs review" (`ledgerly_assist::review::NEEDS_REVIEW_PERMILLE`,
  0). No figure is invented. Such a value is kept only when the person confirms it
  (`DraftEdits::confirmed`, logged in the accept's human edits as `confirmed`); while any read field
  is unconfirmed the engine check blocks on it, so a printed total is never silently left out of
  the comparison. `OcrModels::read_png` reads a PNG with the loaded models (boxes from the
  characters, empty lines left out). Checked once by hand with the owner-fetched models on
  2026-11-27: a bill drawn in the reader's `Label value` layout came back line for line and the
  reader drafted the drawn values (fixture `ledgerly-assist/tests/fixtures/ocrs/bill-left.*`).
- **FIX** (round G) `ai.purchase-bill.needs-review` is built (mock state first; capture fixture
  `GodavariBillOcrs`, the bill fixture at the needs-review confidence; `s21-review-sheet-1/2`).
  Each value read by ocrs is tagged "needs review" with a Confirm button until confirmed, then
  "you confirmed"; the check counts the values still waiting and Accept stays blocked on them. A
  line that names no ledger is chosen in its select and is not counted as a value to confirm (as in
  the mock). The card now always shows the supplier GSTIN and the printed tax, as read or "Not
  read" (mock `ledger-lines` and `needs-review` updated): before, a read GSTIN or tax that needed
  review had no Confirm anywhere on screen, so the draft could never be accepted.
- **NOT BUILT** a bill whose amounts are printed right-aligned, apart from their labels: ocrs
  returns the labels and the amounts as separate lines (fixture `bill-right.*`, observed), and the
  reader reads `Label value` on one line only, so it reads no amount from such a bill.
- **FIX** (round D) `ai.purchase-bill.choose`, `.reading`, `.ledger-lines`, `.rejected` and
  `.error` are built on the bill photo screen with the assistant (mock states added first;
  `s21-d-sheet-1/2`). The photo is drawn with a box and label on each value the reader took; the
  taxable line has a ledger list the person chooses from (`DraftEdits::line_ledgers`, logged as a
  human edit); the engine total is the engine voucher's party line, shown only when the engine
  makes the voucher, never a sum on screen.
- **NOT BUILT** `purchase-bill.proposal`, `.low-confidence`, `.mismatch`: the mock drafts item rows
  (qty, rate, items matched to masters); the bill reader reads no item rows yet, so the app drafts
  ledger lines (`ai.purchase-bill.ledger-lines`, added to the mock for this). Ctrl+I "Show source"
  and Enter "Next field" are not built.
- **NOT BUILT** `ai.purchase-bill.accepted` capture: Accept is reached only after choosing a ledger
  in a list, which the capture tool cannot drive; it is covered by the app test
  `choosing_the_purchase_ledger_lets_the_engine_balance_and_accept_post_it`. On the sample company
  the Rupa Polymers bill would then stop at the engine's Unsupported verdict (a supplier outside
  the company's registration state, `validate::check_supplier_state`) and Rupa is bill-wise (a
  draft proposes no bill allocations), so Accept stays off there; recorded, not hidden.
- **NOT BUILT** the place of supply is not asked on the bill screen, so a GST purchase draft in a
  registered company stays Unsupported ("the place of supply is never guessed"); an owner/design
  decision whether to take it from the supplier master.
- **NOT BUILT** `purchase-bill.duplicate` (no engine check finds an earlier read of the same image or
  bill number), `.cloud-consent` (cloud provider refuses documents; owner decision on cloud), JPEG
  and PDF reading (PNG only: JPEG needs the `image` crate's jpeg feature and a cargo deny check;
  PDF needs a renderer; both refused with a typed reason, mock error state changed to say so), and
  keeping the image with the voucher (no image store; the choose text says so, mock changed first).
- **NOT BUILT** Esc while reading is honoured only before the provider runs (`BillRead::Cancelled`);
  a read already past the provider stores its draft, and the screen says it waits in the review
  queue. No app test drives Esc mid-read (it races the worker).
- **NOT BUILT** the AI-off note's AI and privacy button is a secondary button; the mock's is a ghost
  (the kit has no ghost `ButtonKind`).
- **DELIBERATE** Ask's box holds a hint and is disabled and the key bar lists only the name, because
  nothing would answer a typed question.
- **NOT BUILT** `purchase-bill.offline-ai` has no form or image beside it and its head lacks "Read
  another" (Ctrl+Alt+O); the key bar is named "Bill photo".

## Setup (`setup.*`)

- **NOT BUILT** the Keyboard step's preset is shown but not stored; the backup folder is not saved
  and no backup runs on a schedule.
- **DELIBERATE** release 1.0.0 ships without the AI assistant (owner decision of 2026-11-26). The AI
  step selects Off; On this computer and Cloud are shown disabled with a "Not in this release" tag
  and the line "AI arrives in a later version. This company stays Off.", as the mock does with
  `AI_IN_RELEASE=false`. Nothing but Off is stored by the wizard. One switch, `release::AI_IN_THIS_RELEASE`,
  brings the AI screens back; the sidebar's Plain-words entry, the palette routes and the purchase
  form's Bill photo button and Ctrl+Alt+O are hidden until then.
- **DELIBERATE** the import card on Start opens the Import screen with no company open; that screen
  says to create or open one first and offers Back to setup, as the mock's `import.no-company` does,
  so nothing can finish wrongly.
  Its hint says so: create the company first, then import a whole company from a running app or
  masters and vouchers from an XML file.
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
  Sales invoice because it has no such drill. Ctrl+P on an entry screen (every voucher screen and the item
  invoice) previews the voucher that screen saved last, headed the same ("Journal JV/0033", "Saved · date"),
  with the entry screen's own crumb; Esc returns to the empty next voucher (tests/s18_entry_print*.rs).
  Source: help.tallysolutions.com/print-documents/ and /print-invoices-reports/, read 2026-11-26.
- **DELIBERATE** the product has no voucher alteration screen yet, so Ctrl+P prints only the voucher this
  screen saved; unsaved lines get "Save the voucher first with Ctrl+A", a screen that saved nothing says so.
  Printing a voucher opened from a list is the voucher view's Ctrl+P (above).
- **DELIBERATE** Alt+P is not bound: the help page lists it as the Print menu (printer set-up), not as
  print-current (help.tallysolutions.com/print-invoices-reports/, read 2026-11-26).
- **NOT BUILT** "print after saving" (a voucher-type option in the accounting product): `VoucherType` has no
  such field, the voucher-type master has no screen or mock state to set it, and adding one changes the
  stored schema; so it is open, not decided. An accounting invoice previews as "not
  printed" with the print crate's own reason (it has no item table to lay out).

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
- Fixed in s22: masters and transactions import from an XML file into the open company (Alt+O, the
  palette's "Import masters or vouchers from a file", the import screen's file form), with the
  duplicates choice (Combine Opening Balance, Ignore Duplicates, Modify with New Data) and a summary of
  created, altered, combined, ignored and not imported, every exception listed
  (help.tallysolutions.com/import-data-from-xml-or-json and /import-data-faq, read 2026-11-27).
- Fixed in s22: the "Overwrite voucher when a voucher with same GUID exists" choice is offered for a
  transactions file, default No (the import FAQ, help.tallysolutions.com/import-data-faq/, read
  2026-11-27, describes both settings but states no default; No is the coordinator's brief). With No a voucher whose GUID is in the company is posted again as a new voucher, with
  a warning, and a later import overwrites the newest copy; with Yes it alters the voucher in place.
  So importing the same file twice is safe only with Yes: with No, the default here, the second
  import duplicates the vouchers that the engine accepts again.
- **DELIBERATE** with overwrite No, a second import of a sales voucher whose New Ref bill name is
  already on the party is refused by the engine ("bill ... already exists") and listed as an
  exception; the other product's behaviour for that case is not in the pages read.
- Fixed in s22: the file form asks "Take a backup of the company data before importing", Yes or
  No, for both masters and transactions, as the import help page's steps do ("Select Yes to take a
  backup of the company data before importing", help.tallysolutions.com/import-data-from-xml-or-json,
  read 2026-11-27). The page states no default, so the form has none: Import refuses until one is
  chosen. With Yes the company is backed up first and the summary names the backup; if the backup
  fails nothing is imported.
- **DELIBERATE** the backup goes to a `Backups` folder beside the company file, with the app's
  ordinary backup name; the help page does not say where the other product puts it. The name is to
  the second, so two backups of one company in the same second fail (and so the import does not run).
- Re-importing the same file: choose Overwrite = Yes. The import FAQ says of a file imported again
  and again that "transactions will get duplicated" (help.tallysolutions.com/import-data-faq/, read
  2026-11-27); here, with Overwrite = No every voucher in the file is posted again. The file form
  says this under the GUID choice.
- **NOT BUILT** voucher types named in a file are not created, a voucher of an unknown type is an
  exception.
- Fixed in s22: a ledger created by an import under Sundry Debtors or Sundry Creditors (or a group
  under them) with no ISBILLWISEON keeps balances bill-by-bill, as one created by hand under those
  groups (help.tallysolutions.com/set-up-party-ledger/ and /sample-xml/, read 2026-11-27); an explicit
  ISBILLWISEON wins. Bill allocations on a ledger that is not bill-wise are not imported, with a note
  (the TDL appendix lists BILLALLOCATIONS only for bill-wise ledgers; derived). A bill-wise party line
  with no allocations goes On Account, with a note (receivables-payables-tracking-methods; derived).
- Fixed in s22: a new company starts with the predefined Cash ledger under Cash-in-Hand, reserved
  so it cannot be deleted (help.tallysolutions.com/ledgers-in-tallyprime/, read 2026-11-27). A
  whole-company import into a new company takes the export's own Cash in its place. Company files
  made before s22 have no Cash and are not migrated.
- Fixed in s22: a new company also starts with the predefined Profit & Loss A/c, under Primary
  (`Ledger::group` is `None`; no other ledger may sit there), reserved so it cannot be deleted but
  can be renamed (help.tallysolutions.com/tally-prime/accounting/ledgers-and-groups-in-tallyprime/,
  read 2026-11-27: "grouped under the Primary group", "cannot be deleted but it can be altered",
  takes adjustment journals, carries the previous year's profit or loss as its opening balance,
  shown in the balance sheet under liabilities). The balance sheet adds its closing balance to the
  computed Profit and Loss A/c row; the trial balance lists it as its own line after the primary
  groups when it is not zero; the profit and loss statement leaves it out. The masters list, the
  ledger view, the voucher form's group column and the palette show its group as "Primary".
  A file import matches it by name: "Ignore duplicates" keeps it, "Combine opening balances" adds
  the file's opening (last year's profit) to it, "Modify with new data" takes the file's opening,
  and a voucher that posts to "Profit & Loss A/c" posts to it. A whole-company import keeps the
  export's own Profit & Loss A/c (under Primary, reserved) in place of the new company's, so a
  journal to it imports; an export without one keeps the new company's. Company files made before
  s22 have no Profit & Loss A/c and are not migrated.
- **NOT BUILT** a Profit & Loss A/c renamed in the other program: a file import matches the ledger by
  name, so a file's ledger under Primary with another name is an exception (it has no parent group
  here), and so is each voucher that posts to it; a whole-company import refuses such a ledger
  (LedgerWithoutGroup). Neither guesses which ledger was meant.
- **DELIBERATE** the trial balance shows Profit & Loss A/c as a top-level ledger line after the
  primary groups, not inside one, since it is under no group; where the other program places it in
  its own trial balance was not observed.
- Fixed in s22: Alt+O is the keymap's "Import masters or vouchers from a file" in both presets and
  is on the keyboard-map screen under Go anywhere. Bill reading on the purchase form is Ctrl+Alt+O
  (s21, below), so no screen binds Alt+O to anything else.
- Fixed in s22: Enter imports the file, and the key bar says "Enter: Import the file", while the file
  form is where the user works (opened with Alt+O, the path has the keyboard, or a choice or Choose
  file was used last); a field of the one-step form takes Enter back.
- Fixed in s22: the file import's summary and "Error:" lines are in Hindi in the mock (from the
  app's `import.file.*` strings, which `gen-hi.py` now copies into `hi.js`) and in the app; the
  Hindi file states are captured in both. The engine's own reasons stay in English. The Hindi text is
  unsourced (`hi.toml`).
- Fixed in s22: engine messages count lines from 1 ("line 2" is the second line); the line fields
  of the errors stay 0-based.
- Fixed in s22: the bill messages name the ledger ("bill 'INV-1' on ledger 'Debtor A' already
  exists"), and the unbalanced, bill-allocation and over-settle messages print amounts in the app's
  money format (10,000.00), not debug form. The reserved-ledger and reserved-group messages and
  "unknown ledger" still print an internal name or id.
- Fixed in s22: the "Read but not imported" and warning lists of a file import show the object and
  the note in their own columns for every row; a note whose text held ": " was split in the wrong
  place before.
- Fixed in s22: the Hindi mock keeps keycaps as written ("Enter", not translated), as the app does.
- Fixed in s22: Alt+O while the import screen is open goes back to the file form (scrolls to it and
  gives the path the keyboard) instead of opening a second import screen.
- **FIX** (s21) bill reading moved from Alt+O to Ctrl+Alt+O in mock and app, so Alt+O is Import on
  every screen, the purchase form included. Checked against
  help.tallysolutions.com/keyboard-shortcuts-tally-prime/ (read 2026-11-27, page updated
  2026-09-24): every Alt+letter is bound on some screen there (Alt+O is the Import menu), and the
  documented Ctrl+Alt letters are B, F, I, N, R, T, U and W, so Ctrl+Alt+O clashes with none.
  Ctrl+Alt+O is also fixed in the purchase form, not in the keymap.
- **NOT BUILT** Telugu names draw in Noto Sans Telugu, but egui does not shape Indic conjuncts, so a
  name such as a ledger in Telugu can show unjoined letters.

## Captures with no real cause

- **DELIBERATE** `gst.gstr1-unsupported`, `gst.gstr1-failed`, `audit.failed`, `profit-and-loss.print-error`
  are not captured: the sample books never make the engine refuse; inventing a failure would be a
  wrong picture.
- **DELIBERATE** the open-company busy line ("Working on the company file...", `shell/welcome.rs`) has
  no mock state; the masters balance hint is hover text because the mock shows dashes.

## Real launch path (cold start, `s15-cold-start-sheet.png`)

Seen in the release binary under a nested X server with an empty config and data directory.

- **DELIBERATE** the title bar shows today's date and "AI off" on a cold start; the fixture
  captures pin the date to 21 Nov 2026. A company file stored with the Local tier by an older build
  opens and shows AI off (`release::effective_tier`).
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
