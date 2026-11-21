# Round 3: open decisions settled by Tally's documented behaviour (2026-11-21)

Owner instruction (2026-11-21): model TallyPrime for every open behaviour question. Confirm each
rule from a public page, never from memory, and copy the behaviour, not the text. Where no page
settles a question, keep the safe default (`Unsupported`, refuse, or no binding) and mark it
pending. This supersedes the owner defaults listed at the top of
`2026-11-21-review2-decisions.md` for M3 and G7, and the K3, U3, U4 and U11 items there.

The coordinator read every page below in the browser on 2026-11-21. Tests that rely on them cite
the URL and the date as `derived`.

| Page | What it settles |
|---|---|
| <https://help.tallysolutions.com/docs/te9rel65/Reports/Display_Registers_Ledgers/Bank_Reconciliation.htm> | K3: BRS from book balance and bank dates |
| <https://help.tallysolutions.com/tally-prime/keyboard-shortcuts/keyboard-shortcuts-tally-prime/> | U3, U4, U11: Ctrl+A accept, Ctrl+Enter alter, Ctrl+J exceptions, Alt+S stock query |
| <https://maas.freshdesk.com/support/solutions/articles/501000041719-alter-and-delete-ledgers-in-tallyprime> | E2: no delete while a voucher or another master uses it |
| <https://help.tallysolutions.com/tally-prime/inventory/inventory-storage-using-godowns-locations-tally/> | M3: no inventory prerequisite for godowns; E2: godown delete only when unused |
| <https://help.tallysolutions.com/docs/te9rel66/Tax_India/gst/gst_accounting_for_ineligible_itc.htm> | G7: user marks ineligibility on group/ledger, stock group/item, or company; invoice may change it |
| <https://help.tallysolutions.com/stock-transfer-between-godowns/> | S5: stock journal source reduces, destination increases, total unchanged |
| <https://help.tallysolutions.com/physical-stock/> | S5: the count becomes the book quantity |
| <https://help.tallysolutions.com/tally-prime/sales-process/sales-return-tally/> | S5: a sales return (credit note with items) increases stock |
| <https://help.tallysolutions.com/tally-prime/financial-controlling/scenarios-tally/> | S5: optional, memorandum and reversing vouchers stay out of the books, seen only in scenarios |

## What changed (branches r3/bank, r3/app, r3/engine, r3/gst, r3/store, merged into integration)

- **K3 (bank):** the books side of the BRS is always a value: balance as per books on the as-of
  date, uncleared deposits and payments (bank date blank or after as-of), and balance as per
  bank = books − deposits + payments. The statement balance and the difference are a separate
  `Outcome`: a value only when the statements given cover the as-of date exactly. No statement,
  a date in a gap, disagreeing overlaps, or an as-of on or after the last statement line make
  only that side `Unsupported`. `Brs` fields renamed (`statement_balance`, `balance_as_per_bank`)
  and `is_reconciled` now returns `Outcome<bool>`. The page does not state the sign of each
  adjustment or the direction of the difference: the sign follows from the identity, and the
  difference (statement − balance as per bank) is our convention.
- **U11 (app):** assistant Accept is Ctrl+A. Ctrl+Enter no longer accepts.
- **U3 (app):** Ctrl+J is free on the home screen. Describe-a-transaction is Ctrl+Shift+J, a key the
  shortcut page does not list. Tally has no assistant, so this choice is ours.
- **U4 (app):** Alt+S no longer switches Dr/Cr. No page documents a key for that, so the on-screen
  switch stays and the keyboard binding is pending. The U2 keymap test that every action has a
  key now exempts SwitchSide and asserts that it has none (a scope change, flagged here).
- **E2 (engine):** new `EngineError::InUseByMaster { what, by }` for every master-on-master delete
  refusal (unit, stock group, stock item, godown, batch, cost category, cost centre, group).
  Stock movements that block a delete stay `Invalid`. Several of these cases are pending: the
  pages settle ledgers, groups, units and items, not batches, cost centres or compound units.
- **G7 (gst + model):** new `ineligible_itc: Option<bool>` on Ledger, Group, StockItem, StockGroup
  and LedgerLine (the invoice override), plus `Company::ineligible_itc_default`. An explicit
  setting decides: invoice line, then the nearest master, then company. **When nothing is set at
  any level, the round-2 HSN/SAC rules still apply** (blocked s.17(5) category, missing HSN or
  mixed categories give `Unsupported`, asking the user to mark the item or ledger). Tally would
  claim the credit there. We refuse rather than show a wrong number (rule A1). Conflicts the page
  does not settle (item against ledger, master against company) are `Unsupported`, pending. The
  new fields are serde-default, and old data decodes as `None` (store round-trip test added).
- **M3 (model):** godowns no longer require inventory. Batches still do.
- **S5 (model + store):** `StockMove` has a `status`. A credit note with items is a stock-in. An
  optional voucher is carried with status Optional and left out of actual stock. Memorandum and
  reversing journal vouchers are left out. A physical stock count equal to the book quantity
  gives no move. New typed `StoreError::Unsupported { voucher, reason }` replaces the misuse of
  `Encoding`.

## Still pending (no page settles these)

- **Stock journal projection:** `InventoryLine` has no source/destination side, so the documented
  rule cannot be applied to stored data. Stays `StoreError::Unsupported`. Interface request filed.
- **Physical stock adjustment value:** the page gives the quantity, not the value or the order
  against other movements on the same date. A count that differs from the book quantity, or one
  on the same date as another movement, stays `Unsupported`.
- **Post-dated, cancelled and debit-note stock effects:** not documented. Stay `Unsupported`.
- **K3 statement end date:** an as-of on the last statement line date stays `Unsupported` until the
  statement's closing date is carried through `RawStatement` (request already filed).
- **G7:** reversal of credit that becomes ineligible later (journal) is not modelled. GSTR-3B
  placement of ineligible purchases differs between Tally's page (4D(1)) and the CBIC circular
  noted in `_reports/tally-research/gst.md` (4(B)(1)). Unchanged for now. No Tally XML tag is
  mapped for the setting yet.
- **U4:** a keyboard key for the Dr/Cr switch.
- **Kept pending from round 2:** B1 vs X1 company-list request (needs an observed capture), cheque
  bounce rule (K4), name case and space rules, amount expressions (U5), credit-note posting
  sides, and the GST JSON format items (F3). The owner has no TallyPrime install, so observed
  captures wait.

## Integration fixes after merge

Merges `9b83435`..`d92d6e1` and the r3/gst merge, plus two integration commits. Fixes: lint (a
redundant closure, doc URLs, a `&str` argument, `assert_eq` for empties, `too_many_lines` expects
on two GST fixtures, a test allow-list, a doc-list indent), rustfmt, and the G7 agent's scripted
field insertion, which had also put the new field into six enum variants (removed). The s5
load_books test now expects the typed `StoreError::Unsupported`. The g10 fixture gained the
`GroupId(1)` group that its ledgers already named, because ITC now walks the ledger's group chain.
No expected value changed in those two fixes.
