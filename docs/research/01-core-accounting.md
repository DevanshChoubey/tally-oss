# 01 - Core accounting model (incumbent analysis)

Researcher area: company setup, chart of accounts, ledgers, voucher types and numbering, bill-wise
tracking, cost centres, budgets/scenarios, multi-currency, year-end. Research date: 2026-11-20.

**Method and clean-room note.** Web research only, mostly the vendor's public help site
(help.tallysolutions.com) and product pages, plus a few third-party sources. Nothing was downloaded,
installed or decompiled. Everything below is our own paraphrase. Many help pages fetched are for the
older ERP 9 line (URLs containing `te9rel`); where a detail rests only on those, it is marked
"(ERP 9 source)". Where sources disagree or a point was not confirmed, it is marked **unconfirmed**.
Web-search budget ran out before the last three verification searches (double-entry modes, cost-centre
allocation totals, post-dated behaviour in 7.x); those items are listed under Open questions.

Version context: the current line is 7.x; 7.1 shipped 27 June 2026 (release notes: invoice templates,
Schedule III Division I statements, IMS reconciliation, OAuth email, backup log). One search snippet
said 7.2 has also been released - **unconfirmed**. Nothing found suggests the core accounting model
changed in 7.x; the model described here has been stable since the ERP 9 era, with 3.0 adding
numbering series and insertion/deletion behaviour.

Brand rule: product names appear only in this research file and in Sources. Proposed entities use
neutral names.

---

## Summary

The incumbent is a classic voucher-centric double-entry system:

- **One company = one data set**, with a financial-year start and a books-beginning date. Data can span
  many years inside one company: you just change the current period, and balances roll forward
  automatically with no closing entry. An optional "split" utility cuts the data at a date into one or
  two child companies.
- **Chart of accounts = a tree of groups with ledgers as leaves.** There are 28 reserved groups
  (15 primary, 13 sub-groups). The nature of a primary group (Asset / Liability / Income / Expense)
  decides whether it lands in the Balance Sheet or the P&L. Flags such as "affects gross profit"
  decide whether an income or expense is direct (trading account) or indirect. Two reserved ledgers
  exist: Cash, and Profit & Loss A/c (which sits under the root and holds retained P&L).
- **Every transaction is a voucher of a voucher type.** There are about 24 reserved base types
  (accounting, inventory, order, payroll, plus non-posting Memorandum and Reversing Journal). Users
  create their own voucher types that inherit from a base type. Voucher classes are templates inside a
  type that pre-fill ledger allocations.
- **Double entry is enforced at save time:** a voucher whose debit and credit totals differ is rejected.
  Opening balances are *not* forced to balance. Any imbalance shows up as a synthetic "Difference in
  opening balances" line in the Trial Balance and Balance Sheet.
- **Sub-ledger tracking is layered onto ledger lines:** bill-wise references (New / Against / Advance /
  On Account), cost-centre allocations (category -> centre, optionally via percentage classes), interest
  parameters, and forex amounts with exchange rates.
- **Provisional data** (optional vouchers, memorandum, reversing journals, post-dated vouchers) never
  touches the books unless the user explicitly brings it in through a "scenario" overlay in reports.
- **Features are switched on per company** (the F11 screen) and **behaviour is tuned per screen**
  (F12 configuration). Many fields only show once a feature is on.

---

## Feature inventory

