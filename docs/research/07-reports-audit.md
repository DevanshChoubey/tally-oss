# 07 — Reports, MIS and Audit

Researcher area: financial statements, books and registers, MIS, report configuration, drill-down,
export/print, statutory audit trail (edit log), auditor features, dashboards.
Date: 2026-11-20. Current vendor release: TallyPrime 7.1 (launched 27 June 2026, vendor claim);
7.0 launched 19 Dec 2025 (vendor claim).

Clean-room note: everything below is summarised in our own words from public web pages (vendor help
centre, vendor marketing, legal commentary, Big-4 publications). No binaries were downloaded or run.
"Vendor claim" = stated by Tally Solutions and not independently checked. "Unconfirmed" = inferred
or from older (Tally.ERP 9) docs and not re-verified against 7.x.

Research limit: the shared web-search budget ran out before the "what users love/hate" sweep of
forums (Reddit, Tally community, Play Store-style reviews) was done. That section is built from what
the vendor and practitioner sources reveal indirectly and is marked accordingly. Follow-up needed.

---

## Summary

1. **Reports in TallyPrime are a small set of generic engines, not hundreds of bespoke screens.**
   Almost every report is one of: (a) a group/ledger hierarchy roll-up (Balance Sheet, P&L, Trial
   Balance, Group Summary, Ratio Analysis, Cash/Funds Flow), (b) a voucher list filtered by
   something (Day Book, Ledger Vouchers, registers, Cash/Bank Book, exception lists), or (c) a
   bill-wise outstanding view (receivables/payables/ageing). All share one configuration grammar:
   F2 period, F4 context switch, Ctrl+B "basis of values", F12 configure, Alt+C/Alt+N columns,
   Ctrl+J exception filter, Ctrl+H change view, Alt+F1/Alt+F5 detailed/condensed, Ctrl+L save view,
   Ctrl+E export, Alt+M share. Drill-down is Enter on any line, Esc goes back, ending at the voucher
   in alteration mode. Copying this "uniform grammar over a few engines" is the single biggest
   design lever.
2. **Statutory audit trail is non-negotiable for companies.** Rule 3(1) proviso, Companies
   (Accounts) Rules 2014, for FYs starting on/after 1 April 2023: software must record an audit trail
   of every transaction, an edit log of every change with date, and the trail must not be capable of
   being disabled. Auditors report on it under Rule 11(g) (enabled, operated all year, all
   transactions, not tampered, preserved). Retention: books, including the trail, 8 financial years
   (Section 128(5)). Daily backup on servers in India (Rule 3(5), from 5 Aug 2022).
3. **Tally ships two editions** — regular TallyPrime (Edit Log optional, can be toggled) and
   "TallyPrime Edit Log" (always on, no off switch); only the latter is presented as compliant
   (vendor claim). Edit Log covers vouchers, ledgers, stock items and groups (from Rel 2.1), stores
   full versions, shows a version list per object (Alt+Q) and a red-highlighted diff between
   versions, plus voucher-type-wise and master-wise summaries. It does **not** track Company
   Alteration changes (vendor docs). Big real-world failure mode (KPMG FY25 study of top listed
   companies): 59% had modified Rule 11(g) reports; the top causes were trail not enabled for
   privileged users (32 of 55) and not enabled at database level (31 of 55). A local-first app must
   make the log tamper-evident at the storage layer, not only in the UI.
4. **Schedule III is not native in Tally.** 7.1 added an Excel-based add-on (Division I, non-Ind AS
   companies; separate NCE add-on for individuals/HUF/firms/LLPs) that exports the trial balance
   into a mapping template. Single company only, no deferred tax, current/non-current split is
   manual, optional vouchers excluded. This is a clear gap where a native, ledger-mapped Schedule
   III with notes, ageing schedules and ratios would beat Tally.
5. **Dashboards exist since TallyPrime 4.0** (tiles: sales/purchase trend, assets/liabilities,
   cash/bank, cash in/out, receivables/payables, top groups/ledgers, ratios; Banking Activities tile
   from 6.0). Tiles have their own period, can be added/removed/arranged, and are exportable
   (vendor claim). Charts are otherwise rare (Cash Flow has a monthly bar graph).

---

## Report inventory

Priority: **MVP** = needed to replace Tally for a typical SME on day one; **v2** = needed for
CA/company users within the first year; **later** = niche or superseded.

