# 03 — Inventory and Order Processing (TallyPrime reference study)

Researcher area: stock masters, godowns, batches, order processing, inventory vouchers,
manufacturing/job work, pricing, valuation, closing stock, negative stock, reorder, POS, barcode,
inventory reports.

Date: 2026-11-20. Current vendor line is TallyPrime 7.x (7.0 released 19 Dec 2025 per vendor;
the help site banner shows 7.2 as current, pages last updated Nov 2026).

Method and caveats (clean-room):
- Web research only. All content below is our own summary; no vendor text is reproduced and the
  worked numeric examples are our own, not the vendor's.
- Many findings come from the vendor help site (help.tallysolutions.com). Statements that come
  only from vendor marketing are tagged **[vendor claim]**. Statements we could not confirm in a
  source are tagged **[unconfirmed]**. Statements sourced only from Tally.ERP 9-era help pages
  (URLs containing `te9relNN`) are tagged **[ERP9-era]**; behaviour is believed to carry over to
  TallyPrime but was not re-verified on a 7.x page.
- The 7.0 release notes list no inventory changes; 7.2 adds purchase-order print templates. So the
  inventory module is mature and stable; most pages describe long-standing behaviour.
- The session's web-search budget ran out before every gap could be closed; remaining gaps are
  listed under Open questions.

---

## Summary

1. **Inventory is a quantity ledger; value is computed, not posted.** With the company feature
   "integrate accounts with inventory" on, closing stock in the P&L and Balance Sheet is derived at
   report time from the inventory records by applying each item's costing method. No per-voucher
   cost-of-goods-sold journal is visible to users (our inference from the docs; no source shows a
   COGS posting). Changing an item's costing method leaves vouchers unchanged but revalues stock
   in every report. With the feature off, the user types closing stock values into a Stock-in-Hand
   ledger by date. The Balance Sheet can show several valuation methods side by side as columns.
2. **The voucher line is item -> godown -> batch allocation.** Each inventory line can be split
   across godowns (locations, hierarchical) and, within a godown, across batches (with mfg/expiry
   dates). Opening stock is entered the same way, per godown/batch, and only as of the books
   beginning date.
3. **Physical and financial events are decoupled via "tracking numbers".** Receipt notes and
   delivery notes move stock. Purchase/sales invoices carry the accounting. A tracking number
   links them in either order. Orders (PO/SO) add a third layer: they change neither stock nor
   accounts, but they track pending quantity per line with due dates, partial fulfilment and
   pre-closure.
4. **Nine costing methods per item, with Average Cost as default.** They are zero cost, average,
   FIFO, FIFO perpetual, last purchase cost, LIFO annual, LIFO perpetual, standard cost and monthly
   average. There are also four "market valuation" methods that only suggest selling rates. LIFO is
   not permitted under Indian AS 2, Ind AS 2 or ICDS-II, so a statutory product should not offer it
   for financial statements.
5. **Negative stock is allowed, with at most a warning.** Built-in control is a per-voucher-type
   "warn on negative stock" option. Hard blocking needs custom TDL or paid add-ons [ERP9-era for
   the TDL FAQ]. An exception report lists negative items and negative batches.
6. **Retail and pharma basics are present but thin.**
   - Barcodes are stored in the item alias field. There is no native label printing; the barcode
     number can only be printed on the invoice.
   - POS supports split tender (cash, card, cheque, gift voucher).
   - Batch expiry hides expired batches at sale time, but FEFO batch picking is manual.
   - Schemes and free goods are handled by separate actual and billed quantity columns, plus
     zero-valued lines.

---

## Feature inventory

Priority key: **MVP** = needed for first usable release for a trading/GST business; **v2** =
needed to win distribution/pharma/manufacturing users; **later** = niche or replaceable.

