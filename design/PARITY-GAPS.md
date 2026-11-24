# Known gaps between the app and the mock

Differences seen in the side-by-side captures (`design/COMPARE.md`) that a parity round left on
purpose or could not close yet. Each entry says where the mock shows it, why it was left, and what
closing it needs. Remove an entry only in the commit that closes it.

Kinds: **bug** (the app is wrong or missing something it should do), **deferred** (waits on a
feature that is not built), **data** (the capture company lacks what the mock's sample shows),
**by design** (we differ on purpose; kept here so nobody "fixes" it back).

## Home (light states round of 2026-11-24)

Captured: `home.empty` (Fictional Traders' masters with no voucher, a second capture company)
and `home.ai-off`. Not captured: `home.loading` and `home.nudge-open`.

- **gap: the new-company card has no icon and no "Import from another app" button.** The mock's
  card (index.html, `SCREENS.home`, `empty`) has a book icon over the heading, icons on its
  buttons and a middle button that opens setup's import. The import wizard is not routed from
  Home, so the button is left out; the icons are not drawn. The note under the card has no info
  icon and is not full width. The line under the title now names the financial year with its
  days, as the mock's does (`home::new_company_line`, tested in `tests/s6_home_empty.rs`).
- **gap: the AI-off note.** The mock's note (`home.ai-off`) starts with a bold clause, has an
  icon and an "AI and privacy" button; ours is the plain note. The KPI tiles' layout differs, and
  the mock shows the chart and the nudges, which are not built (see "Review queue").
- **deferred: `home.loading` is not captured.** The skeleton shows only while a refresh waits on
  the worker, which takes a few frames on the capture company; the harness cannot hold it there.
  Capturing it needs a way to hold the worker (a job that blocks until released).
- **deferred: `home.nudge-open` is not reachable.** It expands a possible-duplicate nudge; the
  nudge rules (D-02) are not built, so Home has no nudge to open.

## Palette (round of 2026-11-23, commit d4f0bcb)

- **deferred: the mock's other commands.** After the four voucher commands the mock's `>` list
  has Read a bill photo, Import bank statement, Import 2B / IMS JSON, Export GSTR-1 JSON, Switch
  theme, Collapse side bar and Back up now (index.html:1110). Ours follows with the keymap's
  other commands; the mock's are not built as commands.
- **gap: a recent GSTR-1 row has no live detail.** The mock's Recent rows say "As at 21 Nov
  2026", "3 sections ready, 1 needs correction" and "Ledger · Sundry debtors · 1,93,500.00 Dr"
  (index.html:1115). A recent balance sheet and a recent ledger now show the date it opens at and
  the engine's balance, fetched on every opening (`palette/recent.rs`). The GST return is a
  screen, not a palette target, so it is never a recent row; its section count is not built.
- **deferred: Alt+C keycap on 'Create ledger "q"'** (index.html:1129). Not drawn because the
  palette takes the keyboard, so Alt+C would not work there. Needs the palette to pass row
  shortcuts through.
- **deferred: Ask rows.** The no-match "Ask: q" row and the "Or" Ask/Draft rows (index.html:1129,
  1127) are left out while AI is off, as the mock's own `tierOff` does. Return with the assistant.
- **data: `#0412` finds no voucher.** The capture company has no voucher numbered 0412, so the
  voucher state shows "No match for "#0412"". Needs a voucher with that number in the capture
  company, or the capture typing a number that exists.
- **by design: Search vouchers detail.** Ours says "Number, party, amount or date" (what our search
  matches, `MatchedOn`); the mock says "Narrations and party names".

## Balance sheet (round of 2026-11-23)

- **gap: Export has no PDF.** The mock's Export offers "PDF, XLSX or CSV" (index.html:1624).
  Ours writes CSV and XLSX; PDF comes with printing (M5; the Print section below says what
  prints). The palette's Export command still
  says it is not available there: Ctrl+E and the button on the report screen export.
  TallyPrime's "How to Export Data" page (help.tallysolutions.com/export-data-in-tally/, read
  2026-11-23) has Ctrl+E (Current) export any open report and lists ASCII, Excel, HTML, JPEG,
  PDF, XML and JSON; it states no default file name or folder. Ours suggests the report's title
  as the name and leaves the folder to the save dialog.
- **by design: Ctrl+E works on the day book.** The mock's day book key bar has no Ctrl+E
  (index.html, `SCREENS['day-book'].keys`), but Tally's page above exports any open report with
  Ctrl+E, so the key works there; it is not listed in the key bar, as in the mock.
- **bug: other tables keep the older row height.** The balance sheet, the day book, the masters
  list and the generic report table (every drill: group summary, ledger vouchers) draw the
  mock's `table.t` lines by hand (34-point rows, rules, the selected bar: `ui::table::paint_row`).
  So does the bank reconciliation statement. The bank lines and invoice tables still use
  `TableBuilder` at 30 points with egui's selection tint; each moves over in its own round.
- **data: the drill reports' columns.** Captured as `balance-sheet.group` and
  `balance-sheet.ledger`. The mock's group drill lists Ledger or group, GSTIN, Opening, Debit,
  Credit and Closing, with "Group total, same as the balance sheet row"; ours is the engine's
  group summary (Particulars, Debit, Credit, a "Net balance" row and Total). The ledger state's
  side card (group, GSTIN, credit days, open bills, Alter ledger) is not built, and the ledger
  list has the engine's columns (Particulars, Voucher Type, Voucher No, Debit, Credit, Balance)
  in the engine's order, not the mock's Date, Voucher, Type, Narration. Each needs the engine's
  report to carry the figures first; the screen computes none. Closed in the S8 round (tested in
  `tests/s8_report_parity.rs`): the drill titles and crumbs read as the group or ledger name
  ("Sundry Debtors"), the subtitle is "As at ... Enter goes deeper, Esc goes back up", Debit and
  Credit figures drop the Dr/Cr word, the repeated Particulars column is not drawn twice, and the
  footnote is small text under the card.
- **needs a decision: sub-groups in the condensed view.** The mock's condensed statement shows
  each top group's sub-groups (Loans > Secured loans; index.html:1588, `depth===0` children).
  Ours shows the primary groups only, which is what the engine's condensed report gives. Before
  changing it, check the TallyPrime help on what its condensed balance sheet lists (not checked
  in this round); showing sub-groups needs the engine, or the detailed report cut by depth.
- **needs a decision: indent past two levels.** The mock indents two steps (`ind1`, `ind2`) and
  stops; ours keeps adding 16 points per level so deeper groups stay apart.
- **by design: F2 says "Period".** The mock says "F2 Date"; ours opens a from-to period editor.
- **by design: section and group names** follow decision 2 of the 2026-11-22 parity report:
  "Liabilities" (not "Equity and liabilities"), "Capital Account", "Profit and Loss A/c".
- **by design: the as-at date** is the last voucher date (decision 1), so the capture says 20 Nov
  where the mock says 21 Nov.
- **by design: the line under the statement** keeps our words (the check, then the engine's
  notes). The mock's "Difference ..." and "Profit to date ... includes closing stock ..." are
  figures the screen would have to work out, which it never does.
- **data: the capture company is small.** Fictional Traders has four primary groups, so the
  capture is shorter than the mock's Godavari Drip statement.
- **closed (S8): a provisional figure overlapped its row label.** The figure is drawn bare, the
  line wears a "Provisional" badge and the reason is said once in a warning note at the top of the
  card (`balance::provisional_reasons`, tested in `balance.rs`). Still different from the mock: the
  banner has no icon, no bold lead-in and no "Open stock items" button, and its words are ours
  ("Some figures are provisional and are marked. Why: ...") because the engine's reason is an
  internal id, not the mock's "Steel pickaxe 2.5 kg shows -30 nos" (next entry). The engine's own
  note under the statement repeats the reason.
- **bug: the provisional reason is not readable.** The engine's reason reads "negative stock:
  item StockItemId(1) first went below zero on 2026-11-10" (`ledgerly-reports`
  `stock_summary/valuation.rs`): an internal id and an ISO date. The mock names the item and the
  quantity ("Steel pickaxe 2.5 kg shows -30 nos"). Needs the report to carry the item's name.