| Report | Purpose | Key options (Tally behaviour) | Drill-down path | Priority |
|---|---|---|---|---|
| Balance Sheet | Position at a date: Capital/Liabilities vs Assets by primary group | F2 date; Alt+F1 detailed; Alt+C new column (another period, another company, or "method of stock valuation" to compare stock values); Ctrl+B scale factor; F12 vertical/horizontal, show percentages, show profit opening/current split | Line (group) → Group Summary → sub-group → Ledger Vouchers → voucher. Closing Stock line → Stock Summary → stock group → item → Stock Item Monthly Summary → Stock Item Vouchers → voucher. "Profit & Loss A/c" line → P&L | MVP |
| Profit & Loss A/c | Trading section (gross profit) + P&L section (net profit) for a period | F2 period; columns per period/company; F12 show gross profit section, percentages vs sales; detailed/condensed | Group → Group Summary → ledger → vouchers. Opening/Closing Stock → Stock Summary | MVP |
| Schedule III Balance Sheet / Statement of P&L + notes | Statutory format for companies (Division I: AS; II: Ind AS; III: NBFC) | Tally: external Excel add-on only (7.1); lakhs/crores rounding; PY comparatives; ratios; ageing schedules if bill-wise used; MSME classification (vendor claim) | In Tally the add-on is disconnected Excel (no drill back). Ours should drill: Schedule III line → mapped ledgers → vouchers | v2 (Div I), later (Div II/III) |
| Trial Balance | Debit/credit balances by group/ledger; arithmetic check | Opening, transactions (Dr/Cr), closing columns via F12; group-wise vs ledger-wise (Ctrl+H/Alt+F1); period; "Difference in opening balances" row appears if openings don't net to zero | Group → Group Summary → ledger → Ledger Vouchers → voucher | MVP |
| Day Book | All vouchers in date order | F2 period (default: today/last voucher date); Ctrl+B: altered/non-altered/deleted vouchers, accounting-only / inventory-only; Ctrl+J: optional, post-dated; F4 voucher type filter; Ctrl+F filter by value/narration | Line → voucher (alteration) | MVP |
| Ledger Vouchers (Ledger account) | Running statement of one ledger | Period; running balance on/off; show narration; show bill-wise detail; Alt+C columns; "Confirmation of accounts" letter export | Line → voucher | MVP |
| Group Summary / Group Vouchers | Balances of ledgers within a group; or all vouchers touching that group | Period; detailed/condensed; columns; Ctrl+H switch between summary and vouchers | Ledger → Ledger Vouchers → voucher | MVP |
| Cash Book / Bank Book | Ledger vouchers for cash-in-hand and bank ledgers | Period; monthly summary first, then vouchers; bank: reconciliation status (see banking researcher) | Month → vouchers → voucher | MVP |
| Sales Register / Purchase Register | Monthly totals then vouchers of a voucher type | Default monthly summary; columnar register (F12) with one column per ledger (tax, discount); Alt+N period columns | Month → voucher list (or columnar) → voucher | MVP |
| Journal / Payment / Receipt / Contra / Debit Note / Credit Note registers | Same engine as above for other voucher types | Same | Same | MVP |
| Outstandings — Bills Receivable / Bills Payable | Open bills by party, bill date, due date, overdue days | F5 bill view; F6 ageing by bill date or due date with user-defined age buckets; Ctrl+B include post-dated; pending vs all bills; F12 show on-account amounts | Party/bill → bill history (all vouchers adjusting the bill) → voucher | MVP |
| Ledger outstandings / Group outstandings | Bill-wise detail for one party or a group (Sundry Debtors/Creditors) | Same ageing options; reminder letter export (Alt+E → Others) | Bill → vouchers | MVP |
| Ageing analysis | Bucketed outstanding (e.g., 0–30, 30–60, …) | Custom buckets; bill date vs due date basis | Bucket → bills → voucher | MVP |
| Interest calculation reports | Interest on outstanding bills/ledgers at configured rates | Per-ledger interest parameters | Ledger → bill → voucher | v2 |
| Cash Flow | Monthly inflow/outflow of cash+bank, net flow; bar graph | Alt+N weekly/fortnightly/daily columns; Ctrl+B scale | Month → Cash Flow Summary (by group, voucher type) → vouchers | v2 |
| Cash Flow Projection | Forecast: current cash/bank/OD balance + receivables − payables by due period | Ctrl+B include cash, include scenarios (memo/reversing/optional), include overdues (all off by default); overdraft limits | Receivables/payables row → party bills with due dates | later |
| Statutory Cash Flow Statement (AS 3 indirect) | Companies' cash flow in Schedule III pack | Tally: indirect method only in the add-on (vendor claim) | Line → mapped ledgers | v2 |
| Funds Flow | Opening/closing working capital per month and change; sources vs applications | Alt+N periodic columns; Ctrl+J negative ledgers | Month → Funds Flow Summary (sources/applications incl. net profit) → ledger → vouchers | later |
| Ratio Analysis | Principal groups (working capital, cash+bank, bank OD, debtors, creditors, sales, purchases, stock, net profit, WC turnover, inventory turnover) and principal ratios (current, quick, debt/equity, GP%, NP%, operating cost %, receivable days, ROI %, return on WC %) | Period; columns | Each figure → underlying group/report | v2 |
| Cost Centre reports (category summary, cost centre breakup, ledger breakup, group breakup) | Allocation of income/expense to cost centres/categories | Alt+C compare centres side by side; period | Centre → ledger → vouchers | v2 |
| Budgets & Variance | Budget vs actual for groups/ledgers/cost centres, on closing balance or net transactions basis | Multiple budgets; select budget as column; variance amount/% | Line → actual ledger vouchers | v2 |
| Scenario reports | Provisional reports including memorandum / reversing / optional vouchers | Scenario selects which non-regular voucher types are included | Same as base report | later |
| Exception reports | Negative stock, negative ledgers (wrong-side balances, e.g. debtor credit, bank OD where not expected), overdue receivables/payables, memorandum vouchers, reversing journals, optional vouchers, cancelled vouchers, post-dated vouchers; unused ledgers (via Ctrl+J in audit listing) | Ctrl+J from most reports applies the relevant exception filter | Line → master/voucher | MVP (negative stock/ledger, post-dated, optional); v2 rest |
| Statistics | Count of vouchers by type and masters by type | Period | Type → list → item | MVP (cheap) |
| Stock Summary, item/godown/batch reports, movement analysis | Inventory side (owned by inventory researcher) | Ctrl+B valuation method (FIFO/LIFO/avg), scale; F12 show consumption, gross profit | Group → item → monthly → vouchers | MVP |
| GST reports (GSTR-1/3B/2B/IMS) | Statutory (owned by GST researcher) | — | Return section → voucher | MVP |
| Edit Log (per object) | Version history of a voucher or master | F12: altered values only, show config-based values, show internal fields changed by the app | Version list → diff vs previous version (changed values highlighted) | MVP (companies) |
| Edit Log Summary (voucher-type-wise, master-wise) | Counts of altered / non-altered / total vouchers per type; master changes | Period | Type → vouchers → version list → diff | MVP |
| Day Book: altered/deleted vouchers view | Find edited/deleted vouchers quickly | Ctrl+B in Day Book | Line → version history | MVP |
| Tally Audit listings (legacy-style) | Auditor reviews entered/altered vouchers & ledgers by voucher type, master, user; "accept" removes from list | F8 today's changes; monthly register; bulk accept | Line → voucher | v2 (our "audit review workflow") |
| Verification of vouchers (Auditor edition, Tally.ERP 9) | Auditor marks each voucher verified / internal observation, with notes | Statuses incl. "internal observation" for escalation (Tally.ERP 9 docs; unconfirmed in TallyPrime 7.x) | Voucher list → voucher | v2 |
| Multi-account printing | Print/export many ledgers/books in one go; confirmation of accounts; reminder letters | Select group/all ledgers | — | v2 |
| Dashboard | Tiles: sales/purchase trend, trading details, assets/liabilities, cash/bank, cash in/out, receivables/payables, top groups/ledgers, ratios, banking activities | Per-tile period; add/remove/arrange tiles; saved views; export tile | Tile → full report | v2 |

