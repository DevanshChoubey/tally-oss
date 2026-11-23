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
  `.phead` has `margin-bottom:16px` (index.html:121); the balance sheet and day book add it
  (`HEAD_GAP`), the other screens still follow the head with egui's item spacing. Move it into
  `ui::page_head` in a round that recaptures every screen.
- **data: voucher numbers and the sample day.** The capture company numbers vouchers 1, 2, ...
  per type and its last voucher is 20 Nov; the mock shows "SI/0417" and five vouchers on 20 Nov.
- **by design: Status column.** Kept from the engine (it shows a cancelled voucher as such) until
  Entered by and Origin exist.