| Feature | What it does | How the user interacts (Tally) | Priority |
|---|---|---|---|
| Stock groups | Hierarchical classification of items (Primary root cannot be altered/deleted; groups optional). Can carry GST/HSN defaults and reorder levels. Option to add up quantities of child items (only meaningful when units match) [unconfirmed field name]. | Create master > Stock Group; Under = parent group. | MVP |
| Stock categories | A second classification, separate from groups (e.g. brand vs product type). Enabled in F12 at any time, but each item must then be altered to assign a category. | Create master > Stock Category; item field "Category". | v2 |
| Stock items | Core master. Fields: name, aliases (barcode goes here), language alias, group, category, base unit, alternate unit and conversion, GST details (taxability, HSN/SAC, rate, slab-by-value, cess), MRP (optionally state-wise), standard cost and standard selling price (dated), costing method, market valuation method, opening balance (qty, rate, value; allocations per godown/batch), batch flags (maintain batches, track mfg date, use expiry), reorder levels, behaviour options, description, part number [part number field unconfirmed; only third-party source]. | Create master > Stock Item; F12 toggles reveal optional fields; Ctrl+I "more details". | MVP |
| Simple units | Single unit (pcs, kg, ltr) with the GST UQC mapping (decimal-places setting **[unconfirmed]**) (required for GST HSN summary). | Create master > Unit, type Simple, choose UQC. | MVP |
| Compound units | Fixed relationship between two units (e.g. 1 box = 10 pcs). Can be nested to many levels. Cannot be changed once defined. | Unit type Compound: first unit, conversion, second unit. | MVP |
| Alternate units | Per-item second unit whose conversion varies by item and can be overridden on a voucher (buy by box, sell by piece). | Item F12 "use alternate units"; enter "X <alt> = Y <base>". | MVP |
| Actual vs billed quantity | Separate quantity shipped from quantity charged (free goods, schemes). | F11 "use separate actual and billed quantity columns". | v2 |
| Godowns / locations | Hierarchical storage locations. A parent can be set to "no storage" so it only groups children. Optional address. Third-party flags: "our stock with third party" (consignment or job worker) and "third party stock with us" (excluded from books). | Create master > Godown; allocate godown per voucher line. | MVP (single + hierarchy), v2 (third-party) |
| Batches | Lots per item, optionally per godown, with mfg date (defaults to voucher date, cannot be after it) and expiry (absolute date or a period after mfg; not before mfg). Expired batches are hidden from the active-batch list at sale date **[ERP9-era]**. Batches persist after they reach zero. | Item flags; batch created inline in voucher allocation. | v2 (MVP for pharma) |
| Tracking numbers | Link receipt/delivery notes with purchase/sales invoices in either order. "Not Applicable" means the invoice moves stock itself. **[ERP9-era for the feature toggle; delivery-note linkage confirmed on 7.2-era page]** | F11 "use tracking numbers"; Tracking No. field in allocations, with "New Number" or pick existing. | v2 |
| Purchase order | Records an order to a supplier; no stock or accounting effect. Lines have due dates; can be pre-closed with a quantity and reason. | F11 "enable purchase order processing"; voucher under Other Vouchers. | v2 |
| Sales order | Same, for customers. Optional order number per line, due date per line, partial billing. Warns on credit limit but allows saving. Can be marked optional and printed as a quotation. | F11 "enable sales order processing". | v2 |
| Receipt note (GRN) | Inward stock against PO or "Not Applicable"; accounts posted later by purchase invoice. | Voucher type Receipt Note. | v2 |
| Delivery note | Outward stock against SO or none; invoice later or earlier, linked by tracking number. | Voucher type Delivery Note. | v2 |
| Rejections in / out | Goods returned by customer / to supplier as inventory documents (quantity). Rejections Out with amount only and zero quantity is a documented way to write down closing stock value. | Inventory vouchers. | v2 |
| Pending orders and bills | Purchase/Sales Order Outstandings, Order Books, Order Details (drill from item). Reports for notes and bills still pending matching [report names unconfirmed for 7.x]. | Display > reports; Alt+W pre-close in Order Details. | v2 |
| Stock journal | Transfers between godowns, and consumption-to-production conversions. Can record additional costs (freight etc.) as a percentage or amount loaded onto stock value. | Inventory voucher with source (consumption) and destination (production) sides. | MVP (transfer), v2 (additional cost) |
| Physical stock | Records counted stock to reconcile book versus physical. Also used to set opening stock on a date other than books beginning (quantity only). Whether it records an absolute count or a delta is **[unconfirmed]**; docs imply absolute count. | Inventory voucher. | MVP |
| Material in / out | Movement of goods to and from job workers without sale. Job-work variants linked to job work orders. | Voucher types Material In / Material Out, "use for job work". | v2 |
| Job work (principal and job worker) | Principal: Job Work Out Order, Material Out (raw material), Material In (finished goods), purchase voucher for job charges. Job worker: Job Work In Order, Material In (raw), Material Out (finished). Reports include Stock With Job Worker and Material In/Out/Movement registers. GST ITC-04 relevance **[unconfirmed]** in Tally. | F11 job order processing; voucher types. | later (v2 for manufacturing segment) |
| Bill of materials | Per-item component list for a quantity of finished item, with optional co-products, by-products and scrap. Multiple BoMs per item [unconfirmed count semantics]. Main item must be deleted before its components. | Item master, BoM component entry [exact label unconfirmed]. | v2 |
| Manufacturing journal | Stock journal variant using a BoM. Consumes components, produces main item plus co/by-products and scrap. Each secondary output takes a cost percentage, and the main item gets the remainder. Scrap goes to a designated godown. | Stock-journal voucher type configured for manufacturing [exact label unconfirmed]. | v2 |
| Price levels and price lists | Named levels (Wholesale, Retail, Dealer) assigned to party ledgers. Each list gives item, date-effective, quantity slab (from / less-than), rate and discount %. Old revisions are kept. Can be exported and imported via XML. | F11 enable price levels; Alt+G price list; level auto-applied in voucher. | v2 |
| Discount column | Item-level discount % on invoice lines (also comes from the price list). | Invoice F12 [exact option name unconfirmed]. | MVP |
| Standard cost and price | Standard rates (date-effective history **[unconfirmed]**). They prefill vouchers and drive valuation when the costing method is Standard Cost. | Item master, set standard rates. | MVP |
| Costing methods | Valuation of closing stock and consumption (see Data model). Set per item, and can be changed later with retroactive report effect. No bulk change; vendor docs suggest an Excel round-trip. | Item master, F12 costing and market valuation methods. | MVP (avg, FIFO), v2 (standard, monthly avg, last purchase, zero) |
| Market valuation methods | Rate suggestion on sales vouchers: Average Price (default), Last Sales Price, Standard Price, Zero Price (manual). | Item master. | v2 |
| Integrate accounts with inventory | When on, P&L and BS closing stock come from inventory **[ERP9-era page]**. When off, closing stock is a manual dated value on a Stock-in-Hand ledger (confirmed in TallyPrime FAQ). | F11 company feature. | MVP |
| Behaviour options | Per item: "treat all sales as new manufacture" (a sale also counts as inward, so the item never goes negative; for made-to-order items) and "treat all purchases as consumed". Others reported in community material ("ignore negative balances", "ignore differences due to physical counting", "treat rejections inward as scrap") are **[unconfirmed]**. | Item master, provide behaviour options. | later |
| Negative stock warning | Voucher-entry F12 option "warn on negative stock balance". Warns but does not block. | Voucher F12 (configure). | MVP (with a hard-block option, which Tally lacks) |
| Zero-valued entries | Voucher-type option to allow lines with zero value (free items, value-only adjustments). | Voucher type "allow zero-valued transactions". | MVP |
| Reorder levels | Per item, group or category. Simple: reorder quantity plus minimum order quantity. Advanced: computed from consumption over a past period, choosing the higher or lower figure. The report computes net available = stock + pending POs − pending SOs; shortfall = reorder level − net available; order to place = max(shortfall, MOQ) if shortfall > 0, else 0. | Alt+G Reorder Level; Reorder Status report (F8 shows only items to reorder). | v2 |
| POS invoicing | Sales voucher class for counter billing. Party can be "Not Applicable". Quantity defaults to 1. Split tender (Alt+I) across cash with change calculation, card or cheque to bank ledgers, and gift voucher ledgers. POS printer. POS Register report. | Create voucher > Sales (F8) > POS voucher type; a POS voucher class can predefine payment modes and ledgers. | v2 |
| Barcode | Barcode stored in the item alias. Scanning into the item field selects the item. No native label printing (officially); the barcode number can be printed on the invoice. | Scanner as keyboard wedge. | MVP (scan), v2 (label print, multiple barcodes per item/unit) |
| Stock summary | Opening, inwards, outwards, closing quantity/rate/value by group or category, with drill-down to item, then stock vouchers, then voucher. Auto-columns allow comparison across periods, godowns or valuation methods. | Display > Stock Summary. | MVP |
| Stock vouchers / item register | Per-item movement with running balance. Shows "default" (costing method) totals and "transaction" value totals; the transaction value is for reporting only, not P&L. | Drill-down. | MVP |
| Movement analysis | Inward/outward by item, group, category or party. Only counts accounting-integrated vouchers (sales, purchases); pure inventory vouchers are excluded. Stock transfer analysis is a separate view. | Display > Movement Analysis. | v2 |
| Stock ageing | User-defined age slabs; values remaining stock at actual purchase cost after outwards have been consumed from the earliest purchases (FIFO-based ageing). Expiry-based batch view. | Display > Ageing Analysis. | v2 |
| Batch reports | Batch Summary per item, Batch Vouchers, expired / near-expiry batches, Negative Batches. | Display > Batch. | v2 |
| Godown summary | Stock by location, consolidated through the godown hierarchy. | Display > Godown Summary. | MVP |
| Negative stock report | Exception report listing items with negative closing quantity **[ERP9-era]**. | Display > Exception Reports. | MVP |