### Report configuration grammar (Tally, 7.x; keys are Tally's, ours may differ)

- **Period/date**: F2. Balance-type reports take an "as on" date; flow-type reports take a range.
- **Context switch**: F4 (e.g. another ledger, voucher type, stock group) without leaving the report.
- **Change view**: Ctrl+H (alternate layouts of the same data, e.g. register monthly vs columnar).
- **Basis of values**: Ctrl+B — scale factor (hundreds/thousands/lakhs/crores), valuation method,
  include post-dated, altered/deleted filters, stock position (in hand vs net of orders).
- **Configure**: F12 — show/hide columns and sections, stripe view (from 5.x), narrations etc.
- **Columns**: Alt+C new column (another period, company, cost centre, valuation method, budget);
  Alt+N auto-columns (monthly/quarterly/each company); Alt+A alter; Alt+D delete.
- **Filters**: Ctrl+F (range/value filters; Ctrl+F12 value filter mentioned in stock docs);
  Ctrl+J exception filter.
- **Detail level**: Alt+F1 / Alt+F5 toggles detailed/condensed for all lines; Shift+Enter expands
  one line.
- **Save view**: Ctrl+L stores the current configuration as a named view; can be set as the default
  view for that report; scope options for who it applies to; recalled via Go To (Alt+G).
