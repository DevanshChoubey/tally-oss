# 04 — GST: TallyPrime behaviour and the rule set our engine must implement

Researcher area: GST. Research date: 2026-11-20. Method: web research only (clean-room). Everything is
summarised in my own words. Nothing was downloaded, run or decompiled.

**Provenance tags used in this document**

| Tag | Meaning |
|---|---|
| **[V]** | Verified this session against a source listed in Sources (official advisory, notification copy, or a reputable secondary summary of one). |
| **[S]** | Secondary sources only (tax blogs, firm alerts). The date or number should be checked on cbic-gst.gov.in / gst.gov.in before we encode it. |
| **[K]** | From the researcher's background knowledge of the GST law (stable since 2017–2024). Not re-fetched this session because the search budget ran out. Treat as **pending** under our engineering rules until someone adds a citation. |
| **[Vendor]** | A Tally Solutions claim, from its own help pages, release notes or marketing. |
| **(unconfirmed)** | Sources conflict, or no official text was found. |

The search budget for this session was used up before every item could be checked against a primary
source. The [K] items need a follow-up pass that adds notification or rule citations.

---

## 1. Summary

1. **GST is a time-versioned rule system, not a fixed rate table.** Rates, thresholds, form layouts and
   portal locks change several times a year. Recent examples:
   - GST 2.0 slabs (0/5/18/40), effective 22 Sep 2025 [V].
   - Tobacco compensation cess reduced to nil, effective 1 Feb 2026 [V].
   - The Section 13(8)(b) intermediary place-of-supply rule removed, effective 30 Mar 2026 [S].
   - Changes to post-sale discounts and credit notes, effective 1 Nov 2026 [S].
   - Delivery services through e-commerce operators at 5% under Section 9(5), effective 1 Nov 2026 (unconfirmed).

   Every rate, HSN mapping and rule needs `effective_from` / `effective_to` dates. The date that decides
   which rule applies is the time of supply, not the date the voucher was entered.
2. **The portal now decides most of GSTR-3B.**
   - Outward liability in Tables 3.1 and 3.2 is auto-populated from GSTR-1, GSTR-1A and IFF, and locked (from the July 2025 and Nov 2025 tax periods) [S].
   - Interest in Table 5.1 is locked from the Jan 2026 tax period [S].
   - ITC in Table 4 comes from GSTR-2B as shaped by IMS actions. A full lock on Table 4 ITC was "expected July 2026", but **no official GSTN advisory confirming it was found as of Sep–Nov 2026 (unconfirmed)**.

   Our app should treat GSTR-3B as a reconciliation between books and portal, not something it computes alone.
3. **IMS (Invoice Management System) is the centre of input-side compliance.** Live from the Oct 2024
   period [S]. From the Oct 2025 tax period:
   - Credit notes and some amendments can be kept "Pending" for one tax period.
   - The recipient can declare how much ITC was actually reversed.
   - Remarks can be added to rejected or pending records [S].
   - A record the recipient does not act on counts as accepted [S].
4. **What Tally exposes today** [Vendor]:
   - Multiple GSTINs in one company (3.0).
   - Connected GST upload and download (5.0).
   - HSN summary split into B2B and B2C (6.0).
   - IMS download, action and upload, plus GSTR-2B recompute (6.1).
   - ITC reduction in IMS (7.0, 19 Dec 2025).
   - IMS Books View, HSN validation and 30-day login (7.1, 20 May 2026).
   - GSTR-9 Excel export for the offline tool (7.2).

   Gaps Tally documents itself: no GSTR-1A support, and no online filing of GSTR-9.
5. **Tally's best GST idea is the exception bucket.** Each return report sorts vouchers into ready,
   not relevant, mismatch, and "Uncertain Transactions (Corrections needed)". Users fix entries by
   drilling down, or in bulk with "Update as per Masters". This fits our "never a wrong number" rule
   closely, and we should copy the concept, not the screens.

---

## 2. GST rule set our engine must implement

Statute abbreviations:
- CGST = Central Goods and Services Tax Act, 2017.
- IGST = Integrated Goods and Services Tax Act, 2017.
- R. = CGST Rules, 2017.
- N. = notification, with the series named, for example 9/2025-CT(R) for Central Tax (Rate).

### 2.1 Registration model

| Rule | Legal ref | Effective | Tag |
|---|---|---|---|
| One PAN can hold one GSTIN per State/UT. Optional separate registrations for business verticals in the same state. An SEZ unit or developer needs a separate registration from the domestic unit. | CGST s.25; R.8, R.11 | 1 Jul 2017 | [K] |
| A GSTIN has 15 characters: 2-digit state code, 10-character PAN, entity number, "Z" by default, and a check digit. Each GSTIN is a separate "distinct person" (s.25(4)). Supplies between them are taxable, so stock transfers and cross-charges must be invoiced. | s.25(4)–(5); Sch. I | 2017 | [K] |
| **Registration types the engine must model:** Regular, Composition (s.10), Casual taxable, Non-resident (s.24), ISD, TDS deductor (s.51), TCS/e-commerce operator (s.52), OIDAR, UIN holder (embassies and UN bodies), SEZ unit, SEZ developer. Counterparties also include **Unregistered** and **Consumer**. | s.22–25, 51, 52 | 2017 | [K] |
| **ISD is mandatory** for common input services (bought by one GSTIN on behalf of distinct persons). This matters for multi-GSTIN companies. | s.20 as substituted by Finance Act 2024 | 1 Apr 2025 | [K] |
| Registration thresholds: goods ₹40 lakh (₹20 lakh in special-category states), services ₹20 lakh (₹10 lakh). Compulsory registration under s.24 for inter-state suppliers, RCM recipients, ECO sellers and others. Services sold through an ECO under s.9(5) are exempt from compulsory registration. | s.22, 24; N.10/2019-CT | 2019 | [K] |
| **Composition (s.10):** turnover limit ₹1.5 cr (₹75 lakh in special-category states). Rates: 1% for manufacturers and traders (on taxable turnover for traders), 5% for restaurants. Service composition (N.2/2019-CT(R)) up to ₹50 lakh at 6%. Composition dealers cannot make inter-state outward supplies, cannot collect tax, issue a Bill of Supply with a mandatory legend, and cannot take ITC. Supplying goods through an ECO became allowed from 1 Oct 2023. | s.10; R.3–7 | 2017; ECO from 1 Oct 2023 | [K] |

### 2.2 Rates and rate history

| Rule | Legal ref | Effective | Tag |
|---|---|---|---|
| **GST 2.0 slabs:** 0, 5, 18 and 40% (CGST 2.5/9/20 + SGST of the same amount, or the full IGST). Special rates remain: 0.25% for rough diamonds, 3% for gold and jewellery, 1.5%, 0.125%, 0.75%, and 6% CGST on some construction items under N.14/2025. The goods schedule N.1/2017-CT(R) was superseded. Corrigenda were issued, so encode the corrected text. | N.9/2025-CT(R) (goods), N.10/2025 (exemptions), N.15/2025 (services), all dated 17 Sep 2025 | **22 Sep 2025** | [V] |
| States issued mirror SGST notifications. IGST and UTGST have parallel notifications (9/2025-IT(R) and 9/2025-UTT(R)). | — | 22 Sep 2025 | [V] |
| **Compensation cess:** ended for most goods when GST 2.0 began. For tobacco it was reduced to nil from 1 Feb 2026 by N.03/2025-Compensation Cess (Rate), dated 31 Dec 2025. Tobacco products moved to 40% GST plus an additional central excise duty. Pan masala moved to 40% plus the "Health Security-cum-National Security Cess" (levied outside GST under a 2025 Act). Bidis are at 18%. | N.03/2025-CC(R) | **1 Feb 2026** | [V] dates; [S] rates |
| The 22 Sep 2025 end of cess for non-tobacco goods comes from secondary sources. | — | — | (unconfirmed) |
| Restaurant service outside "specified premises": 5%, no ITC. Restaurant in a hotel where any room was priced above ₹7,500 per night in the previous FY: 18% with ITC. Hotel accommodation up to ₹7,500 per day: 5% (was 12%). The hotelier can opt to declare its premises specified. A standalone restaurant cannot. | N.11/2017-CT(R) entries 7(ii), 7(vi), as amended | 22 Sep 2025 (accommodation); 1 Apr 2025 (specified-premises definition) | [S] |
| Merchant exporter concessional rate 0.1%. | N.40/2017-CT(R) | 23 Oct 2017 | [K] |
| **Time of supply decides the rate when the rate changes:** s.14 CGST, using the dates of invoice, payment and supply, two out of three. **The engine must use s.14, not the voucher date, around 22 Sep 2025 and any later rate change.** | s.12, 13, 14 | 2017 | [K] |

