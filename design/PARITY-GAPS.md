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

- **NOT BUILT** Export has no PDF (the mock offers "PDF, XLSX or CSV", index.html:1624); PDF comes
  with report printing, which another round is building.
- **FIX** Export of the balance sheet writes the engine's primary-groups-only condensed report, not
  the sub-group view on screen.
- **FIX** the audit screen's own table still uses `TableBuilder` (`screens/audit.rs`); every other
  report, list and invoice table draws the mock's 34-point `table.t` rows.
- **DELIBERATE** F2 says "Period" and opens a from-to editor; the mock says "F2 Date".
- **DELIBERATE** the as-at date is the last voucher date, 20 Nov, where the mock reads its own clock,
  21 Nov (help page "Trial Balance", help.tallysolutions.com/tally-prime/accounting-financial-reports/trial-balance-tally/,
  read 2026-11-21; `navigate::default_period`).
- **DELIBERATE** section and group names are the engine's (decision 2 of the 2026-11-22 parity report).
- **DELIBERATE** the line under the statement keeps the engine's notes; the mock's "Difference ..."
  and "Profit to date ..." lines would be a second computation in the screen.
- **DELIBERATE** the provisional reason names the item, quantity and voucher the engine gives.
- **FIX** (in the mock) the provisional figures differ: the app's fixture posts the real vouchers
  that make stock negative, while the mock overrides the stock only; post them or say so.
- **NOT BUILT** the voucher view's Audit card shows the engine's trail only: no attachment, no bank rule.
- **DELIBERATE** the capture company (Fictional Traders) is small, so its statement is shorter.

## Profit and loss and report tables (`profit-and-loss.*`, `trial-balance.*`, `outstanding.*`, `stock-summary.*`, `audit.*`)

- **DELIBERATE** the two-sided trading account and profit and loss account follow "How to View Profit
  and Loss Statement in TallyPrime" (help.tallysolutions.com/profit-and-loss-statement-tally/, read
  2026-11-25); the mock draws Income and Expenses and must change to the two-sided form.
- **DELIBERATE** reports open at the last voucher's date, 20 Nov (the Trial Balance page above).
- **NOT BUILT** `profit-and-loss.print-preview`, `.print-saved`, `.print-failed` and `*.print-busy`:
  another agent is building the preview.
- **DELIBERATE** `audit.unsupported` is a table here: the engine returns an `EditsAfterLock` table;
  capturing the mock's claim would be a wrong picture.
- **DELIBERATE** the audit edit log lists the engine's audit events (Seq, Date, Time, User, Action,
  Object, Reason, Field, Before, After), not the mock's vouchers.
- **FIX** the integrity certificate is a table of the engine's result, not a card; the head hash is
  clipped by its column.
- **FIX** outstanding bills: Overdue names the engine's ageing bucket and there is no red ink on an
  overdue bucket; the lines are in the engine's party order (the mock's are by due date).
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
- **FIX** F2 offers the shown day; TallyHelp says F2 first offers the day after the current date
  (only the Tally.ERP 9 page was read; read the TallyPrime page, then change).
- **FIX** (in the mock) the mock marks PY/0316 as a bank-rule draft; the sample posts it as typed.
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
- **FIX** "monthly" in the subtitle comes from the shell, which always opens one calendar month
  (`gst_return_period`); QRMP needs a registration filing frequency first.
- **DELIBERATE** the capture company's GSTIN and figures differ from the mock's Godavari Drip; the
  sample's GST ledgers are one per head and its GSTINs use the fake PAN pattern `ZZZZZ####Z`.

## Masters (`masters.*`)

- **FIX** the create ledger form is longer than the mock's compact one (opening balance, cost
  centres, party, bank, tax and GST details follow the TallyPrime ledger master).