- **Navigation**: Alt+G "Go To" opens any report from anywhere and returns to the previous screen;
  "Switch To" replaces it.
- **Share/export**: Ctrl+E export current report; Alt+E export others (multi-account, letters);
  Ctrl+P print; Alt+M share via export/email/WhatsApp/print.

### Export / print formats

Tally 7.x export formats (vendor docs): ASCII text, Excel (.xlsx, can keep on-screen formatting),
HTML, JPEG, PDF (orientation/paper size; digital signature support claimed), XML, and JSON (new in
7.0). Multi-account exports: reminder letters from outstanding reports, confirmation of accounts
from ledger/cash/bank books. GSTR-3B export as single JSON/Excel (7.0). Printing supports
multi-account printing and stripe shading carries into print/export.

---

## Calculation notes

These describe accounting mechanics as implemented by Tally per its docs plus standard Indian
accounting practice. For our engine each rule needs a derived-provenance test. Items marked
"unconfirmed" must be verified against TallyPrime output before we claim parity.

### Group structure that drives the statements

Tally's 28 predefined groups split into Balance Sheet groups (Capital Account, Loans (Liability),
Current Liabilities, Fixed Assets, Investments, Current Assets, Branch/Divisions, Suspense,
Misc. Expenses (Asset)) and P&L groups (Sales Accounts, Purchase Accounts, Direct Incomes, Direct
Expenses, Indirect Incomes, Indirect Expenses). Each group has a "nature" (asset, liability,
income, expense) and P&L groups carry a flag "affects gross profit" (true for Sales, Purchase,
Direct Incomes, Direct Expenses). The statements are pure roll-ups over this tree — no separate
statement-mapping table exists in Tally (that's why Schedule III needs an external mapping).
(Group list is well-known Tally behaviour; the gross-profit flag name is unconfirmed.)

### Gross profit and net profit (for period P = [from, to])

```
Trading section (groups with affects-gross-profit = true):
  Debit side  = Opening Stock(at from) + Purchases + Direct Expenses
  Credit side = Sales + Direct Incomes + Closing Stock(at to)
  Gross Profit = Credit side − Debit side      (negative → Gross Loss)

P&L section:
  Net Profit = Gross Profit + Indirect Incomes − Indirect Expenses
```

- Amounts are **net movements within P** for P&L ledgers (not closing balances), because P&L
  ledgers have no carried-forward balance into a new year — their result is absorbed into the
  "Profit & Loss A/c" primary ledger.
- When "from" is not the FY start (e.g. a quarter), Opening Stock = closing stock valuation as at
  the day before "from". (Standard; unconfirmed that Tally recomputes rather than uses FY opening.)
- Returns (credit/debit notes) post to the same sales/purchase ledgers or separate return ledgers
  under the same groups, so they net inside the group totals.

### Closing stock

Two modes, chosen by the F11 feature "Integrate accounts with inventory":

1. **Integrated (Yes)**: closing stock value is computed from inventory: for each item, closing
   quantity × rate per the item's costing/valuation method (average cost default; FIFO, LIFO,
   standard cost, last purchase rate etc. are selectable per item), summed over items. It appears as
   (a) a credit in the Trading section and (b) "Closing Stock" under Current Assets on the Balance
   Sheet. Consumption (COGS) per outward = outward qty × valuation rate; gross profit per item =
   outward value − consumption. Total consumption = total inward value − closing value (with
   opening included). Balance Sheet Alt+C can show closing stock under an alternative valuation
   method without changing the default.
   - Physical Stock vouchers adjust quantity (and hence closing value); "value-only" rejection or
     debit notes adjust value without quantity.
   - Average cost rounding can create a small difference between one period's closing and next
     period's opening (vendor FAQ cites a Rs 0.05 example). **For us: this is a "wrong number"
     risk — define rounding precisely and test it.**
   - Negative stock: Tally allows it and shows it in an exception report; how it values negative
     closing quantities is unconfirmed. **For us: never silently value negative stock; flag it in
     the statement (Unsupported / warning) per our rules.**
