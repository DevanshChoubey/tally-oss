# 05 — TDS, TCS, payroll and other non-GST statutory features

Research date: 2026-11-20. Researcher area: TDS/TCS, payroll, MSME 43B(h)/37(2)(g), interest, legacy VAT/excise.
Method: web research only (clean-room). Everything here is paraphrased. Nothing was downloaded and run.

Labels used below:

- **[vendor]**: a claim made by Tally Solutions (or another vendor) about its own product.
- **[secondary]**: a professional blog or news article, not the statute or a CBDT/Protean document.
- **[unconfirmed]**: rule taken from 1961-Act-era knowledge, or a figure no fetched source in this session supported. Must be verified before it goes into a rule table.
- **[conflict]**: the sources disagree. Both readings are recorded.

Research limits, disclosed:

- incometaxindia.gov.in returned HTTP 403 to automated fetch, so the primary text of sections 392–402 of the Income-tax Act, 2025 and of the Income-tax Rules, 2026 could not be read. Section and table references below come from secondary sources and one CA-society deck (BCAS, April 2026).
- The shared web-search budget ran out before user-sentiment research and several confirmations could be done. The "What users love / hate" section is therefore inferred from vendor FAQs. It is not independent user research.

---

## Summary

1. **Two tax regimes in one book.** The Income-tax Act, 2025 applies from 1 April 2026. For TDS it applies where the *earlier of credit or payment* falls on or after that date [secondary: India Briefing]. Old-regime periods (up to FY 2025-26, including their correction statements) still use section codes, Forms 24Q/26Q/27Q/27EQ and FVU 9.x. New-regime periods (Tax Year 2026-27 onward) use:
   - 4-digit "tax/payment codes" (1001–1092), instead of section codes;
   - Forms 138/140/143/144, plus Form 141 for the challan-cum-statements and Form 142 for VDA;
   - "Tax Year", instead of "Financial Year";
   - a challan simplified to four parts (TDS/TCS, Interest, Fee, Others);
   - FVU 1.x.

   Our engine needs effective-dated rule tables keyed on the transaction date, with both regimes live at once.
2. **The section structure changed; rates mostly did not.** Section 192 salary TDS becomes s.392. The 194-series becomes rows of tables in s.393: 393(1) residents, 393(2) non-residents, 393(3) any person, 393(4) no-deduction cases, 393(6) declarations. Section 195 is absorbed into 393(2). Section 197 becomes s.395, and lower-deduction certificates are now available for *all* TDS provisions. 206AA and 206CC are unified in s.397(2). 206C becomes s.394. Most rates and thresholds carried over from the 1961 Act as amended by Finance Act 2025.
3. **Non-filer higher rates are gone.** 206AB and 206CCA were omitted from 1 April 2025 (Finance Act 2025). TCS on sale of goods, 206C(1H), was also switched off from 1 April 2025. TDS on purchase of goods (old 194Q) survives as payment code 1031: 0.1% above ₹50 lakh, where the buyer's turnover exceeds ₹10 crore.
4. **Tally's support.** TallyPrime 7.1 (20 May 2026) added export of Forms 138/140/143/144 [vendor]. TallyPrime 7.2 (12 Oct 2026) added support for FVU 1.2, interest/others allocation and date-of-payment validation [vendor]. Tally's own FAQ still lists gaps [vendor]:
   - no s.395 lower-deduction option, so users keep the old 197/197A flags;
   - no s.194N cash-withdrawal calculation;
   - nature of remittance for Form 144 entered by hand;
   - the no-PAN rate for code 1031 defaults to 20%, not 5%, and must be fixed by hand.

   Tally's "no upgrade needed" FAQ line and the 7.1/7.2 notes most likely reconcile like this (my reading): masters can be created by hand on old releases; exporting the new forms needs 7.1; FVU 1.2 validation needs 7.2.
5. **Payroll parameters moved in 2025–26.**
   - The four Labour Codes took effect 21 Nov 2025.
   - Under the uniform "wages" definition, allowances above 50% of remuneration are added back to wages for PF, ESI, gratuity and bonus.
   - Central Code on Wages Rules were notified 8 May 2026 [secondary].
   - The EPF/EPS/EDLI wage ceiling rose from ₹15,000 to ₹25,000 per month by S.O. 5109(E), effective 17 Sep 2026.
   - EPS-side pension amendments followed: G.S.R. 847(E) of 25 Sep 2026 and G.S.R. 890(E) of 15 Oct 2026 [secondary: KS&K].

   Payroll must treat every statutory number as a dated parameter, never a constant.
6. **MSME 45-day rule.** Old 43B(h) corresponds to **s.37(2)(g)** of the 2025 Act (Finance Ministry Rajya Sabha reply, 21 Jul 2026 [secondary report]). The MSMED Act s.15/s.16 rules still govern the payment deadline and the interest: compound with monthly rests at 3× the RBI bank rate.

---

## Feature inventory