---

## Data-model implications

### Entities (proposed)

- `Unit { id, symbol, formal_name, decimal_places: u8, uqc: GstUqc, kind: Simple | Compound { first: UnitId, factor: Decimal, second: UnitId } }`
  - Invariant: compound graphs are acyclic. A compound unit is immutable once used.
  - Conversion to the base simple unit is exact (Decimal, never float).
- `StockGroup { id, name, parent: Option<StockGroupId>, add_quantities: bool, gst_defaults: Option<GstRateHistory>, reorder: Option<ReorderRule> }` (tree; root implicit)
- `StockCategory { id, name, parent: Option<StockCategoryId> }`
- `StockItem`
  - Identity and classification: `id`, `name`, `aliases: Vec<String>`, `barcodes: Vec<Barcode { code, unit: UnitId, qty_per_scan }>`, `group`, `category`.
  - Units: `base_unit`, `alt_unit: Option<AltUnit { unit, base_qty: Decimal, alt_qty: Decimal }>`.
  - Tax and price: `gst: GstRateHistory` (date-effective: taxability Taxable/Exempt/NilRated/NonGst, HSN/SAC, rates, optional value-slabs, cess), `mrp: Vec<Dated<MrpByState>>`, `standard_cost: Vec<Dated<Decimal>>`, `standard_price: Vec<Dated<Decimal>>`.
  - Valuation: `costing_method: CostingMethod`, `market_method: MarketMethod`.
  - Batch flags: `batches: BatchPolicy { enabled, track_mfg, track_expiry }`.
  - Planning: `reorder: Option<ReorderRule>`, `bom: Vec<Bom>`, `behaviour: BehaviourFlags`.
  - Opening stock: `opening: Vec<OpeningAllocation { godown, batch: Option, qty, rate, value }>`.
  - Invariant: `opening` is dated at the books beginning date only. Later openings are physical-stock or stock-journal vouchers.