- **FIX** the GSTIN error reads "check character is 7, expected P; the GSTIN has a typing error"
  (the engine's `GstinError`); the mock words it differently. Pick one wording.
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
- **FIX** the "Missing: add HSN" badge has no alert triangle (the kit has no such `BadgeKind`).
- **NOT BUILT** Alt+C on the item invoice's item field (index.html:390, 1253): no TallyPrime page read
  documents it.
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
- **FIX** no "Reconciled" or "Not yet reconciled" card beside the statement (index.html:1746); the
  engine's difference can drive it ("Review suggestions" waits for the assistant).
- **DELIBERATE** "Reconciliation statement as of 20 Nov 2026" stays over the card: the as-of date is
  the engine's, and the subtitle's month alone would name the wrong day.
- **NOT BUILT** the lines table lacks Exact, Rule and "AI suggested" badges and a confidence meter.
- **DELIBERATE** the key that changes account is C, not F4, which is Contra in this app.
- **NOT BUILT** `bank.suggested-ledger` and `bank.ai-suggestions` need the in-app assistant.
- **DELIBERATE** `bank.default` shows the empty state because the fixture has no statement.

## Payment, receipt, contra, journal (`payment.*`, `receipt.*`, `contra.*`, `journal.*`)

- **FIX** `payment.bill-wise` is captured before the narration is typed, so the popup shot shows an
  empty narration; the capture flow should type it first.
- **FIX** the payment shot ends after the empty-row Enter, so it lacks the mock's second hint row and
  the narration holds the keyboard; the capture flow should stop mid-entry.
- **DELIBERATE** the mid-entry lilac fill is not drawn for a row the keyboard is not on: it would mislead.
- **DELIBERATE** the Mode is bare text once set but a box with a list while empty or focused; the
  mock's "NEFT / UPI" names two instruments the engine stores as one each.
- **DELIBERATE** group names keep the sample's capitalisation ("Direct Expenses").
- **DELIBERATE** a prefilled amount is selected when the keyboard arrives; a literal finer than a
  paisa is refused.
- **NOT BUILT** eligible ITC on a purchase: `Api::invoice_gst_with_bills` does not return it for a draft
  and the UI must not compute it.
- **DELIBERATE** the balance line can say "balance not shown": the API answers Unsupported, never zero.
- **NOT BUILT** the credit note hint's figures ("14 Nov, 67 sets invoiced"): no return-against-invoice
  quantity exists.
- **DELIBERATE** a debit note goes back to the supplier, as a purchase return is a Debit Note against
  the supplier in the accounting product; a note is titled "Credit Note" or "Debit Note".

## Sales invoice and item invoices (`sales-invoice.*`)

- **DELIBERATE** a company without inventory gets an accounting invoice
  (help.tallysolutions.com/sales-of-goods-services/ and
  help.tallysolutions.com/tally-prime/sales-process/faq-sales/, both read 2026-11-23).
- **NOT BUILT** As Voucher mode (Ctrl+H, help.tallysolutions.com/sales-of-goods-services/): the GST
  engine refuses it.
- **NOT BUILT** the e-Invoice cell says "Not built yet": nothing maps a voucher to an `EInvoice`.
- **DELIBERATE** the sample GSTIN is synthetic and the rule pack reads "2026.1-slabs".
- **NOT BUILT** the toast has no Undo Ctrl+Z: no undo exists.
- **DELIBERATE** the empty tax box shows dashes, not 0.00, until the engine has answered (never a
  wrong number); the rate shows as typed because rate parsing stays strict.
- **FIX** the party list offers every ledger; TallyPrime's pages do not say which it lists
  (help.tallysolutions.com/sales-of-goods-services/, read 2026-11-23); needs a page that does.
- **FIX** the place of supply is not filled from the party; needs a page that says it is.
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
- **FIX** the redactor does not remove email addresses (the mock's "Removed before sending" lists them).
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
- **FIX** the import card on Start opens a screen that cannot finish with no company open.
- **NOT BUILT** the import screen's step-by-step path (the five-step wizard) is not connected.
- **NOT BUILT** F11 on the Features step: no feature screen exists; rows with no model switch say so.
- **DELIBERATE** the "Company file" field is ours (the wizard writes a file); City is kept as the
  first address line because `Address` has no city field.
- **DELIBERATE** the mock's sample is prefilled and the wizard starts blank; refusals are a toast.

## Print (card P1; another agent owns the preview)

- **NOT BUILT** in-app print preview and configuration; printing to a device; the trial balance,
  balance sheet, profit and loss and day book; any voucher but sales invoices, credit notes and
  debit notes; a UPI QR (no UPI address is held).
- **DELIBERATE** printed text is plain ASCII (the print crate refuses other characters); Ctrl+P is
  not on the key bars, as the mock's (index.html:1634).

## Accessibility, themes, keys

- **FIX** the divider border fails 3:1 non-text contrast in light and dark.
- **FIX** table rows do not grow by the extra rule pixel in high contrast; keycaps do not grow in hc
  (the mock's `kbd` has 2 px borders, index.html:75).
- **FIX** egui's own widgets (the "More master kinds" list) fade when disabled in hc.
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