| Feature | What it does | How the user interacts | Priority |
|---|---|---|---|
| Deductor/collector profile | TAN, PAN, deductor type (company, government, individual and so on), responsible person (name, designation, PAN, mobile **with country code**, country of residence; new in FVU 1.0), address, AO details | Company setup wizard step "Statutory → TDS/TCS" | MVP |
| Nature-of-payment / tax-code master | One row per 2025-Act payment code (1001–1092), linked to the old 1961 section for pre-April-2026 dates. Holds rate, threshold type (single/aggregate/per-month/per-transaction), threshold amount, "deduct on excess only" flag, the no-PAN rate, the applicable-from date | Ships as a built-in, versioned rule pack. The user only chooses a code per ledger | MVP |
| Ledger to code mapping | Expense, purchase or income ledgers carry a default tax code. Overridable per voucher line | Ledger master field. Command palette "Set TDS code" | MVP |
| Party (deductee) TDS profile | PAN plus PAN status (valid / applied / invalid / not available), deductee type (company / non-company, resident / non-resident), lower/nil certificate (s.395 certificate no., 15 chars per Tally FAQ, validity dates, limit amount, rate), 393(6) declaration (old 15G/15H) | Party master "Tax" tab. Lower-certificate limit consumption shown live | MVP |
| Auto-deduction in vouchers | Computes TDS at booking (credit) or payment, whichever is earlier. Tracks the aggregate threshold per party per code per tax year. Handles TDS on advances, then adjusts against the later bill | Inline in purchase/journal/payment voucher. A "TDS" row appears with an explanation popover (rule, threshold used, rate source) | MVP |
| Explainable TDS computation | Shows why TDS was or was not deducted (threshold not crossed, certificate, no PAN, declaration) | Hover/F-key on the TDS row | MVP |
| 194Q-equivalent (code 1031) | 0.1% on purchases above ₹50 lakh per seller per year, where buyer turnover in the previous year exceeds ₹10 crore. No-PAN 5% | Company flag "turnover > ₹10 cr last year". Per-party running total | MVP |
| TDS payable / outstanding | Deducted-not-paid by code, period, party | Report | MVP |
| Challan entry (ITNS 281 / e-pay) | Records CIN parts: BSR code, date, serial, amount split into TDS/TCS, Interest, Fee, Others (new 4-part challan) | Payment voucher "Stat payment" mode, or import the challan CSV/PDF from the portal (v2) | MVP |
| Challan reconciliation | Allocates deductee rows to challans. Flags over- and under-allocation and overlapping periods | Grid: challans on top, unallocated deductions below. Keyboard allocate | MVP |
| Interest and late-fee calculator (TDS/TCS) | Late-deduction and late-payment interest per month or part month; late-filing fee per day | Auto-computed on challan entry and shown as a suggested "Interest/Fee" split | MVP |
| Quarterly statement generation | Builds the NSDL/Protean text file for 138/140/143/144 (new) and 24Q/26Q/27Q/27EQ (legacy periods) | "Returns → TDS → Q2 TY 2026-27 → Generate". Pre-flight exception list first | MVP (140, 138); v2 (143, 144); later (141, 142) |
| Pre-flight validator | Our own clean-room checks mirroring public FVU rules: PAN format, date of payment inside the quarter/tax year (FVU 1.2), challan totals, certificate numbers | Exceptions panel with jump-to-voucher | MVP |
| FVU hand-off | We do **not** bundle the FVU. User downloads it from Protean; we generate the .txt and tell the user which FVU version applies; user validates with the .csi from the portal | Guided step with the file paths pre-filled | MVP |
| Correction statements | Imports the consolidated file (.tds) from TRACES and produces C1–C9 style corrections | Wizard | v2 |
| Correction time limit | 2-year correction window under s.397(3)(f), down from 6 years under old 200(3) [secondary: BCAS] | Warning banner on old quarters | v2 |
| Certificates (Form 131 / old 16A; Form 130 / old 16) | Statutory certificates must be downloaded from TRACES. We print an *advice* copy and track issue status | Report plus PDF | v2 |
| Lower/nil certificate tracking (Form 128 / old 13) | Validity, limit consumed, expiry alerts | Party panel plus alert | MVP |
| TCS on sales (s.394) | Codes 1068–1092: scrap, minerals, liquor, forest produce, motor vehicles > ₹10 lakh, parking/toll/mining leases, plus LRS/tour for authorised dealers | Ledger/stock item flag. Auto-collected on sales invoice or receipt, whichever is earlier | v2 (MVP if the target segment includes scrap/motor dealers) |
| TCS collected-not-paid, Form 143 | As for TDS | Report plus return | v2 |
| 194N cash withdrawal, 194R perquisites, 194S VDA, 194-O e-commerce | Niche codes | Rule pack only | later |
| Non-resident TDS (393(2), Form 144) | DTAA rate override with TRC/Form 10F evidence; nature of remittance; Form 145/146 (old 15CA/CB) not generated | Party flag "non-resident" | v2 |
| Payroll: employee master | Identity, PAN, Aadhaar, UAN, ESI IP no., bank, joining/leaving dates, state (for PT/LWF), tax regime choice, previous-employer income | Employee form plus bulk CSV import | v2 |
| Payroll: pay heads | Earnings, deductions, reimbursement, employer contributions, loans/advances. Calculation: flat, on attendance, on production, formula on other heads, slab table | Pay-head editor with a formula bar (expression language, not free code) | v2 |
| Payroll: attendance and production types | Present/absent/leave/OT units; piece-rate production units | Attendance voucher or grid; CSV import from biometric systems | v2 |
| Salary structures | Templates per employee group with effective dates and revision history (arrears) | Structure editor; "apply to group" | v2 |
| Labour-code wage test | Computes the 50% exclusion add-back to derive "wages" for PF/ESI/gratuity/bonus | Automatic; shown on the payslip breakdown | v2 |
| PF (EPF/EPS/EDLI) | Contributions on wages up to the dated ceiling (₹25,000 from 17 Sep 2026); EPS split; voluntary higher wages; ECR text file | Auto; ECR export | v2 |
| ESI | Employee and employer contributions under the coverage ceiling; contribution-period continuity rule | Auto; monthly contribution file | v2 |
| Professional tax | State slab tables (monthly/half-yearly, gender/age variants) | Rule pack per state | v2 |
| Labour Welfare Fund | State-specific periodic contributions (Tally's page does not mention LWF) | Rule pack per state | later |
| Gratuity | Provision and payout per Social Security Code (incl. fixed-term employees) | Report plus accrual journal | v2 |
| NPS (corporate) | Employer/employee contributions; tax treatment differs by regime | Pay head type | v2 |
| Salary TDS (s.392) | Projects annual taxable salary by regime, spreads the tax over the remaining months, handles declarations and proofs | Per-employee tax sheet; "what-if regime" toggle | v2 |
| Payslips, pay sheet, bank advice | PDF/email payslip; bank bulk-transfer file | Reports | v2 |
| Payroll statutory reports | PF ECR, ESI return, PT returns, Form 138 annexures, Form 130 advice | Reports | v2 |
| MSME vendor tracking | Udyam number, enterprise class (micro/small/medium), agreed credit days (cap 45), acceptance date | Party master | MVP |
| 45-day overdue and s.37(2)(g) disallowance report | Unpaid MSE bills past their due date at year end; amount not deductible this year | Report; dashboard alert "₹X becomes disallowable in N days" | MVP |
| MSME-1 half-yearly return data | Outstanding to MSEs > 45 days (MCA form) | Export | v2 |
| MSMED interest calculator | Compound monthly at 3× RBI bank rate from the appointed day | Per-bill interest column; journal suggestion | v2 |
| General interest on overdue receivables/payables | Simple/compound, per party or per bill, rate slabs by date | Party setting plus interest report and voucher generation | v2 |
| Legacy VAT / Central Excise | Pre-GST (before 1 Jul 2017) taxes; today relevant only for state VAT on alcohol/petroleum and excise on some petroleum/tobacco | Not built in; import of historical data only | later |

---

## Rule set our engine must implement

All amounts are in INR. "TY" means tax year. Each rule must be stored with `effective_from`, `effective_to` and a provenance citation, per the engineering rules. Any rule marked [unconfirmed] is `pending` until a primary source is read.

### A. Regime switch

- **A1.** Income-tax Act, 2025 in force from 1 Apr 2026. TDS/TCS sections 392–402 replace 192–206CCA [secondary: Taxmann 17 Apr 2026; BCAS].
- **A2.** Which law applies to a transaction depends on the earlier of credit or payment. Earlier than 1 Apr 2026 means the 1961 Act [secondary: India Briefing].
- **A3.** Statements for periods up to 31 Mar 2026 use legacy forms and FVU 9.x. Statements for TY 2026-27 onward use the new forms and FVU 1.x [secondary: TDSMAN on Protean FVU 1.0, 1 Jul 2026]. FVU 9.4 (1 Apr 2026) validates only up to FY 2025-26 [secondary].
- **A4.** Quoting old section numbers on new-regime transactions may fail validation [secondary: India Briefing].

### B. Section map (old to new)

| Old (1961) | New (2025) | Notes |
|---|---|---|
| 192 salary | 392 | Payment code 1002 [secondary: BT/H. Sheth] |
| 193 interest on securities | 393(1) Table Sl. 5(i) | Uniform ₹10,000 threshold; 10% [secondary: BCAS] |
| 194 dividend | 393(1) Sl. 7; 393(4) Sl. 10 | ₹10,000; code 1029 [secondary] |
| 194A interest (other) | 393(1); no-deduction cases in 393(4) Sl. 7 | Exemption now limited to banking companies; co-operative banks dropped from the exemption [secondary: BCAS] |
| 194C contractors | 393(1); codes 1023/1024 | 1% individual/HUF, 2% others; ₹30,000 single / ₹1,00,000 aggregate. From 1 Apr 2026, "work" in s.402(27) includes supply of manpower under the payer's control (Finance Act 2026) [secondary: BCAS; BT] |
| 194D insurance commission | 393(1); code 1005 | Rate in force; ₹20,000 [secondary] |
| 194DA life-insurance payout | 393(1) Sl. 8(i) | 2% when the payout is **above** ₹1,00,000. The old rule applied at ₹1,00,000 or more [secondary: BCAS] |
| 194H commission | 393(1) Sl. 1(ii); code 1006 | 2%; ₹20,000; "professional services" now includes advertising (s.402(28)) [secondary] |
| 194-I rent | 393(1) Sl. 2; codes 1008/1009 | 2% plant/machinery, 10% land/building; ₹50,000 per month [secondary] |
| 194-IB rent by individuals | 393(1) Sl. 2(ii) | Rent definition widened to land appurtenant / factory buildings [secondary: BCAS] |
| 194-IA / 194LA immovable property | 393(1) Sl. 3; code 1012 | 1% above ₹50 lakh. 194LA compensation: 10% at the earlier of credit or payment (was payment only) [secondary: BCAS] |
| 194J professional / technical | 393(1); code 1027 (codes unconfirmed) | 10% professional / 2% technical; ₹50,000 (Finance Act 2025) [secondary] |
| 194N cash withdrawal | 393(3) Sl. 5; codes 1064/1065 | Flat 2% above ₹1 crore (₹3 crore for co-operatives); non-filer slabs removed [secondary: BCAS] |
| 194Q purchase of goods | 393(1); code 1031 | 0.1% above ₹50 lakh; buyer turnover > ₹10 crore [secondary]. Table serial **[conflict]**: "393(1)(h)" vs "Table Sl. 8(ii)" |
| 194T partner payments | 393(3); code 1067 | 10%; ₹20,000 [secondary] |
| 195 non-residents | 393(2); codes 1039–1056 | Rates in force or DTAA [secondary] |
| 197 lower/nil certificate | 395 (Form 128, old Form 13) | Now available for all TDS provisions [secondary: BCAS]; electronic process from 1 Apr 2026 (Finance Act 2026, clause 74) [secondary] |
| 197A declarations (15G/15H) | 393(6) | From 1 Apr 2027 a declaration can be filed once with the depository [secondary: BCAS]. Successor form numbers [unconfirmed] |
| 206AA / 206CC no PAN | 397(2), unified | See C3 |
| 206AB / 206CCA non-filers | Omitted from 1 Apr 2025 | Finance Act 2025 [secondary, multiple] |
| 206C TCS | 394 | Codes 1068–1092 [secondary: TaxGuru] |
| 206C(1H) TCS on sale of goods | Omitted from 1 Apr 2025 | Finance Act 2025 |
| 200(3) correction | 397(3)(f) | Time limit 2 years (was 6) [secondary: BCAS] |
| 200A processing | 399 | [secondary] |
| 201 failure consequences | 398 | [secondary: Taxmann] |

**[conflict] Referencing style.** BCAS and several blogs cite "Table Sl. No. x of 393(1)". Other blogs cite "393(1)(f)" or "393(1)(h)". Store the **payment code** as the primary key, plus a free-text statutory reference, until the gazette text is checked.

**[conflict] Payment codes.**

- Interest is code 1019 in India Briefing and 1022 in Business Today.
- India Briefing lists code 1052 for two different rows.
- Motor-vehicle TCS is 394(1) Table Sl. 6(a) in BCAS and 394(3) in TaxGuru.

The authoritative code list should be taken from the Protean FVU 1.x file-format specification, which is public. It has not been read yet.

### C. Computation rules

- **C1. Threshold kinds:**
  - per transaction (e.g., contractor ₹30,000);
  - aggregate per party per tax year (contractor ₹1 lakh; 1031 at ₹50 lakh, taxed only on the excess);
  - per month (rent ₹50,000);
  - nil.

  Once an aggregate threshold is crossed, the general rule is to deduct on the whole year's amount, not just the excess. Code 1031 is the exception: only the excess over ₹50 lakh is taxed [secondary; Tally FAQ exposes an "excess only" flag (vendor)]. Every row in the rule table must carry an explicit `on_excess_only` flag.
- **C2. Timing:** deduct at the earlier of credit (including credit to a "suspense / provision" account) or payment. Advances are taxed when paid and adjusted against the later invoice.
- **C3. No PAN:**
  - The 1961 rule (206AA): the higher of the section rate, the rate in force, or 20%. For 194Q/194-O the rate is 5% (code 1031: 5% per Tally FAQ [vendor]).
  - The 2025 Act unifies this in s.397(2) [secondary]. The exact 2025 wording has not been read [unconfirmed].
  - TCS without PAN **[conflict]**: TaxGuru says the highest of twice the rate, 5%, or 20%. The 1961 206CC rule was the higher of twice the rate or 5%. Pending.
- **C4. Lower/nil certificate:** apply the certificate rate only within its validity dates and up to its amount limit. Above the limit, revert to the normal rate. Report with remark A/B and the certificate number (15 chars) [vendor: Tally FAQ; FVU 1.0 field "certificate issued u/s 395"].
- **C5. Surcharge and cess:** in new-regime statements they are no longer reported separately. They are included in the TDS amount [secondary: TDSMAN on FVU 1.0]. For resident non-salary TDS, surcharge/cess are generally nil [unconfirmed].
- **C6. Rounding:** TDS rounded to the nearest rupee [unconfirmed under 2025 Act; 1961 s.288B].
- **C7. Interest for late deduction or payment** (1961 s.201(1A)): 1% per month or part month from the date deductible to the date deducted; 1.5% per month or part month from deduction to payment [unconfirmed; 2025-Act section not verified, likely in s.398].
- **C8. Late-filing fee** (1961 s.234E): ₹200 per day, capped at the TDS amount [unconfirmed for 2025 Act]. Penalty for non-filing (1961 s.271H): ₹10,000–1,00,000 [unconfirmed].
- **C9. Due dates** (1961 Rules 30/31A; carry-forward [unconfirmed]):
  - Deposit by the 7th of the next month; March deductions by 30 April. Government deductors deposit the same day if paid without challan.
  - Quarterly statements due 31 Jul, 31 Oct, 31 Jan, 31 May.
  - Salary certificate (Form 130) due by 15 June after the tax year [secondary: SCC Online].
  - Non-salary certificate (Form 131) due 15 days after the statement due date [unconfirmed].
- **C10. FVU 1.2** (19 Aug 2026): the date of payment must fall inside the statement's quarter and tax year [secondary: TDSMAN].

### D. TCS (s.394; rates for TY 2026-27)

These figures come from TaxGuru and Business Today (secondary). Both say Finance Act 2026 set the rates; neither is the enacted text.

| Item | Code | Rate | Threshold |
|---|---|---|---|
| Alcoholic liquor | 1068 | 2% (was 1%) | none |
| Tendu leaves | 1069 | 2% (was 5%) | none |
| Timber / forest produce | 1070–1072 | 2% | none |
| Scrap | 1073 | 2% (was 1%) | none |
| Minerals (coal, lignite, iron ore) | 1074 | 2% (was 1%) | none |
| Motor vehicle | 1075–1085 | 1% | > ₹10 lakh per transaction; collected at the earlier of debit or receipt (was at receipt) [BCAS] |
| LRS: education/medical | 1086 | 2% | above ₹10 lakh |
| LRS: loan-funded education | none | nil | none |
| LRS: other | 1087 | 20% | above ₹10 lakh |
| Overseas tour package | 1088/1089 | 2% (was 5%/20%) | none |
| Parking lot, toll plaza, mining lease | 1090–1092 | 2% | none |

Previous-rate figures are [secondary] and the 2% uniformity is attributed to Finance Act 2026. Verify against the enacted Act.

### E. Payroll

- **E1. Labour Codes in force 21 Nov 2025** (Code on Wages 2019, IR Code 2020, Social Security Code 2020, OSH Code 2020) [secondary: WTW, Deccan Herald]. Central Code on Wages Rules notified 8 May 2026 [secondary: LKS]. State rules vary by state and date.
- **E2. 50% rule.** "Wages" means basic + DA + retaining allowance. Specified exclusions (HRA, conveyance, overtime, commission, and so on) that together exceed 50% of total remuneration are added back, by the excess, to wages for PF, ESI, gratuity and bonus [secondary: KS&K; BT]. The exact exclusion list is [unconfirmed]; read s.2(y) of the Code on Wages.
- **E3. EPF wage ceiling:** ₹25,000 per month from 17 Sep 2026 (S.O. 5109(E), superseding S.O. 2702(E) of 29 May 2026). Previously ₹15,000 [secondary: SCC Online, KS&K].
  - Employee 12% and employer 12% unchanged; EDLI 0.5% [secondary].
  - The EPS split (employer 8.33% to EPS on a wage base not yet confirmed, and the rest to EPF) is **[unconfirmed]** after G.S.R. 890(E): one source says the central-government EPS subsidy remains on ₹15,000. Do not hard-code the EPS base.
  - Proration for a month that straddles 17 Sep 2026 is [unconfirmed].
- **E4. ESI** [unconfirmed]: employee 0.75%, employer 3.25%, coverage ceiling ₹21,000 per month (₹25,000 for persons with disability). Contribution-period rule (April–September, October–March): an employee stays covered for the whole period once covered.
- **E5. Professional tax:** state laws; Constitution Art. 276(2) caps it at ₹2,500 per year [unconfirmed]. Slabs per state; some states have none.
- **E6. Gratuity:** Social Security Code Chapter V. 15 days' wages per completed year, using a 26-day month. Ceiling ₹20 lakh [unconfirmed]. Fixed-term employees eligible pro-rata after 1 year [secondary, details unconfirmed]. Wages per E2.
- **E7. Bonus:** Code on Wages; 8.33%–20% of wages under a wage ceiling [unconfirmed figures].
- **E8. Salary TDS (s.392):** estimate annual income under the employee's chosen regime, the new regime being the default; deduct evenly over the remaining months [secondary: Taxmann]. Slabs, standard deduction and rebate come from the Finance Act for the year and must be a dated table [unconfirmed figures for TY 2026-27]. Successor forms for 12BA/12BB [unconfirmed].
- **E9. NPS employer contribution:** deduction limit differs by regime [unconfirmed figures].

### F. MSME payments and interest

- **F1. Deduction timing:** s.37(2)(g) of the 2025 Act (old 43B(h), inserted by Finance Act 2023, effective AY 2024-25). Amounts owed to a **micro or small** enterprise (Udyam-registered supplier; traders excluded by MSME classification [unconfirmed]) are deductible only in the year paid, unless paid within the MSMED s.15 time limit [secondary].
- **F2. MSMED s.15:** pay by the agreed date, capped at 45 days from acceptance or deemed acceptance. With no agreement, pay within 15 days [secondary, multiple].
- **F3. MSMED s.16:** compound interest with monthly rests at 3× the RBI bank rate from the appointed day. Courts treat this as mandatory. Medium enterprises do not get the 3× rate [secondary: LiveLaw, Calcutta HC]. The bank rate itself must be a dated input table.
- **F4. MSMED s.23:** that interest is not tax-deductible [unconfirmed under the 2025 Act].
- **F5. MSME-1 (MCA):** half-yearly return of dues to MSEs outstanding beyond 45 days [vendor: Tally 4.1 notes; unconfirmed rule text].

### G. Legacy indirect taxes

- **G1.** GST replaced central excise (most goods), service tax and state VAT from 1 Jul 2017. State VAT/sales tax still applies to alcoholic liquor for human consumption and to petroleum crude, diesel, petrol, natural gas and ATF. Central excise still applies to petroleum and tobacco products [unconfirmed; general knowledge, not fetched].
- **G2.** We should not build VAT/excise computation for MVP. Offer historical import and a configurable "other tax" ledger type.

---

## Return/file formats

| Return | Legacy (up to FY 2025-26) | New (TY 2026-27 onward) | Notes |
|---|---|---|---|
| Salary TDS statement | 24Q | **Form 138** | Annexure II (salary detail) in Q4 only [vendor: Tally payroll page] |
| Non-salary, residents | 26Q | **Form 140** | |
| Non-salary, non-residents | 27Q | **Form 144** | Nature of remittance field |
| TCS statement | 27EQ | **Form 143** | |
| Challan-cum-statement (property / rent by individuals / contracts by individuals / VDA) | 26QB / 26QC / 26QD / 26QE | **Form 141** | From 1 Oct 2026, resident individual/HUF buyers from NRI sellers use PAN instead of TAN [secondary: BCAS, BT] |
| VDA statement | 26QF | **Form 142** | |
| Government book-entry challan statement | 24G | **Form 137** | Referenced in FVU 1.0 column headings |
| Salary certificate | Form 16 | **Form 130** | TRACES-generated part; first issue by 15 Jun 2027 for TY 2026-27 [secondary] |
| Non-salary certificate | 16A | **Form 131** | TRACES-generated |
| TCS certificate | 27D | Form 132 or 133? | [unconfirmed] |
| Lower/nil certificate application | 13 | **Form 128** | |
| TAN application | 49B | **Form 135** | |
| Annual tax statement | 26AS | **Form 168** | |

**File format.** The quarterly statement is a fixed-structure, caret-delimited text file. Record types are FH (file header), BH (batch header), CD (challan detail), DD (deductee detail) and SD (salary detail, salary form). Protean publishes a file-format specification (FFS) per form version [unconfirmed for record names under the new forms]. TDSMAN's summary of Protean's FVU 1.0 key-features note records these changes:

- Form names are updated.
- "Financial Year" becomes "Tax Year".
- Contact number with country code is added for the deductor and the responsible person.
- Surcharge, education cess and penalty/others fields are removed from the challan.
- Challan "Interest" and "Fee" columns become "Total Interest" and "Total Fee".
- Column numbers 301–327 (138) and 401–425 (140) are renamed to letter columns A–O.
- The lower-deduction certificate column now refers to s.395.
- Section codes are replaced by 4-digit tax codes.

FVU versions:

| Version | Released | Scope / change |
|---|---|---|
| 9.4 / 2.190 | 1 Apr 2026 | Legacy forms, up to FY 2025-26 |
| 9.5 | 2026 | Legacy forms, up to 31 Mar 2026 |
| 1.0 | 1 Jul 2026 | New forms, TY 2026-27 onward |
| 1.1 | 8 Jul 2026 | Fix of 1.0, no functional change |
| 1.2 | 19 Aug 2026 | Date-of-payment-in-quarter validation |

**Filing flow** (same in both regimes) [vendor: Tally help; Income Tax Dept generic page]:

1. Generate the .txt file.
2. Download the CSI (challan input file) from the e-filing/OLTAS portal.
3. Run the Protean FVU, which produces a .fvu file plus Form 27A (the legacy name; successor name [unconfirmed]), or an error HTML.
4. Upload via the e-filing portal or a TIN-FC.

**Design implication.** We need a clean-room writer for each FFS version, selected by the statement's period. We also need a golden-file test corpus built *from the public FFS*, not from our own output. The FVU is a Java tool distributed by Protean. We must not bundle or decompile it. Users run it themselves.

**Payroll files:**

- **EPF ECR:** a text file with a "#~#" separator, one member per line (UAN, gross wages, EPF/EPS/EDLI wages, contributions, NCP days, refunds) [unconfirmed field list; check the EPFO ECR 2.0 spec].
- **ESI:** monthly contribution Excel upload on the ESIC portal [unconfirmed].
- **PT:** state portals, various formats.

---

## What users love / hate

**Caveat.** Independent user-sentiment research was **not** done: the web-search budget ran out before forum and review searches could run. The points below are inferred from pain points in Tally's own FAQs and help pages, plus general domain knowledge. Treat them as hypotheses to validate in user interviews.

**Likely loved (inferred):**

- TDS is computed automatically inside the voucher from ledger/party masters, with no separate tool needed [vendor].
- Generating the return text file directly from the books, then a familiar FVU step [vendor].
- Integrated payroll with PF/ESI/PT reports and ECR export in the same company file [vendor].
- MSME overdue views with ageing and Form MSME-1 data (from TallyPrime 4.1) [vendor].

**Likely hated (inferred from vendor FAQ workarounds):**

- **The recompute-and-overwrite trap.** After challan reconciliation, users must "recompute and overwrite" Form 26Q, or deductee rows are missing from the e-TDS file [vendor FAQ]. This is a stale-cache design smell. Our engine should derive returns live, with no manual refresh.
- **Rule lag after law changes.** New sections arrive late, so users must create masters by hand, set the no-PAN rate by hand, and use old 197 flags for s.395 certificates [vendor FAQ]. Our rule packs should ship as data with dated provenance, updatable without a binary release.
- **Opaque "why wasn't TDS deducted?" debugging.** Users walk a checklist of threshold, ledger, party, PAN, duty ledger and override [vendor FAQ]. We should explain each computation in place.
- **Optional vouchers lose TDS** when converted to regular vouchers [vendor FAQ].
- **"Partially overlapped" errors** on statutory payments, caused by period mismatches [vendor FAQ].
- **Payroll gaps:** no Labour Welfare Fund on Tally's payroll features page; no visible support yet for the Labour Code wage test or the ₹25,000 EPF ceiling in the 7.2 notes [vendor, by omission; unconfirmed].
- **No salary-TDS projection or regime comparison** surfaced in Tally docs [unconfirmed].

---

## Open questions

1. **Primary text.** We need the gazette text of ss.392–402 (2025 Act) and Rules 2026 (incl. Rule 215, Rule 219, the due-date rules). incometaxindia.gov.in blocked automated fetch. Someone should save the PDFs manually for clean-room reading.
2. **Authoritative payment-code list (1001–1092)** and the Protean FFS for Forms 138/140/143/144 (FVU 1.2). These resolve the code conflicts (1019 vs 1022, duplicated 1052, motor-vehicle 394(1) vs 394(3)).
3. **Due dates, interest, fee and penalty** under the 2025 Act and Rules 2026: did the 1961 values (7th / 30 Apr; 31 Jul / 31 Oct / 31 Jan / 31 May; 1% / 1.5%; ₹200 per day) carry over unchanged?
4. **TCS without PAN:** is 20% an alternative or a cap? What is the s.397(2) wording for TDS?
5. **Successor forms** for 15G/15H, 12BA/12BB, 27D, 27A, 15CA/15CB.
6. **EPS base** after S.O. 5109(E) and G.S.R. 890(E): does EPS run on ₹15,000 or ₹25,000? What happens to the September 2026 transition month?
7. **ESI ceiling and rates** under the Social Security Code as of Nov 2026, and any Code-era change.
8. **Labour Code "wages" exclusion list** and how states' rules differ. Do we compute the 50% test per month or per year?
9. **Gratuity ceiling and fixed-term pro-rata details** under the SS Code.
10. **Product scope:** do we file Form 141 (property/rent by individuals)? It is usually done on the portal by the payer, not from accounting books, so probably not for MVP.
11. **Do we target statutory payroll in v1 at all,** or integrate with dedicated payroll tools? Payroll multiplies per-state rule maintenance (PT and LWF in about 20 states).
12. **User sentiment:** we need forum, review and interview research on Tally TDS/payroll pain. Not done here because the search budget ran out.

---

## Sources

Primary or near-primary:

- Protean RPU/FVU 1.0 key-features note (via TDSMAN mirror), Jul 2026: https://blog.tdsman.com/downloads/key_features_fvu_1_0.pdf
- Income Tax Dept, FAQs and guidance notes on forms under the Income-tax Rules, 2026 (fetch blocked; listed for follow-up): https://www.incometaxindia.gov.in/faqs-and-guidance-notes-on-forms-as-per-income-tax-rules-2026
- Income Tax Dept, section 393 / 394 pages (fetch blocked): https://www.incometaxindia.gov.in/w/section-393-5 , https://www.incometaxindia.gov.in/w/section-394-6
- Finance Bill 2026 (not read in full): https://incometaxindia.gov.in/Budgets%20and%20Bills/2026/Finance_Bill-2026.pdf

Vendor (Tally Solutions):

- TallyPrime 7.1 release notes (20 May 2026): https://help.tallysolutions.com/release-notes-tallyprime-7-1/
- TallyPrime 7.2 release notes (12 Oct 2026): https://help.tallysolutions.com/release-notes-tallyprime-7-2/
- TDS FAQ: https://help.tallysolutions.com/tally-prime/india-tds/tds-faq-tally/
- TDS overview: https://help.tallysolutions.com/tally-prime/taxation-compliance/india-tds/
- TDS return filing: https://help.tallysolutions.com/tally-prime/india-tds/tds-filing-returns-tally/
- Payroll features: https://help.tallysolutions.com/tally-prime/payroll/payroll-features-of-tally-erp-tally/
- MSME features: https://help.tallysolutions.com/tally-prime/accounting/msme-features/
- TallyPrime 4.1 MSME article: https://tallysolutions.com/tally/streamlining-compliance-and-payments-to-msmes/

Secondary:

- Taxmann, "TDS & TCS under Income Tax Act 2025 – A Practical Guide" (17 Apr 2026): https://www.taxmann.com/post/blog/opinion-tds-tcs-under-income-tax-act-a-practical-guide
- BCAS / CA Avinash Rawani, "Convergence & Divergence … TDS and TCS … 2025 vs 1961" (Apr 2026): https://bcasonline.org/wp-content/uploads/2026/04/Convergence-Divergence-Provisions-Procedures-of-TDS-and-TCS-Changes-in-Income-Tax-Act-2025-vs-Income-Tax-Act-1961.pdf
- India Briefing, Section 393 guide: https://www.india-briefing.com/news/section-393-income-tax-act-2025-tds-rules-rates-compliance-guide-44450.html
- Business Today, TDS/TCS explainer (16 Apr 2026): https://www.businesstoday.in/personal-finance/tax/story/bt-explainer-tds-tcs-shake-up-decoding-new-tax-rates-codes-and-compliance-rules-525983-2026-04-16
- TaxGuru, TCS rate chart TY 2026-27: https://taxguru.in/income-tax/tcs-rate-chart-tax-year-2026-27-income-tax-act-2025.html
- Taxscan, Forms 138/140: https://www.taxscan.in/top-stories/income-tax-forms-138-140-replace-24q-26q-new-tds-reporting-rules-for-salary-and-non-salary-payments-1444578
- TDSMAN, FVU 1.0 / 1.1 / 1.2 / 9.4 posts: https://blog.tdsman.com/2026/07/new-file-validation-utility-fvu-version-1-0-released-on-1st-july-2026/ , https://blog.tdsman.com/2026/08/new-file-validation-utility-fvu-version-1-2-released-on-19th-august-2026/ , https://blog.tdsman.com/2026/04/new-file-validation-utilities-fvus-version-9-4-2-190-released-on-1st-april-2026/
- SCC Online, Form 130 vs Form 16 (15 Jun 2026): https://www.scconline.com/blog/post/2026/06/15/overview-comparison-of-form-16-and-form-130-tds-certificate/
- TaxGuru, 206C(1H) removal FAQs: https://taxguru.in/income-tax/tcs-sale-goods-removed-april-1-2025-faqs.html
- TaxAJ, 206AB/206CCA omitted: https://www.taxaj.com/learn/section206ab-206cca-194q/
- SCC Online, ₹25,000 EPF wage ceiling (Sep 2026): https://www.scconline.com/blog/post/2026/09/19/labour-ministry-notifies-25000-wage-ceiling-under-social-security-code/
- KS&K, EPF ceiling employer compliance (2026): https://ksandk.com/newsletter/epf-wage-ceiling-25000-employer-compliance-2026/
- KS&K, 50% wage rule: https://ksandk.com/labour-employment/50-wage-rule-labour-codes-salary-pf-gratuity/
- WTW, Labour codes in effect (Dec 2025): https://www.wtwco.com/en-in/insights/2025/12/india-long-awaited-labor-codes-in-effect
- LKS, central and state rules status: https://employmentlaw.lkslaw.com/rules
- TaxGuru, Finance Ministry on 43B(h) (Rajya Sabha reply): https://taxguru.in/income-tax/finance-ministry-clarifies-section-43bh-45-day-msme-payment-rule.html
- LiveLaw, MSMED interest provisions: https://www.livelaw.in/law-firms/law-firm-articles-/the-stringency-of-interest-provisions-under-msmed-act-implementation-and-judicial-interpretation-310929