- `Godown { id, name, parent, allows_storage: bool, address, ownership: Own | OursAtThirdParty { party } | ThirdPartyWithUs { party } }`
  - Invariant: `ThirdPartyWithUs` stock is excluded from valuation and the Balance Sheet.
  - Invariant: a non-storage godown has no allocations.
- `Batch { id, item, godown: Option<GodownId> /* policy: common across godowns or per godown — open question */, name, mfg_date: Option<Date>, expiry: Option<Expiry::Date | Expiry::AfterMfg(Period)> }`
  - Invariant: mfg ≤ the voucher date of first inward.
  - Invariant: expiry ≥ mfg.
- `PriceLevel { id, name }`
- `PriceList { level, item, effective_from: Date, slabs: Vec<{ from_qty, below_qty: Option, rate, discount_pct }> }`
  - Invariant: slabs are non-overlapping and contiguous.
  - Lookup rule: the latest `effective_from` ≤ voucher date, then the slab containing the quantity.
- `Bom { item, name, output_qty, components: Vec<{ item, godown, qty }>, secondary_outputs: Vec<{ item, kind: CoProduct|ByProduct|Scrap, qty, cost_pct }> }`
  - Invariant: Σ cost_pct ≤ 100. The main output gets (100 − Σ)%.
- `Order { kind: Purchase|Sales|JobWorkIn|JobWorkOut, party, number, date, lines: Vec<OrderLine { item, godown, qty, rate, discount, due_date, preclosed: Option<{ qty, reason }> }> }`
  - No stock or GL effect.
  - Pending quantity = ordered − Σ fulfilled (by notes or invoices) − preclosed.
  - Invariant: pending quantity ≥ 0. Over-fulfilment must be either rejected or explicit (open question).
- `InventoryLine` (inside any voucher)
  - Fields: `{ item, actual_qty, billed_qty, unit, rate, discount, value, allocations: Vec<Allocation> }`.
  - `Allocation { godown, batch, tracking: TrackingRef, order: OrderRef, qty, rate, value }`.
  - `TrackingRef = NotApplicable | New(id) | Existing(id)`.