**Engine design consequences:**
- Store rates as `(HSN/SAC or rule key, effective_from, effective_to, rate components, cess components, conditions such as an MRP slab, a price threshold, or ITC/no-ITC)`.
- Some rates depend on value, and some on the place (specified premises).
- If no rate is found, return `Unsupported`, never 0%.

### 2.3 HSN/SAC requirements

| Rule | Legal ref | Effective | Tag |
|---|---|---|---|
| Minimum HSN digits on invoices and in GSTR-1 Table 12, based on prior-year aggregate annual turnover (AATO). Up to ₹5 cr: 4 digits, mandatory for B2B and optional for B2C. Above ₹5 cr: 6 digits on all invoices. Some chemicals need 8 digits in notified cases (unconfirmed). | N.78/2020-CT (15 Oct 2020) | 1 Apr 2021 | [V] |
| **Table 12 Phase III:** codes are picked from a dropdown, with no free text. Table 12 has separate **B2B and B2C tabs**. The B2B tab cannot be empty when B2B supplies appear in other tables. The portal validates Table 12 against Tables 4A, 5A, 6A, 7, 8, 9, 10 and 15. | GSTN advisories of 22 Jan, 11 Apr and 1 May 2025 | Return period May 2025 (announced earlier, then deferred) | [V] date conflict noted; May 2025 is the latest |
| Table 13 (documents issued) became mandatory from the May 2025 return period. | — | — | (unconfirmed) |
| Annual return GSTR-9 HSN tables (17/18) follow the same digit rule. | N.78/2020; GSTR-9 instructions | FY 2021-22 onward | [K] |

### 2.4 Place of supply and the tax head (IGST vs CGST+SGST/UTGST)

| Rule | Legal ref | Effective | Tag |
|---|---|---|---|
| **Inter-state** (IGST): the supplier's location and the place of supply are in different States/UTs. **Intra-state** (CGST + SGST, or + UTGST): both are in the same State/UT. | IGST s.7, s.8 | 2017 | [K] |
| **UTGST** applies in UTs without a legislature: Chandigarh, Ladakh, Lakshadweep, Andaman & Nicobar, and Dadra & Nagar Haveli and Daman & Diu. Delhi, Puducherry and J&K use SGST. The engine needs a State/UT master with a `levy = SGST/UTGST` flag and the 2-digit codes. Code 97 is "Other Territory"; code 96 for foreign is (unconfirmed). | UTGST Act | 2017; DNH-DD merged 26 Jan 2020 | [K] |
| Supply **to or by an SEZ unit or developer** is always **inter-state** (IGST), even inside the same state. | IGST s.7(5)(b) | 2017 | [K] |
| Supply in the course of import or export is inter-state. | IGST s.7(2), (5)(a) | 2017 | [K] |
| **Goods, domestic (s.10):** movement involved means the place where movement ends. Bill-to-ship-to means the third party who directs the delivery (the "bill-to" principal). No movement means the location of goods at delivery. Assembly or installation means the place of installation. On board a conveyance means the place where the goods were loaded. | IGST s.10(1)(a)–(e) | 2017 | [K] |
| Goods sold to unregistered persons: a later amendment points to the recipient address on the invoice (s.10(1)(ca)). | — | — | (unconfirmed) |
| **Services, domestic (s.12):** default is the recipient's location if registered, otherwise the recipient's address on record, otherwise the supplier's location. Special rules cover immovable property (property location), restaurant, training, events, transport of goods and passengers, telecom, banking, insurance and advertising. | IGST s.12(2)–(14) | 2017 | [K] |
| **Services involving a party outside India (s.13):** default is the recipient's location. Exceptions cover services on goods physically present, immovable property, events, and others. | IGST s.13 | 2017 | [K] |
| **Intermediary services:** s.13(8)(b), which placed supply at the supplier's location, was **omitted** by s.157 of the Finance Act 2026. The default s.13(2) recipient-location rule now applies, so intermediary services to overseas clients can qualify as exports. | Finance Act 2026 s.157 | **30 Mar 2026** (date of assent) | [S] |
| Ship-to or bill-to state, and state data for unregistered B2C, must be captured. Rule 46 requires the name, address and state of an unregistered recipient when invoice value ≥ ₹50,000, or on request. | R.46(e), (f) | 2017 / amended | [K] |

**Determination algorithm (must be explicit, never defaulted):**
- Inputs: supplier GSTIN state, counterparty registration type, recipient GSTIN state, ship-to state, supply kind (goods or service, plus the IGST s.12/13 category), SEZ flag, export flag, voucher-level override.
- Output: one of `Intra(SGST|UTGST)`, `Inter`, `ZeroRatedExport{withPayment|LUT}`, `ZeroRatedSEZ{withPayment|LUT}`, `DeemedExport`, or `Unsupported{reason}`.
- Tally lets a voucher carry a place-of-supply field that the user can override. We should do the same and log every override.

### 2.5 Reverse charge (RCM)

| Rule | Legal ref | Effective | Tag |
|---|---|---|---|
| **Notified categories (s.9(3)):** services under N.13/2017-CT(R), including GTA, legal services, sponsorship, director services, security services by non-body-corporate, and renting of motor vehicles in some cases. Goods under N.4/2017-CT(R), including cashew, tendu leaves, and others. **Renting of commercial immovable property by an unregistered landlord to a registered tenant** is included from 10 Oct 2024 (N.9/2024-CT(R), with the 22 Oct 2024 corrigendum "any property" → "any immovable property"). Composition taxpayers are later excluded as recipients. | s.9(3); N.13/2017 & 4/2017 CT(R), as amended | 2017; rent 10 Oct 2024 | [V] rent / [K] rest |
| The exact date the composition exclusion took effect could not be confirmed (16 Jan 2025 was not confirmed). | — | — | (unconfirmed) |
| Metal scrap supplied by unregistered to registered persons falls under RCM. | N.6/2024-CT(R) | 10 Oct 2024 | [K] |
| Purchases from unregistered persons in general: RCM under s.9(4) applies only to notified classes. For promoters, this means purchases from unregistered suppliers below the 80% threshold. | s.9(4); N.7/2019-CT(R) | 1 Feb 2019 | [K] |
| **The recipient issues a self-invoice** within 30 days of receipt, plus a payment voucher (R.52). RCM tax **must be paid in cash** (it cannot be paid from ITC). ITC is then available if eligible. | s.31(3)(f), (g); R.47A | R.47A from 1 Nov 2024 | [K] |
| **Time of supply under RCM:** for goods, the earliest of receipt, payment, or 30 days after the invoice. For services, the earlier of payment or 60 days after the invoice. | s.12(3), s.13(3) | 2017 | [K] |
| GSTR-3B Table 3.1(d) (inward RCM liability) is **not auto-populated** and stays editable. | GSTN advisories | — | [S] |

### 2.6 Exempt, nil-rated, non-GST, zero-rated

