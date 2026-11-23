# Known gaps between the app and the mock

Differences seen in the side-by-side captures (`design/COMPARE.md`) that a parity round left on
purpose or could not close yet. Each entry says where the mock shows it, why it was left, and what
closing it needs. Remove an entry only in the commit that closes it.

Kinds: **bug** (the app is wrong or missing something it should do), **deferred** (waits on a
feature that is not built), **data** (the capture company lacks what the mock's sample shows),
**by design** (we differ on purpose; kept here so nobody "fixes" it back).

## Palette (round of 2026-11-23, commit d4f0bcb)

- **bug: command rows are in keymap order.** The mock's `> ` list starts New sales invoice, New
  purchase, New payment, New receipt, Read a bill photo, Import bank statement, ... (index.html,
  `palResults`, Commands). Ours lists New contra voucher first. Needs an order for command rows.
- **bug: Recent rows have a plain detail.** The mock's Recent rows say "As at 21 Nov 2026",
  "3 sections ready, 1 needs correction", "Ledger · Sundry debtors · 1,93,500.00 Dr"
  (index.html:1115). Ours keep the detail the row had when chosen. Live details need the engine's
  figures at reopen time.
- **bug: "Up / Down" on other screens.** The palette shows "↑ ↓" and the balance sheet and day
  book "J / K" (the mock's words); the masters, generic report and invoice screens still say
  "Up / Down".
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

- **bug: no Export.** The mock has an Export button and Ctrl+E (index.html:1576, 1624, 1634).
  Export of the report on screen is not built, so neither the button nor the key is offered.
- **bug: other tables keep the older row height.** The balance sheet and the day book draw the
  mock's `table.t` lines by hand (34-point rows, rules, the selected bar: `ui::table::paint_row`).
  The masters, bank, invoice and generic report tables still use `TableBuilder` at 30 points
  with egui's selection tint; each moves over in its own round.
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

## Day book (round of 2026-11-23)

- **deferred: Entered by and Origin columns, and Ctrl+I Origin** (index.html:1650-1653, 1659).
  The engine records neither who entered a voucher nor whether it came from an AI draft or a
  bank rule (engine item E-4). The table keeps the engine's Status column in their place and the
  footnote says so; Ctrl+I is not offered.
- **deferred: narration under the party** (index.html:1652, the `meta` line). The engine's day
  book has no narration field, so Particulars shows the party alone and every row is one line.
- **deferred: the empty state's card and voucher buttons** (index.html:1647). Ours shows the
  shared empty state with the keys in its sentence; not captured yet (`day-book.empty`).
- **needs a decision: F2 offers the shown day.** TallyHelp says F2 in the Day Book first offers
  the day after the current date; ours fills in the day shown. Check the TallyPrime page (only
  the Tally.ERP 9 page was read) before changing it.
- **bug: the 16-point gap under the page head is only on the report screens.** The mock's
  `.phead` has `margin-bottom:16px` (index.html:121); the balance sheet, day book and GST
  returns add it (`HEAD_GAP`), the other screens still follow the head with egui's item spacing. Move it into
  `ui::page_head` in a round that recaptures every screen.
- **data: voucher numbers and the sample day.** The capture company numbers vouchers 1, 2, ...
  per type and its last voucher is 20 Nov; the mock shows "SI/0417" and five vouchers on 20 Nov.
- **by design: Status column.** Kept from the engine (it shows a cancelled voucher as such) until
  Entered by and Origin exist.

## GST returns (round of 2026-11-23)

Captured states: `gst.gstr1`, `gst.gstr3b`, `gst.recon-empty`. Not captured yet:
`gstr1-correction`, `recon`, `recon-explain`, `recon-error` and `recon-locked`. The app cannot
reach them, because no 2B file can be read.

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