- Voucher roles (an enum, not stringly-typed). The quantity effect (Q) and GL effect (A) differ. **This table is our proposed model**; the no-GL effect of rejections, material in/out and a standalone delivery note is inferred from ERP9-era docs, not confirmed on 7.x:
  | Role | Q | A |
  |---|---|---|
  | Purchase invoice, Sales invoice | yes, unless linked to a note | yes |
  | ReceiptNote, DeliveryNote | yes | no |
  | RejectionIn, RejectionOut | yes | no |
  | PurchaseOrder, SalesOrder, JobWork orders | no | no |
  | StockJournal | yes (both sides, value carried, plus additional cost) | no |
  | ManufacturingJournal | yes | no |
  | PhysicalStock | adjusts book quantity to the count | no |
  | MaterialIn, MaterialOut | yes; ownership may not change | no |
  | DebitNote, CreditNote with items | yes | yes |

  - Key invariant: for a tracking number, Σ quantity moved by notes equals Σ quantity on linked invoices, so stock is counted once, never twice.
  - A linked invoice must not move stock again (or a linked note must not move it again, if the invoice came first).

### Stock position

The stock position is a pure function of the movement ledger:

`qty(item, godown, batch, as_of) = opening + Σ inward − Σ outward`

Keep it as a derived index, recomputable from vouchers. Never store it as the source of truth.

### Valuation algorithms (precise; textbook definition first, then Tally-specific notes)

Common definitions:
- **Inwards** are quantity increases with a cost: purchases, receipt notes, the production side of stock or manufacturing journals, rejections-in, positive physical-stock adjustments, and opening stock.
- Which inward types Tally includes in each method's cost pool is **[unconfirmed]**. In particular:
  - rejections-in (valued at the original sale cost or the current cost?);
  - value-only debit notes (the FAQ says such a note is treated as an extra cost loaded onto the first purchase);
  - additional cost on stock journals.
- Valuation granularity: Tally appears to value **per item across all godowns and batches** (one method per item). Batch-level actual costing is a reporting view [unconfirmed]. Proposal: value per item by default; per-batch specific identification as an option (Ind AS 2 allows specific identification for non-interchangeable items).

Our own worked data, used for all methods. Item has opening 0.

| Date | Event | Qty | Rate |
|---|---|---|---|
| 01-Apr | Purchase | 100 | 10 |
| 10-Apr | Purchase | 50 | 13 |
| 15-Apr | Sale | 120 | — |
| 20-May | Purchase | 40 | 15 |
| 31-May | Sale | 30 | — |
| Close (31-May) | | 40 | |

1. **At Zero Cost**: closing value = 0 and consumption cost = 0. Quantity is tracked; value is suppressed.
2. **Average Cost (Tally default)**:
   - The vendor formula is total inward cost ÷ total inward quantity over the period, applied to closing quantity.
   - The docs do not say whether opening stock is included or whether it is a periodic or moving average. Opening is presumably included; periodic versus moving is **[unconfirmed]**.
   - Periodic over the period: (1000 + 650 + 600) / 190 = 11.842…, so closing 40 × 11.842 = 473.68.
   - Moving average would differ:
     - After 10-Apr: 1650 / 150 = 11.00.
     - The sale of 120 leaves 30 @ 11.00.
     - Add 40 @ 15: 930 / 70 = 13.2857.
     - The sale of 30 leaves 40, so closing = 531.43.
   - The two give materially different numbers. We must pick one explicitly and document it. Ind AS 2 permits either a periodic or a moving weighted average.
   - Rounding: the vendor FAQ admits rounding/truncation drift of the per-unit rate between dates. We should carry full-precision Decimal and round only at display or posting.
3. **Monthly Average**:
   - The periodic average is recomputed per calendar month. Each month's closing value becomes the next month's opening.
   - April: (1000 + 650) / 150 = 11.00. Closing 30 → 330.
   - May: (330 + 600) / 70 = 13.2857. Closing 40 → 531.43.
   - Note: with this data monthly average equals the moving average because each month has only one outward after all its inwards. A test that separates them needs an inward after an outward within a month, e.g. add 25-Apr purchase 20 @ 16 (moving: 30 @ 11 + 20 @ 16 = 650/50 = 13.00 before May; monthly: April pool (1650 + 320)/170 = 11.588…, closing 50 → 579.41).
4. **FIFO (annual)**:
   - Outwards consume the oldest lots first. Closing = the remaining lots at their own rates.
   - The 15-Apr sale of 120 takes 100 @ 10 and 20 @ 13. That leaves 30 @ 13.
   - The 31-May sale of 30 takes those 30 @ 13. That leaves 40 @ 15, so closing = 600.
   - Year-end carry-forward: the vendor says the next year's opening rate is the average of the closing lots (value ÷ quantity), so lot identity is collapsed at year boundaries.
5. **FIFO Perpetual**:
   - Lots are carried across financial years from company creation, with no year-end collapse.
   - The vendor text also says remaining stock is referenced to the most recent purchase price. This is ambiguous and possibly a summarisation artefact: **[unconfirmed]**. Do not implement it until it is observed in a real build.