| Category | Meaning | Return placement | Tag |
|---|---|---|---|
| Nil-rated | A rate of 0% is notified for the item. | GSTR-1 Table 8; GSTR-3B 3.1(c) | [K] |
| Exempt | Exempted by notification (N.2/2017-CT(R) for goods, N.12/2017-CT(R) for services, now restated in GST 2.0 N.10/2025 and later). | GSTR-1 Table 8; 3B 3.1(c) | [K] |
| Non-GST | Outside GST: alcohol for human consumption, 5 petroleum products, electricity (treated as non-GST in practice). | GSTR-1 Table 8 (non-GST column); 3B 3.1(e) | [K] |
| Zero-rated | Exports and SEZ supplies (IGST s.16). | GSTR-1 6A/6B; 3B 3.1(b) | [K] |
| No supply | Schedule III items, for example high-seas sales and sales from a duty-free shop. | Reported in 3B 3.1(c)/(e) in some cases | [K] |

- Exempt and nil turnover drives the **Rule 42/43 reversal**, so the engine must classify every line.
- Exempt turnover for R.42 includes the value of Schedule III items 5 and 8 in some cases (land, completed buildings, securities), and **excludes** some items, such as interest on deposits for non-banks.

### 2.7 Exports and SEZ

| Rule | Legal ref | Effective | Tag |
|---|---|---|---|
| **Two options:** (a) supply under a **LUT/bond without paying IGST**, then claim a refund of accumulated ITC (R.89, R.96A); or (b) **pay IGST**, then claim a refund of that IGST. For goods, the shipping bill acts as the refund application (R.96, through ICEGATE). | IGST s.16(3); R.89, 96, 96A | 2017 | [K] |
| R.96(10), which restricted the "pay IGST" route for some importers, was omitted. | N.20/2024-CT | 8 Oct 2024 | [K] |
| **SEZ zero-rating** applies only to supplies for **authorised operations**. | IGST s.16(1)(b) as amended by Finance Act 2021 | 1 Oct 2022 | [K] |
| Under LUT, goods must be exported within **3 months** of the invoice (extendable) and payment for services received within **1 year**. Otherwise IGST plus interest is payable. | R.96A(1) | 2017 | [K] |
| The export invoice must carry the endorsement "supply meant for export / supply to SEZ unit or developer for authorised operations, under bond/LUT without payment of IGST" (or "on payment of IGST"). Foreign currency, port code, shipping bill number and date are captured, and the shipping bill details can be added later in an amendment. | R.46 | 2017 | [K] |
| **Deemed exports** (supplies to EOUs, advance-authorisation holders, and similar) are not zero-rated. Tax is paid, and either side can claim a refund. | s.147; N.48/2017-CT | 2017 | [K] |
| **From 1 Nov 2026:** provisional refund of 90% extended to inverted-duty claims (s.54(6)). The minimum refund of ₹1,000 no longer applies to goods exported on payment of tax (s.54(14)). | Finance Act 2026; N.03/2026-CT (26 Oct 2026) | **1 Nov 2026** | [S] |

### 2.8 Advances received and paid

| Rule | Legal ref | Effective | Tag |
|---|---|---|---|
| **Advances for goods:** no GST at the time of the advance. Time of supply for goods is the invoice date for all registered persons. | N.66/2017-CT (all except composition); N.40/2017-CT (composition) | 15 Nov 2017 / 13 Oct 2017 | [K] |
| **Advances for services:** GST is payable when the advance is received. The supplier issues a receipt voucher (R.50). The advance is reported in GSTR-1 **Table 11A** and adjusted against the invoice in a later period in **Table 11B**. If the supply is cancelled, a refund voucher (R.51) reverses the tax. | s.13(2); R.50, 51 | 2017 | [K] |
| If the place of supply or rate is unknown when the advance arrives: use 18% on the advance, and treat it as inter-state if the place of supply cannot be determined. | s.13(2) proviso / s.31 | 2017 | [K] |
| **Advance paid under RCM:** the recipient's liability arises on payment, so a self-invoice or payment voucher is needed (R.52). | s.12(3)/13(3) | 2017 | [K] |

### 2.9 ITC eligibility, blocked credits and reversals

| Rule | Legal ref | Effective | Tag |
|---|---|---|---|
| **Conditions (s.16(2)):** (a) holds a tax invoice or debit note; **(aa)** the invoice appears in **GSTR-2B** (from 1 Jan 2022); (b) goods or services received; **(ba)** not restricted in the GSTR-2B ITC statement (s.38) (from 1 Oct 2022); (c) the supplier actually paid the tax; (d) the recipient filed its return. Goods received in lots: ITC on the last lot. | s.16(2) | 2017; (aa) 1 Jan 2022; (ba) 1 Oct 2022 | [K] |
| R.36(4): **ITC cannot exceed what appears in GSTR-2B**. The 5% tolerance was removed. | R.36(4) | 1 Jan 2022 | [K] |
| **Time limit (s.16(4)):** ITC must be claimed by the earlier of **30 Nov of the following FY** or the annual return. s.16(5) gives special relief for FY 2017-18 to 2020-21 (returns filed by 30 Nov 2021). s.16(6) covers registrations restored after cancellation. | s.16(4)–(6); Finance (No.2) Act 2024 | 30 Nov limit since Oct 2022; (5)/(6) retrospective from 1 Jul 2017 | [K] |
| The debit-note ITC year is the debit note's FY, not the original invoice's FY. | s.16(4) amended | 1 Jan 2021 | [K] |
| **180-day rule:** if the supplier is not paid (value plus tax) within 180 days of the invoice, the ITC is reversed in proportion to the unpaid amount, with interest. The ITC is re-availed on payment. Deemed values such as related-party supplies under Sch. I are excluded. | s.16(2) 2nd–3rd proviso; R.37 (amended by N.19/2022-CT) | 2017; proportionate form from 1 Oct 2022 | [K] |
| **Interest on ITC:** 18% only on ITC that was **wrongly availed and utilised**. ITC is treated as utilised when the electronic credit ledger balance falls below the wrongly availed amount. | s.50(3) substituted retrospectively; R.88B | 1 Jul 2017 (retrospective) | [K] |
| **R.37A:** if the supplier has not filed GSTR-3B for the period by 30 Sep of the following FY, the recipient reverses the ITC by 30 Nov. It is re-availed once the supplier files. | R.37A | 26 Dec 2022 | [K] |
| The portal keeps an **Electronic Credit Reversal and Re-claimed Statement** (RCL) that tracks temporary reversals. The opening balance had to be declared. | GSTN advisory | Aug 2023 period onward | [K] |
| **Blocked credits (s.17(5)) — the engine must flag these.** Detailed list below this table. | s.17(5) | see the list | [K]/[S] |
| **Rule 42 (inputs and input services used partly for exempt or non-business purposes):** each month, take total ITC T and remove T1 (used only for non-business), T2 (only for exempt) and T3 (blocked) to get C1. T4 (used only for taxable supplies) goes straight to C2. The common credit C2 is split. D1 = C2 × (exempt turnover E ÷ total turnover F) is reversed. D2 = 5% of C2 for non-business use is reversed. Recompute for the full year and true up by the **September return of the following FY** (or the annual return), with interest if under-reversed. | R.42 | 2017 | [K] |
| **Rule 43 (capital goods):** monthly common credit Tc ÷ 60 months = Tm. The exempt share of Tm, based on E/F, is reversed each month. Capital goods used exclusively for exempt or non-business purposes give no ITC. | R.43 | 2017 (amended 2020) | [K] |
| **R.86B:** if taxable value exceeds ₹50 lakh in a month, at least 1% of output tax must be paid in cash (with exceptions). | R.86B | 1 Jan 2021 | [K] |
| **R.88C / R.88D:** the portal sends an intimation when liability in GSTR-1 exceeds GSTR-3B (DRC-01B), and when ITC in 3B exceeds 2B (DRC-01C). The taxpayer must respond or pay. | R.88C, 88D | 2023 | [K] |
| **Order of ITC use:** IGST credit is used first against IGST, then CGST and SGST in any order. CGST credit cannot pay SGST, and the reverse. IGST credit must be fully used before CGST/SGST credit. | s.49(5), s.49A, R.88A | 2019 | [K] |

