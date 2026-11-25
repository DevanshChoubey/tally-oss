# Known gaps between the app and the mock

Differences seen in the side-by-side captures (`design/COMPARE.md`) that a parity round left on
purpose or could not close yet. Each entry says where the mock shows it, why it was left, and what
closing it needs. Remove an entry only in the commit that closes it.

Kinds: **bug** (the app is wrong or missing something it should do), **deferred** (waits on a
feature that is not built), **data** (the capture company lacks what the mock's sample shows),
**by design** (we differ on purpose; kept here so nobody "fixes" it back).

## Home (round S9, 2026-11-25)

Captured: `home.default`, `home.empty`, `home.ai-off`, `home.nudge-open`, `home.loading` (light,
dark, hc). The four tiles, the November bars, the review rows, the nudges and the
new-company card (book icon, three buttons with icons, info note) follow the mock; every figure
comes from `Api::home_figures` and `Api::home_nudges`.

- **gap: GST cash payable is unsupported.** The mock shows 39,498.00 "after accepted ITC". The
  engine has no set-off, so cash payable cannot be worked out (the 2B reader exists since s11); the tile keeps its slot and size
  and says "Not supported yet" and "Needs GSTR-2B: not readable yet".
- **gap: no IMS nudge.** "3 2B records have no IMS action" is not wired yet (s11 records IMS actions; the nudge count is a later step).
- **gap: GSTR-1 due date is the monthly one.** The 11th of the next month (CBIC Notification
  83/2020-Central Tax). Quarterly (QRMP) filers' 13th and holiday shifts are not modelled; no
  filing record is kept, so the nudge shows until the month passes. The count is of vouchers
  needing correction, not "items"; "Open GSTR-1" opens the GST returns screen.
- **gap: the compare dialog differs from the mock's.** It has a "Bank date" row (the date a
  user-confirmed bank match cleared each payment, or "Not matched to a statement") and the
  matching status line, in place of the mock's bank-line text ("L03, 12 Nov 2026"), which needs
  statement line numbers per payment. It has no "Adjust rule" or "Not a duplicate" buttons: the
  engine cannot record that a pair is not a duplicate. Esc closes it; Investigate (Ctrl+A) opens
  the day book. The nudge names the party both vouchers share, else the debit ledger (Electricity
  on Godavari, whose payments have no party). D-02 is our own rule (documented on
  `Api::home_nudges`); the seed's PY/0304 narration was set to "City Power, October bill" so that
  it can fire on Godavari.
- **gap: review rows.** No age ("2 min ago"): the engine's `ReviewItem` carries no created time.
  Rows are bank matches (bank icon) and rule D-02 possible duplicates (alert icon, "Rule D-02 ·
  15 days apart"). The kinds that need the in-app AI stay out and are named in the "Not connected
  yet" card: bill-photo drafts, plain-words drafts, new-ledger suggestions, 2B explanations.
  The sidebar count stays withheld while those are unlisted. J/K/Enter on Home open the generic
  queue, not the row.
- **gap: review decisions (s11).** Y and Ctrl+A accept the highlighted row (the mock's Ctrl+A is
  the same as Y; it is not "accept all"). A bank match goes to `confirm_bank_match`, which checks
  it again; a Rule suggestion is refused by the engine and says why; a duplicate has nothing to
  post, so Y explains the two ways out. N and Alt+D hide the row for this session only: the
  engine stores no rejection, and the notice says so (the mock says "Rejected" with an optional
  reason). Feedback is a note above the table, not a toast. Enter on a duplicate opens the day
  book, not the two vouchers side by side.
- **gap: the entry bar.** It says "not connected in this build" where the mock says "runs on this
  computer"; the greeting has no name and the "Start" card is dropped, by design.
- **gap: new `Icon` variants** (Alert, Info, Clock, Camera) are drawn from the mock's paths.
- **data: `home.loading` is taken with preset books (s10).** The capture holds the worker
  (`ApiWorker::hold`) so the refresh never answers; the shell's company, lock date and books are
  preset because the open-company answers would queue behind the hold. The tile and panel
  heights are slightly taller than the mock's (the mock's skeleton is 84 px tiles, ours follow the
  real tile); the AI chip reads "AI: not set up" (see the topbar entry).

## Palette (round of 2026-11-23, commit d4f0bcb)

- **gap: three of the mock's other commands are not built.** Read a bill photo, Import bank
  statement, Switch theme and Collapse side bar now follow the four voucher commands
  (`RunCommand`; the first two open the purchase-bill and bank screens, the others run in the
  shell). Left out: Import 2B / IMS JSON (no 2B reader), Export GSTR-1 JSON (it needs the GST
  screen's file flow, so it is not a one-step command) and Back up now (no backup folder setting
  to run it against). Their shortcuts (Alt+O, Ctrl+Shift+L, Ctrl+E) are not bound, so no key is
  shown, except Ctrl+\ for the side bar.
- **gap: a recent GSTR-1 row has no live detail.** The mock's Recent rows say "As at 21 Nov
  2026", "3 sections ready, 1 needs correction" and "Ledger · Sundry debtors · 1,93,500.00 Dr"
  (index.html:1115). A recent balance sheet and a recent ledger now show the date it opens at and
  the engine's balance, fetched on every opening (`palette/recent.rs`). The GST return is a
  screen, not a palette target, so it is never a recent row; its section count is not built.
- **deferred: Ask rows.** The no-match "Ask: q" row and the "Or" Ask/Draft rows (index.html:1129,
  1127) are left out while AI is off, as the mock's own `tierOff` does. Return with the assistant.
- **data: `#0412` (resolved by the sample company, S8).** Godavari has SI/0412, so a search for it
  now finds the voucher (see "Sample company").
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
- **closed (s10): the drill reports' columns and the ledger side card.** The group drill is the
  engine's new `ReportKind::GroupLedgers` (GSTIN, Opening, Debit, Credit, Closing, "Total of
  <group>"; `tests/rp5_ledger.rs`, derived by hand, seen failing first). The ledger drill reads
  Date, Voucher, Type, Particulars, Debit, Credit, Balance and has a side card: group, GSTIN and
  credit days copied from the master, Open bills from the engine's outstanding report, Alter ledger
  (Ctrl+Enter). Still different: the group drill's period is the financial year, so Opening is
  0.00 Dr; Opening and Closing keep the Dr/Cr word; Particulars is the engine's counter-ledger
  text, not the voucher narration, and rows do not wrap; the Open bills badge now reads "36 days
  overdue" in full (days from the engine's ageing rule, as at the report date, 20 Nov; the mock
  shows 37 for its 21 Nov); the sample GSTINs are fake. The mock's ledger drill has no Audit card,
  so none was added. Closed in the S8 round (tested in
  `tests/s8_report_parity.rs`): the drill titles and crumbs read as the group or ledger name
  ("Sundry Debtors"), the subtitle is "As at ... Enter goes deeper, Esc goes back up", Debit and
  Credit figures drop the Dr/Cr word, the repeated Particulars column is not drawn twice, and the
  footnote is small text under the card.
- **closed (s9): sub-groups in the default view.** The default (Alt+F1 "condensed") view now
  lists each top group, its sub-groups and the closing stock, with no ledgers, as the mock's
  `balanceSheet()` does (index.html:1581-1589). The screen asks the engine for the detailed report
  and drops ledger rows (`remote::groups_and_sub_groups`); the engine's own `Condensed` (primary
  groups only) is unchanged and still what Export writes. Help page read 2026-11-25
  (help.tallysolutions.com/docs/te9rel66/Reports/Display_Financial_Statements/display_balance_sheet.htm):
  the standard view is condensed and Alt+F1 shows the detailed view; the page does not say which
  group levels the condensed view lists, so the sub-group level follows the mock.
  Tested in `tests/s9_reports.rs`.
- **gap: Export of the balance sheet writes the engine's primary-groups-only condensed report**,
  not the sub-group view on screen.
- **closed (s9): indent past two levels.** The default view stops at depth 1; the detailed view
  still indents each level by 16 points.
- **by design: F2 says "Period".** The mock says "F2 Date"; ours opens a from-to period editor.
- **by design: section and group names** follow decision 2 of the 2026-11-22 parity report (a
  recorded user decision, kept in s9 although the brief asked for the mock's heading):
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
- **gap: the provisional reason is still plain.** The balance sheet's and the profit and loss's
  reason now names the item ("negative stock: item Steel pickaxe first went below zero on
  2026-11-10", closed in s9; an item missing from the masters shows its number). Still different
  from the mock's "Steel pickaxe 2.5 kg shows -30 nos": the date is ISO and the quantity is not
  in the reason. The stock summary's own reason (`stock_summary/valuation.rs`, pinned by
  `rp9_stock_summary.rs` as "item 1") is unchanged.
- **gap: the voucher view (s11).** Item table, Ledger posting with a bold Total row (from
  `Api::voucher_detail`, checked sums) and an Audit card (who entered it, when, how, alterations,
  period lock, audit log entry and hash) are built. Still different: the Party field is dropped;
  the Audit card says nothing about AI draft names or edit counts (the log does not keep them),
  adds a log-entry/hash bullet and a "Read-only here" bullet that the mock lacks; tax lines show
  as ledger lines, not a separate block; the subtitle is "Read-only. Esc goes back up" (no
  "As at"); the breadcrumb has an extra "Current Assets" level; Alter and cancel keys are not
  built. Sample vouchers are stamped at noon on their own date.
- **data: `balance-sheet.loading` is taken with preset books (s10).** The worker is held so the
  report's request never answers (the card, the clock icon, twelve bars and the key bar
  "Computing / Esc Cancel" are real). Differences left: "As at 20 Nov" against the mock's 21 Nov
  (the report reads the last voucher date, the mock the shown day), and the breadcrumb and
  buttons follow the mock.