2. **Non-integrated (No)**: inventory values ignored. User creates ledger(s) under the
   Stock-in-Hand group with an opening balance and enters **closing balances against dates** in the
   ledger master (multiple dates allowed). The P&L uses the closing balance for the period's end date;
   journals cannot post to Stock-in-Hand ledgers. A ledger can only be put under Stock-in-Hand at
   creation (vendor FAQ).

### Opening balances and the year boundary

- Tally keeps multiple FYs in one company with no closing entries. Balance Sheet ledgers carry
  closing → opening automatically. P&L ledgers start each FY at zero; the prior years' accumulated
  net result is shown as the opening balance of the "Profit & Loss A/c" primary ledger on the
  Balance Sheet (under Liabilities if profit), with lines for opening balance and current period.
  (Well-known behaviour; unconfirmed against 7.x screens.)
- At books beginning, ledger opening balances are entered in masters; stock opening is entered per
  item (quantity, rate, value). If the sum of ledger openings plus opening stock doesn't balance,
  Trial Balance and Balance Sheet show a "Difference in opening balances" line. (Whether opening
  stock participates in that check in integrated mode is unconfirmed; verify.)
- Opening stock can only be set at books beginning; later corrections use Physical Stock or Stock
  Journal vouchers (vendor FAQ).
- Bill-wise openings: party ledgers can carry opening bills (bill no., date, due date, amount) so
  outstandings ageing works from day one.