**Blocked credits under s.17(5).** Amendment dates are [K] unless tagged otherwise.
- (a) Motor vehicles with 13 seats or fewer, unless used for further supply, passenger transport, driving training, or (from 1 Feb 2019) goods transport.
- (aa) and (ab): vessels and aircraft, and insurance, repair and maintenance of them.
- (b) Food and beverages, outdoor catering, beauty treatment, health, cosmetic surgery, leasing or renting of vehicles, life and health insurance, club membership, health and fitness, and employee travel benefits. Exceptions: the same category supplied onward as an outward supply, or provision made mandatory by law.
- (c) Works contract for immovable property, except plant and machinery.
- (d) Construction on own account. Under the Finance Act 2025 amendment, "plant or machinery" was read as "plant and machinery", applied retrospectively from 1 Jul 2017 to displace the Supreme Court's *Safari Retreats* functionality test. Reported commencement is 1 Oct 2025 by N.16/2025-CT [S] (unconfirmed).
- (e) Tax paid under composition.
- (f) Supplies to a non-resident taxable person, except imports.
- (fa) CSR expenditure, from 1 Oct 2023.
- (g) Personal consumption.
- (h) Goods lost, stolen, destroyed, written off, or given as gifts or free samples.
- (i) Tax paid under s.74, s.129 and s.130. s.74 covers periods up to FY 2023-24. s.74A covers FY 2024-25 onward.

### 2.10 Credit notes, debit notes, amendments, IMS

| Rule | Legal ref | Effective | Tag |
|---|---|---|---|
| **Credit note grounds (s.34(1)):** taxable value or tax charged too high, goods returned, deficient supply. **From 1 Nov 2026**, post-supply discounts under s.15(3)(b) are also a ground. | s.34(1); Finance Act 2026 | 2017; discount from 1 Nov 2026 | [S] |
| **Credit note deadline:** it must be declared by the **earlier of 30 Nov of the following FY** or the annual return. Liability can be reduced only if the recipient has not claimed the ITC, or has reversed it. | s.34(2) | 30 Nov limit since Oct 2022 | [K] |
| **Post-supply discount (s.15(3)(b)) from 1 Nov 2026:** the discount no longer needs to be agreed before the supply or linked to specific invoices. It is excluded from value if the supplier issues a credit note and **the recipient reverses the related ITC**. The old rule (pre-agreed and linkable) applies **before** 1 Nov 2026. The engine must pick the rule by the credit note date and, for some cases, by the supply date (unconfirmed which date decides). | Finance Act 2026 ss.153–155; N.03/2026-CT dated 26 Oct 2026 | **1 Nov 2026** | [S] |
| Debit notes have no time limit for issue. The ITC deadline follows the debit note's own FY. | s.34(3)–(4), s.16(4) | 2021 | [K] |
| **Amending outward documents:** in a later GSTR-1, through Table 9A (B2BA, B2CLA, EXPA), 9C (CDNRA) and 10 (B2CSA). Since July 2024, **GSTR-1A** allows corrections in the same period, after GSTR-1 is filed and before GSTR-3B. | R.59; N.12/2024-CT | GSTR-1A from Jul 2024 period | [K] |
| **IMS:** the recipient Accepts, Rejects or keeps Pending each inward record (B2B invoice, debit note, credit note, ECO documents and amendments). Records not acted on are **deemed accepted** when GSTR-2B is generated (around the 14th). Actions can be changed and GSTR-2B recomputed until the GSTR-3B for that period is filed. After that the actions are final. | s.38 / R.60 as amended in 2025 (citation unconfirmed); GSTN advisories | Live from the Oct 2024 period; legal backing in 2025 (unconfirmed) | [S] |
| **IMS changes from the Oct 2025 tax period:** credit notes, upward CN amendments, and some downward amendments of invoices or debit notes already accepted can be kept **Pending for one tax period** (one month, or one quarter for QRMP). The recipient can declare the **ITC actually availed or reversed** on a credit note, which partly controls the reversal. Optional **remarks** on Reject and Pending are shown to the supplier. These apply only to records filed after the rollout. | GSTN advisory, 23 Sep 2025 (secondary) | Oct 2025 period | [S] |
| If the recipient rejects a credit note, the supplier's liability goes up again, through GSTR-1A or the next GSTR-1 / 3B. | GSTN | Oct 2024 onward | [S] |
| Invoice series: up to **16 characters**, unique per FY, using letters, digits, `-` and `/`. | R.46(b) | 2017 | [K] |

### 2.11 Returns: what, when, which tables

| Return | Who / frequency | Due date | Key rules | Tag |
|---|---|---|---|---|
| **GSTR-1** | Regular taxpayer. Monthly, or quarterly under **QRMP** (AATO ≤ ₹5 cr). | 11th of next month; QRMP 13th after quarter end | Tables 4 (B2B), 5 (B2CL: inter-state B2C above **₹1 lakh** from 1 Aug 2024, previously ₹2.5 lakh, N.12/2024), 6 (exports/SEZ/deemed), 7 (B2CS), 8 (nil/exempt/non-GST), 9 (amendments, CDNR, CDNUR), 10, 11 (advances), 12 (HSN), 13 (documents), 14/15 (ECO, from Jan 2022) | [V] B2CL / [K] rest |
| **IFF** | QRMP, months 1 and 2 of the quarter | 1st–13th of next month | B2B only, optional, capped at ₹50 lakh per month | [K] |
| **GSTR-1A** | Optional amendment, same period | After GSTR-1, before 3B | Changes flow into 3B. **Tally does not support it** [Vendor]. | [K]/[Vendor] |
| **GSTR-3B** | Regular. Monthly, or quarterly for QRMP (with PMT-06 monthly payment by the 25th) | 20th; QRMP 22nd/24th depending on the state group | Table 3.1/3.1.1 outward liability **auto-populated and locked** from the July 2025 period (corrections only through GSTR-1A). Table 3.2 (inter-state supplies to unregistered, composition and UIN) locked from the **Nov 2025 period**. Table 5.1 interest locked from the **Jan 2026 period**. A "Save" on the tax-liability breakup is needed from the Feb 2026 period. Table 4 ITC auto-populated from GSTR-2B and IMS (editable). A **Table 4 lock is expected around July 2026 but not confirmed by any advisory found**. | [S] (unconfirmed for Table 4) |
| **GSTR-2B** | Auto-drafted ITC statement (static per period) | Generated around the 14th | Recomputed after IMS actions | [S] |
| **GSTR-9** | Annual, regular taxpayers | 31 Dec of the following FY | **Optional/exempt for AATO ≤ ₹2 cr**, permanent from FY 2024-25 (N.15/2025-CT, 17 Sep 2025). FY 2024-25 changes: Table 6A1 (prior-year ITC claimed this year), Table 8A based on GSTR-2B, GSTR-1A amendments in Tables 4/5, Tables 12/13 mandatory for ITC moving across years. | [S] |
| **GSTR-9C** | AATO > ₹5 cr | With GSTR-9 | Self-certified reconciliation statement (CA certification removed from FY 2020-21) | [S]/[K] |
| **CMP-08** | Composition, quarterly statement-cum-challan | 18th after quarter end | Tax on turnover, plus RCM liability | [K] |
| **GSTR-4** | Composition, annual | **30 Apr** of the following FY (from FY 2024-25; previously 30 Jun) | Outward totals, inward from registered and unregistered, RCM, TDS/TCS credit | [K] |
| **GSTR-8** | ECO, TCS under s.52 | 10th | **TCS rate 0.5%** (0.25 + 0.25, or 0.5 IGST) from **10 Jul 2024** (N.15/2024-CT), previously 1% | [K] |
| **Three-year bar** | All of 1, 1A, 3B, 4, 5, 5A, 6, 7, 8, 9/9C | Return cannot be filed more than 3 years after its due date | Finance Act 2023 / N.28/2023-CT. Portal enforcement started in the 2025 tax periods (dates moved between Jul, Sep and Oct 2025). | [S] |