6. **LIFO Annual**:
   - Outwards consume the newest lots in existence at the time of the outward.
   - The 15-Apr sale of 120 takes 50 @ 13 and 70 @ 10. That leaves 30 @ 10.
   - The 31-May sale of 30 takes 30 of the newest lot, which is 20-May's 40 @ 15. That leaves 30 @ 10 and 10 @ 15, so closing = 450.
   - The year-end collapse to an average is the same as FIFO.
   - **Not permitted for Indian statutory accounts (AS 2, Ind AS 2, ICDS-II).**
7. **LIFO Perpetual**: lot identity is carried across years from company creation. Not permitted for statutory accounts.
8. **Last Purchase Cost**: closing qty × the rate of the latest purchase on or before the as-of date: 40 × 15 = 600.
9. **Standard Cost**:
   - Closing qty × the standard cost effective at the as-of date. For example, std 12 gives 40 × 12 = 480.
   - Vendor FAQ: if no standard rate is defined, the last purchase rate is used for consumption.
   - That silent fallback is something we must instead surface (`Unsupported`/error or an explicit, labelled fallback).

Other valuation rules:
- **Gross profit / consumption**: the vendor defines consumption as total inward value − closing value (that is, opening + inwards − closing). Example, periodic average: 2250 − 473.68 = 1776.32 cost of the 150 units sold.
- **Market valuation methods** never affect closing stock. They only prefill the selling rate:
  - Average Price = Σ sales value ÷ Σ sales qty to date;
  - Last Sales Price;
  - Standard Price (dated);
  - Zero, meaning manual entry.
- **Retroactive method change**: changing the method revalues all periods in reports. Our model should treat the method as a dated attribute, or warn when a change alters closed or filed periods.
- **Lower of cost and NRV** (AS 2 requirement): there is no evidence that Tally does an NRV comparison. Its write-down workaround is a value-only Rejections Out voucher or a manual adjustment. A `NrvWriteDown` voucher/adjustment is an opportunity.

### Negative stock

Tally behaviour:
- Allowed. The voucher type can warn.
- Behaviour flag "treat all sales as new manufacture" hides negatives.
- Exception reports list negative items and batches.
- How negative quantity is valued is **[unconfirmed]**.

Our recommendation (the engine should never show an unflagged wrong number):
- Policy per company and per voucher type: `Block | Warn | Allow`. The default is `Warn` for trading and `Block` per godown/batch for pharma.
- Any valuation computed over a period where an item's running quantity went negative should be tagged (e.g. `Valuation::Provisional { reason: NegativeStock { item, date } }`) or returned as unsupported, rather than silently producing a number.
- Report renderers show the flag.

### Closing stock to P&L and Balance Sheet

The textbook trading-account form, which Tally's P&L follows:

`Gross profit = Sales + Closing stock − Opening stock − Purchases − Direct expenses`

- Opening and closing stock appear on the P&L.
- Closing stock also appears on the Balance Sheet under Current Assets / Stock-in-Hand.
- **Integrated mode**: closing stock = Σ over items of `valuation(item, method, as_of)`, excluding third-party-with-us godowns and including ours-at-third-party.
- **Non-integrated mode**: closing stock = the user-entered dated value on Stock-in-Hand ledger(s). Journals cannot be posted to those ledgers.

Design implications:
- The engine must compute closing stock lazily for any as-of date. This is periodic inventory; no perpetual COGS journals are needed.
- Results must be cacheable but invalidated on any back-dated voucher.
- This is a strong reason for an append-only movement ledger and a deterministic valuation function with golden tests. Expected values in tests should come from hand derivation, as in the table above, and record that provenance.

---

## Industry needs

- **Pharma (distribution and retail chemists)**
  - Mandatory: batch number with mfg and expiry on every inward and outward line; expiry blocking on sale; near-expiry reports and returns to supplier (rejections out / debit note) by batch.
  - Also needed: MRP on invoice; schemes (buy 10, get 1 free, via actual vs billed quantity); and per-batch MRP and purchase rate [Tally per-batch MRP **unconfirmed**].
  - Tally gap: no automatic FEFO picking (manual batch choice). Also needed but not researched here: drug licence numbers on party masters, and Schedule H/H1 register [out of area; flag to compliance researcher].
- **FMCG distribution**
  - Price levels per customer type, with quantity slabs and dated revisions.
  - Free goods and trade schemes; line discounts plus cash discounts.
  - Alternate units (case, inner, piece) with variable conversion.
  - Sales orders with salesman and beat [beat **unconfirmed** in Tally].
  - Delivery note → invoice flow with tracking numbers, and pending order and bill reports.
  - Multi-godown, including van stock as a godown; reorder status.
  - High voucher volumes make the vendor's data-size slowdowns relevant (see below).