| Feature | What it does | How the user interacts | Priority |
|---|---|---|---|
| Company creation | Creates a data set: name, mailing name, address, country/state, contacts, FY start, books-beginning date, base currency, optional access control, encryption password, edit log | One form; FY start is pre-filled from the OS locale and country, and books-beginning defaults to FY start | MVP |
| Company features (F11) | Per-company switches: maintain accounts, bill-wise entry (on by default), cost centres, interest calculation, inventory, integrate accounts with inventory, GST/TDS/TCS (plus legacy VAT/excise/service tax), payroll, multiple addresses, mark modified vouchers, browser access, remote sync | Single screen with progressive disclosure ("show more", "show all") | MVP (core switches); v2 (rest) |
| Screen configuration (F12) | Per-screen behaviour, e.g. skip date field, Cr/Dr vs To/By labels, warn on negative cash, pre-allocate bills, use default bill-wise details, grouping of cost centres (ERP 9 source for most labels) | Context-sensitive config panel on each screen | MVP (a handful); v2 |
| Financial year / books beginning | FY start date plus a later books start for mid-year adoption; reports run over any period inside the data | Set at creation; "change period" later | MVP |
| Edit log / audit trail | Change history of masters and vouchers, presented as meeting the Indian MCA audit-trail requirement; can be made mandatory at creation | Toggle at company creation; Edit Log report | MVP (India-mandatory) |
| Data encryption | Optional company-level encryption password, required on every open | Set at creation or later | v2 |
| Access control | Users, roles, admin user | Set at creation or later | v2 |
| 28 reserved groups | Fixed starting chart of accounts (listed below) | Pre-created; can be altered (renaming is discouraged); cannot be deleted | MVP |
| User groups | Nested groups under any group; flags: nature (top level only), affects gross profit, behaves like sub-ledger, net Dr/Cr balance in reports, used for calculation (tax/discount %), purchase-invoice cost-allocation method (none / by quantity / by value) | Group master form; can also be created from the chart-of-accounts view | MVP (nature, GP flag); v2 (rest) |
| Ledgers | Leaf accounts: name/alias, parent group, opening balance (Dr/Cr), bill-by-bill flag, default credit period, credit-days check, cost-centres-applicable, interest parameters, currency, inventory-values-affected, mailing/bank/tax details | Ledger master form; can also be created inline while entering a voucher | MVP |
| Reserved ledgers | "Cash" (under Cash-in-hand) and "Profit & Loss A/c" (under root, carries accumulated P&L, cannot be deleted). Whether Cash can be deleted: the help page contradicts itself (**unconfirmed**) | Pre-created | MVP |
| Opening balances | Ledger-level Dr/Cr as at books-beginning. Party ledgers break the opening balance into individual bills; cost-centre and foreign-currency openings are also supported | Entered in the ledger master; bill breakup sub-screen | MVP |
| Opening-balance difference | Imbalanced openings are allowed and surfaced as a synthetic "Difference in opening balances" line | Trial Balance shows it; a verification report compares against the prior-year company | MVP |
| Accounting vouchers | Contra (F4), Payment (F5), Receipt (F6), Journal (F7), Sales (F8), Purchase (F9), Credit Note (Alt+F6), Debit Note (Alt+F5) | Keyboard-first entry screen, one function key per type | MVP |
| Inventory vouchers | Stock Journal (Alt+F7), Physical Stock (Ctrl+F7), Delivery Note (Alt+F8), Receipt Note (Alt+F9), Rejection In (Ctrl+F6), Rejection Out (Ctrl+F5), Material In / Material Out (job work) | Same entry screen | Other researcher (inventory) |
| Order vouchers | Sales Order (Ctrl+F8), Purchase Order (Ctrl+F9), Job Work In/Out Orders | Same | Other researcher |
| Payroll vouchers | Payroll (Ctrl+F4), Attendance | Same | Later |
| Memorandum voucher | Non-posting note voucher kept in its own register; can later be converted to a real voucher. A third-party source says it has no date limit (**unconfirmed**) | Under "other vouchers"; Memorandum Register | v2 |
| Reversing journal | Non-posting provisional journal with an "applicable up to" date; only shows in reports through a scenario; can be converted to a regular voucher by changing its type | Must be activated on first use; Reversing Journal Register | v2 |
| Optional vouchers | Any voucher marked optional (Ctrl+L) or a type that is "optional by default"; does not affect the books; can be regularised later; usable in scenarios. Not applicable to Memorandum or Reversing Journal | Toggle on the voucher | v2 |
| Post-dated vouchers | Voucher flagged post-dated (Ctrl+T); ledgers are updated only from its date; shown in an exception report; excluded from scenarios. Used mainly for post-dated cheques. ERP 9 pages contradict each other on timing (**unconfirmed for 7.x**). Current help also mentions "use effective dates" on a voucher type | Toggle on the voucher; PDC reports | v2 |
| Voucher types (user-defined) | New type inherits from a base type; has name, abbreviation, activate flag, numbering method, effective dates, allow zero-valued, optional by default, narration options, print/share/payment-link after save, POS flag, default accounting allocations, classes | Voucher-type master | MVP (basic); v2 |
| Voucher classes | Named templates inside a type: preset ledger allocations for items (item-invoice mode), additional entries (tax, freight), restricted ledger groups, a forex-adjustment class for journals, inter-godown classes for stock journals | Defined in the voucher-type master; picked when the voucher opens | v2 |
| Voucher numbering | Methods: Automatic, Automatic (manual override), Manual, Multi-user auto, None. Extras: starting number, width + zero-fill, prefix/suffix with effective dates, restart by date/periodicity (e.g. monthly or yearly), prevent duplicates, behaviour on insert/delete (renumber vs retain, with optional reuse of unused numbers) | Voucher-type master sub-screen | MVP |
| Multiple numbering series (3.0+) | Several series per voucher type, e.g. per GST registration, inter- vs intra-state, e-way bill threshold | Enabled in voucher-type configuration; series picked per voucher | MVP (GST needs it) |
| Invoice modes | Item invoice (stock lines plus ledger lines), Accounting invoice (ledger lines only, for services), As voucher (plain Dr/Cr). Switched with Ctrl+H at creation; official FAQ says a saved voucher cannot be switched between item and accounting invoice | Mode switch on the entry screen | MVP (accounting invoice and as-voucher); item invoice with inventory |
| Bill-wise references | On ledgers flagged bill-by-bill, each party line splits into references: New Ref, Agst Ref (settles an open bill), Advance, On Account; each has a credit period or due date | Bill allocation sub-screen during entry; "use default bill-wise details" auto-fills | MVP |
| Pre-allocation of bills | Choose bills before entering the amount in payment/receipt/journal (ERP 9 source) | F12 option | v2 |
| Credit limits / credit-days check | Warns at entry when a party is past its credit period or has overdue bills | Ledger flag | v2 |
| Outstandings | Receivables/payables ageing by bill, party, group | Reports | MVP |
| Cost categories | Parallel dimensions (e.g. Department, Project); each enables allocation for revenue items, non-revenue items, or both (at least one); a default "Primary Cost Category" exists | Master; F12 enables the revenue/non-revenue flags | v2 |
| Cost centres | Hierarchical within a category; may carry opening balances; ledgers opt in via "cost centres applicable" | Master; allocation sub-screen per ledger line | v2 |
| Cost centre classes | Percentage split templates across centres, applied automatically at entry | Master; picked on the voucher | Later |
| Budgets | Named, hierarchical; period from/to; targets on ledgers, groups or cost centres; type "on net transactions" or "on closing balance" | Budget master; variance from Trial Balance (Budget / Actual / Variance columns) | v2 |
| Scenarios | Named overlays that include chosen provisional voucher types (optional, memorandum, reversing) and exclude others; shown as an extra column against actuals in reports | Scenario master; add a column in a report | v2 |
| Interest calculation | Per-ledger parameters: rate, style (365-day vs calendar year), on all / debit-only / credit-only balances; advanced mode with dated rate slabs and party applicability (always / after due date plus grace days); bill-by-bill interest needs bill-wise. Bank basis: voucher date or bank-reconciliation date. ERP 9 sources say simple or compound | F11 enable, then ledger parameters; interest reports; book via voucher class | v2 |
| Multi-currency | Currency masters (symbol, formal name, ISO code, decimals); dated standard / selling / buying rates; foreign-currency amounts on vouchers; foreign-currency opening balances | Currency master; Rates of Exchange screen; rate typed at entry | v2 |
| Forex gain/loss | The difference between booking and settlement rates accumulates as "Unadjusted Forex Gain/Loss" on the Balance Sheet. The user posts a journal, using a special forex-adjustment class, to an indirect income or expense ledger to move it to P&L. Per the help page the amount is entered manually. Gains and losses are tracked by ledger, not by item | Journal with forex class | v2 |
| Change period / multi-year | Same company holds many years; opening balances for a period are computed as of its start | Change period | MVP |
| Split company data | Splits at a date into "from split date", "before split date", or two companies; the original is kept. Recommended only after the books are audited; pending bills and forex should be settled first | Data utility | Later |
| New company from closing balances | Export closing balances of ledgers and stock (Excel/XML), import as openings | Export/import | v2 |