### 2.12 Section 9(5) and e-commerce operators

| Rule | Legal ref | Effective | Tag |
|---|---|---|---|
| The ECO is liable as if it were the supplier for: passenger transport (some modes), accommodation and housekeeping, restaurant services (from 1 Jan 2022), and **local delivery services** (from 22 Sep 2025, where the underlying provider is not liable to register under s.22(1)). | s.9(5); N.17/2017-CT(R) as amended by **N.17/2025-CT(R)** | 22 Sep 2025 | [V] |
| The local-delivery rate (18%) comes from secondary sources. | — | — | (unconfirmed) |
| **From 1 Nov 2026:** the scope widened to "delivery services" other than postal and courier, at **5% without ITC** when supplied through an ECO under s.9(5). Recommended by the 57th GST Council (8 Oct 2026). The notifications are reported as 06, 07 and 09/2026-CT(R), dated 27 Oct 2026. | — | 1 Nov 2026 | (unconfirmed, secondary only) |
| Suppliers selling through an ECO report those sales in GSTR-1 Table 14. The ECO reports in Table 15. | GSTR-1 format | Jan 2022 | [K] |

### 2.13 E-invoice (IRN / QR / IRP)

| Rule | Legal ref | Effective | Tag |
|---|---|---|---|
| **Mandatory** if AATO exceeded **₹5 cr in any FY from 2017-18**. Covers B2B invoices, credit and debit notes, exports, SEZ supplies, deemed exports, and B2B under RCM. **B2C supplies are excluded.** Exempt entities: SEZ **units** (not developers), insurers, banks, financial institutions and NBFCs, GTAs, passenger transport, multiplex admission, government departments and local authorities. | R.48(4); N.13/2020-CT as amended by **N.10/2023-CT** | ₹5 cr from **1 Aug 2023** | [K] |
| **30-day reporting window:** taxpayers with **AATO ≥ ₹10 cr** cannot report an invoice, CN or DN to the IRP more than 30 days after the document date. This was earlier applied to AATO ≥ ₹100 cr, from 1 May 2023. | GSTN advisory, 5 Nov 2024 | **1 Apr 2025** | [V] |
| The IRP validates the JSON (schema INV-01, version 1.1 in the notified format) and returns an **IRN** (a 64-character hash of supplier GSTIN, FY, document type and document number), an ack number and date, and a **signed QR**. Duplicate IRNs are rejected. **Cancellation** is allowed only within **24 hours**, whole invoice only. After that, use a credit note or a GSTR-1 amendment. IRN data auto-populates GSTR-1 and can generate the e-way bill (Part A). | R.48(4)–(6); N.60/2020-CT schema | Oct 2020 onward | [K] |
| Multiple IRPs exist (NIC-run and private). The private ones are vendor-run, and the exact list is not given here. | GSTN | 2023 onward | [K] |
| 2FA for e-invoice and e-way bill login is in place for larger AATO bands. The applicability thresholds and dates conflict across sources. | NIC | 2023 | (unconfirmed) |
| **B2C dynamic QR** (UPI-payable QR on B2C invoices) is required for AATO > ₹500 cr. | N.14/2020-CT | 1 Dec 2020 (penalty waivers later) | [K] |
| **Rounding and tolerance on e-invoice:** item and document totals can differ from the IRP's own computation only within a small tolerance. Invoice-level **round-off** goes in a separate field. Older validation documents (v1.03) allowed a computed value anywhere from the exact value up to the next rupee, and a round-off of up to ₹9.99. The current limits are (unconfirmed). | e-invoice validation docs | — | [S] |

### 2.14 E-way bill

| Rule | Legal ref | Effective | Tag |
|---|---|---|---|
| Required for movement of goods with consignment value **> ₹50,000**, inter-state or intra-state (states set their own intra-state limits). Part A holds invoice and parties. Part B holds the vehicle. Validity is **1 day per 200 km** (20 km for ODC cargo). Exemptions are in R.138(14) and Annexure. | R.138, 138A–138E | 1 Apr 2018; 200 km/day from 1 Jan 2021 | [K] |
| A **base document older than 180 days** cannot be used to generate an EWB. **Extension cap: 360 days** from original generation. | GSTN/NIC advisory | **1 Jan 2025** | [V] |
| EWB generation is **blocked** if GSTR-3B or CMP-08 is not filed for 2 or more consecutive periods. | R.138E | 2021 | [K] |
| Kerala: gold and precious stones intra-state need an EWB without Part B (Tally supports this from 7.1). | Kerala SGST rules | 2025 (unconfirmed date) | [Vendor] |

### 2.15 Rounding

| Rule | Legal ref | Tag |
|---|---|---|
| Tax, interest, penalty, fine and other sums payable are **rounded to the nearest rupee**. Fifty paise or more rounds up. | CGST s.170 | [K] |
| The portal and the returns accept amounts with **2 decimals**. Invoice-level tax is usually calculated per line (or per rate group), and the invoice total is rounded to the rupee with an explicit "Round off" line. **The law does not fix whether tax is rounded per line or per invoice.** Both are used. | Practice; e-invoice schema has a `RndOffAmt` field | [K] |

- The engine must store unrounded line tax, the chosen rounding policy, and the round-off amount, and must reproduce the IRP's arithmetic exactly.
- Return totals are sums of document-level values. They are not recomputed from rates.

---

## 3. How Tally exposes each area

All of this section is **[Vendor]**: TallyPrime help, release notes and Tally blogs.

### 3.1 Release timeline of Tally's GST features
- **2.0:** connected e-way bill (generate, cancel, track from a voucher). Online GSTIN and HSN validation.
- **3.0:** **multiple GST registrations in one company**, with separate returns and voucher series per GSTIN. GSTR-1, 2A and 2B reconciliation from imported portal JSON.
- **3.0.1:** e-invoice error correction, GSTR-3B JSON export, blank HSN allowed when turnover is ≤ ₹5 cr.
- **5.0:** **Connected GST**, which uploads and downloads GSTR-1 and 3B directly through Tally's own GSP. Multi-registration and multi-period upload. Real-time party validation. B2CL ₹1 lakh threshold.
- **6.0:** HSN summary with a B2B/B2C split, effective 1 Apr 2025.
- **6.1:** **IMS**: download, act, upload and reset. GSTR-2B recompute. Offline GSTR-1 upload through Excel/CSV.
- **6.2:** "Tax Invoice Format 2" (the government-recommended layout) as the default.
- **7.0** (19 Dec 2025):
  - IMS ITC reduction for B2BA, CDNRA and B2BDNA, with declared ITC set per invoice and synced with the portal.
  - IMS remarks.
  - Single-step cancellation of voucher, e-invoice and EWB together.
  - GSTR-3B export as one JSON or Excel file per period.
  - Duplicate-voucher detection in Uncertain Transactions.
- **7.1** (20 May 2026):
  - Fetch party name and address from a GSTIN, even when no ledger exists.
  - **IMS Books View**: reconciled, mismatched, only-in-books, needs-action.
  - GST login session of up to 30 days.
  - **Tobacco sales on a tax-inclusive MRP.**
  - HSN/SAC validation in masters and in GSTR-1 / HSN summary.
  - Excluding an individual voucher from EWB.
  - Bulk "GST Return Effective Date".
  - Kerala gold EWB without Part B.