- **gap: the voucher view has no item lines, tax, total or audit card.** `balance-sheet.voucher`
  is now two cards: the header fields (Type, Number, Date, Status, Party) over the ledger lines in
  Debit and Credit columns, and a "Read-only" card. The mock's item table and tax lines need the
  stock lines and GST the screen does not show yet (it says so in the card). No Total row: the
  screen never adds figures (rule C4). The mock's "Audit" bullets (created by, from an AI draft)
  need audit data the voucher view does not read. The subtitle is "Read-only. Esc goes back up"
  because "As at ..." means nothing for one voucher.
- **deferred: `balance-sheet.loading` is not captured.** The card is built (a line saying it is
  computing, then twelve placeholder bars; tested in `report_view::loading_card`) but the
  computing state lasts a few frames on the capture company and the harness cannot hold the
  worker. It has no clock icon (the icon set has none) and the key bar does not switch to
  "Computing / Esc Cancel".

## Day book (round of 2026-11-23)

- **deferred: Entered by and Origin columns, and Ctrl+I Origin** (index.html:1650-1653, 1659).
  The engine records neither who entered a voucher nor whether it came from an AI draft or a
  bank rule (engine item E-4). The table keeps the engine's Status column in their place and the
  footnote says so; Ctrl+I is not offered.
- **deferred: narration under the party** (index.html:1652, the `meta` line). The engine's day
  book has no narration field, so Particulars shows the party alone and every row is one line.
- **closed (S8): the empty state is a card** with the book icon, title, sentence and the four
  voucher buttons (they open the voucher forms); the sentence leaves out the mock's "with who
  entered it" because that is not recorded (E-4). The buttons sit centred; the mock's sit at the
  left of the card.
- **by design: the month view keeps the cursor on the voucher it was on.** `day-book.month`
  (Alt+F2, 1 Nov to 20 Nov) keeps the selected voucher selected when the period changes
  (`report_view::replace_report`), so the cursor is not on the first row as in the mock.
- **needs a decision: F2 offers the shown day.** TallyHelp says F2 in the Day Book first offers
  the day after the current date; ours fills in the day shown. Check the TallyPrime page (only
  the Tally.ERP 9 page was read) before changing it.
- **data: voucher numbers and the sample day.** The capture company numbers vouchers 1, 2, ...
  per type and its last voucher is 20 Nov; the mock shows "SI/0417" and five vouchers on 20 Nov.
- **by design: Status column.** Kept from the engine (it shows a cancelled voucher as such) until
  Entered by and Origin exist.

## GST returns (round of 2026-11-23)

Captured states: `gst.gstr1`, `gst.gstr3b`, `gst.recon-empty`, `gst.gstr1-correction` (on
Fictional Pumps, which sold pickaxes with no HSN). Not captured: `recon`, `recon-explain`,
`recon-error` and `recon-locked`: they need a 2B file, which the app cannot read (no 2B reader,
engine item E-3); `recon-explain` is an AI state.