- **Manufacturing (SME)**
  - BoM with co-products, by-products and scrap with cost percentages.
  - Manufacturing journal; stock journal with additional costs.
  - Job work in both directions, with stock-with-job-worker tracking.
  - WIP is effectively only representable as a godown or item [inference].
  - Standard costing. There is no routing, capacity planning or production order beyond job work orders in Tally [inference from absence in docs].
- **Retail**
  - POS with fast barcode scan, split tender, change calculation and thermal printing.
  - Barcode label printing (missing natively in Tally).
  - MRP-inclusive pricing. Many SKUs with size and colour variants (no variant/matrix concept found in Tally; **[unconfirmed]**).
  - Item ageing for dead stock.

---

## What users love / hate

Evidence is limited: there are few user-voice sources beyond review aggregators and vendor pages.
Forum and Reddit searches returned little.

Love:
- Keyboard-fast voucher entry. Inventory gets 89% "important/highly important" in one Capterra
  summary **[aggregator]**; reviewers call inventory "excellent".
- Inventory and accounts integrated in one product. Drill-down from the Balance Sheet to the
  stock register and then to the voucher.
- Flexible units (compound plus alternate), godown hierarchy, batches and expiry: enough for most
  SMEs without an ERP **[vendor claim + reviews]**.
- Side-by-side valuation-method comparison columns on the Balance Sheet and stock summary.

Hate / pain:
- Slowdowns with large data and multiple users. Vendor guidance is to split the company data
  periodically **[reviews; vendor guide]**.
- Negative stock cannot be hard-blocked without TDL or paid add-ons. Users get wrong closing stock
  after mis-selecting a godown or batch. The vendor's own FAQ documents this cause.
- No bulk change of costing method (Excel round-trip workaround). Valuation changes are
  retroactive, so past reports silently change.
- Rounding drift in average-cost closing values between dates (vendor FAQ acknowledges it).
- Barcode is an alias hack with no label printing; POS is basic.
- Opening stock only at books beginning; later openings need workaround vouchers.
- No automatic FEFO, NRV write-down or manufacturing planning.
- Dated UI, weak mobile/cloud, add-on and customisation costs **[reviews]**.

---

## Open questions

Resolve by observing a real TallyPrime 7.x install (observed provenance) or by deriving from
standards.

1. Is Average Cost a periodic or a moving weighted average? Is opening stock included in the pool,
   and are rejections-in or outwards at a different rate treated specially?
2. Exact semantics of FIFO Perpetual ("last purchase price as reference"?) and LIFO Perpetual.
   Does the year-end collapse to an average happen for annual FIFO and LIFO?
3. How is a negative running quantity valued (zero, last rate, average)? How are later inwards
   matched against the deficit?
4. Valuation granularity: per item only, or per godown or batch, under each method?
5. Physical Stock voucher: does it record the absolute counted quantity per godown/batch (and so
   imply the delta), and is the delta valued at the costing rate?
6. Are batches unique per item globally or per godown? Can one batch span godowns?
7. Do rejection vouchers affect valuation the same way as a purchase return or sales return? How
   do they interact with debit and credit notes that carry items?
8. Behaviour options beyond the two confirmed ones: do they exist in 7.x?
9. Pending-reports naming in 7.x (Sales Bills Pending, Receipt Notes Pending, etc.) and whether
   over-fulfilment of an order is allowed.
