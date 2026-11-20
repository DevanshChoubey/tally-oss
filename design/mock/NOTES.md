# Ledgerly (working name) UI mock: notes

`index.html` is a single self-contained file (inline CSS and JS). The only network request is
Google Fonts (Public Sans); offline it falls back to the system UI font. It opens from `file://`.
All companies, parties, GSTINs, invoice numbers and amounts are fictional. GSTIN check digits are
not valid, and the GST rates on items are for illustration only. Real rates come from the item
master and the current rate notifications.

Written on 2026-11-20, before any file in `docs/research/` existed. See "Revisit" below.

## Screens

| Screen | How to reach it | What it shows |
|---|---|---|
| Home | `H`, `Esc` from most screens | KPI tiles (cash and bank, receivables, payables, GST payable for the month), entry points for vouchers, reports and masters, a "needs attention" list (IMS pending, unmatched bank lines, due dates). Each KPI drills into the Balance sheet group or GSTR-3B. |
| Sales invoice | `F8`, `S` | Item-invoice mode on a ruled "ledger sheet". Enter moves field to field. Type-ahead lists for party, place of supply and items accept only existing masters. GST recomputes on every keystroke: CGST+SGST when place of supply is Maharashtra (company state 27), IGST otherwise. Rate-wise tax table, round-off, total, amount in words in the Indian system (lakh, crore). Enter on an empty item row jumps to narration; Enter on narration shows a save confirmation. |
| Payment / Receipt | `F5` / `F6` | Single-entry style: choose the cash or bank account once, then the ledgers. Shows the account balance now and after saving, and flags overdraft. |
| Balance sheet | `B` | Vertical (Schedule III style) layout. Drill path: group, sub-group, ledger (running balance), voucher (read-only detail). Breadcrumbs, `Esc`/`Backspace` up one level, condensed/detailed toggle (`Alt+D`), period switch. Totals are summed from ledgers and a "both sides agree" check is shown. |
| Day book | `D` | Vouchers for 20 Nov 2026, each drillable to the voucher. |
| GST returns | `G`, tabs `1` `2` `3` | GSTR-1 section summary and HSN summary. GSTR-3B 3.1(a), 4(A)(5), net cash payable. 2B and IMS reconciliation: match status (matched, value differs, date differs, not in books, possible duplicate, not in 2B yet), books vs 2B values, difference, and Accept / Reject / Pending per row. Changing an action recomputes GSTR-3B. |
| Masters | `M` | Read-only list of the sample parties and items. |
| Assistant panel | `Ctrl+J`, `F9` (bill) | Three tabs: **Bill photo** (read a sample bill, then a proposed purchase voucher with an engine check against the printed total), **Describe** (natural-language entry such as "paid rent 25k to Sharma Properties by HDFC"), **Bank matching** (statement lines with suggested matches or new vouchers). |
| Go to | `Ctrl+K` or `Alt+G` | Command palette over screens, reports, ledgers, groups, voucher numbers (`#`) and commands (`>`). |
| Keyboard map | `?` or `Ctrl+/` | Full shortcut overlay. |

Light and dark themes follow the OS by default. `Ctrl+Shift+L` or the sun button switches theme,
and the choice is kept in localStorage.

## Keyboard map (highlights)

- **Anywhere:** `Ctrl+K` / `Alt+G` go to, `?` keyboard map, `Ctrl+J` assistant, `Alt+F3` company,
  `Alt+F2` period, `Esc` back.
- **Voucher types:** `F8` sales, `F5` payment, `F6` receipt, `F9` purchase from bill. These follow
  the habit Indian accountants already have, see the open questions.
- **Single-letter navigation when focus is not in a field:** `H` home, `B` balance sheet, `D` day
  book, `G` GST, `M` masters, `S` sales.
- **Voucher entry:** `Enter` next field, `Shift+Enter` previous, `↑`/`↓` in lists, Enter on an empty
  item row ends the list, `Ctrl+Enter` save from anywhere, `Ctrl+D` delete line, `Alt+C` create a
  master inline (stub).