## Day book (round of 2026-11-23)

- **closed (s9): Entered by, Origin and the narration line.** The facade adds `Narration`,
  `Entered by` and `Origin` to the day book from each voucher's `VoucherPosted` record
  (`ledgerly-api` `day_book_meta`); user names come from `SettingChanged user.<id>.name`
  (`Api::set_user_name`). The sample company posts each November voucher as the mock's user, and
  PY/0315 and PB/0141 as the assistant acting for Ananya K. Tested in
  `ledgerly-api/tests/s9_day_book.rs`, `ledgerly-sample/tests/sample.rs`, `ledgerly-app/tests/s9_reports.rs`.
- **gap: Origin shows Typed, AI draft, accepted or Imported only.** The mock also shows "Image
  attached" (PB/0144) and "Bank rule" (PY/0316) and names the draft kind ("Plain-words draft",
  "Bill photo draft"). The company file stores no attachment, no bank-rule posting and no draft
  kind, so PB/0144 and PY/0316 read Typed and the two AI drafts read "AI draft, accepted".
- **deferred: Ctrl+I Origin.** The original proposal is not opened from the day book, so the
  footnote leaves out the mock's Ctrl+I sentence and the key bar does not offer it.
- **by design: a non-regular status is a badge in the Origin cell.** The mock has no Status column;
  a cancelled, optional or post-dated voucher still has to say so.
- **closed (S8): the empty state is a card** with the book icon, title, sentence and the four
  voucher buttons (they open the voucher forms); the sentence now includes the mock's "with who
  entered it" (s9). The buttons sit centred; the mock's sit at the
  left of the card.
- **by design: the month view keeps the cursor on the voucher it was on.** `day-book.month`
  (Alt+F2, 1 Nov to 20 Nov) keeps the selected voucher selected when the period changes
  (`report_view::replace_report`), so the cursor is not on the first row as in the mock.
- **needs a decision: F2 offers the shown day.** TallyHelp says F2 in the Day Book first offers
  the day after the current date; ours fills in the day shown. Check the TallyPrime page (only
  the Tally.ERP 9 page was read) before changing it.
- **data: voucher numbers and the sample day (resolved, S8).** Godavari carries the mock's own
  numbers (SI/0417 ...) and its five vouchers on 20 Nov, entered by the mock's two users (s9).

## GST returns (round of 2026-11-23)

Captured states: `gst.gstr1`, `gst.gstr3b`, `gst.recon`, `gst.recon-empty`, `gst.recon-error`,
`gst.recon-locked`, `gst.gstr1-correction` (on Fictional Pumps, which sold pickaxes with no HSN).
Not captured: `recon-explain` (AI).

- **closed (s11): the 2B reconciliation is built.** The Api imports a 2B file for a period,
  lists the engine's rows (`reconcile_2b`), records IMS actions and marks a period's GSTR-3B
  filed; three new store events keep them in the company file (old files open and verify, see
  `crates/ledgerly-store/tests/s11_event_compat.rs`). Captured in light, dark and hc: `recon`
  (4 rows, side card), `recon-empty`, `recon-error` (wrong GSTIN, refusal only) and
  `recon-locked` (October final, lock bar). The sample now has October bills KP/2205 and VL/858
  and 2B files for October and November; every earlier pinned figure holds.
- **gap (s11): the GSTR-2B JSON shape is `pending observed`** (rule A5). No authoritative public
  GSTN schema was found. The parser keeps its items[] shape; third-party descriptions of the
  newer flat shape would be read as NotComparable, never as numbers. The sample 2B files are
  ours, written to that shape, with fictional GSTINs.
- **gap (s11): left out of the 2B tab because no public source was found.** "A record with no
  action by 2B generation is deemed accepted", "a change now goes through an amendment in a
  later month" (lock bar), "The file looks like a customer's 2B" and "Drop it here". Also not
  built: Explain / X, Y Take suggestion and the bill-photo draft link (AI, see the AI section),
  and purchase vouchers with no 2B document (books-only rows are not listed).
- **by design (s11): extras and differences on the 2B tab.** A "Mark GSTR-3B filed" button sits in
  the side card (the mock has no way to record a filing). The IMS cell stacks the badge over
  "Suggested: X"; dates print dd-mm-yyyy; the top bar says "AI: not set up" as on every capture.
  Row heights and column widths follow the mock; supplier names wrap to three lines.
- **gap (s11): Command enum not used.** The new capabilities are `Api` methods, not `Command`
  variants, because the frozen `Command` enum has no 2B or IMS variant.

- **by design: the correction state is built from the sample (s9).** `gst.gstr1-correction` now
  runs on Godavari: Table 12 shows 5 records, 3,49,385.00 and Needs correction, selected with J / K,
  and the side card names the item with a "Fix the item master" button (Enter opens the item
  master). Differences: the card shows the engine's reason without the voucher number and date,
  and says "Export JSON is refused until it is fixed. Nothing is guessed." where the mock says
  the voucher is held back. A missing HSN keeps the invoice in Tables 4, 5, 7, 8, 9B and 13 and
  only Table 12 waits, since HSN is a field of Table 12 only (GSTN GSTR-1 user guide,
  https://tutorial.gst.gov.in/userguide/returns/Creation_of_Outward_Supplies_Return_in_GSTR-1.htm,
  read 2026-11-25). JSON export is still refused while any table needs correction.

- **by design: GSTR-1 by section is built, with differences** (index.html:1673-1676, s9). The
  engine now summarises the return per table (`Gstr1Return::sections`: 4A, 5, 7, 9B, 12, 13;
  6A and 8 only when the return holds something there) and the screen shows it with Records,
  Taxable, Tax and Status. The tiles count sections. The voucher table stays beneath as the
  drill-down ("Vouchers behind the sections"), and clicking a tile still filters it.
  The voucher table is folded under a "Show vouchers (N)" button while sections are shown (s9-all),
  so the section table and side card fit as the mock; it stays open when no sections exist.
  Differences: (1) a held-back voucher row appears only for duplicates, which sit in no table.
  (2) Sections are selectable (J / K, Enter) with a side card. Sources: the offline tool's
  worksheets b2b, b2cl, b2cs, cdnr, hsn and docs, https://gstzen.in/a/gst-manuals-user-manuals-downloading-installing-and-uploading-invoices-in-gstr-1-taking-actions-in-gstr-2.html
  and https://help.tallysolutions.com/file-gstr-1-using-offline-tool/ (both read 2026-11-25).
- **by design: the agreement line under the GSTR-1 table is built.** The engine compares 4A
  (regular), 5 and 7 with GSTR-3B 3.1(a) (`Gstr1Return::taxable_vs_gstr3b`) and the line gives
  both figures when they differ. It is Unsupported (no line) when the return holds credit or
  debit notes (s.34 pending) or SEZ or deemed supplies.
- **deferred: GSTR-3B rows from IMS and set-off** (index.html:1683-1686). GSTR-3B now computes
  on the sample (blocked credit under s.17(5) applied), but the cash payable of 39,498, which the
  Home tile also shows, cannot be computed. These are "Accepted
  in IMS", "No action yet: deemed accepted", "6.1 Payment of tax", "Left after using accepted
  credit" and "Payable in cash". The 2B and IMS match now exists (s11) but set-off (engine item E-1) does not, so these rows
  stay unwired. Ours draws 3.1(a), "In your books" (4(A)(5)) and the blocked or ineligible
  credit, all from the engine's working.
- **by design: a Cess column on GSTR-3B.** The mock has no cess column, but the engine works out
  cess, and a figure is never dropped.
- **by design: our own words in "What happens next" and "Read this before filing".** The mock's
  bullets claim things the app does not do: "Mark the month filed here", "Credit counts only what
  you accepted in IMS" and the set-off order. Ours say what the engine does.
- **by design: the period button opens a month picker (Alt+F2).** It lists the 12 months of the
  financial year; choosing one reloads the screen. The mock has no picker state, so its look is ours.
- **deferred: the GSTR-3B key bar** (index.html:1721). The mock offers J / K, Enter and Ctrl+E
  there. Ours has no rows on that tab and exports only GSTR-1, so it offers just 1 2 3 and Esc.