- **7.2** (2026; exact date not captured): GSTR-9 export into the portal's **offline-tool Excel** (HSN summary, or all sections of the "Annual Computation"). **No online GSTR-9 filing.** Extended e-invoice sessions.

### 3.2 Masters and setup
- Turning on GST:
  - Company Features → GST → **Tax Registration Details** holds the state, registration type, GSTIN, applicable-from date, periodicity and e-invoice/EWB applicability.
  - From 3.0 there can be several registrations. Each voucher type or series is tied to one GSTIN.
- **Rate hierarchy** (where Tally looks for rates):
  - Ledger → Ledger Group → Stock Item → Stock Group → Company by default. Users can make the stock item win instead.
  - **GST Classification** masters act as reusable rate-plus-HSN bundles.
  - The company-level rate needs "Set/Alter Company GST Rate".
  - Rates and HSN are stored **by "Applicable From" date**, with a history view.
  - The **GST Rate Setup** report (Alt+G) edits rates in bulk with a revised applicability date. This is how users moved to the GST 2.0 rates.
  - Help docs warn that a ledger-first setting can quietly ignore a newer stock-item rate. That is a "wrong number" trap.
- **Party ledgers:** registration type (Regular, Composition, Consumer, Unregistered, Unknown), GSTIN (online validation), state, place of supply, "is e-commerce operator", "deemed exporter", "transporter". SEZ is a separate party type.
- **Tax ledgers:** Duties & Taxes with type GST and head IGST, CGST, SGST/UTGST or Cess. A rounding method can be set on the ledger.
- **Nature of transaction and taxability** on each voucher line: Taxable, Exempt, Nil Rated, Non-GST, plus reverse charge and ITC eligibility (ineligible under 17(5), and others).

### 3.3 Vouchers
- **Sales and purchase vouchers** compute tax lines automatically. Inter-state vs intra-state comes from the company state against the party's state and place of supply.
- **Overrides:**
  - Tax amounts can be edited by hand. A mismatch is then reported later as an uncertain transaction ("tax amount mismatch").
  - **Journal voucher** with nature "GST adjustments" or "Stat adjustment" (in older releases) for ITC reversal (17(5), R.42/43, 180 days), RCM liability, refund of advances, and TDS/TCS credit.
- **Receipt and payment vouchers** carry advance details for services, linked later to the invoice.
- **RCM:** the purchase is marked RCM, and a journal raises the liability. A self-invoice print is available.
- **Credit note and debit note voucher types:** original invoice reference, reason, and pre-GST flag.
- **E-invoice and EWB:**
  - When the voucher is saved, Tally offers to generate the IRN and EWB online, or to export JSON for manual upload to the IRP or EWB portal.
  - A failed generation goes into the e-invoice report's "pending / rejected" buckets.
  - The QR can only be fetched within 72 hours of generation.

### 3.4 Return reports and the exception UI (the key idea)
- Reports: GSTR-1, GSTR-3B, GSTR-2A, GSTR-2B, IMS, CMP-08, GSTR-4, and the annual computation for GSTR-9.
- **GSTR-1 report layout:**
  - Header: registration (F3), period, signed or not signed.
  - **Status summary:** Ready for Export, Ready for Upload, Not Uploaded, Modified in Books after export or upload, **Mismatch with GST Data** (books against imported portal data), **Uncertain Transactions (Corrections needed)**, Marked for Deletion on Portal, No Action Required, Not Relevant for this Return, Moved to other return periods, Amendments marked for future returns, Additional pending actions (HSN summary, document summary, advance adjustments, RCM).
  - **Return View:** the portal's table order (4A/4B/4C/6B/6C, 5, 6A, 7, 8, 9B, 11A/11B, 12, 13).
  - **Nature View (F5):** Local, Interstate, Exports and SEZ, split by Taxable, Exempt, Nil and Non-GST.
- **Uncertain-transaction reasons and fixes:**
  - Applicable tax ledger not selected.
  - Tax amount does not match the computed value.
  - HSN/SAC invalid or missing.
  - Masters changed after the voucher was entered. Fix in bulk with **Alt+W "Update as per Masters"**.
  - Voucher modified after filing or signing.
  - Duplicates (7.0).
- **Signing:** F10 "signs" the return in books and Alt+F10 unsigns it. Signing locks the period. Later edits come back as uncertain transactions.
- **Effective Date:** each voucher has a "GST return effective date", so it can be moved into a different return period without changing the accounting date. Tally uses it for late-reported invoices.
- After filing, users can continue with the mismatch and fix it next period, or fix it in books or on the portal and refile.

### 3.5 GSTR-3B, 2B, IMS
- **GSTR-3B report:**
  - Computes from books.
  - "Comparison of Books and Portal Values" after downloading the portal's auto-drafted 3B.
  - If books and portal differ, Tally pauses the upload.
  - Users still file and pay on the portal.
- **GSTR-2B reconciliation:**
  - Import the JSON (Alt+O) or download it (Alt+Z).
  - Status buckets: Reconciled, Unreconciled, Mismatched, Available only in Books, Available only on Portal, Mismatch in return period.
  - The user resolves value, period and document-number mismatches. "Recompute GSTR-2B" runs from Alt+Z.
- **IMS report:**
  - Download inward records, mark Accept, Reject or Pending against books, add remarks, upload actions, reset.
  - 7.0 adds declared ITC reduction for amendments and credit notes.
  - 7.1 adds Books View statuses.

### 3.6 Composition
- The composition registration type in Tax Registration Details changes the voucher types to Bill of Supply.
- Reports: **CMP-08** (quarterly) and **GSTR-4** (annual) reports with JSON export (based on help pages from earlier releases; online upload support is unconfirmed).

### 3.7 Connected GST login
- Menu paths: Alt+Z → All GST Options → Upload / Download GST Returns.
- Login uses the GST portal username plus an OTP, and the API access setting must be enabled on the portal.
- **GSP/ASP:** Tally (India) Pvt Ltd is the GSP and Tally Solutions Pvt Ltd is the ASP.
- **GSTN now requires each GSTIN to explicitly authorise its GSP–ASP pair on the portal**, renewed every 30 days. Tally gave a deadline of 30 Sep 2026; the current status is unconfirmed.

---

## 4. Return formats and portal JSON

| Artifact | Format | Public documentation? | Notes |
|---|---|---|---|
| GSTR-1 / IFF upload | JSON produced by the GST **Returns Offline Tool**, or Excel templates converted by the tool | **The JSON schema is not formally published to taxpayers.** It is effectively defined by the offline tool and by the **GSP APIs** (developer.gst.gov.in, which needs GSP empanelment). Vendors reverse-engineer the offline-tool JSON. | Table keys follow return sections: b2b, b2cl, b2cs, cdnr, cdnur, exp, at, txpd, nil, hsn, doc_issue, ecom, plus amendment sections. **Clean-room note:** we must build from the official offline-tool Excel templates and GSTN's public API documentation, not from another vendor's files. |
| GSTR-1 Excel | Official offline-tool Excel workbook, one sheet per section | Yes (downloadable template) | Safest fully documented interchange. Tally 6.1 supports offline GSTR-1 upload through Excel/CSV. |
| GSTR-3B | Portal-generated JSON; the offline utility is limited | Partial | Tally 7.0 exports a single JSON or Excel file per period. Because Table 3 (and possibly Table 4) is locked, upload is mostly about comparison. |
| GSTR-2A / 2B / IMS download | JSON from the portal (download links) or the API | Format visible on download; no formal schema doc | Reconciliation input. |
| GSTR-9 | Offline-tool Excel, then JSON | Excel template public | Tally 7.2 fills the Excel. |
| GSTR-4 / CMP-08 | Offline tool, or online entry | Template public | — |
| **E-invoice** | **JSON schema INV-01 v1.1**, notified in the gazette (N.60/2020-CT and later); validation rules and API specs on the NIC e-invoice developer portal | **Yes, the best-documented format** | Bulk upload tool, API (through GSP or direct for large taxpayers), signed QR in JWT format. |
| **E-way bill** | JSON for bulk generation; documented API | Yes (NIC EWB portal developer docs) | — |