### Reserved groups (28)

Primary (15). The incumbent's help gives only the statement each group belongs to (BS / P&L). The
nature column is **our inference** from standard accounting and the group names.

| Group | Statement | Nature (inferred) |
|---|---|---|
| Capital Account | BS | Liability |
| Loans (Liability) | BS | Liability |
| Current Liabilities | BS | Liability |
| Fixed Assets | BS | Asset |
| Investments | BS | Asset |
| Current Assets | BS | Asset |
| Misc. Expenses (Asset) - deferred items written off over time (preliminary / pre-operative); historically a statutory-disclosure group | BS | Asset |
| Suspense A/c - temporary unclassified items. Help notes that "suspense" *expenses* belong under a revenue group | BS | Liability (**unconfirmed**) |
| Branch/Divisions - inter-branch / subsidiary accounts treated like parties. Full branch books need a separate company | BS | Liability (**unconfirmed**) |
| Sales Accounts | P&L | Income, affects GP |
| Purchase Accounts | P&L | Expense, affects GP |
| Direct Incomes | P&L | Income, affects GP |
| Direct Expenses | P&L | Expense, affects GP |
| Indirect Incomes | P&L | Income, does not affect GP |
| Indirect Expenses | P&L | Expense, does not affect GP |

Sub-groups (13), with their parents:

- Current Assets: Bank Accounts, Cash-in-hand, Deposits (Asset), Loans & Advances (Asset),
  Stock-in-hand, Sundry Debtors
- Current Liabilities: Duties & Taxes, Provisions, Sundry Creditors
- Loans (Liability): Bank OD A/c, Secured Loans, Unsecured Loans
- Capital Account: Reserves & Surplus

Stock-in-hand is special. When accounts are integrated with inventory, its value comes from the stock
ledger and cannot be posted to. When they are not integrated, only opening and closing values are typed
in and vouchers cannot hit it.

### Reserved voucher types

The count is 24 per most current pages. One page says 28, and that may be confusing them with groups
(**unconfirmed**). Our reconstructed list: Contra, Payment, Receipt, Journal, Sales, Purchase,
Credit Note, Debit Note, Memorandum, Reversing Journal (accounting and provisional); Stock Journal,
Physical Stock, Delivery Note, Receipt Note, Rejection In, Rejection Out, Material In, Material Out
(inventory); Sales Order, Purchase Order, Job Work In Order, Job Work Out Order (orders); Payroll,
Attendance (payroll). That makes 24, but the list is reconstructed from scattered pages and shortcut
tables and has **not** been checked against one official enumeration.

---

## Data-model implications for our Rust engine

### Entities (neutral names)

- **Company**: id, name, mailing_name, addresses[], country, state, contacts, fy_start (month/day),
  books_begin (date), base_currency_id, feature_flags (typed struct, not a bag of strings),
  edit_log_enabled (cannot be turned off once on, for India), encryption settings.