- **gap: the correction state lists vouchers, the mock lists sections.** `gst.gstr1-correction`
  shows the engine's per-voucher table ("Voucher 3: item 'Steel pickaxe 2.5 kg' has no HSN or
  SAC code") and counts vouchers; the mock lists the return's sections with "12 HSN summary:
  Needs correction" selected and a side card naming the item with a "Fix the item master"
  button. The section table and the button are not built. The capture also holds the pipe
  invoice back because unit `coil` has no unit quantity code: that is the engine's finding on
  the capture data, not a screen difference.

- **deferred: GSTR-1 by section** (index.html:1673-1676). The mock's table has one row per
  return section (4A B2B, 5 B2C large, 7 B2C others, 9B notes, 12 HSN, 13 documents), with
  records, taxable value, tax and a status for each. Its tiles count sections too. The engine
  buckets vouchers, not sections, and `Gstr1` has no per-section totals. The UI may not add them
  up (rule C4). Ours shows the vouchers with the engine's bucket, the tiles count vouchers
  ("vouchers", not "sections"), and the return's own figures sit in a second card. Needs an
  engine section summary with its own tests.
- **deferred: the agreement line under the GSTR-1 table** ("Taxable value agrees with GSTR-3B
  table 3.1(a): ..."). No engine check compares the two, so the line is not drawn.
- **deferred: GSTR-3B rows from IMS and set-off** (index.html:1683-1686). These are "Accepted
  in IMS", "No action yet: deemed accepted", "6.1 Payment of tax", "Left after using accepted
  credit" and "Payable in cash". They need the 2B and IMS match (engine item E-3) and set-off
  (engine item E-1). Ours draws 3.1(a), "In your books" (4(A)(5)) and the blocked or ineligible
  credit, all from the engine's working.
- **by design: a Cess column on GSTR-3B.** The mock has no cess column, but the engine works out
  cess, and a figure is never dropped.
- **by design: our own words in "What happens next" and "Read this before filing".** The mock's
  bullets claim things the app does not do: "Mark the month filed here", "Credit counts only what
  you accepted in IMS" and the set-off order. Ours say what the engine does.
- **deferred: the period button, "November 2026 Alt+F2"** (index.html:1671). The screen cannot
  change its return period, so the button and the key are not shown.
- **deferred: importing a 2B file** (index.html:1692). The mock offers "Import 2B JSON Ctrl+O"
  and "drop it here". There is no 2B reader (engine item E-3), so the empty state says so and
  offers no import button and no key.
- **deferred: Y Take suggestion and X Explain on the 2B tab** (index.html:1721). They need the
  engine's match and explanation (E-3). The key bar offers A, R and P only when rows are loaded.
- **deferred: the GSTR-3B key bar** (index.html:1721). The mock offers J / K, Enter and Ctrl+E
  there. Ours has no rows on that tab and exports only GSTR-1, so it offers just 1 2 3 and Esc.
- **needs a decision: "monthly" in the subtitle.** It comes from the shell, which always opens
  one calendar month (`gst_return_period`), not from the registration. Quarterly filing (QRMP)
  is not modelled; the registration needs a filing frequency first.
- **deferred: the match table's look.** The loaded 2B table still uses egui's grid inside a card.
  It cannot be reached or captured until 2B import exists; restyle it then.
- **gap: Re-apply masters and Export IMS action file.** The view model records both requests
  (`take_reapply_request`, `take_export_request`), but nothing in the shell takes them, so the
  buttons did nothing and are no longer drawn. Wire them to the shell before showing them again.
- **by design: the bucket filter moved onto the tiles.** The mock has no chip row. Clicking a
  tile shows only that bucket in the table; clicking it again shows every voucher. The selected
  tile has the accent border.
- **by design: Export JSON is disabled off the GSTR-1 tab.** The engine exports GSTR-1 only. The
  button says why on hover, and Ctrl+E says why in a notice.
- **data: the capture company.** It has five vouchers in November and the GSTIN
  27ABCDE1234F1Z0; the mock shows Godavari Drip's figures.

## Masters (round of 2026-11-23)

Captured states: `masters.ledgers`, `masters.ledger-detail`, `masters.items`,
`masters.create-ledger`, `masters.gstin-error` (the form filled by clicks and typed text). Not
captured: `suggested-ledger` (needs bank matching to propose a ledger, which is not built).

- **needs a decision: the GSTIN error's words.** Ours says "check character is 7, expected Z; the
  GSTIN has a typing error" (the engine's `GstinError`); the mock says "Check character should be
  Z, not 7. One of the first 14 characters is probably mistyped." `tests/w2_masters.rs` pins
  "expected <character>" in the reason, so taking the mock's words means changing that test's expectation
  on purpose, with a decision, not in a capture round.
- **bug: Save stays enabled with a bad GSTIN, and the field has no red border.** The mock
  disables Save while the GSTIN is wrong and outlines the field in red (`masters.gstin-error`).
  Ours shows the reason under the field and refuses on save.

- **by design: a nil balance is blank, not "0.00 Dr".** The Balance column is the engine's
  trial balance, joined by ledger id (`screens/masters/balances.rs`). The trial balance leaves
  out a ledger with a nil balance, and the screen does not fill in a figure the engine did not
  give, so the mock's "0.00 Dr" rows (Rupa Polymers, CGST input) are blank here. Showing 0.00
  needs the engine to give a row for every ledger.
- **by design: a figure the engine could not give says why on hover.** When the whole trial
  balance or stock summary fails, a note above the list gives the engine's reason. When one
  cell cannot be computed it shows a dash, and its hover gives the engine's reason; a
  provisional stock value's hover says why it is provisional, and so does the Closing stock
  line's. The mock has no such cells.
- **by design: balances are as of the last voucher date**, the date the balance sheet opens on
  (`navigate::default_period`). The mock does not say which date its balances are as of, and
  the list does not say it either; a date in the subtitle would make it plain.
- **by design: the value heading is "Closing value", not "Value at average cost"**
  (index.html:1798). Stock and value are the engine's stock summary, joined by item id
  (`screens/masters/stock.rs`), and the engine values each item by its own method (average cost,
  FIFO, last purchase or standard cost), so the heading is the stock summary's own. The
  "Closing stock" line is the summary's own total.
- **by design: a credit or provisional stock value is marked.** The mock's value is
  `inr(max(qty,0)*cost)`, so stock below zero shows 0.00. Ours shows the engine's figure: a
  credit value keeps "Cr", and a value the engine marks provisional (negative stock) says
  "(provisional)", with the engine's reason on hover.
- **by design: stock is over the stock summary's opening period**, the financial year to its
  last voucher, as the stock summary report opens.
- **by design: an open bill's badge names the outstanding report's bucket, not a day count**
  (index.html:1806: `x.overdue+' days overdue'`). The card is the party's rows of the engine's
  detailed outstanding report (`screens/masters/bills.rs`), which ages a bill into "0-30 days",
  "31-60 days", "61-90 days" or "Over 90 days"; the screen counts no days itself (rule C4), so
  the badge says "61-90 days overdue". A day count needs the engine to give one.
- **by design: open amounts carry their side** ("1,18,000.00 Dr"). The mock's amounts are bare;
  a party's bills can be on either side, so ours keep "Dr" or "Cr".
- **by design: the card has lines the mock does not show.** A bill with no due date shows a
  "No due date" badge, and the party's unallocated amount shows as an "On account" line with no
  badge, as the outstanding report has them. Nothing open says "Nothing is open for this
  party."; a report the engine cannot give says why and estimates nothing.
- **by design: the Balance field sits after Bill-wise and Credit days.** The mock places it
  after City and Bill-wise (index.html:1805); ours keeps the ledger master's field order and
  shows the balance read-only at the end of the party fields, as the list gives it
  (`LedgerBalances`, as of the last voucher date).
- **by design: Alt+D Delete and Alt+Left / Right Change kind stay in the key bar.** The mock's
  masters have only ledgers and stock items; ours keep every master kind TallyPrime has (groups,
  voucher types, units, godowns, batches, cost categories and centres), so changing kind and
  deleting need keys. The other kinds sit in a "More master kinds" list in the page head.
- **by design: "Create {kind}" follows the tab.** The mock's button always says "Create ledger";
  ours names the kind on screen ("Create stock item"), since Alt+C creates that kind.
- **by design: the filter is on every tab.** The mock's items tab has no filter box.
- **by design: Alt+C creates where TallyPrime documents it.** Alt+C is "To create another master
  on the fly while creating masters and vouchers" (help.tallysolutions.com/keyboard-shortcuts-
  tally-prime/). Ours does it on this screen's list (the tab's kind), on the ledger form's Under
  (a new group, and the ledger comes back with it under; help.tallysolutions.com/ledgers-in-
  tallyprime/), and on a voucher's ledger selection fields ("In the Voucher Creation screen, when
  your cursor is on the ledger selection field, press Alt+C", same page): voucher entry's ledger
  lines and the invoice screen's party, sales or purchase ledger and Particulars ledgers.