**API access route:**
- Direct GST-portal APIs for returns are only available through an **empanelled GSP**. The taxpayer authorises through the portal (OTP, plus the newer GSP/ASP selection).
- An open-source app needs either a GSP partnership or an offline JSON/Excel workflow.
- This is a **major product decision**; see Open questions.

---

## 5. Connected and online features (Tally vs our plan)

| Capability | Tally (vendor claim) | Our implication |
|---|---|---|
| GSTIN and party lookup | Yes (7.1 fetches the name and address without a ledger) | Uses the public search-taxpayer API through a GSP, or the portal's public search (rate-limited). |
| HSN validation | Yes (7.1) | We can ship the HSN/SAC master (public CBIC tariff) offline with an effective-dated rate map. |
| Upload GSTR-1 / 3B | Yes (5.0+). Filing (EVC/DSC) and payment are still done on the portal [Vendor]. | MVP: **offline Excel/JSON export plus guided portal steps**. Later: a GSP adapter as a plug-in. |
| GSTR-1A | **No** | An opportunity: support GSTR-1A, because 3B Table 3 is locked and corrections must go through GSTR-1A. |
| Download 2A / 2B / 3B / IMS | Yes | Import of downloaded JSON files works offline. |
| IMS actions | Yes, with ITC-reduction declaration and remarks (6.1 / 7.0) | Core feature. Suggest actions from the reconciliation result, but a user must confirm every action (never auto-accept). |
| E-invoice / EWB | Yes, from the voucher; bulk; cancellation | Through an IRP API (NIC offers direct API access to eligible taxpayers, and GSPs/private IRPs for others), plus an offline JSON export. |
| GSTR-9 | Excel export only (7.2) | Annual computation drawn from the same ledger. |

---

## 6. What users love and hate

User-voice evidence was **thin**. Searches returned mostly Tally's own pages, and Reddit or forum threads
did not come up before the search budget ran out. Treat this as a hypothesis list.

**Love** (vendor-reported, plus common practitioner commentary):
- One place for books, returns and e-invoice/EWB from the voucher.
- The exception buckets ("Uncertain Transactions") make it clear what blocks a clean return.
- Rate history by "Applicable From" made the GST 2.0 changeover manageable.
- Keyboard-driven bulk fixes (Update as per Masters).