- **Group**: id, name, aliases, parent_id (None only for the root's direct children), nature
  (enum Asset | Liability | Income | Expense; stored on primaries and inherited below), affects_gross_profit
  (bool, meaningful only for Income/Expense), behaves_like_subledger, net_balance_reporting,
  used_for_calculation, purchase_cost_allocation (enum None | ByQuantity | ByValue), reserved (bool),
  reserved_kind (enum identifying each of the 28, so behaviour never keys off a renamable name).
- **Ledger**: id, name, aliases, group_id, currency_id, opening (signed Money in ledger currency, plus
  base equivalent), bill_wise (bool), default_credit_days, check_credit_days, cost_centres_applicable,
  interest_params (Option), inventory_values_affected, party/bank/tax detail sub-records,
  reserved_kind (Cash | ProfitAndLoss | None).
- **VoucherType**: id, name, base_kind (enum of the ~24 reserved kinds; closed enum, no catch-all),
  abbreviation, active, numbering (NumberingConfig), numbering_series[], optional_by_default,
  allow_zero_value, effective_dates, classes[], default allocations.
- **NumberingConfig**: method (Auto | AutoManualOverride | Manual | MultiUserAuto | None), start, width,
  zero_fill, prefix/suffix tables keyed by effective date, restart rules (date + periodicity),
  prevent_duplicates, on_insert_delete (Renumber | Retain), reuse_unused.
- **VoucherClass**: id, voucher_type_id, purpose-specific settings (default allocations, restricted
  groups, forex-adjustment flag, ...).
- **Voucher**: id, voucher_type_id, class_id?, series_id?, number (string as displayed, plus a numeric
  part), date, effective_date?, status (Regular | Optional | PostDated | Cancelled), mode
  (AsVoucher | AccountingInvoice | ItemInvoice), narration, lines[], created/modified audit fields.
  Memorandum and Reversing-journal come from base_kind, so they need no extra status.
  Reversing journal adds applicable_upto.
- **LedgerLine** (Entry): ledger_id, dr_cr, amount_base (Money), forex (Option<{currency, amount, rate}>),
  bill_allocs[], cost_allocs[], bank_allocs[], narration?.
- **BillAlloc**: kind (New | Against | Advance | OnAccount), bill_ref (name; for Against, it must resolve
  to an existing open New/Advance ref on the same ledger), amount, due_date or credit_days.
- **Bill** (derived or materialised): ledger_id, ref_name, origin voucher, original amount, due date,
  open amount. It is a projection over BillAllocs, never edited directly.
- **CostCategory**: id, name, allocate_revenue, allocate_non_revenue (at least one true).
- **CostCentre**: id, name, category_id, parent_id?, opening?
- **CostAlloc**: category_id, centre_id, amount.
- **CostCentreClass**: id, per-category percentage splits (summing to 100%).
- **Currency** + **ExchangeRate**: currency_id, date, standard, selling, buying.
- **Budget**: id, name, parent?, period, lines {target (ledger | group | cost centre), kind (NetTransactions | ClosingBalance), amount}.
- **Scenario**: id, name, include_types[], exclude_types[], include_actuals (bool).
- **InterestParams**: slabs [{from_date, rate, style (Days365 | CalendarYear), applies_to (All | DrOnly | CrOnly), applicability (Always | PastDue{grace_days}), calculate_from}].

### Invariants (proposed; each should be a typed error, never a silent fix)

1. **Voucher balance.** For every posting voucher, the base-currency Dr total equals the Cr total
   exactly. Use fixed-point decimal money (paise, or the currency's minor unit), never floats. The
   incumbent rejects an unbalanced voucher at save time. We should do the same in the engine, not only
   in the UI.
2. **Allocation sums.** Bill allocations on a bill-wise line sum to the line amount. Cost allocations per
   category sum to the line amount when the ledger is cost-centre-applicable. That second rule was **not
   confirmed** for the incumbent; whether partial allocation is allowed is an open question. Decide it
   ourselves and write the decision down.
3. **Against-ref validity.** An Against reference must point to an existing open reference on the same
   ledger, and it must not over-settle unless the user explicitly allows that and the overflow becomes a
   new ref.
4. **Provisional isolation.** Optional, memorandum, reversing and post-dated (before its date) vouchers
   never contribute to ledger balances in the base books. They contribute only inside a named scenario
   overlay. A report must declare which scenario it uses.
5. **Opening balance difference is shown, never hidden.** Allow an imbalanced opening trial balance
   (real migrations need it), but always surface the difference as an explicit synthetic line, and give
   the AI assistant and checks a "fix openings" workflow.
6. **Reserved masters.** The 28 groups and the two reserved ledgers cannot be deleted. Behaviour keys off
   reserved_kind, not name, so renaming is harmless.
7. **Nature inheritance.** Only top-level groups carry a nature; children inherit it. Moving a group
   across natures re-classifies history, so it needs an explicit confirmation and an audit event.
8. **Date bounds.** Voucher date >= books_begin. Whether the incumbent requires books_begin >= fy_start
   was not confirmed; we should require fy_start <= books_begin < fy_start + 1 year.
9. **Numbering uniqueness** per (voucher_type, series, numbering period) when prevent_duplicates is on.
   Multi-user auto needs an atomic allocator in the store.
10. **Locked periods.** The incumbent's split is a heavyweight substitute for period locking. We should
    offer a lightweight "lock books up to date X" invariant instead.

### Design decisions to surface

- **Renumber-on-delete vs audit trail.** The incumbent's default renumbers later vouchers when one is
  inserted or deleted. That clashes with an immutable audit log and with GST invoice numbering, where a
  number issued to a customer must not change (**unconfirmed**, hand to the GST researcher). Proposal:
  sales-side series always "retain"; renumbering allowed only for internal types, and logged.
- **Multi-year continuous data with no closing entry.** Balances roll forward by computation: P&L for
  prior years folds into the reserved P&L ledger at query time. This is elegant and avoids year-end
  journals, but every balance query must be period-aware. Proposal: keep the incumbent's continuous
  model (users love not having to "close"), add period locks, and make year-end an optional snapshot or
  archive rather than a data split.
- **Store vouchers as events; derive balances.** Ledger balances, bill outstandings, cost-centre totals
  and forex exposure should all be projections over the voucher log. This is easy to audit and fits a
  local-first sync model.
- **Forex.** Store per-line forex amount and rate, and compute realised gain/loss on bill settlement
  automatically. Improve on the incumbent's manual adjustment journal by generating a draft adjustment
  for the user to approve. Period-end revaluation of open foreign balances is not documented by the
  incumbent (**unconfirmed**) and should be v2.
- **Feature flags as a typed struct** with dependency rules (interest-per-bill requires bill-wise; payroll
  statutory requires payroll; integrated inventory changes Stock-in-hand behaviour). Changing a flag
  that has data behind it is a migration, not a toggle.
- **Invoice mode is per voucher and fixed after save** (incumbent rule). We can choose to allow
  conversion from accounting invoice to item invoice when stock lines are supplied.

---

## India-specific rules

- **Financial year April-March** is the Indian default. The incumbent pre-fills it from country and
  locale (the April-March specifics are general knowledge, not taken from a page).
- **Audit trail / edit log.** The incumbent presents its edit log as satisfying the MCA requirement that
  companies keep a non-disableable audit trail of changes to books (Companies (Accounts) Rules, in force
  since FY 2023-24). Rule details are **unconfirmed** here; hand to the compliance researcher.
- **Schedule III.** 7.1 adds financial statements in Schedule III Division I format. Our group-to-line
  mapping must support Schedule III Div I, II and III (**the Div II/III need is unconfirmed**). The
  "Misc. Expenses (Asset)" group is a legacy of the older Schedule VI layout.
- **GST-driven numbering.** Multiple numbering series per GST registration (GSTIN), inter- vs
  intra-state, and e-way bill thresholds motivated the 3.0 numbering series. The GST invoice-number rules
  (a 16-character limit, unique per FY, allowed characters) are **unconfirmed** from my sources; hand to
  the GST researcher.
- **Duties & Taxes group** holds GST/TDS/TCS ledgers. Tax-type sub-classification belongs to the GST/TDS
  research areas.
- **Indian number formatting** (lakh/crore grouping, "amount in words" in Indian style) is part of the
  base-currency settings. The incumbent offers base-currency extras such as suffix, spacing, words and
  decimals; that it uses lakh/crore grouping specifically is **unconfirmed**.
- **Post-dated cheques** are common in Indian trade, which is why post-dated vouchers and PDC reports
  exist.

---

## What users love / hate

The evidence is thin. Capterra was blocked (403), Reddit returned nothing, and the main "hate" source is
a competitor's blog (biased).

**Love** (largely inferred from how ubiquitous the product is, its marketing, and its shortcut-heavy
design; weakly sourced):

- Very fast keyboard-only entry, with one key per voucher type and on-the-fly master creation.
- One continuous data set across years; no forced year-end closing.
- Drill-down from any report total to the voucher.
- Bill-wise outstanding tracking is on by default and simple.

**Hate / limitations:**

- A competitor blog (biased) claims: Windows desktop-bound data; a mobile app that is mostly read-only;
  the single-user tier forces an upgrade for concurrent users; a steep learning curve that assumes
  accounting knowledge; statutory updates tied to an annual subscription; no automatic backups;
  customisation needs paid partner scripting.
- Review snippets (G2, Capterra, techjockey) report: slowness on large data; data corruption during data
  splitting, fixed by "repair"; licence reactivation friction; dated remote-access and collaboration.
- The vendor claims (marketing) that data is safe from corruption during power or network failures.
- Pricing per the vendor page: about Rs 22,500 + GST (single user) and Rs 67,500 + GST (multi-user),
  plus an annual renewal subscription from year 2. Treat these as indicative only.
- Specific to the model: invoice mode cannot be changed after save; split company data is fragile and
  only recommended after audit; forex gain/loss must be journalled by hand; forex tracking is by ledger,
  not by item.

---

## Open questions

1. Exact enumeration of the 24 reserved voucher types (24 vs 28 conflict). Verify against one official list.
2. Single-entry vs double-entry entry mode for payment/receipt/contra (a known config in the older line):
   does it still exist, and which ledgers does each type allow (e.g. contra limited to cash/bank)?
   The search budget ran out before this could be checked.
3. Must cost-centre allocations sum to the line amount, and is partial allocation allowed?
4. Can foreign-currency party ledgers be bill-wise? One search summary said no (bill-wise only for
   base currency), but realised forex gain on bill settlement implies yes. This is important for the
   design. Unresolved.
5. Is forex gain/loss computed per bill at settlement automatically, or only as an aggregate
   "unadjusted" figure? Is there period-end revaluation?
6. Prevent-duplicate numbering: which methods support it? One page says override + multi-user auto;
   another says override + manual.
7. Post-dated vouchers in 7.x: same as "effective date" vouchers? Do they hit ledgers on their date
   automatically? The ERP 9 pages contradict each other.
8. Books-beginning vs FY-start ordering rule; can books-beginning be changed after vouchers exist?
9. Is the Cash ledger deletable? The help page contradicts itself.
10. Interest: "calculate from" options, rounding, compound interest in Prime, and how interest is posted
    (debit note / credit note / journal class).
11. Are budgets and scenarios on by default, as one fetch reported? Is there a "net credit/debit flow"
    budget type?
12. Is 7.2 released, and does it touch the core model?
13. Natures of Suspense A/c and Branch/Divisions (our inference is Liability; confirm).

---

## Sources

- https://help.tallysolutions.com/ledgers-and-groups-in-tallyprime/ (28 groups, statements, special groups)
- https://help.tallysolutions.com/groups-in-tallyprime/ (group behaviour flags)
- https://help.tallysolutions.com/tally-prime/accounts/ledgers/ (reserved ledgers, stock-in-hand)
- https://help.tallysolutions.com/company-features-f11-tally/ (feature list and defaults)
- https://help.tallysolutions.com/set-up-company-tally/ (company creation fields)
- https://help.tallysolutions.com/tally-prime/data-management/moving-to-the-next-financial-year/ (change period, split)
- https://help.tallysolutions.com/voucher-types-tally/ (voucher type master, optional default, 24 types)
- https://help.tallysolutions.com/use-voucher-numbering-methods/ (numbering methods, insert/delete behaviour, series)
- https://tallysolutions.com/tally/flexible-voucher-numbering-for-accurate-recordkeeping (vendor marketing)
- https://help.tallysolutions.com/keyboard-shortcuts-tally-prime (voucher shortcuts, Ctrl+H/L/T)
- https://help.tallysolutions.com/create-reversing-journal/ (reversing journal)
- https://help.tallysolutions.com/scenarios-tally/ (scenarios include/exclude)
- https://help.tallysolutions.com/budgets/ (budget fields, variance)
- https://help.tallysolutions.com/tally/cost-centre-allocation-tally/ (cost centres, classes, reports)
- https://help.tallysolutions.com/docs/te9rel66/Advanced_Features/Advanced_Accounting_Features/C_Catg.htm (cost categories, ERP 9)
- https://help.tallysolutions.com/interest-calculation-tally/ (interest parameters)
- https://help.tallysolutions.com/knowledgebase/multi-currency-tally/ (currency master)
- https://help.tallysolutions.com/account-for-forex-gain-or-loss/ (forex adjustment)
- https://help.tallysolutions.com/currency-in-tallyprime-faq/ (forex FAQ)
- https://help.tallysolutions.com/tally-prime/accounting-financial-reports/manage-receivables-outstanding-tally (bill-wise refs)
- https://help.tallysolutions.com/set-up-party-ledger/ (party ledger, credit period)
- https://help.tallysolutions.com/docs/te9rel66/Creating_Masters/Accounts_Info/Creating_a_Party_Ledger.htm (credit-days check, opening bill breakup; ERP 9)
- https://help.tallysolutions.com/accounting-faq/ (voucher totals mismatch error, ledger deletion rules, invoice mode)
- https://help.tallysolutions.com/trial-balance-tally/ (difference in opening balances)
- https://help.tallysolutions.com/docs/te9rel62/Voucher_Entry/Optional_Non-Accounting_Vouchers/Post_Dated_Vouchers.htm (ERP 9)
- https://help.tallysolutions.com/docs/te9rel65/Maintaining_Company_Data/F12_Config/Voucher_Entry.htm (F12 options; ERP 9)
- https://help.tallysolutions.com/docs/te9rel66/Creating_Masters/Accounts_Info/Voucher_Class.htm (voucher classes; ERP 9)
- https://help.tallysolutions.com/release-notes-tallyprime-7-1/ (7.1 release)
- https://tallysolutions.com/tally/multi-currency-in-tally-prime/ (vendor marketing)
- https://accountune.com/tally-prime-limitations (competitor blog, biased)
- https://www.g2.com/products/tallyprime/reviews (user reviews, via search snippets)
- https://www.capterra.com/p/127762/Tally-ERP-9/reviews (user reviews, via search snippets; direct fetch blocked)