10. Per-batch MRP and rates; item variants; part numbers (only third-party claims).
11. Job work and GST ITC-04 support in Tally (hand to the GST compliance researcher).
12. Whether the stock-item GST rate history is date-effective per item (it appears so via "GST rate
    and related details" history) and how it interacts with ledger-first versus item-first
    precedence.

Product decisions for us (not Tally questions):
- Offer LIFO at all? It is non-statutory, so if offered at all it belongs in management reports only.
- Default negative-stock policy per industry template.
- Whether to post a perpetual COGS journal (an audit-friendly alternative) or stay periodic like Tally.
- NRV write-down as a first-class voucher.

---

## Sources

Vendor help (help.tallysolutions.com). Pages are TallyPrime unless the URL contains `te9rel`
(Tally.ERP 9 era):
- Stock valuation methods: https://help.tallysolutions.com/tally-prime/inventory/stock-valuation-methods-tallyprime/
- Costing methods: https://help.tallysolutions.com/costing-methods/
- Market valuation methods: https://help.tallysolutions.com/market-valuation-methods/
- Stock valuation FAQ: https://help.tallysolutions.com/tally-prime/inventory/stock-valuation-faq/
- Stock items (manage): https://help.tallysolutions.com/tally-prime/inventory/manage-stock-item-tally/
- Stock items FAQ: https://help.tallysolutions.com/tally-prime/inventory/stock-items-faq/
- Units of measurement: https://help.tallysolutions.com/units-of-measurement/
- Set up stock items for GST: https://help.tallysolutions.com/set-up-stock-items-for-gst/
- GST rate / HSN setup: https://help.tallysolutions.com/set-up-gst-rate-hsn-sac-details/
- Godowns/locations: https://help.tallysolutions.com/tally-prime/inventory/inventory-storage-using-godowns-locations-tally/
- Third-party godowns: https://help.tallysolutions.com/tally-prime/inventory/manage-godowns-for-third-party-tally/
- Batches: https://help.tallysolutions.com/tally-prime/inventory/manage-inventory-batch-wise-tally/
- Mfg/expiry dates [ERP9-era]: https://help.tallysolutions.com/docs/te9rel66/Voucher_Entry/Advanced_Voucher_Entry/Using_Mfg.htm
- Purchase order: https://help.tallysolutions.com/tally-prime/purchase-process/purchase-order-tally/
- Sales order: https://help.tallysolutions.com/tally-prime/sales-process/sales-order-tally/
- Delivery note: https://help.tallysolutions.com/delivery-note/
- Tracking numbers [ERP9-era]: https://help.tallysolutions.com/article/te9rel64/Voucher_Entry/Advanced_Voucher_Entry/Using_Tracking_Numbers.htm
- Inventory vouchers: https://help.tallysolutions.com/inventory-vouchers-tallyprime/
- Manufacturing: https://help.tallysolutions.com/tally-prime/inventory/inventory-in-manufacturing/
- Manufacturing journal: https://help.tallysolutions.com/manufacturing-journal/
- Job work: https://help.tallysolutions.com/tally-prime/inventory/job-work/
- Price lists FAQ: https://help.tallysolutions.com/tally-prime/inventory/price-lists-and-price-levels-faq/
- Reorder: https://help.tallysolutions.com/tally-prime/inventory/reorder-stock-items-reorder-status-and-reorder-quantity/
- POS: https://help.tallysolutions.com/tally-prime/sales-process/point-of-sale-tally/
- Barcodes: https://help.tallysolutions.com/barcodes/
- Stock summary / behaviour options: https://help.tallysolutions.com/stock-summary/
- Sales FAQ (warn on negative stock): https://help.tallysolutions.com/tally-prime/sales-process/faq-sales/
- Negative stock report [ERP9-era]: https://help.tallysolutions.com/article/te9rel64/Reports/MIS_Reports/Negative_Stock.htm
- Integrate accounts with inventory [ERP9-era]: https://help.tallysolutions.com/docs/te9rel65/Reports/Display_Financial_Statements/Integrate_Accounts_with_Inventory.htm
- Stock ageing: https://help.tallysolutions.com/tally-prime/inventory-reports/stock-ageing-analysis-report-tally/
- Movement analysis: https://help.tallysolutions.com/tally-prime/inventory-reports/movement-analysis-tally/
- Release notes 7.0: https://help.tallysolutions.com/release-notes-tallyprime-7-0
- Negative stock control TDL FAQ [ERP9-era, developer reference]: https://help.tallysolutions.com/article/DeveloperReference/faq/7727.html

Vendor marketing [vendor claim]:
- https://tallysolutions.com/tally/price-lists-discounts-dealer-schemes-tallyprime/
- https://tallysolutions.com/darpan/product-update-tallyprime-7-0/
- https://tallysolutions.com/smart-moves/expert-insights/can-your-accounting-software-handle-high-transaction-volumes-an-expert-performance-guide/

Third party:
- Barcode guide (conflicts with official docs on label printing/part numbers): https://tallyatcloud.com/articles/barcode-software-tally-prime-can-you-implement-barcode-management-in-tally-2025-full-guide
- Reviews: https://www.capterra.com/p/127762/Tally-ERP-9/reviews ; https://www.g2.com/products/tallyprime/reviews ; https://techjockey.com/reviews/tally-erp-9
- Indian standards on inventory cost formulas (LIFO excluded): https://www.patronaccounting.com/accounting-standards/ind-as-2-inventories ; https://www.patronaccounting.com/accounting-standards/as-2-valuation-of-inventories ; https://cleartax.in/s/indian-accounting-standards-ind-as-inventories-2