- **deferred: Alt+C on the item invoice's item field.** The mock creates an item there
  (index.html:390, 1253). None of the TallyPrime pages read documents Alt+C on a stock item
  field, so it stays out until a cited page does; Enter on an unknown item still says to create
  it on the masters screen. The bill photo screen has no fields: it reads no bill (see "Assistant
  screens" below).
- **by design: Enter walks every field, as Tab does.** Enter in a one-line field moves to the next
  field and Shift+Enter to the previous one (index.html:1831, 1344), including the lists and
  switches between them; in a multi-line field (the address) Enter is a new line. Enter on a
  list or switch opens or flips it, as egui's own keys do.
- **by design: the forms have more fields than the mock.** Opening balance, cost centres, party,
  bank, tax and GST details follow the TallyPrime ledger master; the mock's form shows the party
  fields only. They come after the mock's fields, in the same four-column sheet.
- **by design: no Delete button under the list.** Deleting is Alt+D, as on every other list;
  the old Alter and Delete buttons are gone because the mock has none.
- **by design: Enter opens a ledger's vouchers.** On the mock, Enter on a row opens the ledger
  report (`hashFor('balance-sheet','ledger')`) or, for one party, the detail. Ours opens the
  ledger's vouchers for every ledger, and Ctrl+Enter opens the detail (the alter form). Enter is
  not offered on other kinds, which have no report.
- **by design: State is a list, as in TallyPrime, not read-only from the GSTIN** (index.html:1804,
  1812: a read-only state with a "from GSTIN" chip). TallyPrime's State is chosen under Mailing
  Details ("Select the required State", help.tallysolutions.com/tally-prime/vat-masters/india-
  vat-party-ledger-tally/) and altered there ("Update State under Mailing Details",
  help.tallysolutions.com/state-field-required/). The pages read say nothing of a state taken
  from the GSTIN, so ours keeps the list and, left blank, uses the GSTIN's state and says so.
- **by design: GSTIN shows only with Party details on.** The mock's create form has GSTIN, State
  and the footer note from the start, because its sample is a party. Ours keeps party fields
  under Party details (the ledger master's own section), so a blank form has no GSTIN field and
  no footer note until the switch is on.
- **by design: Credit days shows only when Bill-wise is on.** Credit days belong to bill-wise
  details; the mock shows both because its party is bill-wise.
- **by design: Bill-wise is a checkbox, as in TallyPrime** (index.html:1805, 1811: a read-only
  "On" badge). TallyPrime's "Maintain balances bill-by-bill" is a Yes/No set in the ledger
  creation or alteration screen (help.tallysolutions.com/set-up-party-ledger/), and credit
  periods show only with bill-wise on (help.tallysolutions.com/set-up-receivables-and-payables-
  tracking-in-tallyprime/), as ours do.
- **by design: no City field, as in TallyPrime** (index.html:1805). TallyPrime's ledger mailing
  details are "Name, Address, State, Country, and Pincode" (help.tallysolutions.com/ledgers-in-
  tallyprime/, Alter Ledger), with no city; the city goes in the address lines, as ours does.
- **by design: the tabs and More master kinds are dimmed while a form is open.** Switching kind
  would drop the draft; the mock never shows them with a form open in another kind.
- **by design: ledgers are in name order.** The mock lists its sample in its own order. Ours
  sorts by name so the cursor and the filter behave the same on any company.
- **by design: the engine chips sit at the column's right edge** (index.html:1796, 1798). The
  Balance heading's "engine" and the value heading's "stock engine" chips show once the figures
  are in; the mock's sit just after the word.
- **data: the capture company.** Fictional Traders has ten ledgers and two stock items with no
  opening stock and no movement (so no other screen's figures move), so the items capture shows
  0 quantities and 0.00 values. Sample Buyer Pvt Ltd is bill-wise with 30 credit days and one
  open bill (SI-3, not due), so the party detail's card has one line where the mock has two. The mock shows Godavari Drip's ledgers and seven items with stock.

## Bank reconciliation (round of 2026-11-23)

- **gap: no Export of the reconciliation statement.** The mock's statement state offers Ctrl+E
  Export (index.html:1784). The bank screen has no export yet; the report screen's Export does
  not cover it, since the statement is not an engine report.

- **data: the statement lists totals, the mock lists each line.** The mock's statement
  (index.html:1741-1745) heads "In the bank, not yet in the books" and "In the books, not yet in
  the bank" and lists each line under them, with provenance chips (books, engine, statement).
  Ours shows the engine's totals ("Amounts not reflected in bank: deposits" and "payments"),
  drawn as the mock's indented lines. Listing them needs the engine's `Brs` to carry the lines.
- **bug: no reconciled card beside the statement.** The mock has a card with "Reconciled" or
  "Not yet reconciled" and Review suggestions (index.html:1746). Not built.
- **bug: an "as of" line above the statement.** Ours says "Reconciliation statement as of 20 Nov
  2026" over the card; the mock has the account and month in the subtitle and nothing above the
  card. The tabs (All lines, Reconciliation statement) are also ours only.
- **data: the lines table is not captured.** The capture company has no imported statement, so
  `bank.default` shows the empty state and the lines table (still `TableBuilder`, with the
  mock's rows wrapping to several lines) has no capture to compare with. Fictional Pumps (the
  third capture company) has a stored statement and could serve it in a later round.
- **gap: the empty state's look.** `bank.empty`: the mock's card has an icon, a narrower wrapped
  sentence and the account number and month in the subtitle; ours has the tabs, a File layout
  row under the card and the account name in the breadcrumb.
- **deferred: `bank.import-error` is not captured.** The engine refuses a statement whose running
  balances do not add up, but Import opens the system file dialog, which the capture harness
  cannot drive. Needs a way to hand the screen a file without the dialog.

## Payment (light states round of 2026-11-24)

- **deferred: `payment.bill-wise` is not reachable.** Payment to a bill-wise party needs the bill
  allocation sub-form; voucher entry refuses bill-wise ledgers on a payment
  (`voucher_entry::draft::entry_support`). Not built.
- **deferred: `payment.overdraft` is not reachable.** No warning is given when a payment takes a
  bank or cash ledger below zero; there is no such check in voucher entry or the engine.

## Sales invoice (round of 2026-11-23)

- **by design: a company without inventory gets an accounting invoice.** F8, the sidebar's Sales
  row and the palette open the invoice screen in accounting invoice mode when Maintain Inventory
  is off: the party, place of supply and reference, then ledgers under Particulars with their
  amounts, and no sales ledger field. TallyPrime documents the mode ("select Accounting Invoice
  mode", "Select the service ledger under Particulars, and enter the Amount",
  help.tallysolutions.com/sales-of-goods-services/) and the missing field ("The Sales ledger field
  will not appear", help.tallysolutions.com/tally-prime/sales-process/faq-sales/), both read
  2026-11-23. Choosing the mode from Maintain Inventory is our choice: the pages read are silent
  on what that setting does to the mode.
- **data: the capture is an accounting invoice; the mock's is an item invoice.** Fictional
  Traders keeps no inventory, so `sales-invoice.default` shows an empty accounting invoice where
  the mock (index.html, `sales-invoice`) shows Godavari Drip's three-item invoice with its tax.
  The pair cannot match until the capture company keeps inventory or the mock has an accounting
  invoice state.
- **deferred: As Voucher mode.** TallyPrime's Ctrl+H offers Item Invoice, Accounting Invoice
  and As Voucher (help.tallysolutions.com/sales-of-goods-services/). Ctrl+H here switches
  between the first two only, as the mock's toast says; As Voucher is not built (the GST engine
  refuses it).
- **deferred: More details (Ctrl+I), the e-Invoice status, the voucher number in the title, the
  party balance line and the place-of-supply note** (index.html, `sales-invoice`). Not built.
- **needs a decision: the party list offers every ledger.** The Customer and Supplier fields list
  all ledgers (Capital, Output CGST, Sales, ...). TallyPrime's pages say what to pick there:
  "Select Cash or Bank for cash sales and the party name for credit sales"
  (help.tallysolutions.com/sales-of-goods-services/) and "For credit purchases, select the
  supplier ledger" (help.tallysolutions.com/tally-prime/purchase-process/purchase-goods-and-services-tally/),
  both read 2026-11-23. Neither says which ledgers the list shows or leaves out. Limiting it to
  cash, bank and party groups needs a page that documents the list before it is changed.

### Light states round of 2026-11-24 (Fictional Pumps, `snapshot-stocked.ledgerly`)

The item-invoice states are captured on a second company file with inventory on, so the
Fictional Traders figures are unchanged.

- **deferred: the item list is a bare list.** `sales-invoice.item-list` lists matching items
  only; the mock adds a header row, each item's stock and rate, and a Create row.
- **needs a decision: the place of supply is not filled from the party.** The capture types it.
  No help.tallysolutions.com page was read on whether the party's state fills it; it stays as is
  until one is.
- **deferred: inter-state and B2C details.** `sales-invoice.gst-inter-state` and `.b2c` show the
  IGST or CGST/SGST block, but not the mock's place-of-supply note or per-rate breakdown.
- **deferred: validation look.** `sales-invoice.validation-error` lists one reason as a note, with
  no red outline on the field; the mock lists every problem and outlines the fields.
- **deferred: period-locked look.** `sales-invoice.period-locked` shows the lock note (the lock
  date now comes from the company file, `Api::lock_date`), but not the mock's banner layout.
- **deferred: save confirmation.** `sales-invoice.save-confirm` is a compact prompt; the mock
  shows a summary card with the total.
- **bug: the saved state draws nothing in the harness.** Answering the save question in the
  capture harness leaves the content area empty (breadcrumb only), so `sales-invoice.saved` is
  not captured. Not yet traced to the screen or the harness; it needs a failing test first.

## Review queue (card W4, round of 2026-11-23)

- **deferred: the queue lists bank matches only.** The mock lists six kinds (index.html:1023 and
  1203). The engine lists bank matches for the newest stored statement of each bank ledger
  (`Api::review_queue`, `ledgerly-api/src/review.rs`). Five sources have no engine list yet and
  are named on the screen and on Home with the engine's reason: bill photo drafts (no bill
  reader), plain-words drafts (no assistant), new ledger suggestions, possible duplicates (rule
  D-02, index.html:1036), and 2B explanations (no 2B reader, engine item E-3). Closing each needs
  its own engine list; nothing is shown for them until then.
- **deferred: the sidebar count (6 in the mock) is off.** `ReviewQueue::count` is `None` while any
  source is unlisted, so no partial count is shown. It comes back when every source is listed.
- **deferred: Y accept, N reject and Ctrl+A on the queue.** The mock's decisions
  (index.html:1224) need a reject command and a queue-side accept. Enter opens the row's bank
  ledger (bank reconciliation), but does not yet select the suggested line or its tab. Accepting
  there goes through `Api::confirm_bank_match`.
- **deferred: Home rows do not open their proposal.** The Home card lists the same rows as the
  queue (index.html:1066). Its button says "Review queue" until the count exists. Rows are not
  clickable yet.
- **deferred: the empty state's action.** The mock's review.empty has "Read a bill" (Alt+O,
  index.html:1206). It needs the bill reader. The app's empty state says which sources are not
  connected instead of the mock's "Nothing to review", because that wording claims every source
  is empty.
- **by design: no optional-voucher list.** Tally's nearest page is "Display Optional Vouchers"
  under Exception Reports (help.tallysolutions.com/docs/te9rel66/Reports/MIS_Reports/Display_Optional_Vouchers.htm,
  read 2026-11-23). It lists vouchers marked Optional, which are not posted. It is not a queue of
  proposals awaiting a decision, so the queue is not built from it.
- **bug: the queue is not a table.** `review.default` (Fictional Pumps, a stored statement with
  two suggested matches) draws each proposal as a wrapped line of text in a card, with the
  second row out of line with the first. The mock has a table (Proposal, Source, Confidence,
  Amount) with an icon per row, the selected row's bar, and a "How decisions work" card beside
  it. review.empty is captured too and shows the connected-sources wording above, not the
  mock's "Nothing to review" heading.

## AI and privacy (round of 2026-11-23, card W4)

- **deferred: the cloud tier.** `ai-settings.cloud` (index.html:1907, 1912-1913) shows the provider
  card as set with its key in the system keychain, and a "Removed before sending" list. The app
  has no keychain store wired in and no provider form, so the engine refuses the choice
  (`Api::set_ai_tier(Cloud)` is `Unsupported`, reason shown on screen) and the cloud panel reads
  "Not available" with that reason. The "Removed before sending" switches are not drawn.
- **deferred: the local model rows.** index.html:1914 shows the bill reader (410 MB) and the
  small model (1.8 GB) as on disk and "None used" verified. No local model is built, so the panel
  says "Not installed" for each row and the network row says nothing is sent yet.
- **deferred: the feature switches.** index.html:1908-1910 draws an On switch per feature. The app
  draws the same switch in an On column, read-only, because no feature can be turned on yet. The
  "Not built yet" note sits under the features table when the tier is not Off, not in a column.
- **by design: no tier is chosen by default.** The mock's first state is "On this computer", which
  would claim a tier nobody chose. A company with no stored tier shows neither option as current,
  with a note that nothing is sent until one is chosen. The top bar says "AI: not set up".
- **by design: the log headers.** index.html:1915 has Feature and Input columns. The company file
  records the model, an input hash, the output and the decision, not the feature or the input text,
  so the columns are When, Model, Input hash, Output and Your decision. The hash shows its first 12
  characters.
- **data: no log rows yet.** The mock's five sample rows (index.html:1896-1900) are not drawn. No
  AI feature writes `AiProposalRecorded` events yet, so the log says no activity is recorded. Ctrl+E
  is offered only when rows exist, and it writes CSV through the save dialog.
- **deferred: the empty-log wording.** The mock's off state says "No AI events since AI was turned
  off" (index.html:1915). It is not used: the log is empty for every tier until a feature records
  an event.
- **deferred: AI off and the screens it hides.** The off note (index.html:1911) is shown. Ask your
  books, Plain-words entry and the bill photo read the tier and say AI is off (see "Assistant
  screens" below); they stay in the sidebar and palette, as the mock's own AI-off states keep
  them reachable (index.html:1169, 1430). Bank suggestions are not yet wired to the tier.
- **by design: the "Current" badge.** The mock marks the chosen card with a border only
  (index.html:1905-1907). The app puts a "Current" badge on the tier stored in the company file, so
  a refused cloud choice still shows the stored tier as current. The app also draws the selected
  card with an accent fill and border, so the stored tier stands out in both ways.

## Assistant screens: Ask your books, Plain-words entry, bill photo (round of 2026-11-23)

No model or bill reader exists in this build: `ledgerly_assist::nl_entry::ModelClient` has only
the recorded test double (`RecordedModel`), no OCR engine produces `ledgerly_assist::ocr::OcrOutput`
from an image, and `Api::set_ai_tier(Cloud)` is refused (no keychain store). So no question
becomes a query, no sentence a draft and no photo a reading. Each screen reads the company's tier
through the worker and says why for that tier (`screens/ai_gate.rs`); none shows a sample. The
accounting product has no AI features, so these follow the mock and spec 6.8 only.

- **deferred: ask.result, ask.loading, ask.low-confidence, ask.unsupported.** Each needs a
  question compiled to a `ReportQuery` (index.html:382-385, 1171-1190). Nothing compiles one, so
  the box is disabled and only ask.offline-ai's report list is built, for every tier. Closing
  needs a model client and the query runner.
- **by design: the report list shows for every tier.** The mock shows it when AI is off
  (index.html:1169-1170). With no model, it is the only answer the screen has, so Local, Cloud and
  no tier show it too, with their own reason.
- **by design: the reason for tiers the mock has no state for.** Local says no local model (or no
  bill reader) is installed; no tier says none is chosen; Cloud says the cloud tier is not
  available without a keychain store. The wording is in `ai_gate::reason`.
- **gap: the note is plain.** The mock bolds the note's first clause and draws its AI and privacy
  button as a ghost (index.html:1169, 1430). Ours is one weight with a secondary button.
- **gap: Ask's question box is empty and has no Enter keycap.** The mock fills it with a sample
  question and lists Enter "Ask" in the key bar (index.html:1168, 1199). Nothing would answer it,
  so it holds a hint, is disabled, and the key bar lists only the name.
- **gap: the receivables button's icon.** The mock's `clock` is not in the app's icon set; the
  calendar stands in.
- **deferred: every quick-entry state.** quick-entry.proposal, .hinglish, .unknown-ledger,
  .loading and .rejected (index.html:404-408) all need a draft from a model. The mock has no AI-off
  state for this screen; the app's capture is `quick-entry.offline-ai` (no mock capture). It keeps
  the head, the disabled box and the rules card (index.html:1542-1545), and offers F5 Payment and
  F6 Receipt in place of the draft card. The "Try" sentences are left out.
- **deferred: purchase-bill.reading, .proposal, .low-confidence, .mismatch, .duplicate,
  .cloud-consent, .accepted.** Each needs a reading of the bill (index.html:395-403). The draft
  would go through the normal purchase submit path after the user's accept; nothing reaches it.
- **deferred: purchase-bill.error.** The unreadable-file state (index.html:402) needs a file to
  have been chosen and read. No file dialog is offered, since no reader exists to read the file.
- **gap: purchase-bill.offline-ai has no form or image beside it.** The mock embeds a purchase form
  under the note and keeps the image attached to the voucher (index.html:1430, 1504-1506). Ours
  offers F9, which opens the purchase form on its own screen, and the image column says no image
  is read or kept (no attachment store exists). The note's second sentence says so instead of
  "Type the bill below; the image stays attached to the voucher".
- **gap: the bill photo's head.** The mock's crumbs are Home, Purchase, From bill photo and its
  head has "Read another" (Alt+O) and the subtitle "... Alt+O read another bill · the image is
  kept with the voucher" (index.html:1422). Ours: Home, Purchase from bill photo; no Alt+O; the
  subtitle is "F9 purchase". The key bar is named "Bill photo", not "Draft purchase". The
  sidebar keeps Purchase selected, as the mock does.
- **by design: the review queue's unlisted sources stay.** Bill drafts and plain-words drafts stay
  in `Api::review_queue`'s unlisted list (`ledgerly-api/src/review.rs`), since no draft store
  exists.

## Company setup (round of 2026-11-23)

Read on help.tallysolutions.com/set-up-company-tally/ (2026-11-23): the financial year beginning
and books beginning dates are both prefilled and may be changed, and books may begin after the
financial year starts. The model supports that (`Company::books_begin`), so it is offered.

- **bug: keyboard preset is not saved.** The Keyboard step's choice is shown and explained on the
  step, but no store holds it; every company uses the classic keys (`keymap::preset`, "no
  persistence yet"). Closing it needs a stored preset per company.
- **deferred: the cloud AI choice is not saved.** Off and On this computer are stored with the new
  company (`form::create_company` calls `Api::set_ai_tier`). The engine refuses the cloud tier
  until a keychain store exists (see "AI and privacy" above), so the AI step says so when Cloud is
  picked, and Done shows the engine's reason; the company starts with AI not set up.
- **bug: backup folder is not saved and no backup runs on a schedule.** `backup_to` writes one
  file when called; nothing stores a folder or runs daily. The Backup step says so.
- **gap: GST filing frequency (Monthly) is not saved.** The model has no filing-frequency field.
- **gap: the "Company file" field is ours.** The mock has no file location; the wizard asks for
  it, defaulting to `Documents/Ledgerly/<company>.ledgerly` when blank.
- **gap: City is kept as the first address line.** The model's `Address` has no city field.
- **gap: the state mismatch has no red outline or icon.** `setup.gstin-error` (24 Gujarat chosen
  against a GSTIN starting 27, by clicks on the State list) shows the mock's words and disables
  Continue, as the mock does, but the GSTIN and State fields are not outlined in red and the
  message has no cross icon. The rule is tested in `tests/w4_setup.rs`.
- **gap: import, open-file and sample cards on Start are shown, not working.** Import needs the
  import wizard (not routed from here); open-file is on the welcome screen; no sample data ships.
- **gap: Done's "Make your first invoice" is disabled.** No route to a sales invoice from the
  wizard yet, so its card is drawn with a muted label, takes no click and F8 does nothing there;
  the other two launches open their screens, by click or by their keys M and B.
- **gap: a breadcrumb sits above the setup card.** The mock draws the card with no crumbs; the
  shell draws "Home > Company setup" for every screen opened over Home.
- **gap: the unavailable cards on Start carry a muted line saying why.** The mock draws the
  import, open-file and sample cards as plain choices; ours cannot be chosen (see above), so each
  says so under its text.
- **gap: no F11 on the Features step.** The mock's line ends "F11 changes this later"; no F11
  feature screen exists, so the sentence is left out. Rows the model has no switch for show an
  off switch that does not move and say why.
- **gap: the backup note has no info icon.** The kit's note draws text only.
- **gap: fields are the kit's text inputs and combo boxes.** The mock's inputs on Company and
  Backup are drawn by the shared egui styling, not the mock's `.fld input`; read-only values
  (GST filing, Keep, When) are text with a "Not saved yet" hint instead of inputs.
- **data: the mock's sample is prefilled; the wizard starts blank.** The capture types the mock's
  company (name, GSTIN, file). The mock's City "Pune" is not typed.

## Print (card P1, round of 2026-11-24)

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

## Accessibility (card S7, round of 2026-11-24)

Tests: `s7_a11y_inputs` (every text input named by its caption), `s7_a11y_rows` (masters rows
read their cells), `s7_a11y_labels`, `s7_a11y_screens` (names, read from the AccessKit tree),
`s7_contrast` (WCAG 2.1 ratios over theme pairs), `s7_focus` (Tab order; suggestion entries are
not Tab stops, on the invoice and in voucher entry (`s7_a11y_inputs`); Up, Down and Enter move and pick the highlighted entry), `s7_zoom` (Ctrl+= /
Ctrl+- / Ctrl+0), `s7_colour` (status words). Unit tests name GST, bank, balance sheet and day
book rows from their cells, the day book in drawn column order. Token values are unchanged.

Keys for the suggestion lists are from help.tallysolutions.com/keyboard-shortcuts-tally-prime/
(read 2026-11-24): Up and Down move one line in a list, Tab and Shift+Tab move between fields.

- **gap: divider border fails 3:1 non-text in light and dark.** `border` (light `#E2DFD8`, dark
  `#34322E`) measures 1.33:1 and 1.35:1 on the surface. It is used for hairline dividers.
  Pinned by `divider_border_is_a_known_gap_below_the_non_text_minimum`. A control outline should
  use `border_strong`, which passes 3:1 in light and dark.
- **by design: zoom is egui's built-in, session-only.** Ctrl+= / Ctrl+- / Ctrl+0 step the scale
  by 0.1 between 0.2 and 5.0 (egui 0.37 `gui_zoom.rs`). The scale lives in the egui context, so it
  lasts the session and is not saved, the same as the theme. The mock (`design/mock-v2/index.html`)
  has no zoom key, so there is no mock state to match.
- **audit: colour-only signals.** Debit and credit read "Dr" and "Cr"; bank matches read "Matched",
  "Not matched" or the suggestion kind; GST buckets and review states read their word; notes carry
  their text beside the colour; the setup rail check has a check icon. No listed signal is colour only.
  `s7_colour` pins the words of each badge and note kind.
- **focus rings.** The focus stroke is thicker in high contrast (`u1_theme`). No per-screen pixel
  check of the ring in all three themes was added in this card.

## Dark and high-contrast themes (round of 2026-11-24)

Every light state with a dark or hc mock capture is captured in both themes (40 each; the
quick-entry offline-AI state has no dark or hc mock capture). Theme differences left:

- **gap: widgets outside the kit fade when disabled.** egui's own widgets (the "More master kinds"
  combo on the ledger detail) draw at half opacity when disabled; kit buttons keep their tokens.
  The faded text stays readable (about 5:1) but is greyer than the mock's disabled colours.
- **gap: keycaps do not grow in hc.** The mock's `kbd` (index.html:75) has 2 px borders and grows
  to 20 px; ours draw the 2 px border inside the light 19 px height.
- **gap: table rows do not grow by the extra rule pixel in hc.** Rules are 2 px in hc, drawn
  inside the row height the light theme uses.
- **gap: the hc focus ring is 3 px.** theme.rs `focus_width` widens it on purpose; the mock keeps
  2 px. The masters filter box draws a literal 2 px outline (masters/look.rs), which matches the
  mock but not the app's own hc focus width.

## Reports not in the mock (S8)

The mock has no state for the profit and loss, trial balance, outstanding, stock summary or
register reports, so they have no capture to compare. They draw as the generic table, which now
shares the drill levels' fixes (bare Debit and Credit figures, footnote style).