**Hate** (from Tally community threads, competitor blogs, and Tally's own FAQ topics, which show common pain):
- E-invoice generation errors and "not exported after save" threads on the Tally community forum.
- QR fetch limited to 72 hours.
- Validation rules change with IRP updates, and invoices fail until users correct them by hand (Vyapar blog anecdote; competitor source).
- A ledger-first rate lookup can quietly ignore a stock-item rate change (Tally's own help warns about this).
- No GSTR-1A, and no online GSTR-9 filing [Vendor docs].
- Mismatch between books and portal blocks the upload, so users must understand portal state.
- The GSP/ASP re-authorisation every 30 days per GSTIN adds friction for CA firms with many clients.
- A competitor (Ginesys) says Tally's support does not handle GST-specific queries well. This is a competitor's view and biased.

---

## 7. Open questions

1. **GSTR-3B Table 4 lock:** is it in force for any 2026 tax period? Secondary sources say "July 2026, expected". Several say no GSTN advisory had been issued as of Sep 2026. Check the gst.gov.in advisory list.
2. **IMS legal citation:** the exact amendments to s.38 and Rule 60 that give IMS legal force, and the date they were notified. Also whether GSTR-3B can be filed when 2B is not regenerated after IMS changes.
3. **Delivery services at 5% from 1 Nov 2026** (57th Council; N.06, 07 and 09/2026-CT(R)): confirm on CBIC.
4. **Post-sale discount rule (1 Nov 2026):** which date decides the rule, the supply date or the credit-note date? How does the recipient's ITC reversal link to IMS's "ITC reversed" declaration?
5. **Tobacco after 1 Feb 2026:** is GST now charged on the **retail sale price** basis? Tally 7.1 added tax-inclusive MRP for tobacco. Find the valuation notification.
6. **GSP strategy:** become a GSP, partner with one, or stay offline with Excel/JSON? GSP empanelment has eligibility, net-worth and security-audit conditions. Also check whether NIC's direct e-invoice and EWB APIs are open to software vendors without being a GSP.
7. **GSTR-1 JSON schema:** is there an officially published schema we can implement clean-room? If not, build from the Excel templates and the GSTN public API specs.
8. **Current e-invoice tolerance and round-off limits** (v1.1 validations); 2FA applicability.
9. **Composition:** the exact date composition recipients were excluded from RCM on commercial rent.
10. **Tally composition (CMP-08/GSTR-4):** is online upload supported in 7.x, or is it JSON export only?
11. All **[K]** items need a citation pass (notification PDF links) before they become expected values in tests. Until then they are `pending` under our engineering rules.

---

## 8. Sources

Official and quasi-official (state copies of GSTN advisories, gazette copies):
- Maharashtra GST mirror of GSTN advisories:
  - [IMS initial phase](https://www.mahagst.gov.in/public/uploads/gstnadvisory/1760531435_297%20Advisory%20regarding%20IMS%20during%20initial%20phase%20of%20its%20implementation.pdf)
  - [Important advisory on IMS](https://www.mahagst.gov.in/public/uploads/gstnadvisory/1768890416_375%20Attention%20%20Important%20Advisory%20on%20IMS.pdf)
  - [Non-editable auto-populated liability in GSTR-3B](https://www.mahagst.gov.in/public/uploads/gstnadvisory/1760596521_352%20Advisory%20regarding%20non-editable%20of%20auto-populated%20liability%20in%20GSTR-3B.pdf)
  - [Hard-locking of GSTR-3B liability](https://www.mahagst.gov.in/public/uploads/gstnadvisory/1760597577_326%20Attention%20%E2%80%93%20Hard%20-%20Locking%20of%20auto-populated%20liability%20in%20GSTR-3B.pdf)
  - [Table 3.2 of GSTR-3B](https://www.mahagst.gov.in/public/uploads/gstnadvisory/1768891338_386%20Advisory%20on%20reporting%20values%20in%20Table%203.2%20of%20GSTR-3B.pdf)
  - [HSN in Table 12 and documents in Table 13](https://www.mahagst.gov.in/public/uploads/gstnadvisory/1760596735_345%20Reporting%20of%20HSN%20codes%20in%20Table%2012%20and%20list%20of%20documents%20in%20table%2013%20of%20GSTR%201%201A.pdf)
  - [Three-year bar](https://www.mahagst.gov.in/public/uploads/gstnadvisory/1760596541_353%20Barring%20of%20GST%20Return%20on%20expiry%20of%20three%20years.pdf)
  - [E-invoice 30-day limit for AATO ₹10 cr](https://www.mahagst.gov.in/public/uploads/gstnadvisory/1760531580_293%20Advisory%20Time%20Limit%20for%20Reporting%20e%20Invoice%20on%20the%20IRP%20Portal%20%20Lowering%20of%20Threshold%20to%20AATO%2010%20Crores%20and%20Above.pdf)
  - [E-way bill and e-invoice updates (180/360 days)](https://www.mahagst.gov.in/public/uploads/gstnadvisory/1760530021_309.Advisory%20on%20Updates%20to%20E-Way%20Bill%20and%20E-Invoice%20Systems.pdf)
  - [GSTR-9/9C FY 2024-25](https://www.mahagst.gov.in/public/uploads/gstnadvisory/1768890492_376%20Advisory%20for%20GSTR%209%20or%209C%20for%20FY%202024-25.pdf)
  - [N.17/2025 State Tax (Rate)](https://www.mahagst.gov.in/public/uploads/mvatservices/1760687928Notification%20No.%2017_2025-State%20Tax%20(Rate).pdf)
- [GST Council: CGST rate notifications](https://gstcouncil.gov.in/cgst-rate-notification)
- [GST Council: Circular 212/06/2024](https://gstcouncil.gov.in/sites/default/files/2024-09/circular-no-212-06-2024.pdf)
- [Karnataka copy of N.17/2025](https://gst.kar.nic.in/Documents/NOTIFICATIONS/STR1719925.pdf)
- [ICAI reproduction of INV-01 schema v1.1](https://vasai.icai.org/detail/38865.aspx)

Firm and professional commentary:
- [EY: CBIC GST 2.0 notifications](https://www.ey.com/en_in/technical/alerts-hub/2025/09/cbic-issues-notifications-giving-effect-to-the-recommendations-made)
- [EY: IMS changes](https://www.ey.com/en_in/technical/alerts-hub/2025/09/gstn-issues-advisory-on-the-changes-introduced-in-the-invoice-management-system)
- [LKS: Finance Act 2026 ss.153–155 effective 1 Nov 2026](https://www.lkslaw.com/insights/updates/indirect-tax-update/cgst-act-2017-amendments-by-sections-153-to-155-of-the-finance-act-2026-effective-from-1-november-2026)
- [LKS: discount and credit-note changes](https://www.lkslaw.com/insights/news/gst-discount-schemes-and-credit-notes-changes-by-budget-2026)
- [Grant Thornton: intermediary reset](https://www.grantthornton.in/insights/articles/gst-on-intermediary-services/)
- [CAclubindia: intermediary from 30 Mar 2026](https://www.caclubindia.com/articles/a-new-era-for-place-of-supply-of-intermediary-services-exports-vs-imports-of-services-wef-march-30-2026-55072.asp)
- [CAclubindia: N.9/2025 schedule](https://www.caclubindia.com/articles/gst-notification-no-92025-complete-hsnwise-rate-schedule-effective-from-september-22-2025-54119.asp)
- [Taxmann: IMS pending](https://www.taxmann.com/post/blog/gstn-allows-pending-records-in-ims-for-one-tax-period)
- [Taxmann: Table 3.2 lock](https://www.taxmann.com/post/blog/gstn-makes-gstr-3b-table-3-2-auto-populated-and-non-editable)
- [Taxmann: GSTR-3B interest/ITC from Jan 2026](https://www.taxmann.com/post/blog/gstn-enhances-gstr-3b-interest-and-itc-functionality/)
- [Taxmann: e-invoice 30-day limit](https://www.taxmann.com/post/blog/time-limit-of-30-days-for-reporting-e-invoice-on-irp-portal-for-taxpayers-with-aato-of-10-crores-and-above-gstn-update/)
- [Taxmann: RCM on commercial rent](https://www.taxmann.com/post/blog/govt-introduces-rcm-on-renting-of-commercial-property-by-unregistered-to-registered-person-notification)
- [Taxmann: GSTR-9 exemption](https://www.taxmann.com/post/blog/cbic-exempts-small-taxpayers-from-filing-annual-gst-return-gstr-9)
- [Taxscan: July 2026 ITC locking](https://www.taxscan.in/top-stories/big-gst-change-from-july-2026-gstr-3b-itc-locking-explained-1448389)
- [Taxscan: 57th Council delivery services 5%](https://www.taxscan.in/gst/gst-council-recommends-5-gst-without-itc-on-delivery-services-through-e-commerce-platforms)
- [Taxscan: Table 5.1 interest lock](https://www.taxscan.in/top-stories/gstn-revises-interest-computation-in-gstr-3b-from-jan-2026-table-51-of-gstr-3b-to-be-non-editable-1443233)
- [Computax: ITC locking status](https://www.computaxonline.com/blog/post/2026/07/09/gst-itc-auto-restriction-gstr-2b-gstr-3b-hard-locking)
- [Taxaj: Table 4 lock not confirmed](https://www.taxaj.com/learn/gstr-3b-table-4-itc-hard-locking-from-july-2026-no-manual-editing-surv/)
- [TheTaxCorp: ECO delivery from 1 Nov 2026](https://thetaxcorp.in/article/gst-on-eco-delivery-services-from-1-november-2026-liability-rate-and-compliance)
- [TheTaxCorp: s.17(5)(d) retrospective](https://thetaxcorp.in/article/retrospective-gst-change-after-safari-retreats-itc-block-on-construction)
- [Business Today: tobacco regime from 1 Feb 2026](https://www.businesstoday.in/personal-finance/tax/story/tobacco-tax-regime-from-february-1-excise-duty-health-cess-replace-gst-compensation-levy-513811-2026-01-31)
- [AIR News: notification of the 1 Feb date](https://www.newsonair.gov.in/govt-notifies-february-1st-as-date-from-which-additional-excise-duty-to-be-levied-on-tobacco-products)
- [ClearTax: restaurant/hotel GST](https://cleartax.in/s/impact-of-gst-hospitality-industry)
- [ClearTax: Table 12 HSN](https://cleartax.in/s/mandatory-hsn-code-reporting-gstr1-1a)
- [TaxGuru: e-invoice validations v1.03](https://taxguru.in/goods-and-service-tax/e-invoice-gst-validations-version-1-03.html)
- [StudyCafe: N.17/2025 local delivery](https://studycafe.in/gst-to-be-paid-by-e-com-operator-on-services-by-way-of-local-delivery-u-s-95-read-notification-393411.html)

Tally (vendor):
- [Release notes 7.0](https://help.tallysolutions.com/release-notes-tallyprime-7-0)
- [Release notes 7.1](https://help.tallysolutions.com/tally-prime/release-notes-tally-prime/release-notes-tallyprime-7-1/)
- [Release notes 7.2](https://help.tallysolutions.com/release-notes-tallyprime-7-2/)
- [Features by release](https://help.tallysolutions.com/tallyprime-features-release-wise/)
- [GSTR-1 report](https://help.tallysolutions.com/gstr-1-report-in-tallyprime/)
- [GSTR-1 FAQs](https://help.tallysolutions.com/gstr-1-faqs/)
- [GST errors and resolutions](https://help.tallysolutions.com/tally-prime/india-gst/gst-errors-and-resolutions/)
- [Applicable tax ledger not selected](https://help.tallysolutions.com/applicable-tax-ledger-is-not-selected/)
- [GST rate setup](https://help.tallysolutions.com/india-gst-rate-setup-tally/)
- [Updating rates after a rate change](https://tallysolutions.com/gst/gst-rate-change-how-to-update-gst-rates-in-tallyprime/)
- [Upload GSTR-3B](https://help.tallysolutions.com/upload-gstr-3b-from-tallyprime/)
- [GSTR-2B reconciliation](https://help.tallysolutions.com/india-gst-status-gstr-2b-reconciliation-tally-2/)
- [File GSTR-9](https://help.tallysolutions.com/file-gstr-9-tally/)
- [GSP selection](https://tallysolutions.com/gst/gsp-selection-tallyprime/)
- [E-invoice FAQ](https://help.tallysolutions.com/tally-prime/e-invoice-tallyprime/e-invoice-faq/)
- [Product update 7.0](https://tallysolutions.com/darpan/product-update-tallyprime-7-0/)
- [IMS impact on 2B/3B](https://tallysolutions.com/gst/gst-ims-and-gstr-2b-new-mechanism-to-correct-itc-mismatches-in-gstr-3b/)

Competitor and anecdotal:
- [Ginesys blog (competitor)](https://www.ginesys.in/blog/gst-software-that-works-with-tally-for-gst-returns-filing-and-e-invoicing)
- [Vyapar TaxOne blog (competitor)](https://taxone.vyapar.com/post/tallyprime-3-0-1-and-tallyprime-4-0)