- **needs a decision: "monthly" in the subtitle.** It comes from the shell, which always opens
  one calendar month (`gst_return_period`), not from the registration. Quarterly filing (QRMP)
  is not modelled; the registration needs a filing frequency first.
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
captured: `suggested-ledger` (an AI state: the assistant proposes the ledger; no in-app AI yet).

- **gap: the create form is not the mock's compact one (s9-all).** The capture types the mock's
  values (Jadhav Metals, GSTIN, credit days) through the real form, which is longer than the
  mock's and shows the sample's own GSTIN on the company line.
- **needs a decision: the GSTIN error's words.** Ours says "check character is 7, expected P; the
  GSTIN has a typing error" (the engine's `GstinError`); the mock says "Check character should be
  P, not 7. One of the first 14 characters is probably mistyped." `tests/w2_masters.rs` pins
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
  a "Provisional" badge on the row (s13; it used to print the reason in the cell), with the
  engine's reason in the banner above the table.
- **by design: stock is over the stock summary's opening period**, the financial year to its
  last voucher, as the stock summary report opens.
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
- **closed (s9): the capture company has a statement, and the account is chosen.** Godavari has
  two bank accounts, and Bank reconciliation used to refuse to open for any company with more
  than one. It now opens a chooser of the bank ledgers (name, masked account number, books
  balance from `Api::brs`), as the help page "Bank Reconciliation reports"
  (<https://help.tallysolutions.com/bank-recon-reports/>, read 2026-11-25) has the user pick the
  bank and switch with F4 "Bank Ledger". One bank opens directly; the palette's "Bank
  Reconciliation: name" rows still open one bank. `bank.default` and `bank.brs` are Saptagiri's
  (the mock's account, with the sample's imported statement of 11 lines, amounts equal to the
  mock's), `bank.empty` is Mula Valley Co-op (no statement), all on Godavari. The subtitle reads
  "name, XXXX 4471, November 2026" as the mock's does.
- **closed (S12): the chooser follows `bank-chooser.default`.** Head, subtitle, one table
  (account, number, balance from `Api::brs`, a Statement badge with the period from the stored
  statement's first and last line) and the footnote. Still different: the period starts on the
  first statement line (5 Nov 2026) where the mock writes the opening-balance date (1 Nov); the
  sidebar highlights Home, not Bank reconciliation, while the chooser is open; the mock's row
  highlight is the engine's cursor row.
- **by design: the key that changes account is C, not F4.** F4 is Contra in this app (the shell's
  voucher keys). The head carries a "Change account" button with the C keycap; the mock's head has
  only Import statement and Reconciliation statement, so the head differs by that button when the
  company has more than one bank ledger.
- **gap: the lines table is still not the mock's.** The mock's lines card has Exact, Rule and
  "AI suggested" badges with a confidence meter, mono narrations, `PY/0315`-style voucher
  numbers, the tabs "AI suggestions" and "Needs a new ledger", and a proposal card with Accept,
  Edit and Reject. Ours has the app's own matcher states (Exact, Not matched), "Matches voucher
  23", the tabs "All lines" and "Reconciliation statement", and a small card for the focused
  line. The lines are the same eleven with the same amounts and dates. The narration wraps to two
  lines in its column (row height 46) where the mock wraps to up to three. This round fixed the
  focus card running off the right edge of the window (the table's remainder column pushed it).
- **closed (s9): the reconciliation statement on Godavari is no longer a wall of uncleared items.**
  The sample seed gives each April to October line on Saptagiri a bank date (`cleared_days`, 2 or
  3 days after the voucher date; all before the statement starts on 5 Nov), and the November
  statement is an MT940 file (same eleven lines and balances as before) whose `:62F:` states its
  end date, 20 Nov. `bank.brs` now shows: books 7,27,524.50; not reflected in bank: deposits
  2,27,384.50 and payments 1,09,860.00 (the six November vouchers, none confirmed against the
  statement yet); balance as per bank 6,10,000.00 (the mock's statement opening); balance as per
  imported statement 8,23,305.50 (the mock's closing); difference 2,13,305.50. All engine values.
- **gap: the reconciliation statement lists totals, not the mock's lines.** The mock's
  statement has the same closing balance but is a different computation: it lists "In the bank,
  not yet in the books" (the five unmatched lines) and treats Exact, Rule and AI-matched vouchers
  as matched, with a difference of 95,781.00. Ours treats a voucher as cleared only once a user
  confirms a match (spec 6.5), so the six November vouchers are uncleared and the difference is
  2,13,305.50 (95,781.00 of lines not in the books plus 1,17,524.50 net of matched-but-unconfirmed
  vouchers). Left: confirm the Exact matches in the sample (it would need audit events) or list
  the lines.
- **gap: the empty state's look.** `bank.empty` has the 36 px icon, the 420 px sentence and an
  Upload-icon button, and hides the tabs; the subtitle now carries the account number and month.
  Left: a File layout row under the card (functional) and the account name in the breadcrumb.
- **closed (s10): `bank.import-error` is captured** (light, dark, hc) with the engine's whole
  working (`balance_refusal`), through a scripted file in place of the native dialog. Left: the
  app shows a "Change account" button and a longer breadcrumb the mock's state lacks; the AI chip
  and the truncated company name differ in the topbar (all shots).
- **gap (s10): `bank.suggested-ledger` needs the in-app assistant**, which is not built; not
  captured.

## Payment and receipt (round s9, 2026-11-25)

Closed in this round: the capture is the mock's Freight payment (and the receipt from Mehta), the
title carries the voucher number from the engine, the account line reads "balance now ... (engine)",
the foot box shows Paid out or Received, Balance now and Balance after (engine, through
`Api::balance_change`), the crumb is "Home > Payment", and the Payment or Receipt sidebar row is
the selected one.

- **closed (s9): `payment.bill-wise`, `receipt.default` and `payment.overdraft`.** Entering the
  amount of a bill-wise party opens the bill popup with the open bills from the engine
  (`Api::open_bills`); Enter accepts when the bills add up to the line, none typed means On
  Account. The overdraft note comes from `Api::balance_change` (Cr after). The Mode field is
  stored on the bank line (`BankAllocation.instrument`).
- **closed (s12): the amounts.** A committed amount reads with Indian grouping ("2,50,000.00");
  while the keyboard is on the field the typed text stays. The foot box writes a debit balance
  plain and a credit one with a minus ("-36,000.00") as the mock does (`inr(after)`), from the
  engine's `Api::balance_change`. The overdraft total is in the bad colour (below).
- **gap: the popup shows its narration empty.** The mock draws the popup over a filled narration;
  here the narration is entered after the popup closes, so the shot with the popup open has none.
- **changed (s12): the Mode is a box with a list of the seven instruments.** The mock draws a
  read-only "NEFT / UPI"; here the box still takes a typed start of a name (the keyboard path) and
  an arrow beside it lists Cheque, NEFT, RTGS, IMPS, UPI, Card and Other. The value stored is the
  same `InstrumentKind` on the bank line. A cash account has none. No overdraft limit or block
  policy exists, so saving is allowed and the note says so.
- **closed (s12): the account hint** under the Account field reads "XXXX 4471 · balance now
  ₹7,27,524.50 Dr (engine)": the bank's number masked to its last four digits, and the engine's
  balance. It appears once a line makes a voucher the engine can answer for.
- **gap (s12): the payment shot's end state.** The mock draws the screen mid-entry: row 1 lilac
  and an empty "Next ledger, or Enter on empty to finish" row 2, narration filled. Our capture is
  taken after Enter on the empty row, so row 2 is gone and the narration has the keyboard.
- **gap (s12): the group reads "Direct Expenses"** (the sample's master) where the mock's reads
  "Direct expenses"; the sample company's group names are shared with other screens.
- **closed (s9): the number and the date.** The sample's voucher types carry the mock's prefixes
  (SI/, PB/, PY/, RC/, JV/, width 4) through `Command::AlterVoucherType`; titles read PY/0319.
  The date reads 21 Nov 2026 and rates 3,850.00 until the keyboard is on the field.
- **closed (s12): the Next-ledger hint row.** A payment, receipt, contra or journal whose last
  line is filled ends the list with a ghost row "Next ledger, or Enter on empty to finish"
  (tested), and the payment and receipt captures show it. They still end with the keyboard on the
  narration (the mock's keyboard stays on row 1).
- **gap (s12): the mock's mid-entry highlight.** The mock lilac-fills row 1 while the narration is
  filled; ours fills the row the keyboard is on, so with the keyboard on the narration no row is
  filled. Not changed: a highlight on a row without the keyboard would mislead.
- **gap (s12): the Mode** is bare text once set (the mock's look) but a box with a list while empty or focused; the mock's "NEFT / UPI" names two instruments the engine stores as one each. The mock's text names two instruments the engine stores as one each.
- **closed (s12): the overdraft total** "Balance after" is drawn in the bad colour in
  `payment.overdraft` ("-36,000.00"), through `Total.bad`.
- **closed (s12): contra and journal** (see the next section): the Dr/Cr and Group columns, the
  Debit total / Credit total / Difference box from the engine (`LineTotals`), the Cash after
  figure, and the two validation notes.
- **mock states added (round S12, 2026-11-25).** The mock now has `purchase.default`,
  `purchase.validation-error`, `credit-note.default`, `credit-note.validation-error`,
  `debit-note.default`, `debit-note.validation-error`, `journal.default` and
  `journal.validation-error`. Ours is still built from the sales invoice and the shared voucher
  sheet, so the differences below stay until the app round redraws it to these states. The
  earlier note said these were modelled on `sales-invoice.default` and `payment.default`; that
  is now the old basis, not the target.
- **changed: a purchase asks for the bill date, a note for the original invoice's date.** The new
  `Field::ReferenceDate` follows the reference number in the Enter order and is optional
  (a blank is stored as none; a wrong date is refused). Rule 53(1A)(vi) CGST Rules asks a note
  for "the serial number and date of the corresponding tax invoice"
  (taxguru.in/goods-and-service-tax/credit-note-debit-note-section-34-rule-53-cgst-act-2017.html,
  read 2026-11-25). A sale keeps the e-Invoice cell in that slot.
- **changed: place note names the supplier on a purchase and a debit note.** IGST Act s.7 and
  s.8: a supply is inter-State when the supplier's location and the place of supply differ; on a
  purchase the supplier is the party ("Supplier in Karnataka (29), place of supply Maharashtra
  (27). IGST applies."). Without the supplier's state no note is made.
- **changed: a note is titled "Credit Note" or "Debit Note", not "... invoice";** the party field
  reads "Supplier" on a purchase and a debit note.
- **changed: F9, F7, Alt+F6 and Alt+F5 now open their forms with a company open.** F9 on Home
  said "not yet"; F7, Alt+F6 and Alt+F5 did nothing outside the palette.
- **by design: a debit note goes back to the supplier.** The accounting product records a
  purchase return as a Debit Note against the supplier, asking for the original invoice number
  and date (help.tallysolutions.com/tally-prime/purchase-process/purchase-returns-tally/, read
  2026-11-25); ours does the same. Under s.34(3) CGST Act it is the supplier who issues a GST
  debit note when the value rises (taxguru.in page above, read 2026-11-25), so ours is the
  buyer's own voucher. Whether the engine leaves such a note out of GSTR-1 was not checked in
  this round; a GST debit note issued by the company as supplier is not built.
- **gap: eligible ITC is not shown on a purchase.** `ledgerly-gst` has `itc_eligibility`, but
  `Api::invoice_gst_with_bills` does not return it for a draft, and the UI must not compute it.
  The tax box shows the engine's tax lines only; a new API call is needed.
- **gap: the balance line can say "balance not shown".** The API answers Unsupported (never
  zero) when the trial balance has no row for the ledger; a supplier with no posting yet in the
  year has none.
- **data: CN/ and DN/ prefixes** were added to the sample company's two note types (the mock
  shows no note numbers); the sample has no notes yet, so the first reads CN/0001.
- **by design: a prefilled amount is selected when the keyboard arrives; a literal finer than a
  paisa is refused.** The balancing amount a journal, payment, receipt or contra offers on its
  next line (and any prefilled figure on the invoice screens) is selected when the keyboard
  reaches the field, so the first typed character replaces it; before, typing 40000 over
  "42000.00" gave "42000.0040000", which `eval_amount` rounded to 42,000.00. `eval_amount` now
  refuses a typed number with more than two decimal places (`AmountError::TooManyDecimals`);
  results of arithmetic (`100/3`) are still rounded half up to paise, as documented. A mouse
  click keeps the caret where it landed. Provenance: none. The public help pages read
  (help.tallysolutions.com/tally-prime/accounting/payments-and-receipts-tally/, read 2026-11-25,
  and the pre-allocation page found with it) say only "Enter the required Amount" and are silent
  on typing over a prefilled amount, so this is a product choice made to prevent a wrong figure.
- **changed: a credit or debit note's head says "Credit note" or "Debit note".** The subtitle
  reads "Credit note · item mode; ..." and the mode button "Item mode" or "Accounting mode"; a
  note is not an invoice (CGST Act s.34). A sale and a purchase keep "Item invoice".
- **closed (s12): the journal and contra entry states.** `journal.default`,
  `journal.validation-error`, `contra.default` and `contra.validation-error` follow the mock:
  Ledger, Dr / Cr, Group and Amount columns; the foot box shows Debit total, Credit total and
  Difference from `LineTotals` (ledgerly-model, never summed in the UI); Save is disabled while
  the totals differ; the notice says by how much and on which side ("Debit and credit differ by
  360.00. Enter 360.00 more on the credit side to save."). Contra's foot shows Cash after from
  `Api::balance_change`. In `contra.validation-error` the capture clicks the narration so the
  ledger list does not cover the notice (a click now gives the capture window the keyboard).
- **closed (s13): the journal's Balance column.** Neither the mock (`SCREENS.journal`) nor the app
  has a Balance column in the journal entry table (# / Ledger / Dr/Cr / Group / Amount), so there
  was nothing to fill; the earlier note was stale.
- **closed (s12): purchase.default, credit-note.default capture data.** The captures enter the
  mock's data (NST/1150 of 21 Nov, 10 HDPE pipe coils at 2,050.00 and 100 PVC ball valves at
  228.40; credit note of 10 brass nozzle sets at 505.00 against SI/0412) and the engine gives the
  mock's figures: taxable 43,340.00, IGST 7,801.20, round off -0.20, total 51,141.00 (derived:
  20,500 + 22,840 = 43,340; 18% = 7,801.20; 51,141.20 rounds to 51,141.00); credit note 5,050.00
  + 454.50 + 454.50 = 5,959.00. Voucher numbers now agree: the mock reads PB/0145 and CN/0001
  (changed in s12-mock-fixes).
- **mock changed (s12-mock-fixes): the credit and debit note states now use data and refusals the
  app can produce.** `credit-note.validation-error` and `debit-note.validation-error` show the app's
  refusal of an item line with no quantity ("Line 2: quantity: type a number.", tax and total Not
  computed) instead of "70 returned against 67" and "no MS angles line", which no engine rule
  gives. `debit-note.default` is DN/0001 to Rupa Polymers against its bill RP/5398 (the sample's only
  purchase with item lines): 2 HDPE pipe coils at 2,050.00 = 4,100.00, IGST 18% 738.00, total
  4,838.00 (derived by hand in the mock's comment). The app captures `credit-note.*` and
  `debit-note.*` must be retaken with this input (the debit note's party is now Rupa Polymers, not
  Nilgiri Steel). The "input tax reversed" and rule 53 sentences were removed from the debit note:
  the commentary read on 2026-11-25 says nothing on a buyer's reversal.

## Mock fixes (s12-mock-fixes, 2026-11-25)

- **trial-balance.default:** rows are the sample's ledgers, group names as the engine's, CGST, SGST
  and IGST as one ledger each (IGST 9,150.00 Dr: output 20,370.00 less input 29,520.00); total
  1,25,33,742.30 (the mock said 1,25,54,112.30). Still different: the engine's balance date is
  20 Nov, the mock's 21 Nov (no voucher falls between). Closed (s13): the row order. The mock sorted
  by its own list; it now follows the chart's group order as the engine does (Capital Account, Bank
  Accounts, Cash-in-Hand, Loans & Advances, Stock-in-Hand, Sundry Debtors, Duties & Taxes,
  Provisions, Sundry Creditors, Direct Expenses, Fixed Assets, Indirect Expenses, Indirect Incomes,
  Secured Loans, Purchase Accounts, Sales Accounts; ledgers within a group in master order), and
  its Opening stock ledger sits under Stock-in-Hand. The help page read 2026-11-25
  (https://help.tallysolutions.com/tally-prime/accounting-financial-reports/trial-balance-tally/)
  describes group-wise and ledger-wise views but states no order.
- **stock-summary.provisional, balance-sheet.provisional, profit-and-loss.provisional:** the mock
  used a typed -30 that no books could give ("-30 after the sales on record": the sales leave 144).
  They now use the app's capture scenario: a sale of 174 on 19 Nov (item 30 below zero), a purchase
  of 100 on 20 Nov, closing 70, value 70 x 470.00 = 32,900.00 kept and marked Provisional; total
  closing stock 13,90,759.00.
- **audit.default, audit.unsupported:** the edit log has the engine's columns and one line per posted
  voucher (seq 135 to 148, "user 1", "assistant assistant for user 1" as the engine labels them),
  period 1 to 20 Nov. Seq comes from the app's capture (RC/0208 is event 109) plus the seed order.
  `audit.unsupported` keeps its id but shows the engine's empty EditsAfterLock table.
- **profit-and-loss.default, .provisional, new .detailed:** trading account then profit and loss
  (Opening Stock 9,12,000, Purchase Accounts 52,94,000, Direct Expenses 1,84,000, Gross Profit c/o
  36,28,764 | Sales Accounts 85,93,225, Closing Stock 14,25,539; Indirect Expenses 19,50,594.30,
  Nett Profit 16,96,569.70 | Gross Profit b/f, Indirect Incomes 18,400), hand-checked in the mock.
  The help page read (https://help.tallysolutions.com/profit-and-loss-statement-tally/, 2026-11-25)
  documents the horizontal form, the gross profit option and a detailed view, not the group order
  (taken from the owner's brief). Detailed is Alt+F1, as on the balance sheet, because Alt+F5 is the
  debit note key in the app.
- **voucher numbers:** PB/0145, CN/0001, DN/0001 (and the bill photo's accepted purchase PB/0145,
  was PB/0147). **Party GSTINs** in the mock are now the sample's (they differed by digits).
- **gap: the mock's day book still marks PY/0316 as an AI draft ("Bank rule")**; the sample has it
  as a typed voucher by Rohan, so the audit edit log says "user 2" there.
- **closed (s12-all): `import.imported`** now reads "Imported from the other program's company ...",
  as the app's import summary does.

## Item invoices and entry leftovers (round s12 item-invoice-layout, 2026-11-25)

Closed: `purchase.*`, `credit-note.*`, `debit-note.*` heads, columns, buttons and foot follow their
mock states (no Date or Place box on a note's head, no Unit column, "Bill photo Alt+O" on a
purchase, Save only on a note, "Note total", bare "Narration" caption, tax rows named "IGST 18%");
the bill-wise popup table (right-aligned figures, header rule, selected row, "Left open" row); the
journal's short side in the error ink and Save icons; contra's "Account" column.

- **closed (s12-all): the mock changed to the documented behaviour.** Purchase Enter order (supplier
  bill no. and date, supplier, purchase ledger, place; the Date box is the screen's date, F2),
  notes (Party, Date, Place; original invoice no. and date; sales or purchase ledger; titles
  "Credit Note" and "Debit Note"), no "Reason" field on the journal or the credit note, contra
  narration in the foot with a Dr/Cr on every line, credit note tax rows CGST 9% and SGST 9% from
  the engine, and the profit and loss in two sides with Debit/Credit headings, "Trading account"
  and "Profit and loss account" bands and period end 20 Nov (the engine's balance date).
  Closed (s13): the engine now lists Opening stock, Purchase Accounts, Direct Expenses on the
  debit side and Sales Accounts before Direct Incomes on the credit side, as the mock does
  (`profit_loss.rs` `account_rank`, test `the_trading_account_lists_purchases_before_direct_expenses`).
  The order is the owner's brief and the standard trading form: the help pages read 2026-11-25
  (https://help.tallysolutions.com/profit-and-loss-statement-tally/ and the ERP 9 page
  https://help.tallysolutions.com/docs/te9rel65/Reports/Display_Financial_Statements/Display_Profit_Loss_Account.htm)
  name Cost of Sales and Direct Expenses as trading items but state no debit-side order text.
- **data: note numbers.** CN/0001, DN/0001, PB/0145 and CT/0022 are the engine's next numbers; the
  debit note is to Rupa Polymers against RP/5398 as the mock's.
- **gap: the figures of the credit-note hint** ("14 Nov, 67 sets invoiced") are not shown: the
  engine has no return-against-invoice quantity, so the UI cannot honestly draw them.
- **gap: Group names** keep the sample's capitalisation ("Direct Expenses", "Purchase Accounts").
- **gap: ghost "Next ledger" row** is drawn on the journal and contra (the keyboard path needs it).
- **gap: first-row lilac** is not drawn when the keyboard is elsewhere (see the payment entry above).

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
- **closed (s9): the capture is Godavari Drip's three-item invoice** (SI/0418 to Shree Ganesh
  Traders, order no. SGT/PO/1131, narration), entered through the real keyboard path.
  Fictional Traders (no inventory) still opens an accounting invoice.
- **deferred: As Voucher mode.** TallyPrime's Ctrl+H offers Item Invoice, Accounting Invoice
  and As Voucher (help.tallysolutions.com/sales-of-goods-services/). Ctrl+H here switches
  between the first two only, as the mock's toast says; As Voucher is not built (the GST engine
  refuses it).
- **closed (s9): More details (Ctrl+I), the voucher number in the title, the party balance line,
  the place-of-supply note, Alt+R, the toast.** More details opens the bill-reference panel (our
  own design: the mock's More details is only a toast). Bill types are New Ref, Agst Ref, Advance
  and On Account (https://help.tallysolutions.com/advances-received/ and
  https://help.tallysolutions.com/sales-of-goods-services/, read 2026-11-25). The voucher number as
  default New Ref name is our own choice; the page only says the number may be used.
- **gap: the e-Invoice cell** says "Not built yet" (or "Not applicable: B2C"). `ledgerly-gst-formats`
  has the INV-01 type, validator and JSON writer, but nothing maps a voucher to an `EInvoice`
  (seller address, HSN per item, tax per item), so "JSON ready (INV-01)" would be a claim we
  cannot back. Needs that mapping first.
- **gap: the sample GSTIN is synthetic** (`27ZZZZZ0005Z1ZL`, not the mock's), by the sample's
  privacy rule; the rule pack reads "2026.1-slabs", not "2026.11".
- **gap: the toast has no Undo Ctrl+Z.** No undo exists.
- **gap: the empty tax box shows dashes**, not 0.00, until the engine has answered.
- **gap: the rate shows as typed** ("3850"), not "3,850.00": the field is text, and rate
  parsing stays strict (a pinned test refuses "1,000").
- **gap: after a save the party and place are not carried over,** and the party list opens over
  the empty form (the mock keeps the party and shows no list); the toast itself matches except
  for its Undo.
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

- **deferred: the item list shows the name and unit only.** `sales-invoice.item-list` floats under
  the item field as the mock's (`.ta`, index.html:263), with a caption, the picked row on the
  accent tint and a Create row; the mock's rows also carry HSN, rate and stock, and the caption
  its "7 of 7" count. Our entries carry the masters' fields the screen reads, no more.
- **needs a decision: the place of supply is not filled from the party.** The capture types it.
  No help.tallysolutions.com page was read on whether the party's state fills it; it stays as is
  until one is.
- **closed (s9): inter-state and B2C details** now show the place note and the per-rate heads.
- **deferred: validation lists one reason.** `sales-invoice.validation-error` outlines the item
  and quantity inputs in red as the mock does, and lists the line problems in the foot; the mock
  also headlines "N things to fix before saving" with every message in one note.
- **by design: the period-locked banner reads "lock date".** The bar is the mock's (inside the
  card, above the head, lock icon, bold lead). Ours says "Choose a later date to save" where the
  mock says "Press F2 to change the date": the date is a plain field here, F2 is not bound.
- **closed (s9): save confirmation.** `sales-invoice.save-confirm` draws the mock's green bar
  inside the card with Yes and No, and scrolls to it so it is not cut off at the foot.
- **closed (s9): after a save the party and place carry over**, as the mock's saved state shows
  (test `a_save_carries_the_party_and_place_and_opens_no_party_list`).
- **fixed: the saved state.** `sales-invoice.saved` drew nothing because the screen replaced
  itself with a bare "Saving..." label while the worker posted, and the capture waited 60 frames.
  The form now stays up, disabled, with "Saving..." in the foot (test
  `the_form_stays_on_screen_while_the_post_is_pending`), and the harness waits
  `SAVE_SETTLE_FRAMES`. The capture shows the fresh invoice with "Saved." and no toast: the mock's
  "Saved SI/0418 ... Undo" toast is not built.

### Voucher sheet (round S8, 2026-11-24)

The sales, purchase, payment and receipt screens draw to one shared sheet
(`screens/voucher_sheet.rs`): four-column head, line table with bare cell inputs, foot with the
340 point tax box, floating type-ahead. Left on purpose or not built:

- **closed (s9): e-Invoice cell, More details button, number in the title, Alt+R, place note,
  party balance line, Paid out / Balance after, the filled payment and receipt captures.**
- **by design: the date and the Sales ledger are inputs drawn bare.** The mock's are read-only
  boxes; ours stay fields (Tab and screen readers reach them) and show their box when the pointer
  or keyboard is on them. The date is dd-mm-yyyy, not "21 Nov 2026".
- **by design: Disc % and Typed amount hide by default** (the mock has neither). Alt+D shows
  them, and they stay up while any line has a value in them: hiding a typed figure would hide a
  number that changes the amount.
- **deferred: the mock's third line state** (a free-text item row "Drip kit deluxe" in
  `sales-invoice.validation-error`): ours does not offer a text-only item row, so the capture
  has two lines and one reason.
- **by design: no Discard button.** The mock has Save only; Esc leaves the screen, with the
  discard question when the voucher has been touched.
- **gap: the focus ring is 2 points outside the field in the accent**, the mock's outline
  (index.html:65); the hc theme widens it as everywhere else.

## Review queue (card W4, round of 2026-11-23)

- **deferred: the queue lists bank matches only.** The mock lists six kinds (index.html:1023 and
  1203). The engine lists bank matches for the newest stored statement of each bank ledger
  (`Api::review_queue`, `ledgerly-api/src/review.rs`). Five sources have no engine list yet and
  are named on the screen and on Home with the engine's reason: bill photo drafts (no bill
  reader), plain-words drafts (no assistant), new ledger suggestions, possible duplicates (rule
  D-02, index.html:1036), and 2B explanations (no 2B reader, engine item E-3). Closing each needs
  its own engine list; nothing is shown for them until then. (s10 did not reach the queue: `Api::home_nudges`
  already finds possible duplicate payments (rule D-02, `ledgerly-api/src/nudges.rs`), so a
  duplicate row can be listed once the queue takes it from there; no kind was added and
  Y/N/Ctrl+A remain deferred.)
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
- **closed (s9-all): the queue is a table.** `review.default` is a table (Proposal, Source,
  Confidence, Amount) with a selected-row bar, beside "How decisions work" and a "Not connected
  yet" card, as the mock. Pinned by `tests/w4_review.rs` (written after the layout, so not seen
  failing first). Still different: the card lists only the keys that work (Enter, J/K, Esc), the
  rows are bank matches only (no draft purchase, New ledger or 2B rows), and review.empty uses
  Fictional Traders with the not-connected wording and no "Read a bill" button.

## AI and privacy (round of 2026-11-23, card W4)

- **deferred: the cloud tier.** `ai-settings.cloud` (index.html:1907, 1912-1913) shows the provider
  card as set with its key in the system keychain, and a "Removed before sending" list. The app
  has no keychain store wired in and no provider form, so the engine refuses the choice
  (`Api::set_ai_tier(Cloud)` is `Unsupported`, reason shown on screen) and the cloud panel reads
  "Not available" with that reason. Since 2026-11-24 the refusal is drawn as the mock draws a
  state, not a banner: the Provider card shows a "Not available" badge and the engine's reason,
  and the "Removed before sending" card is drawn with read-only switches and a "Not built yet"
  line. The mock's Endpoint and "Stored in system keychain: Set" rows are not claimed.
- **gap: the redaction list.** The mock lists email addresses among the removed items; our redactor
  does not remove them, so the list names only what it does (bank accounts and IFSC, PAN, GSTIN
  and Aadhaar, phone numbers). The mock's Ctrl+E keycap in the log meta is plain text in ours
  (a test pins the single `LOG_META` string); the local-model "None used" badge is data.
- **by design: the features and log cards have a 12 px gap**; the mock draws them touching.
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
- **fixed (S9): GST filing frequency is stored.** The company step has a Monthly / Quarterly list
  like the mock's field; the choice is kept in the company file as an audited `gst.filing`
  setting (`Api::set_gst_filing`, `Api::gst_filing`; tests in `ledgerly-api/tests/s9_gst_filing.rs`).
  Nothing reads it yet: the GST screens do not change with it.
- **gap: the "Company file" field is ours.** The mock has no file location; the wizard asks for
  it. Blank, it shows the path it will use as a hint (`~/Documents/Ledgerly/<company>.ledgerly`,
  made from the company name). The capture leaves it blank and runs with a throwaway home folder
  (`set_home_override`), so the shot shows what a user sees and nothing is written to Documents.
- **gap: City is kept as the first address line.** The model's `Address` has no city field.
- **gap: the state mismatch has no red outline or icon.** `setup.gstin-error` (24 Gujarat chosen
  against a GSTIN starting 27, by clicks on the State list) shows the mock's words and disables
  Continue, as the mock does, but the GSTIN and State fields are not outlined in red and the
  message has no cross icon. The rule is tested in `tests/w4_setup.rs`.
- **gap: the import card on Start opens a screen that cannot finish with no company.** The card
  is a choice like the others (arrows move onto it, Enter opens the import screen). With no
  company open the screen says "No company is open yet" and its import button is disabled: the
  one-step import writes into an open, empty company file. "Open a Ledgerly file" works (round of 2026-11-24, card S8): the system file
  dialog, then the company opens and Home replaces the wizard. "Try the sample company" works
  (S8, sample company): it builds Godavari Drip & Pump Works from `ledgerly-sample` into a
  throwaway file under the user's cache folder (`$XDG_CACHE_HOME` or `~/.cache`, else the system
  temp folder; never Documents), replacing the last one, and opens it. The arrow keys move the
  choice over Create, Open and the sample (Create goes to Open; Open goes up to Create or right
  to the sample); Enter acts on it. The wizard says "Opening the sample company. Its data is
  fictional and nothing you do there is kept." while it builds. Once open there is no banner on
  Home saying the books are sample data, and edits are written to that cache file (it is thrown
  away the next time the card is used, not on exit).
- **gap: Done's "Make your first invoice" is disabled.** No route to a sales invoice from the
  wizard yet, so its card is drawn with a muted label, takes no click and F8 does nothing there;
  the other two launches open their screens, by click or by their keys M and B.
- **gap: a breadcrumb sits above the setup card when it is opened over Home.** The mock draws the
  card with no crumbs. The first launch (`Shell::cold_start`, no company) has none, as the mock;
  opened from the palette or sidebar over an open company, the shell draws "Home > Company setup".
- **gap: the import screen's step-by-step path is not connected.** The five-step wizard
  (detect, choose, pull, map, reconcile) is a state machine nothing in the shell drives, so its
  "Look for a running app" and "Read exported file" controls are drawn disabled with the reason
  (`STEP_BY_STEP_CONNECTED` in `screens/import/view.rs`). Only the one-step import works. The
  mock has no import state; the screen is designed in the app's own kit (page head, card, step
  line, kit buttons, notes and badges).
- **by design: refusals are a toast.** Messages that used to be red text in the key bar are a dark
  pill above it (`ui::toast`, the mock's `.toast`, index.html:290), gone after 6 seconds. With no
  company, F4-F9 and D, B, M, G, and screens that need a company, say "X needs a company. Create or
  open one first." with a "Create a company" action.
- **by design: first-launch chrome words.** With no company the top bar shows the mock's one "New
  company" chip. The AI chip says "AI: not set up" in the neutral style where the mock says "AI on
  this computer" in the green style: nothing is stored to say the AI runs, and the app never claims
  it. The sidebar footer keeps the mock's two lines and check icon but names the app version
  ("Ledgerly 1.0.1") where the mock says "Engine 0.1 mock - rule pack ..." (a mock label; no rule
  pack version is shown anywhere in the app). **fixed (S9): "Hold Alt or Ctrl for more"** is drawn
  at the right of the key bar, and holding Alt or Ctrl really swaps the bar to "Holding Alt" or
  "Holding Ctrl" with the chords the keymap binds with that key (none is listed that is unbound).
  **fixed (S9): setup dates and badge.** Books begin from shows "1 Apr 2026" (typed "1 Apr 2026"
  or 2026-04-01 are both read), the Financial year line has the "April to March" badge, and the
  "fills the state" hint stays on one line. The capture harness parks the pointer off-window, so no
  hover highlight leaks into a shot. The sidebar lists every screen with no company open, as the mock does; what each does
  with no company is in `design/REAL-PATH-AUDIT.md`.
- **gap: no F11 on the Features step.** The mock's line ends "F11 changes this later"; no F11
  feature screen exists, so the sentence is left out. Rows the model has no switch for show an
  off switch that does not move and say why.
- **gap: the backup note has no info icon.** The kit's note draws text only.
- **fixed (S9): fields use the mock's input size.** Setup's text inputs now share one helper
  (`input()` in `screens/setup/mod.rs`: 10 x 7 margin, the input font) and combo boxes use the
  kit's `style_combo`, so rows are the mock's height. Read-only values (Keep, When)
  are still text with a "Not saved yet" hint instead of inputs.
- **data: the mock's sample is prefilled; the wizard starts blank.** The capture types the mock's
  company (name, City "Pune", GSTIN).

## Print (card P1, round of 2026-11-24)

- **gap: no in-app print preview or configuration.** TallyPrime's flow is Ctrl+P (Current),
  then C (configure), I (preview), P (print), as read on "How to Print Invoices/Reports in
  TallyPrime" (help.tallysolutions.com/print-invoices-reports/, read 2026-11-24, last updated
  2026-11-20). The mock now has `profit-and-loss.print-preview`, `print-saved` and `print-failed`
  (round S12); it offers Save as PDF only, as the app does. Ours saves a PDF to a file chosen in a save dialog,
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

## Real-path audit findings (2026-11-24, updated S9)

Full table in `design/REAL-PATH-AUDIT.md`. Fixed in S9: the import screen is in the app's kit and
the welcome's Import card opens it; shortcuts and refusals give a toast with a "Create a company"
action; the tab strip is styled with distinct titles. Still open: the import screen's step-by-step
path is not connected. `setup.welcome` SSIM is light 0.8417, dark 0.8473, hc 0.8147 and `setup.company` is
light 0.7966, dark 0.7952, hc 0.7930; the words of the AI chip and the sidebar footer are by
design (setup section).

## Reports (rounds S8 and S12)

Drawn to the mock in S12: `trial-balance.default`, `outstanding.receivables`,
`outstanding.payables`, `stock-summary.default`, `stock-summary.provisional`,
`profit-and-loss.default`, `profit-and-loss.provisional`, `audit.default`, `audit.integrity`
(light, dark, hc). The screen keeps the engine's own report as the model and draws lines made from
it (`report_view/mock.rs`); no figure is computed in the screen. The register report has no mock
state yet and still draws the generic table.

- **gap: the mock's profit and loss must change to the documented two-sided account (mock round).**
  The accounting product's help ("How to View Profit and Loss Statement in TallyPrime",
  https://help.tallysolutions.com/profit-and-loss-statement-tally/, read 2026-11-25) says the
  horizontal form is the default, that Show with Gross Profit splits the trading profit (cost of
  sales and direct expenses) and that indirect expenses are deducted from the gross profit to give
  the nett profit, and that Alt+F5 (our Alt+F1) shows the detailed form. It does not spell out the
  rows; those follow the owner's brief. The app now draws the trading account (debit side: Opening
  stock, Purchase Accounts, Direct Expenses, Gross profit c/o; credit side: Sales Accounts,
  Closing stock) with both sides totalled, then the profit and loss account (Gross profit b/f,
  Indirect Expenses, Indirect Incomes, Net profit c/o) with both sides totalled; groups by
  default, the ledgers under them with Alt+F1. The engine supplies the c/o and b/f lines and the
  totals (`ledgerly-reports`, tested by hand derivation); the screen only places them. The mock's
  `profit-and-loss.*` states draw Income and Expenses tables (with Discount received as income
  beside Sales, one list of every expense ledger, Total income and Total expenses): the mock
  must change to the two-sided form, and its subtitle keeps "Ledgerly's trading form, with
  sub-totals". Until then the app differs from the mock in layout, by the documented form. Also
  different: the line under Enter drills only the debit side's group when both sides have a row;
  closed (s13, second round): only the Closing stock line wears the "Provisional" badge, as in the
  mock; the gross profit, net profit and totals that follow from it are plain figures and the
  banner (engine reason naming the item, quantity and voucher, dates like "19 Nov 2026") says so,
  with the mock's "Open stock items" button (opens the stock summary; the mock's button opens the
  stock items master, which is a list with no figures). Still different: the button sits under the
  banner text, not inline with it; a period starting after the books' first day is `Unsupported` while a Stock-in-Hand
  ledger holds an opening balance.
- **gap: the provisional states' figures differ from the mock's, by cause.** The mock overrides
  the stock position only (`stockOverride`, pickaxe -30): its closing stock 13,57,859 changes while
  sales stay 85,93,225. The app's fixture posts the real vouchers that make the stock negative (a
  19 Nov sale of 174 pickaxes at 640 = 1,11,360 more sales, 87,04,585, and a 20 Nov purchase of
  100 at 470 = 47,000 more purchases, 53,41,000), so sales, purchases and closing stock all move.
  The mock's provisional state should either post those two vouchers or say it overrides the stock
  only.
- **by design: reports open at the last voucher's date, not today's.** The profit and loss and
  the balance sheet read "to 20 Nov 2026" / "As at 20 Nov 2026" while the top bar says 21 Nov: the
  period ends at the last voucher in the year (20 Nov), as the accounting product's reports do
  ("generated as on the date of the last voucher entry",
  https://help.tallysolutions.com/tally-prime/accounting-financial-reports/trial-balance-tally/,
  read 2026-11-21; `shell/navigate.rs` `default_period`). No voucher is dated 21 Nov, so every
  figure equals the mock's 21 Nov figure. The mock reads its own "today" (21 Nov); it should read
  the last voucher date (20 Nov) on reports, as its own day book does.
- **gap: no print preview, saved or failed state.** `profit-and-loss.print-preview`, `.print-saved`
  and `.print-failed` are not captured: the app has no in-app preview panel (see Print).
- **gap: audit, edits after lock is a table here, Unsupported in the mock.** The engine returns an
  `EditsAfterLock` table; the mock's `audit.unsupported` says it is unsupported. The tab opens and
  shows what the engine gives; no capture is taken (it would not match the mock's claim). The
  mock or the engine must change.
- **gap: the audit edit log lists audit events, not vouchers.** The engine's edit log has one line
  per audit event (156 in the sample: group and ledger creations, then vouchers) with Seq, Date,
  Time, User, Action, Object, Reason, Field, Before, After. The mock lists the 14 November vouchers
  (Voucher, Type, Date, Entered by, Origin, Narration). The page head, tabs, subtitle and keys are
  the mock's; the table is the engine's, so `audit.default` differs in its columns and rows. The
  period in its subtitle is the period asked for (1 Apr to 20 Nov), not the mock's "1 Nov to 21 Nov".
- **gap: the integrity certificate is a table, not a card.** The rows are the engine's real result
  (Result PASS, Events checked, Head, First broken event, Checkpoints); the mock shows a card
  with "Vouchers in the period", "Entries checked against the log", "Hash links broken" and
  "Checked by". The "0 broken links" in the mock is a sample: the app prints the verify result. The
  head hash is longer than the column and is clipped.
- **gap: outstanding bills.** The Overdue column names the engine's ageing bucket ("31-60 days",
  "Not yet due"), not the mock's day count ("55 days", "not due"), because the engine ages a bill
  into a bucket and does not give days; there is no red ink on overdue bills; the lines are in the
  engine's party order (the mock's are by due date); the total line says "Total", not "7 open
  bills"; the footnote is the engine's. The tabs Receivables and Payables switch the report.
- **gap: stock summary.** There is no Cost column (the engine's stock summary has no cost per
  unit) and the total line says "Total", not "Total closing stock" with an engine badge. HSN and
  Unit come from the item masters. The subtitle reads "quantity and value from the stock engine"
  because the engine values by each item's own method, not always at average cost. The provisional
  state's banner naming the item is the engine's footnote, not the mock's alert note.
- **gap: reports are as at 20 Nov 2026.** The mock's clock is 21 Nov; the engine's balance date is
  the last voucher date (20 Nov), as in the balance sheet.
- **gap: the trial balance names groups as the engine does.** "Capital Account", "Duties & Taxes"
  and "Bank Accounts" where the mock says "Capital account", "Duties and taxes" and "Secured
  loans" (the sample's chart differs from the mock's: its bank balances are bank ledgers, not a
  term loan). The mock's 2,39,820.00 creditors are not in the sample.
- **gap: start.recent is not built.** The cold Start screen with a recent-companies table needs a
  recent-companies store (a file kept per user, with the last opened time) and its screen; neither
  exists. The cold start still shows the first-run card.

## Masters, GST, keys (round of 2026-11-24, card S8)

- **gap: the items filter box.** The mock's items tab has no filter; ours keeps one because it
  is functional (masters.items).
- **gap: "Missing: add HSN" badge.** The mock draws an alert triangle on a Bad badge; the kit has
  no such BadgeKind and adding one touches every exhaustive match.
- **closed (s10): the create-ledger form is the mock's compact form** (Name, Under, GSTIN or
  Party details switch, Alias/Opening and the rest behind a "More details" row that opens by
  itself when a hidden field holds text). Still different: that "More details" row and a State
  combo exist in ours, there is no City field. `masters.ledger-detail` follows the same form;
  `masters.gstin-error` and `masters.items` were retaken (items keeps its filter box, sorts by
  the engine's order, uses the Bad badge without the alert icon and has the cursor on row one).
- **by design: the keys note keeps the "Keys in use now" line**, which is true of the app.
- **data: `bank.default` shows the empty state because the fixture has no statement.**
- **gap: GST KPI tiles** are unboxed as in the mock but the light capture is pixel-identical to
  before; remaining GST difference is data (the fixture's rows).

## Sample company (card S8, 2026-11-24)

The sample is the crate `crates/ledgerly-sample`: a versioned, human-readable seed
(`assets/godavari.toml`, plus the bank statement `assets/saptagiri-nov-2026.mt940`) of masters and
vouchers, no totals. `ledgerly_sample::create` posts it through `ledgerly-api` (`Api::create`,
`Command::*`, `import_bank_statement`, and `LockPeriod` last, 31 Oct 2026), so every figure on
screen is the engine's. The capture harness opens it as `Fixture::Godavari`, with the clock fixed
to the mock's 21 Nov 2026 (`navigate::set_clock_override`). Captures that need another company
stay on Fictional Traders (`review.empty`, `bank.empty`, `home.empty`) or Godavari
with extra vouchers (`balance-sheet.provisional`, a capture-only fixture: 174 pickaxes sold on
19 Nov with 144 on hand, 100 bought on 20 Nov; its reason text names the item and date, not the mock's wording).

How the figures agree with the mock (engine, light captures, 21 Nov 2026):
- Match to the paisa: every party balance (Shree Ganesh 1,93,500.00; Mehta 1,05,020.00; Lakshmi
  1,87,600.00; Patil 1,32,670.00; Kisan 0.00), receivables 6,18,790.00, payables 7,07,970.00, cash
  78,337.50, Saptagiri 7,27,524.50, closing stock 14,25,539.00, fixed assets 23,62,846.00, the
  balance sheet total 56,09,537.00, the Day book of 20 Nov (five vouchers, 3,74,967.50), GSTR-1
  4A (4 invoices, 3,43,585.00 taxable, 50,535.30 tax), 7 B2C others (9,640.00, 482.00), HSN summary
  3,49,385.00 with the pickaxe missing its HSN, and the Review queue badge (6).

Mismatches between the mock's totals and the engine's, and why:
- **closed (s9): capital, profit, IGST and Opening stock now match the mock.** Capital account
  20,00,000.00, Profit and loss 16,96,569.70 and Current liabilities 10,72,967.30 come out of the
  engine (tested in `ledgerly-sample/tests/sample.rs`). Three seed items had caused the old gap:
  the mock's Opening stock ledger (9,12,000.00 Dr, now seeded as an opening balance on a ledger
  under Stock-in-Hand since s12; it was an expense-nature ledger under Direct expenses), the
  October IGST input credit on the stock bill RP/5398 (2,74,164.36, now set off by a journal against IGST collected in October's receipts, so IGST is
  nil on 1 Nov as in the mock and 9,150.00 Dr on 21 Nov) and that bill's round off (0.36, now
  cleared in the same journal). Capital is no longer the balancing figure: the partners' opening
  capital is 24,00,000 and 11,00,000 less drawings of 9,00,000 and 6,00,000. The stock items still
  have no April quantity (the engine's balance sheet refuses valued opening stock), so the stock
  register shows the quantities as bought in October.
- **data: GST ledgers are one per head.** The mock keeps CGST/SGST/IGST output and input ledgers;
  the engine's GST report needs one ledger per tax head, so the sample has CGST, SGST, IGST.
- **data: opening-bill invoices (SI/0372, 0350, 0381, 0390) have no tax split.** They are dated
  before 1 Nov and carry only the party, sales and bill lines the mock states.
- **data: voucher numbering.** Numbers are typed on each voucher as the mock shows them; the mock's
  next number (SI/0418) is not reproduced: the engine numbers a new voucher from its own series.
- **data: no 2B or IMS rows.** The seed has no portal data, so the mock's reconciliation states
  cannot show. (The day book's "Entered by" and AI origin are recorded since s9; "Image attached"
  is not, see "Day book".)
- **gap: GST cash payable on Home.** The mock shows 39,498.00; GSTR-3B computes but the payable after
  set-off needs 2B/IMS and set-off, so it cannot be computed, so the tile says "Not supported yet" with its reason.
- **gap: the invoice screen cannot save to a bill-wise party.** Every Godavari party is bill-wise
  (as the mock's), and the sales invoice screen does not enter bill references yet, so the
  invoice captures show the engine's "bill reference on a bill-wise party" note in place of the
  tax panel. The seed's own invoices are posted with their bill references through the API.
- **by design: identifiers differ from the mock.** GSTINs use the fake PAN pattern `ZZZZZ####Z`
  with a valid check character (company 27ZZZZZ0000Z1ZQ); bank accounts, IFSC codes and phone
  numbers are obviously fake. The mock now uses the same synthetic GSTINs (27ZZZZZ0000Z1ZQ and the like).
- **by design: UQC codes.** Units map to the GST portal's UQC codes from the portal's published
  list as the author remembers it (roll ROL, set SET, nos NOS), and the coil unit maps to OTH;
  the list was not re-read for this card.

## Round S12b: states the app reaches (2026-11-25)

- **gap: the print busy line.** The app shows no text while a PDF is prepared: `print_job.rs`
  `is_busy` only keeps the frame repainting. The mock state `profit-and-loss.print-busy` shows
  "Preparing the PDF. The save dialog opens when it is ready." That line is a proposal, not a copy
  of the app. Decide whether the app should show it.
- **by design: the print error is the app's words.** `print_job.rs` `print_notice` gives
  "{title} was not printed: {reason}". The mock's `profit-and-loss.print-error` uses that form with a
  sample reason ("the store is closed").
- **gap: the import wizard is not connected.** `import/view.rs:198` sets
  `STEP_BY_STEP_CONNECTED = false`. The app disables the step-by-step controls and says so. The mock
  shows the same (`import.default`). The choose-company, pull, map and reconcile steps have no mock
  state, because no UI path opens them. See MOCK-COVERAGE.md, "Still MISSING".
- **by design: the import summary counts are sample data.** The text is `ImportSummary`'s Display
  (`ledgerly-api/src/tally_import.rs`). The counts come from the mock's sample company, and the
  sample error and refusal texts are illustrative.
- **gap: the stock item form is a separate screen.** The app's empty state for stock items offers
  Create stock item (Alt+C). The mock's `masters.create-item` shows the same fields (Name, Alias,
  Under, Unit, Valuation, HSN/SAC, Track batches) but no working Save. Validation (a unit is needed)
  is not wired.
- **gap: the masters balance hint is hover text only.** `masters/balances.rs` LOADING_HINT and
  FAILED_HINT sit on the heading. The mock shows dashes and does not show hover text in captures.
- **by design: rail resets on navigation in the mock.** The app keeps the rail until it is toggled
  again (`shell/mod.rs` `rail: bool`). The mock sets the rail from each hash, so a navigation
  folds it back out.
- **by design: the new-tab title.** The second tab of the same screen is "Company setup 2" (the
  app's `unique_tab_labels`). The mock's tab strip uses the same rule.
- **by design: the GST tile text.** "Needs GSTR-2B" and "not readable yet" come from
  `screens/home/kpi.rs` (GST_REASON). The mock's home.gst-unsupported shows the tile with those
  words in place of the figure.
- **by design: GST not-loaded and unsupported texts.** "Not loaded for this period yet." and
  "Not supported yet: what (reason)" come from `gst/draw.rs` and `wire.rs`. The sample reasons
  ("no verified rule pack", "rate pack pending") are the app's test fixtures, not engine output.
- **gap: the masters "Not supported yet" notice.** `masters.rs` line 2068 shows it after a masters
  action returns Unsupported. The mock has no state for it, because the sample engine never refuses
  a masters action. See MOCK-COVERAGE.md.
- **gap: the open-company busy line.** `shell/welcome.rs` shows "Working on the company file..."
  while a company opens. `WelcomeScreen` is built only by `Shell::with_welcome`, which the tests call
  and the app does not. The mock has no state for it.
- **data: the credit and debit note footers.** The credit note footer cites CGST Rules r.53(1A)(g).
  The debit note footer no longer cites s.34(3); the debit note is a buyer's return, and its GST
  basis is an open item in MOCK-COVERAGE.md.

- **closed (s13): stock-summary.provisional truncation.** The Value cell and the total draw the
  amount with a Provisional badge and the banner leads the table (`ViewCell::Provisional` drawn by
  `generic.rs`, banner from `mock::provisional_banner`). The balance sheet, trial balance and day
  book were checked: only the balance sheet, P&L and stock summary produce provisional cells, and
  none truncates. Closed (s13, second round): the reason is the engine's sentence ("Steel pickaxe
  2.5 kg first went below zero on 19 Nov 2026 (-30 nos after SI/0416); a purchase on 20 Nov
  brought it back to 70"; item name, quantity and voucher number come from the stock moves via
  `ledgerly_stock::negative_episode`, never from the UI), the stock summary has the Cost column
  (the engine's closing rate, nine columns) and the total reads "Total closing stock" with the
  engine tag; the quantity 70 is right and the mock's table agrees (70, 470.00, 32,900.00). Report
  date: TallyPrime generates a report "as on the date of the last voucher entry"
  (https://help.tallysolutions.com/tally-prime/accounting-financial-reports/trial-balance-tally/,
  read 2026-11-25); the app does this (20 Nov in the capture company) and the mock's report
  screens now say 20 Nov too. Still different: the mock's subtitle says "value at average cost"
  (the app states the engine's method per item), and the Item column is narrower than the mock's.
- **closed (s13): contra.validation-error.** Save is greyed, the error note has the alert icon and
  the "Lines as typed balance" hint sits below the notice. Still different: the keyboard is on the
  narration box in the capture (the mock's is unfocused).
- **gap (s13): sales-invoice.period-locked.** Closed (s13, second round): the "1 thing to fix before
  saving" note, the Locked badge on the date (the app's badge has a cross where the mock's has a
  lock icon) and F2 as a keycap in the bar. Still different: the e-invoice cell reads "Not built
  yet" where the mock says "JSON ready (INV-01)": the engine API (`ledgerly-api`) does not report
  e-invoice JSON for an invoice (only the `ledgerly-gst-formats` writer exists, not wired to the
  voucher), so claiming a file is ready would be a wrong statement; the third empty line is not
  drawn (the app drops a blank trailing line when the keyboard leaves it); the party's balance
  line differs because the capture company's ledger balance is not the mock's.
- **closed (s13): profit-and-loss.detailed** has an app capture (Alt+F1); it differs from the mock
  in the same two-sided layout points as the default state (bold group lines, the figures of the
  capture company).