- **Reports:** `↑`/`↓` rows, `Enter` drill in, `Esc`/`Backspace` up, `Alt+D` detailed.
- **GST reconciliation:** `J`/`K` rows, `A` accept, `R` reject, `P` pending.
- **Assistant proposals:** `Ctrl+Enter` post, `Ctrl+E` open in the voucher screen, `Esc` discard.

A context strip at the bottom always shows the keys for the current screen.

## Design decisions

- **Subject-driven palette.** The look borrows from the Indian account book (bahi and ruled
  ledger paper): cool paper background, blue ruling lines, a red double margin rule on the voucher
  sheet, and indigo ("neel") ink as the single accent. Turmeric marks pending items, leaf green
  marks matched or engine-checked items, and the margin red marks differences and errors. This keeps
  it clearly apart from the incumbent's trade dress.
- **One signature element.** The voucher sheet (ruled lines, red margin, line numbers in the
  margin, a pale "active field band" with an indigo underline) carries the identity. Everything
  else stays quiet: flat surfaces, hairline borders, small radii.
- **Dense, numeric typography.** Public Sans at 13px base, `tabular-nums` everywhere, amounts
  right-aligned with Indian grouping via `Intl.NumberFormat('en-IN')`, totals with a double rule
  (the accountant's convention). No monospace or all-caps labels.
- **Keyboard first, mouse friendly.** Every action has a key and a visible key hint. Type-ahead
  lists never accept free text that is not a master; the escape hatch is explicit (`Alt+C`).
- **Never a wrong number, made visible.** Balance sheet totals are summed, not typed, and show an
  "agree to the paisa" check or a difference. Ledger views check that the running balance equals
  the group figure. Where the sample has only a closing balance, the screen says so instead of
  inventing vouchers.
- **AI proposes, engine computes, you confirm.** A pinned contract at the top of the assistant
  panel explains this. Every proposal has two boxes: "How the assistant read it" (badge:
  assistant, turmeric) and "Proposed voucher" (badge: engine, green), where all amounts come from
  the same tax function the voucher screen uses. The engine checks its total against the printed
  bill total. Nothing posts without an explicit `Post` / `Ctrl+Enter`. The assistant refuses to
  invent ledgers ("Ramesh" example) and explains choices such as debiting Rent instead of the
  landlord.
- **Local-first cues.** "Saved locally" in the top bar. The bill photo note says the image stays
  on the machine unless cloud reading is turned on.

## Mock limitations

- One sample company. Switching company shows a toast.
- The period switcher changes labels only. The note under the report says so.
- GSTR-1 figures are static sample data. The GSTR-3B ITC baseline ("other matched invoices") is a
  constant chosen so that the opening state equals the books' GST payable (₹1,00,994.00).
- Saving a voucher shows a toast and resets the form. It does not update balances.
- IMS "no action" (deemed accepted on filing) is not modelled separately from "Pending". The mock
  only uses explicit actions.

## Revisit when the research lands

- **02-ux-keyboard.md:** confirm the voucher-type keys (`F4`–`F10` habits vs a new scheme). Check
  `Alt`/`Ctrl` chords against muscle memory (for example `Alt+C` create master, `Ctrl+A` accept, and
  Enter-to-advance vs Tab). Decide whether to ship a "classic keymap" profile. Check that F5 and F6
  conflicts with the webview/OS are handled in the Rust shell. Decide whether amounts take
  expressions such as `25k` or `=1450*40`.
- **GST and compliance research:** the exact IMS action set and deemed-acceptance rules, GSTR-3B
  table numbering and the ITC set-off order, e-invoice and e-way bill fields on the voucher, HSN
  digit requirements by turnover, B2C large thresholds.
- **Data model research:** voucher numbering series, the group tree (vertical Schedule III vs
  horizontal T-format as default), bill-wise details on receipts, godown and batch fields on items.
- **AI research:** whether bill reading is on-device or cloud by default, the confidence display,
  and how the TDS/TCS rule checks are presented.
- **Accessibility:** run a contrast audit on the turmeric and leaf badges in both themes. Add screen
  reader announcements for recomputed totals. Check focus order inside the assistant panel.
- **Layout:** the side nav may be unnecessary for keyboard users. Consider collapsing it to a rail by
  default. Test at 1366x768, the most common accountant laptop resolution.