- "Split company data" produces a new company whose openings are the closing balances at the split
  date (behaviour details are the data-management researcher's area).

### Voucher classes and inclusion rules

| Voucher class | In books (BS/P&L/TB)? | Where seen |
|---|---|---|
| Regular | Yes | Everywhere |
| Optional | No until regularised | Day Book with Ctrl+J/basis; exception report; excluded from Schedule III TB export |
| Post-dated | No until date arrives | Excluded by default; Ctrl+B "include post-dated" in reports; exception report |
| Memorandum | No | Exception report; scenarios |
| Reversing journal | No (auto-reverses; scenario only) | Exception report; scenarios |
| Cancelled | No (number kept, amounts zero) | Day Book; exception report (some releases) |

Our engine should make inclusion an explicit, displayed report parameter rather than a hidden
default.

### Ratio formulas (Tally definitions; ours must document exact group membership)

- Working capital = current assets − current liabilities.
- Current ratio = CA / CL; quick ratio = (CA − stock-in-hand) / CL.
- GP % = gross profit / sales; NP % = net profit / sales (base inferred, unconfirmed).
- Receivable turnover in days = debtors closing / sales × days in period.
- ROI % = net profit / (capital account + net profit).
- Return on WC % = net profit / working capital; WC turnover = sales / WC; inventory turnover =
  sales / closing stock.
- Schedule III (Division I, 2021 amendment) requires 11 specific ratios with variance explanations
  for >25% change; these differ from Tally's MIS ratios. (Legal requirement from general knowledge;
  unconfirmed in this session's sources.)

### Funds flow / cash flow

- Funds flow = change in working capital per month; transactions that increase WC are sources,
  those that decrease it are applications; net profit appears as a source (loss as application).
- Cash flow (MIS) = movements in cash-in-hand + bank accounts by month, analysed by counter-group
  and voucher type. Statutory AS 3 indirect-method statement requires mapping of ledgers to
  operating/investing/financing; Tally does this only in the Excel add-on.

---

## Audit trail: legal requirements and design implications

### The law (India, companies)

| Provision | Requirement | Effective |
|---|---|---|
| Companies (Accounts) Rules 2014, Rule 3(1) proviso | Accounting software must (i) record an audit trail of each and every transaction, (ii) create an edit log of each change made in books of account along with the date of the change, (iii) ensure the audit trail cannot be disabled | FYs commencing on/after 1 April 2023 (deferred twice from 2021/2022) |
| Companies (Audit and Auditors) Rules 2014, Rule 11(g) | Auditor reports whether software had the feature, it operated throughout the year for all transactions, it was not tampered with, and the trail was preserved per retention requirements | Same; preservation commentary required from FY 2024-25 (KPMG) |
| Companies Act 2013, Section 128(5) | Books of account (audit trail treated as part of them) kept for at least 8 financial years preceding the current year (longer if an investigation is ordered) | Ongoing; i.e. trail from 1 April 2023 must be kept to at least FY 2031-32 |
| Rule 3(5) (amended 5 Aug 2022) | Electronic books and papers kept accessible in India at all times; daily backup on servers physically located in India | 5 Aug 2022 |
| Rule 3(6) | Annual intimation to Registrar of service provider details / location of books | 5 Aug 2022 |
| ICAI Implementation Guide on Rule 11(g), revised 2024 | Practical guidance; auditors check application- and database-level trail, privileged-user changes, third-party (SOC) reports, config logs of any enable/disable | Advisory |

Applies to all companies under the Companies Act (private, public, OPC, Section 8, Nidhi,
government companies). Does **not** apply to proprietorships, partnerships, LLPs (LLP rules are a
separate open question), trusts, societies. Income-tax books rules (7–8 years) apply more widely
but don't require an edit log (draft Income-tax Rules 2026 propose 7 years; draft only).

What the trail must capture (synthesised from rule + ICAI commentary + practitioner guides):
who (user ID), when (date and time), what object (transaction reference), type of change (create,
modify, delete), what changed (fields; before/after values are the practical expectation though the
rule only says "edit log of each change"), and for back-dated entries both the entry date and the
period it relates to. Deletions must be logged. Masters that change how transactions are
classified (ledger group, GST rates, ledger mapping) are conservatively in scope.

### How Tally's Edit Log works (vendor docs)

- Two editions: regular TallyPrime (Edit Log can be enabled/disabled in Company alteration) and
  TallyPrime Edit Log (always on, cannot be disabled). Only the Edit Log edition is positioned as
  compliant. Companies from older releases/Tally.ERP 9 must be migrated into the Edit Log edition.
- Scope: create/alter/delete of vouchers, ledgers, stock items and accounting groups (from Rel 2.1);
  narration and voucher-number changes; automatic renumbering after insert/delete is itself logged.
  Company-level activities affecting logs are in a Company edit log; **Company Alteration changes
  are not tracked**.
- Logging starts only when enabled; no retroactive history.
- Per-object view: Alt+Q in alteration mode → list of versions (version no., activity, user,
  date-time). Enter on an "Altered" version → diff vs the previous version with changed values
  highlighted. F9 shows all versions. F12 options: show altered values only; show
  configuration-based values; show internal fields updated by the application. "Resaved" versions
  (no change) have no diff.
- Summaries: Edit Log Summary by voucher type (altered / non-altered / total) and master-wise.
  Day Book Ctrl+B can filter altered/non-altered/deleted vouchers.
- Costs: each alteration stores data, so frequent edits and renumbering grow the company file;
  vendor advises "Automatic (Manual Override)" numbering and pre-numbered imports to avoid log
  bloat.
- User identity depends on users being configured (security/"control user access"); without user
  login the username field is weak (unconfirmed how Tally handles no-login companies).
- Tally Audit (separate, older feature): auditor/admin login; listings of entered/altered vouchers
  by voucher type, ledgers, users; F8 today's changes; auditor "accepts" items which then leave the
  listing. Tally.ERP 9's Auditor's Edition had "Audit & Compliance → Verification of Vouchers" with
  statuses and notes, plus statutory/tax audit checklists; its status in TallyPrime 7.x is
  unconfirmed.

### Where real companies fail (KPMG, Sept 2025, 93 large listed cos, FY25)

- 55 of 93 (59%) had modified Rule 11(g) reporting.
- Top exception: trail not enabled for privileged-user changes (32), then not enabled at database
  level (31), third-party/SOC report unavailable (12), not enabled at application level or for some
  fields/masters (9), other (16). Most companies had 2+ exceptions.
- Implication: auditors now ask "can an admin or someone with DB access change data without the log
  noticing?" A desktop app storing a plain SQLite file is exactly the "database level" gap.

### Design implications for us

1. **Event-sourced, append-only ledger store.** Every create/alter/delete/cancel of a voucher or
   master is an immutable event with: event id, object id, version, actor (user id + machine id),
   wall-clock timestamp (UTC + local offset), voucher date vs entry time, full before/after (or
   full snapshot + diff), reason (optional free text), app version. Current state is a projection.
   Deletion is a tombstone event, never a physical delete.
2. **Tamper evidence at storage level, not just UI.** Hash-chain each event (prev-hash) and
   periodically seal (checkpoint hash per day) so any out-of-band SQLite edit is detectable;
   provide a "verify integrity" command that auditors can run and that prints a pass/fail
   certificate. Consider signing checkpoints with a per-company key, and optionally anchoring daily
   checkpoint hashes in the backup. This directly answers the "database level" and "privileged
   user" exceptions.
3. **No off switch for company entities.** Company type = Company (Companies Act) → audit trail
   permanently on, cannot be disabled by admin, cannot be removed by migration/export/split.
   For non-company entities we may still keep it always-on (simpler, one code path; storage cost is
   small); offer compaction only for non-company entities and only beyond retention, and log the
   compaction itself. Recommendation: always on for everyone.
4. **Log configuration changes too.** Unlike Tally, log company-settings changes (features,
   numbering, GST config, users/roles, lock dates). Auditors explicitly ask for evidence that the
   trail was never disabled; a logged settings history plus hash chain gives that.
5. **Retention 8 years + preservation across upgrades.** Schema migrations must preserve the full
   event history and its hashes; split/new-FY operations must not drop history; "archive year"
   produces a read-only, verifiable archive. Never require a paid licence to read old data (a cited
   practitioner pain point: cancelled subscription = preservation failure).
6. **India-located daily backup helper.** Built-in daily backup to user-chosen location with a
   warning for companies that the location must be in India (Rule 3(5)); backup includes the event
   log and checkpoints.
7. **Auditor-facing reports (MVP for companies):** per-object version history with diff; edit log
   summary by voucher type / master / user / period; deleted & altered voucher lists; back-dated
   entries report (entry time vs voucher date, beyond a threshold); entries after a lock/closing
   date; edits after e-invoice IRN generation; privileged-user activity report; integrity
   verification certificate; export of the entire trail (CSV/JSON) for the auditor's own tools.
8. **Auditor workflow (v2):** read-only auditor role; mark vouchers verified / query / internal
   observation with notes; "accepted" state per change; auditor notes never alter the books but are
   themselves logged.
9. **Imports and AI.** Bulk imports and AI-assistant-created vouchers are events with actor =
   import job / assistant on behalf of user X, so the trail distinguishes them.
10. **Clock integrity.** Log both system time and a monotonic sequence; flag events whose system
    clock went backwards (local-first apps can't trust the clock).

---

## What users love / hate

Caveat: forum/review sweep not done (search budget exhausted). Items below come from vendor docs
(which reveal what users ask for), practitioner articles, and well-known community sentiment;
treat as hypotheses to validate.

**Love**
- Speed of keyboard navigation: Enter to drill from Balance Sheet all the way to a voucher, edit it,
  Esc back with the report already updated. Go To (Alt+G) from anywhere.
- Every report shares the same keys (F2, Alt+C, Ctrl+B, F12, Ctrl+J) — learn once, use everywhere.
- No year-end closing entries; multi-year in one company; comparisons via Alt+C columns.
- Exception reports catch data-entry mistakes (negative stock, wrong-side ledgers).
- Bill-wise outstandings with ageing and reminder letters are core to Indian SME cash collection.
- Save View (Ctrl+L) since 3.x/4.x removed the need to reconfigure reports each time.

**Hate / gaps** (mostly unconfirmed from user sources)
- No native Schedule III; the 7.1 add-on is Excel-based, disconnected, single-company, Windows/Excel
  only, not on cloud/Mac, and requires an active subscription (vendor FAQ confirms these limits).
- Edit Log bloats data and requires a separate edition and migration; renumbering noise in the log.
- Few charts/visual analytics outside the dashboard; dashboard arrived late (4.0).
- Excel export is formatted for print, not analysis (merged headings, indentation as structure) —
  community complaint, unconfirmed.
- Report customisation beyond configured options requires TDL (Tally's proprietary language) and a
  partner.
- Consolidation/group companies limited; multi-company comparisons need all companies loaded.
- Average-cost rounding differences between periods (vendor FAQ acknowledges).

---

## Open questions

1. Does Tally's "Difference in opening balances" check include opening stock value in integrated
   mode? Need an observed test (TallyPrime 7.1).
2. How exactly does Tally value negative closing stock under each costing method? Observed test.
3. Is Tally's P&L opening stock for a mid-year period recomputed as closing at (from − 1)? Observed.
4. Does the LLP Act / LLP Rules impose an equivalent audit-trail rule? (Not found; check MCA.)
5. Is the Tally.ERP 9 Auditor's Edition "Verification of Vouchers" and statutory/tax-audit
   checklist still present in TallyPrime 7.x (as "Audit & Compliance")? Not confirmed.
6. ICAI Implementation Guide (2024) FAQ text on database-level logging and master data — read the
   official ICAI copy for exact expectations (only secondary summaries read).
7. Schedule III Division I ratio list and ageing bucket definitions (2021 amendment) — confirm
   from the MCA notification for our native implementation; Tally's add-on uses fixed buckets.
8. Should our audit trail record read access (report views/exports)? Not legally required; may be
   wanted by auditors for privileged users. Decide.
9. Before/after values: the rule says "edit log of each change"; ICAI material implies field-level
   detail. We plan full snapshots — confirm no privacy conflict for payroll data (DPDP Act).
10. Unconfirmed list of Tally dashboard tiles added in 5.x–7.x beyond the 4.0 set and the 6.0
    banking tile.
11. Retention of audit trail when a company is struck off / merged — out of scope for MVP but worth
    noting.

---

## Sources

Vendor (Tally Solutions) help and product pages:
- TallyPrime 7.0 release notes — https://help.tallysolutions.com/release-notes-tallyprime-7-0
- TallyPrime 7.1 release notes — https://help.tallysolutions.com/release-notes-tallyprime-7-1/
- Schedule III for Division I release note — https://help.tallysolutions.com/schedule-iii-for-division-i-release-note/
- Schedule III for Division I FAQ — https://help.tallysolutions.com/schedule-iii-for-division-i-faq/
- Get started with Edit Log — https://help.tallysolutions.com/tally-prime/edit-log/get-started-with-edit-log/
- Edit Log for masters and transactions — https://help.tallysolutions.com/edit-log-for-masters-transactions/
- View Edit Log summary — https://help.tallysolutions.com/view-edit-log-summary/
- Tally Audit — https://help.tallysolutions.com/tally-prime/access-control-data-security/tally-audit-tally/
- Auditing of Vouchers (Tally.ERP 9 Auditor's Edition) — https://help.tallysolutions.com/article/te9rel60/Auditor_Edition/India/Statutory_Audit/Auditing_of_Vouchers.htm
- Working with reports — https://help.tallysolutions.com/working-with-reports/
- Save views of reports — https://help.tallysolutions.com/how-to-save-views-of-reports-in-tallyprime/
- Accounting and financial reports — https://help.tallysolutions.com/tally-prime/accounting-financial-reports/accounting-financial-reports-tally/
- Ratio Analysis — https://help.tallysolutions.com/tally-prime/accounting-financial-reports/ratio-analysis-tally/
- Funds Flow — https://help.tallysolutions.com/tally-prime/accounting-financial-reports/funds-flow-report-tally/
- Cash Flow and projection — https://help.tallysolutions.com/tally-prime/accounting-financial-reports/cash-flow-and-projection-report-tally/
- Stock valuation FAQ — https://help.tallysolutions.com/tally-prime/inventory/stock-valuation-faq/
- Integrate accounts with inventory (Tally.ERP 9) — https://help.tallysolutions.com/docs/te9rel64/Reports/Display_Financial_Statements/Integrate_Accounts_with_Inventory.htm
- Exception reports (Tally.ERP 9) — https://help.tallysolutions.com/docs/te9rel66/Reports/MIS_Reports/Exception_Reports.htm
- Export data — https://help.tallysolutions.com/tally-prime/data-management/export-data-in-tally/
- Dashboard — https://help.tallysolutions.com/tally-prime/dashboard/ and FAQ https://help.tallysolutions.com/dashboard-in-tallyprime-faq/
- Audit trail marketing pages (search snippets only; site blocks fetch) — https://tallysolutions.com/tally/audit-trail-in-tallyprime , https://tallysolutions.com/business-guides/tallyprime-audit-trails-and-controls-to-satisfy-external-auditors/

Legal / professional:
- TaxGuru, "Demystifying audit trail (edit log)" — https://taxguru.in/company-law/demystifying-audit-trail-edit-log.html
- TaxGuru, "Audit trail compliance Rule 3(1) & Rule 11: practitioner's guide" — https://taxguru.in/company-law/audit-trail-compliance-rule-3-1-rule-11-practitioners-guide.html
- ICAI CA Journal, "Audit Trail – Requirements & Responsibilities" — https://cajournal.icai.org/article-details/audit-trail-requirements-responsibilities
- ICAI Implementation Guide on Reporting on Audit Trail (revised 2024), listing — https://publication.icai.org/publication/1063 (secondary summary: https://www.taxmann.com/post/blog/icai-issues-implementation-guide-revised-on-reporting-on-audit-trail/)
- KPMG India, Accounting and Auditing Update Sept 2025, Ch. 2 "Trends in audit trail reporting" — https://assets.kpmg.com/content/dam/kpmgsites/in/pdf/2025/09/chapter-2-trends-in-audit-trail-reporting.pdf.coredownload.pdf
- Free Press Journal on TallyPrime Edit Log edition — https://www.freepressjournal.in/amp/business/tallyprime-edit-log-to-assist-firms-under-mca-to-comply-with-audit-trail-mandate
