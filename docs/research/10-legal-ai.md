# 10 - Legal/compliance constraints and AI features

Researcher area: legal and compliance constraints, and AI features. Research date: 2026-11-20.

**This is not legal advice.** It is engineering research meant to shape design decisions and frame
questions for a lawyer. Items marked **[lawyer]** need advice from Indian counsel (IP, company
law or data protection) before we rely on them.

How claims are labelled:

- **[primary]**: read on the regulator's, court's or vendor's own page.
- **[secondary]**: from law-firm or press commentary, or a search-engine summary of a page we did
  not open.
- **[vendor claim]**: a vendor's statement about its own product, not independently checked.
- **[unconfirmed]**: we could not verify it.
- **[knowledge]**: from the researcher's background knowledge, not re-checked this session.

The session's web-search budget ran out partway through. A few points are therefore marked
[knowledge] or [unconfirmed] and should be re-checked; they are listed under Open questions.

---

## Summary

1. **Building a Tally-compatible clone is legally workable if it is genuinely clean-room. The main
   exposure is trademark and trade dress, not copyright in features.**
   - Indian courts protect program code as a literary work [secondary].
   - Protection for non-literal "look and feel" is unsettled in India. The closest case found is
     Hulm v. Myfab11 (Delhi HC, 2023); its holding was not read [secondary].
   - US cases are persuasive only:
     - *Lotus v. Borland* held that a menu command hierarchy is an uncopyrightable "method of
       operation". The Supreme Court affirmed it 4-4, so it set no national precedent [secondary:
       Wikipedia case summary].
     - *Google v. Oracle* (593 U.S. 1, 2021) was decided on fair use. The Court assumed, without
       deciding, that APIs are copyrightable [secondary: Wikipedia case summary].
   - **New in 2026:** on 9 March 2026 the Calcutta High Court held that GUIs can be registered as
     designs under the Designs Act, 2000 [secondary: WIPR]. Tally (or anyone) can now register
     screen designs, which opens an infringement route beyond copyright. Do not imitate Tally's
     screens pixel for pixel.
2. **Do not use "Tally" in the product name, domain, logo or repo name.** The current folder name
   `tally-oss` is fine internally but must not be the public name. Factual compatibility
   statements are generally allowed ("imports data exported from TallyPrime"; TallyPrime is a
   trademark of Tally Solutions Pvt Ltd). Trade Marks Act s.30 covers honest descriptive use, and
   s.30(2)(d) covers use to show compatibility with parts or accessories [knowledge; lawyer].
3. **Tally's licence terms bar reverse engineering and decompilation** [secondary: Tally T&C
   search snippets]. Whether s.52(1)(ab) of the Copyright Act (the interoperability exception)
   overrides a contract is unsettled, and no case law was found [secondary: SpicyIP]. Rules:
   - Interoperate only from public documentation and from files users export (XML/JSON exports,
     the published XML/HTTP integration interface).
   - Never decompile Tally, copy its TDL source, or use its screenshots as design references.
4. **The Companies Act audit-trail rule applies to companies, not to software vendors, but it
   shapes the product.**
   - Rule 3(1) of the Companies (Accounts) Rules, 2014 (proviso, in force for financial years from
     1 April 2023; sources conflict on the start date) requires accounting software that records an audit trail of every transaction,
     keeps an edit log of each change with its date, and cannot be disabled [secondary].
   - Rule 11(g) of the Audit Rules requires the auditor to report on whether the trail was
     operating all year, tampered with, or preserved [secondary].
   - Rule 3(5) requires a **daily backup on servers physically in India**, even if the primary
     copy is kept abroad [secondary].
   - For open-source software, "cannot be disabled" has to be met by **tamper evidence** (a
     hash-chained, append-only log, always on for company books), not by obscurity. Whether an
     auditor can accept this is an open question [lawyer; ICAI].
5. **DPDP: in local-only mode the project is probably neither a data fiduciary nor a data
   processor. Cloud sync or cloud AI changes that** [lawyer].
   - With optional sync, the operator of the sync service is a data processor for the business,
     which is the data fiduciary.
   - Calling a cloud LLM adds a sub-processor and possibly a cross-border transfer.
   - The DPDP Rules 2025 were notified on 13 Nov 2025. Most substantive obligations apply after
     18 months, around 13 May 2027 [secondary; date unconfirmed from the gazette].
   - Rule 6 requires a one-year retention of logs and personal data, subject to other laws [rule text via
     third-party site dpdpa.com]. Rule 7 requires breach notice to affected people without delay
     and a detailed report to the Board within 72 hours [rule text via third-party site dpdpa.com].
6. **Licence: AGPL-3.0-or-later for the application and any server, with a separate trademark
   policy, plus DCO for contributors.** Optionally put interchange and format crates under
   Apache-2.0 so other tools can adopt our formats. For a desktop binary, AGPL and GPL behave the
   same. AGPL matters only for the sync server and for hosted SaaS forks, which is exactly the
   scenario to protect against.
7. **AI landscape.** Every major Indian competitor now ships AI that turns documents into draft
   vouchers and categorises bank statements. TallyPrime 7.1 (2026) added "Docs by Ira":
   - It extracts English-only invoices into **Draft Transactions**. Nothing is posted until a user
     saves it, and the totals are recalculated on save [primary: Tally help FAQ].
   - It is cloud-metered: 100 free pages for two months, then a paid plan [primary: Tally help
     FAQ].
   - This is direct vendor precedent for our "AI proposes, engine computes" design.
   - Zoho has a broader set: Ask Zia Q&A, anomaly detection, bank categorisation and CoCreate
     [primary: Zoho blog, Apr 2026].
   - Suvit (bought by Vyapar in Nov 2025, now Vyapar TaxOne) and ClearTax cover OCR, bank
     statements and GSTR-2B reconciliation [secondary; vendor claims].
8. **Trust is the deciding factor for AI.**
   - About half of the roughly 1,100 CA members in ICAI's survey use AI rarely or never; the
     barriers are cost, knowledge and unclear benefit [primary: ICAI AI site].
   - 93% of finance professionals question AI-generated insights [secondary: ACCA/CA ANZ, July
     2026, global].
   - Only 14% of CFOs fully trust AI accounting data [secondary].
   - ICAEW (Nov 2026) warns that putting client data into public AI tools can breach
     confidentiality [secondary]. No ICAI-specific guidance on generative AI and confidentiality
     was found.
   - Design response: local-first AI by default; a per-feature, per-company opt-in for cloud;
     redaction; field-level provenance; deterministic checks; a human confirms every posting.

---

## Legal risk table

| # | Risk | Severity | Mitigation |
|---|------|----------|------------|
| L1 | **Trademark infringement or passing off**: "Tally" in the product name, domain, logo, ads or app-store keywords; a confusingly similar name ("Talli", "OpenTally"); a similar colour scheme or logo. TALLY is a long-used Indian mark (we did not check whether it is on the well-known marks list [unconfirmed]). | **High** | Choose a distinct name and clear it with a trademark search in classes 9 and 42 before going public [lawyer]. Rename the repo before publishing. Use Tally's names only in factual compatibility statements, with a trademark-attribution line and a "not affiliated" disclaimer. No Tally logos or icons. Avoid Tally's colour palette, splash screen and gateway layout. |
| L2 | **Trade dress or GUI design rights**: copying the "Gateway of Tally" screen, the yellow/teal palette, or menu-bar visuals [knowledge]. The Calcutta HC (9 Mar 2026) held GUIs registrable as designs; Tally may hold registered GUI designs [unconfirmed]. | **High** | Original visual design (our own mock). Do not reference Tally screenshots in design work. Search the Designs Register for Tally Solutions GUI registrations before launch [lawyer]. Keep keyboard *workflows* (they are functional; see L4) but present them in our own visual language. |
| L3 | **Copyright in code, TDL or documentation**: copying Tally's shipped TDL samples, help text, report layouts or voucher-type names in bulk. | **High** (if it happens) / Low (with clean-room process) | Clean-room rules: (a) no decompiling or disassembling Tally binaries; (b) no copying TDL source; (c) write our own help text; (d) contributors certify by DCO that they have not copied Tally material. Statutory terms (Schedule III headings, GST field names, Companies Act formats) are law, not Tally's copyright. Generic accounting terms ("Sundry Debtors", "Journal") are not Tally's. |
| L4 | **Copyright in UI structure or keyboard shortcuts** (Alt+F2 for date, F4-F9 for voucher types, Ctrl+A to accept). | **Medium** | No Indian ruling found protecting shortcuts [unconfirmed]. *Lotus v. Borland* (1st Cir. 1995; affirmed 4-4, 516 U.S. 233 (1996)) treats a menu hierarchy as a method of operation; persuasive only in India. s.52(1)(ac) allows studying a program to learn its ideas. Users' muscle memory is the strongest legitimate reason to keep the shortcuts. Mitigations: offer a "Tally-style keymap" as one optional preset described functionally; keep our own default keymap; do not copy menu wording or ordering wholesale [lawyer]. |
| L5 | **Breach of the licence by contributors who reverse engineer** file formats or the binary while holding a Tally licence. The Tally T&C bars reverse engineering and decompilation [secondary]. s.52(1)(ab) permits acts needed for interoperability by a "lawful possessor", but there is no case law and its interaction with contract terms is unclear [secondary: SpicyIP]. | **Medium-High** | Interoperate only through documented routes: Tally's published XML/HTTP integration interface [knowledge] and the export formats users produce. Treat Tally's binary data files (`.900`/`.1800` company data [knowledge]) as out of scope unless counsel approves an s.52(1)(ab) approach [lawyer]. Keep a written clean-room log of which public documents each importer was built from. |
| L6 | **Misleading claims** such as "100% Tally compatible" or "drop-in replacement" (passing off; consumer protection). | **Medium** | Use precise, testable claims ("imports masters and vouchers from TallyPrime XML export; see the compatibility matrix"). |
| L7 | **Companies Act audit-trail non-compliance by users**: a company using our app with the log disabled or editable gets a qualified audit report under Rule 11(g). This is reputational for us. | **Medium** | Edit log always on for company books (no setting to turn it off); hash-chained and verifiable; an auditor export. See the compliance section. |
| L8 | **DPDP liability once we run cloud sync or AI relay** (processor duties, breach-notice contract terms). Penalties under the Act reach up to Rs 250 crore for security failures [knowledge]. | **Medium** (High once a hosted service exists) | Local-first default. End-to-end encrypted sync (we cannot read the data). A data processing agreement template. Breach runbook. India-region hosting. |
| L9 | **Data localisation for company books (Rule 3(5))**: daily backup on servers physically in India. | **Medium** | Built-in daily backup scheduler. Any hosted sync or backup offered by the project runs in an Indian region. Document how users can meet Rule 3(5) with local disks or NAS in India. |
| L10 | **E-invoice/e-way bill integration**: shipping shared GSP or API credentials in open source, or the project becoming an unlicensed intermediary. | **Medium** (future) | Users bring their own GSP/IRP credentials (bring-your-own-key, BYOK). If the project ever runs a relay, it becomes a processor and must contract with a licensed GSP. See the compliance section. |
| L11 | **Model or OCR licence contamination**: bundling weights with non-commercial or revenue-capped licences (for example Surya weights: free only for research, personal use and startups under US$5M; modified OpenRAIL-M) [primary: Surya GitHub]. | **Medium** | Bundle only weights under OSI-compatible licences (Apache-2.0: Tesseract tessdata [primary], Qwen3 [primary], Sarvam-30B/105B [secondary]). Make other models user-downloaded plugins with a licence notice. Use `cargo deny` plus a model-licence manifest. |
| L12 | **Professional liability from AI output** (a wrong GST classification, a wrong notice reply). | **Medium** | AI never posts or files; it drafts, a human confirms, and the engine computes. Clear disclaimers. Keep an AI provenance log in the audit trail. |
| L13 | **Use of Tally marks or screenshots in our docs, screenshots or marketing.** | Low | Brand review checklist before each release. |

---

## Licence recommendation with reasoning

**Recommendation: AGPL-3.0-or-later for the application, engine and any server components;
Apache-2.0 (or MIT/Apache dual) for small interchange crates; DCO rather than a CLA, unless the
team decides now that it wants dual-licensing; plus a written trademark policy.**

Reasoning:

1. **What we are protecting against.** The realistic threat is not someone redistributing the
   desktop binary; any licence allows that. It is a well-funded company taking the engine,
   hosting it as "cloud accounting" and contributing nothing back. Only the AGPL's network clause
   (s.13) covers that case. For the desktop app alone, GPL-3.0 and AGPL-3.0 behave almost the
   same, so AGPL costs desktop users nothing.
2. **Permissive (MIT/Apache) for everything** maximises adoption but invites exactly that
   proprietary SaaS fork, and Indian accounting SaaS is a crowded, well-funded market (Zoho,
   Vyapar, ClearTax). It also weakens any later open-core or hosted-service revenue.
3. **Precedent in this space.**
   - ERPNext is GPLv3 [secondary: Frappe docs, via search summary]. It sustains itself through hosting (Frappe Cloud),
     support and implementation, and protects its name with a trademark policy: no "ERPNext" in
     product, company or domain names without permission (April 2021) [primary: Frappe blog].
   - Odoo moved its Community edition from AGPLv3 to LGPLv3 in v9 and keeps a proprietary
     Enterprise layer [secondary]. That is an open-core model, and it requires owning the
     copyright, usually through a CLA.
4. **Commercial sustainability options under AGPL:**
   - paid hosted sync or backup (India region; also meets Rule 3(5));
   - a paid GSP relay for e-invoicing, e-way bills and GSTR filing;
   - paid support and migration from Tally for CA firms;
   - AI cloud credits, mirroring Tally's metered Docs by Ira;
   - certified builds, with signed reproducible binaries for auditors.
5. **CLA vs DCO.** This is the decision that matters most for the future, and it has to be made at
   the first outside contribution.
   - A CLA (assigning or licensing copyright to a foundation or company) keeps the option to
     dual-license (AGPL plus commercial) or to relicense later. It deters some contributors.
   - DCO keeps the community-friendly route but fixes the licence permanently.
   - Our recommendation is DCO, unless dual-licensing is a planned revenue line. In that case,
     use a CLA held by a non-profit entity [lawyer].
6. **Split licensing.** Apache-2.0 for crates others should embed without friction: the Tally
   XML import parser, GST schema types, the e-invoice JSON schema, and the hash-chain verifier.
   An auditor's tool can then verify our audit logs without taking on AGPL obligations.
7. **Trademark policy is separate from the copyright licence.** Neither GPL nor AGPL grants
   trademark rights. Register our own product name and publish a Frappe-style policy: forks must
   rename, and services may describe themselves as "X consulting" but not use the name as their
   own brand.
8. **Licence compatibility checks.** `cargo deny` must allow only AGPL-compatible dependency
   licences (Apache-2.0, MIT, BSD, MPL-2.0, LGPL, GPL-3.0). Model weights are not covered by
   `cargo deny`; keep a `models.toml` manifest with each model's licence and reject non-OSI weight
   licences (custom terms such as Gemma's, Llama community licences [knowledge], and RAIL-M
   variants) from the default bundle.

---

## Compliance obligations (audit trail, DPDP) with design implications

### A. Companies Act audit trail (Rule 3(1), Companies (Accounts) Rules, 2014; Rule 11(g), Audit Rules)

What the law says [secondary: ICAI CA Journal, Taxguru, Taxmann; MCA text not opened]:

- **Who is bound:** every **company** maintaining electronic books. The rule does not bind
  proprietorships, partnerships or LLPs. LLPs have a separate rule; check whether an analogous LLP
  amendment exists [unconfirmed]. The obligation falls on the company and its management, not on
  the software vendor.
- **What is required:**
  - a feature that records an **audit trail of each and every transaction**;
  - an **edit log of each change** with the date of the change;
  - a feature that **cannot be disabled**.
- **Retention:** audit trail preserved for at least 8 financial years under s.128(5) [secondary].
- **Scope:** changes to **books of account**. Purely master or configuration changes may be
  outside scope, but ICAI guidance is cautious about this [secondary: ICAI CA Journal].
- **Auditor:** reports under Rule 11(g) whether the trail operated throughout the year for all
  transactions, was not tampered with, and was preserved. The ICAI *Implementation Guide on
  Reporting on Audit Trail under Rule 11(g)* (Revised 2024 edition, Feb 2024) is the key reference
  [secondary]. Whether it expects vendor confirmations or SOC reports is unconfirmed; read the full
  guide.
- **Manual audit trails do not comply.** The feature must be built into the software
  [secondary: Taxmann FAQ].
- **Rule 3(5):** books must be accessible in India at all times, and a **daily backup** must be
  kept on servers physically located in India (Fourth Amendment Rules, 5 Aug 2022) [secondary].
  Auditors have flagged companies with Indian servers but no daily backup [secondary].
- **Vendor precedent:** TallyPrime ships a separate "TallyPrime Edit Log" product line or release
  alongside TallyPrime (release-note titles: "TallyPrime and TallyPrime Edit Log Release 7.1")
  [secondary: search-result titles of Tally help pages].

Design implications:

1. **Append-only, hash-chained event log as the source of truth.** Every create, alter, delete,
   cancel or optional-to-regular conversion of a voucher, and every change to a master that
   affects the books, writes an immutable event containing:
   - user;
   - timestamp, using both a monotonic counter and wall-clock time;
   - machine id;
   - before and after state (or a diff);
   - reason (optional);
   - `prev_hash`.
   "Deletes" are tombstones.
2. **The log cannot be disabled when the company type is "Company"** (Companies Act).
   - Other entity types may get a toggle, but default it on for everyone. Tally offers the
     choice; we should default to on.
   - There is no setting, no CLI flag and no config key that turns it off for companies.
3. **Tamper evidence instead of tamper-proofing.**
   - Open source means a user can rebuild the binary or edit the SQLite file. The honest guarantee
     is *detectability*.
   - Use a hash chain, plus periodic signed checkpoints: the user's key, plus an optional external
     anchor such as emailing a checkpoint hash to the auditor or an optional timestamping service.
   - Provide an **independent verifier** (Apache-2.0 CLI) that auditors run on their own machines.
4. **Auditor mode and export:** a read-only auditor role; an edit-log report filtered by period,
   voucher type and user; export in an open, documented format (CSV/JSON plus a hash manifest).
5. **Reproducible, signed release builds**, so an auditor can confirm the binary matches the
   tagged source. Whether ICAI or auditors accept "verified OSS build plus hash-chain verifier" as
   evidence is an open question.
6. **Daily backup scheduler** (on by default), with a target location setting and an "India
   location" attestation field. Any project-hosted sync or backup is hosted in India.
7. **8-year retention:** the log is never pruned automatically. Archiving older years moves them
   to a sealed, verifiable archive rather than deleting them.
8. **AI actions are audit events too.** Record AI-drafted fields with model id, version and
   source-document hash, then the human acceptance event.

### B. DPDP Act 2023 and DPDP Rules 2025

Status [secondary unless noted]:

- The Rules were notified 13-14 Nov 2025 (G.S.R. in the gazette). Rules 1, 2 and 17-21 (Board
  setup) apply immediately. Rule 4 (Consent Managers) applies after 12 months, around Nov 2026.
- **Rules 3, 5-16, 22 and 23 (notice, security, breach, retention, rights, children,
  cross-border) apply after 18 months, around 13 May 2027** [secondary: Saga Legal, AZB; exact
  date unconfirmed from the gazette]. The IT Act SPDI Rules 2011 continue in the meantime.
- **Rule 6 security safeguards** [rule text via third-party site dpdpa.com]:
  - encryption, obfuscation or masking;
  - access control;
  - logging and monitoring;
  - backups and continuity;
  - **retention of logs and personal data for one year** (unless another law says otherwise);
  - security clauses in processor contracts;
  - organisational measures.
- **Rule 7 breach notice** [rule text via third-party site dpdpa.com]: notify affected data principals
  without delay; give the Board an initial notice without delay and a **detailed report within 72
  hours**. The duty sits with the data fiduciary, so processors need contract terms that let it
  meet the deadline [secondary].
- **Personal or domestic exemption:** the Act does not apply to individuals processing for
  personal or domestic purposes. Section numbering differs across commentaries.
- **Startup exemption under s.17(3):** a power to exempt, **not yet notified** [secondary;
  unconfirmed].
- **Cross-border transfer:** permitted except to countries the government restricts (a
  negative-list approach) [knowledge]. Sectoral localisation rules, such as Companies Act Rule
  3(5) and RBI payment-data rules, still apply on top.

How roles map onto our data paths [lawyer]:

| Data path | Who is the data fiduciary | Our project's role | Obligations that land on us |
|---|---|---|---|
| **Local-only desktop** (default) | The business using the app, for its customers', vendors' and employees' personal data (names, phones, GSTINs of proprietors, PAN, bank details, payroll) | Probably neither fiduciary nor processor: we never receive the data | None directly. Build features that help the user comply: field encryption, access control, retention and erasure tools, an access log. |
| **Optional cloud sync / backup** run by the project or a company behind it | The business | **Data processor**, and fiduciary for account and billing data about our own users | A data processing agreement; Rule 6 safeguards; breach notice to customers fast enough for their 72-hour deadline; deletion on termination; India-region hosting (also meets Rule 3(5)). End-to-end encryption greatly reduces the exposure. |
| **Cloud AI feature** (OCR or LLM via an API) | The business | Processor (if we relay) or not involved (if the user's own API key goes straight to the provider); the LLM vendor is a sub-processor | Disclose the sub-processor; cross-border transfer check; a no-training/zero-retention contract with the provider; redaction before sending. |
| **Telemetry / crash reports** | The project | **Data fiduciary** | Notice and consent; minimisation; opt-in only. |
| **Payroll module** (if built) | The employer | As above | Higher sensitivity: Aadhaar, bank accounts, salary. Encrypt at rest by default. |

Design implications:

1. **Local-first with no telemetry by default**; crash reports are opt-in and scrubbed of personal
   data.
2. **Encryption at rest** for the company file (passphrase or OS keychain), plus field-level
   encryption for PAN, Aadhaar, bank account and phone numbers.
3. **Sync is end-to-end encrypted**, so the server stores ciphertext only. This simplifies DPDP
   and CA confidentiality, and it is a selling point against cloud competitors.
4. **Data-principal tooling for the user:** export everything held about one party; mask or
   erase a party's personal fields. Statutory retention under the GST Act, Income-tax Act and
   Companies Act (8 years for books) overrides erasure, which must be explained in the UI.
5. **Per-feature, per-company AI consent,** recorded in the audit log: "Send this document to
   [provider] in [region]?", with a remember-for-this-company option.
6. **Retention settings** that respect both the DPDP one-year log minimum and the 8-year books
   requirement.
7. **Ship a DPA template, breach runbook and sub-processor list** before launching any hosted
   service.

### C. Data localisation (summary)

- **Companies Act Rule 3(5):** daily backup of books on servers in India; books accessible in
  India [secondary].
- **DPDP:** no general localisation; there is a negative list for transfers [knowledge;
  unconfirmed whether any country has been listed yet].
- **GST:** e-invoice and GSTN data passes through Indian government systems (IRP/GSTN); GSPs are
  Indian entities [knowledge].
- **RBI:** payment-system data must be stored in India (2018 circular). Relevant only if we ever
  initiate payments through connected banking [knowledge].
- **Design:** any hosted component offers an India-only region. Cloud AI defaults to providers
  with Indian regions or to local models.

### D. ICAI guidance relevant to auditors and AI

- The ICAI *Implementation Guide on Reporting on Audit Trail* (Revised 2024) is the reference
  auditors will apply to our edit log [secondary].
- **ICAI's AI Committee** runs ai.icai.org with use cases and prompt libraries, including an
  "ITC mismatch notice reply" prompt [secondary]. Suvit describes itself as ICAI-recognised
  [vendor claim]. We found **no ICAI pronouncement on putting client data into generative AI tools**
  [unconfirmed gap].
- Proxies: the ICAI Code of Ethics confidentiality duty, and ICAEW's Nov 2026 guidance that the
  confidentiality principle applies fully to AI use [secondary].
- SA 230 (audit documentation) and the Standards on Auditing on using the work of experts and on
  IT general controls (SA 315) will drive what auditors want: exportable, verifiable logs and
  documented controls [knowledge].

### E. E-invoice / GSP integration (high level)

- E-invoicing is mandatory above an aggregate annual turnover of Rs 5 crore [secondary].
- Ways to integrate with an IRP:
  - **Direct API:** reported eligibility thresholds conflict (Rs 5 cr, 10 cr or 100 cr), and they
    differ by IRP; NIC reportedly reserves direct API for large taxpayers [secondary;
    conflicting].
  - **Through a GSP:** a GSTN-licensed intermediary [secondary].
  - **Through the ERP's own GSP partner** [secondary].
- Direct integration involves sandbox testing, a test-report review and IP whitelisting
  [secondary].
- Software applications (ASPs) do not need GSTN certification themselves; they connect through a
  GSP [knowledge; unconfirmed].
- **Design:**
  - Model the e-invoice integration as a connector with BYOK: the user enters their own IRP or GSP
    credentials, stored in the OS keychain.
  - Never embed shared credentials in the open-source build.
  - A project-run relay is a commercial option, but it makes the operator a processor and
    requires a GSP contract.
  - The e-invoice JSON schema and validation run locally and deterministically.
- Pricing is covered by another researcher.

---

## AI feature inventory

Demand-evidence key:

- **S:** survey data.
- **V:** vendor investment (shipping it implies demand).
- **P:** practitioner commentary.

| Feature | Who ships it today | User demand evidence | Our approach | Priority |
|---|---|---|---|---|
| **Bill/invoice OCR to draft voucher** | TallyPrime 7.1 Docs by Ira: English only, no handwriting, draft vouchers, duplicate and total-mismatch flags, 100 free pages then paid [primary: Tally help]; Suvit / Vyapar TaxOne [secondary]; Zoho AI custom fields with OCR [primary: Zoho]; ClearTax [vendor claim] | V (all major vendors); S (ICAI survey: data entry and document analysis are the top AI uses); Tally claims up to 80% less manual effort [vendor claim] | Local OCR (PaddleOCR PP-OCRv5 or Tesseract) plus layout heuristics, with an optional small local LLM for field mapping. Output is a **draft** with per-field confidence and bounding-box provenance. The engine recomputes tax and totals and validates the GSTIN checksum, HSN and rate × taxable value. The user confirms. Optional cloud vision model via BYOK. | **P0** |
| **Bank statement import and categorisation** | Zoho AI bank-statement categorisation (Early Access) and the reconciliation agent [primary: Zoho]; Suvit [vendor claim]; Docs by Ira now reportedly handles PDF bank statements [secondary; conflicting with older FAQ]; TallyPrime Connected Banking (ICICI etc.) [secondary] | V; P | Deterministic parsers for common bank CSV/XLS/PDF layouts first. Learned ledger suggestions from the company's own history using local rules and a statistical model (no LLM needed). LLM fallback for narration parsing only. Suggestions are shown with a reason ("matched 14 earlier UPI entries to 'Swiggy' → Staff Welfare"). | **P0** |
| **GSTR-2B / purchase reconciliation** | ClearTax (fuzzy matching; claims 2B matching in under a minute and 95-99% accuracy) [vendor claim]; Suvit [vendor claim]; Tally (built-in GSTR-2B reconciliation, non-AI) [knowledge] | V; P (3B vs 2B mismatches are the main trigger for ASMT-10 notices [secondary]) | **Deterministic matching engine** (exact match, then normalised invoice number, then tolerance rules, all explainable). AI is used only to *suggest* probable matches for leftovers, with a stated reason; the user accepts. Numbers always come from the engine. | **P0** (engine) / P1 (AI assist) |
| **Natural-language entry** ("paid 5,000 rent to Sharma by UPI yesterday") | Zoho CoCreate (creates invoices and documents from prompts, preview before saving) [primary: Zoho]; Ask Zia actions [primary: Zoho] | V; P | LLM converts text into a **typed voucher proposal** (JSON matching our voucher schema) shown in the normal voucher screen. Amounts typed by the user are copied verbatim and highlighted. The engine computes GST and totals. Works with a local model. Hindi/Hinglish input supported via a multilingual model. | P1 |
| **Q&A over books** ("Which customers are 90+ days overdue?") | Ask Zia (grounded in the organisation's data, can take actions) [primary: Zoho]; Zia Summary [primary: Zoho]; TallyIra roadmap mentions conversational queries [secondary: AIM] | V; S (ACCA: 93% wary of AI insights, so it must be verifiable) | The LLM writes a **query in our report DSL**, not an answer. The engine runs it and renders a normal, drillable report. The LLM may add prose but **cannot include digits** that did not come from the engine (enforced by a post-filter). The query is shown to the user and saved as a report. | P1 |
| **Anomaly detection** (duplicate bills, unusual amounts, weekend postings, round-figure cash, GST rate mismatches, missing invoice numbers in a series) | Zoho Reports AI anomaly detection [primary: Zoho]; Tally Docs by Ira duplicate flags [primary]; tallyatcloud blog claims of AI error detection in 7.0 [unconfirmed; third-party marketing] | P; auditors (audit-trail and fraud scenarios in the ICAI guide [secondary]) | Mostly **deterministic rules** (duplicates, sequence gaps, GST rate vs HSN master, cash receipts at or above Rs 2 lakh (s.269ST of the 1961 Act; check the Income-tax Act 2025 numbering) [knowledge], TDS thresholds) plus simple statistics (z-scores per ledger). Every flag names its rule. No LLM required. | **P0** (rules) |
| **GST/IT notice drafting** | ClearTax notice tracker with AI root-cause analysis [vendor claim]; ICAI AI prompt library includes an ITC-mismatch reply prompt [secondary] | P (ASMT-10/11 volume; 30-day reply window [secondary]) | The LLM drafts **prose only**. Every figure in the draft is a placeholder bound to an engine query (`{{gstr2b_itc_diff:2025-26}}`) and filled from the books. A CA reviews it. Clearly labelled as a draft, never filed automatically. Cloud LLM likely needed for quality; opt-in with redaction. | P2 |
| **Payment and receivable nudges / email drafting** | Zoho invoicing agent and Email with Zia [primary: Zoho] | V | Template-based first; LLM tone rewrite optional. | P3 |
| **HSN/SAC and ledger classification suggestions** | Commonly claimed by OCR vendors [unconfirmed] | P | Local lookup against the official HSN master plus a learned mapping from the item to the user's own past choices. | P1 |
| **Indian-language documents** (Hindi/regional invoices, Hinglish narrations) | Docs by Ira: **English only, no multilingual, no handwriting** [primary: Tally help] | Gap in competitors; Indian SME base | PP-OCRv5 has Devanagari (reported 84.96% accuracy) and Tamil (reported 94.2%) recognition models [secondary; vendor-reported figures, via search summary of PaddleOCR docs]. Tesseract tessdata_best (Apache-2.0) has hin, ben, guj, kan, mal, mar, ori, pan, asm and nep (tam/tel not confirmed in the visible listing) [primary: GitHub]. Sarvam-30B/105B (Apache-2.0, 22 scheduled languages, Feb 2026) are too large for typical desktops but suit an optional server [secondary]. Qwen3-4B (Apache-2.0, 100+ languages) for local mapping [primary: HF]. | P2 (differentiator) |
| **Accounting LLM / tax research assistant** | Tally mentions a possible future accounting LLM [secondary: AIM] | P | Out of scope for v1. Answer with citations to statute text only, never compute. | P3 |

Competitor notes:

- **TallyPrime 7.0** launched 19 Dec 2025 [secondary]; its focus was backup, migration and
  privacy, not AI. Third-party "7.0 AI" articles are speculative.
- **TallyPrime 7.1** dates conflict: the release-notes page shows 20 May 2026 [primary], the
  product-update page shows 27 June 2026 [secondary], and AIM called it beta [secondary]. Docs by
  Ira requires an Indian licence with active TSS (Tally Software Services) [primary/secondary].
  The Docs by Ira FAQ does **not disclose where documents are processed, which AI provider is
  used, retention, or training use**. It points to "terms of use for AI-powered features", which
  we could not fetch (403). Its accuracy description is an F1-style example (about 95 of 100
  fields extracted, about 90 correct), not a guarantee [primary].
- **Zoho Books** has nine AI features, live or in early access, as of an April 2026 update. The
  post says nothing about which LLM is used or training on customer data [primary].
- **Vyapar** acquired Suvit on about 27-28 Nov 2025; Suvit rebranded to Vyapar TaxOne. Suvit
  claims more than 10,000 firms and 30,000 accountants use it [vendor claim]. Suvit is cloud-only,
  according to reviews [secondary].
- **ClearTax**: notice tracking and GSTR-2B matching; accuracy figures come from its own study
  [vendor claim].
- **Giddh**: no specific AI features found [unconfirmed].

---

## AI safety/trust architecture recommendation

**Principle: the AI proposes structured intents; the deterministic engine validates, computes and
displays; a human commits. No number the AI generates is ever displayed as a fact or posted.**

The one unavoidable exception is extraction. OCR has to read an amount off a document. We handle
it as *transcription with provenance*, not as computation.

```
 document / text / question
          │
          ▼
 ┌─────────────────────┐   untrusted zone (local model or opt-in cloud)
 │  AI "proposer"      │──► typed proposal (JSON Schema): VoucherDraft | MatchSuggestion |
 └─────────────────────┘    ReportQuery | DraftText-with-placeholders
          │  (no direct DB access; tools are read-only queries + "propose")
          ▼
 ┌─────────────────────┐   trusted zone (Rust engine, no AI)
 │  Validator/engine   │──► schema check → master resolution (ledger/party/GSTIN exist?)
 │                     │    → recompute tax, totals, round-off from rates in masters
 │                     │    → cross-checks: line sum = total, rate×taxable = tax,
 │                     │      GSTIN checksum + state code vs place of supply, HSN valid,
 │                     │      duplicate (party+inv no+date), period lock
 └─────────────────────┘
          │ results + discrepancies
          ▼
 ┌─────────────────────┐
 │  Review UI          │──► every AI-sourced field visibly marked; click → source crop
 │  (human commits)    │    mismatches block "Accept" until resolved
 └─────────────────────┘
          │ accept
          ▼
   posted voucher + audit events (ai_proposed{model,version,doc_hash,fields},
                                  human_accepted{user, edits})
```

Rules to enforce in code, as candidate engineering rules:

1. **Typed proposals only.** The AI interface returns enums and structs defined by the engine
   (`VoucherDraft`, `ReportQuery`, and so on). No free-form text reaches the books. Parse with
   strict schema validation and reject anything that does not parse.
2. **Engine recomputes all derived numbers.** Totals, tax, round-off, balances and report figures
   come from the engine. If an AI-extracted total differs from the engine's sum of lines, flag it
   to the user; never silently "fix" either side. This matches Docs by Ira recalculating totals
   on save [primary].
3. **Extracted amounts are transcriptions.** Each has a source reference (document hash, page,
   bounding box) and a confidence score. Fields below a threshold are left blank, not guessed.
   The UI marks AI-filled fields until a human has seen them.
4. **The Q&A path never shows LLM digits.** The LLM produces a `ReportQuery`. Any narrative it
   writes passes a post-filter that removes or rejects numerals not bound to engine results; or
   narration is template-only. Show the query ("I ran: outstanding receivables > 90 days as of 20
   Nov 2026") so users can check it.
5. **Draft text uses bound placeholders.** Notice replies and emails contain `{{query}}` slots
   that the engine fills in. A literal number written by the AI is a validation error.
6. **No autonomous posting, filing or sending.** AI cannot call any mutating tool except
   `propose`. Filing (GSTR, IRP) needs an explicit human action outside the AI flow.
7. **Every AI action is logged** in the audit trail (model, version, prompt template version,
   input hash, output, acceptance or edits). This gives auditors full traceability and is
   consistent with Rule 11(g) expectations.
8. **Evaluation and provenance.** Maintain a labelled corpus of synthetic and consented real
   Indian invoices and bank statements, with expected values from hand labelling, never from our
   own engine's output. We recommend adopting this as a project rule. Publish field-level precision
   and recall per document type and language. Do not ship a feature below an agreed threshold.
9. **Privacy tiers** (per company, visible in the status bar):
   - **Tier 0, Off.**
   - **Tier 1, Local (default when AI is enabled):** OCR plus a small local LLM, run by our
     process; nothing leaves the machine.
   - **Tier 2, Cloud with BYOK:** the user's own API key and provider; requests go directly from
     the app to the provider, with redaction of PAN, Aadhaar, bank account numbers and phone
     numbers where the task does not need them. A per-document confirmation is available.
   - **Tier 3, Project-hosted relay** (future, paid): India region, zero-retention and no-training
     contracts, DPA.
10. **Local runtime options** (to be confirmed by the AI/infra researcher; not benchmarked here):
    - OCR: PaddleOCR PP-OCRv5 via ONNX Runtime, or Tesseract (Apache-2.0, `tessdata_best` Indic
      models).
    - LLM: a small Apache-2.0 instruct model in the 3-8B class (for example Qwen3-4B, Apache-2.0,
      100+ languages [primary: HF]) via llama.cpp/GGUF or candle, with grammar-constrained JSON
      decoding so the output always parses.
    - Larger Indic models (Sarvam-30B, Apache-2.0) only on optional servers.
    - Avoid bundling Surya weights (revenue-capped licence) [primary].

Why this matters to users: CA and SME trust evidence is weak and cautious (ICAI survey; ACCA 93%
wary; CFOs 14% full trust). The things that earn trust are clickable provenance, deterministic
recomputation, and the fact that nothing is posted without the user.

---

## Open questions

1. **[lawyer]** Product name clearance (TM classes 9 and 42), and whether TALLY is a declared
   well-known mark. Also whether Tally Solutions holds registered GUI designs, following the
   Calcutta HC ruling of 9 Mar 2026.
2. **[lawyer]** Can contributors rely on Copyright Act s.52(1)(ab) to document Tally's
   binary/data formats despite EULA anti-reverse-engineering clauses? Until answered, use the
   XML/HTTP interface and user exports only.
3. **[lawyer]** Exact scope of Trade Marks Act s.30 for compatibility statements and "Tally-style
   keymap" wording. Not verified this session (search budget exhausted).
4. **[lawyer] / ICAI** Will statutory auditors accept a hash-chained, open-source edit log with
   reproducible builds as meeting "cannot be disabled", given users can recompile? Should we seek
   an ICAI or Digital Accounting and Assurance Board opinion, or a third-party assurance report
   (SOC-style)?
5. Does the ICAI Rule 11(g) Implementation Guide (2024) expect vendor confirmations or SOC
   reports? Read the full PDF.
6. Does an LLP equivalent of the audit-trail rule apply? Is a further MCA amendment pending?
7. **[lawyer]** DPDP role analysis for E2E-encrypted sync: is a processor holding only ciphertext
   still subject to the full Rule 6 obligations? Confirm the 13 May 2027 commencement date from
   the gazette, and the Rule 8 / Third Schedule erasure classes, which we did not read.
8. Have any countries been restricted for transfer under DPDP s.16? Which cloud LLM providers
   offer India-region inference with zero data retention?
9. E-invoice: current, official eligibility for direct IRP API access by turnover and IRP. The
   sources conflict (Rs 5 cr, 10 cr, 100 cr).
10. CLA vs DCO: does the team intend dual licensing or open core? This must be decided before the
    first external PR.
11. Tally's terms for AI-powered features (403 on fetch): where Docs by Ira processes data. This
    is useful competitive positioning if it is offshore or third-party.
12. Gemma and Llama weight licence terms were not re-verified this session. Confirm before
    excluding or including them.
13. Primary-source demand evidence from Indian CAs on AI features is thin. Recommend our own user
    interviews with about 15 CA firms and 15 SMEs, ranking OCR, bank categorisation, 2B
    reconciliation, Q&A and notice drafting, and asking about cloud-AI willingness.
14. Tamil and Telugu availability in Tesseract `tessdata_best` (not visible in the truncated
    listing). Real-world PP-OCRv5 accuracy on Indian GST invoices.

---

## Sources

Legal: IP and trademark

- Lotus v. Borland summary (516 U.S. 233 (1996), 4-4 affirmance): https://en.wikipedia.org/wiki/Lotus_Development_Corp._v._Borland_International,_Inc.
- Google v. Oracle summary (593 U.S. 1 (2021), fair use): https://en.wikipedia.org/wiki/Google_LLC_v._Oracle_America,_Inc.
- SpicyIP: decompilation under the Indian Copyright Act: https://spicyip.com/2013/01/part-i-is-decompilation-of-software.html ; https://spicyip.com/2013/01/part-ii-is-decompilation-of-software.html ; https://spicyip.com/2009/02/decompilation-defence-under-indian.html
- Reverse engineering and India's Copyright Act (law.asia): https://law.asia/reverse-engineering-and-indias-copyright-act/
- Copyright Act 1957 (India Code): https://www.indiacode.nic.in/bitstream/123456789/1367/1/a195714.pdf ; Copyright Office exceptions: https://www.copyright.gov.in/Exceptions.aspx
- WIPR, "India reimagines GUI design protection" (Calcutta HC, 9 Mar 2026): https://www.worldipreview.com/jurisdiction-reports/india-reimagines-gui-design-protection
- Dassault v. Spartan case brief: https://www.thelegallock.com/resources/case-briefs/case-brief-spartan-engineering-industries-pvt-ltd-anr-vs-dassault-system-solidworks-corporation-anr
- Tally terms and conditions (reverse-engineering clauses; search snippets): https://tallysolutions.com/global/terms-and-conditions/ ; https://tallysolutions.com/terms-conditions/
- Frappe trademark/IP policy (13 Apr 2021): https://frappe.io/blog/legal/protection-of-our-intellectual-property
- ERPNext licence: https://docs.frappe.io/erpnext/user/manual/en/open-source
- Odoo AGPL to LGPL (v9): https://www.odoo.com/hi_IN/forum/help-1/why-is-odoo-changing-their-license-from-agplv3-to-lgplv3-for-version-90-75453

Legal: Companies Act and audit trail

- ICAI CA Journal, Audit Trail requirements: https://cajournal.icai.org/article-details/audit-trail-requirements-responsibilities
- Taxmann FAQs on auditor reporting: https://www.taxmann.com/post/blog/faqs-statutory-auditors-duty-to-report-audit-trails
- Taxguru practitioner guide: https://taxguru.in/company-law/audit-trail-compliance-rule-3-1-rule-11-practitioners-guide.html
- ICAI Implementation Guide (Revised 2024) PDF: https://www.eirc-icai.org/uploads/background_materials/Revised%202024_Implementation%20Guide%20on%20Reporting%20of%20Audit%20Trail%20(1)_1712114860.pdf
- Rule 3(5) daily India backup: https://www.grantthornton.in/insights/blogs/recent-amendments-by-mca-in-keeping-of-books-of-accounts ; https://taxguru.in/company-law/companies-daily-backup-books-account-mca.html ; https://www.taxmann.com/post/blog/auditors-observations-on-common-non-compliances-in-electronically-maintained-books-of-account
- Tally release-note title referencing Edit Log: https://help.tallysolutions.com/release-notes-for-tallyprime-and-tallyprime-edit-log-release-7-1-hindi/

Legal: DPDP

- PIB DPDP Rules 2025 document: https://static.pib.gov.in/WriteReadData/specificdocs/documents/2025/nov/doc20251117695301.pdf
- Saga Legal (Legal 500), 18-month countdown: https://www.legal500.com/firms/237811-saga-legal/c-india/news-and-developments/the-privacy-timer-starts-now-analysing-the-dpdp-rules-and-its-18-month-countdown
- AZB: https://www.azbpartners.com/?p=87811
- Rule 6 text: https://dpdpa.com/dpdparules/rule6.html ; Rule 7 text: https://dpdpa.com/dpdparules/rule7.html
- KSK on breach notice and processors: https://ksandk.com/data-protection-and-data-privacy/dpdp-breach-notification-rules-and-modern-incident-response/
- Taxmann exemptions: https://www.taxmann.com/post/blog/exemptions-under-dpdp-act ; s.17: https://www.dpdpa.com/dpdpa2023/chapter-4/section17.html

E-invoice

- ClearTax integration modes: https://cleartax.in/s/e-invoicing-api-integration-modes ; https://cleartax.in/s/gst-e-invoicing-api
- Taxguru summary of GSTN API guidance: https://taxguru.in/?p=893916

AI: competitors

- Tally Docs by Ira FAQ: https://help.tallysolutions.com/docs-by-ira-faq/
- TallyPrime 7.1 release notes: https://help.tallysolutions.com/release-notes-tallyprime-7-1/
- Tally product update 7.1 (403 on fetch; search snippet): https://tallysolutions.com/darpan/product-update-tallyprime-7-1/
- TallyPrime 7.0 product update (snippet): https://tallysolutions.com/darpan/product-update-tallyprime-7-0/
- Analytics India Magazine on Tally AI: https://analyticsindiamag.com/ai-features/selling-ai-tools-to-sceptic-smes-is-tricky-but-tally-solutions-knows-a-way/
- Zoho finance AI features (Dec 2025, updated Apr 2026): https://www.zoho.com/blog/general/zoho-finance-ai-features.html
- Zoho Books banking update: https://www.zoho.com/blog/index.php/books/banking-enhancements.html
- Vyapar acquires Suvit: https://inc42.com/buzz/vyapar-acquires-suvit-to-boost-ai-led-accounting-automation-for-msmes ; https://india.entrepreneur.com/news-and-trends/vyapar-acquires-suvit-to-boost-digital-compliance-for-msmes/500200 ; rebrand: https://www.socialsamosa.com/industry-updates/suvit-rebrands-vyapar-taxone-10924728
- ClearTax AI: https://cleartax.in/s/ai-itc-matching ; https://cleartax.com/in/indirect-tax-compliance ; https://cleartax.in/s/ai-invoice-reconciliation

AI: trust and demand

- ICAI AI survey: https://ai.icai.org/articles_details.php?id=20
- ICAEW on AI and confidentiality (Nov 2026): https://www.icaew.com/regulation/regulatory-news/regulatory-news-2026-11/ai-and-client-data-complying-with-the-principle-of-confidentiality
- ACCA/CA ANZ, 93% question AI insights: https://www.tribuneindia.com/news/business/ai-adoption-rises-but-93-of-finance-professionals-question-ai-generated-insights-report/
- CFO trust gap: https://www.cfodive.com/news/massive-trust-gap-hinders-cfo-ai-ambitions-study-finds/810786/
- ICAS on human judgement: https://icas.com/news-insights-events/news/press-release/ai-can-t-replace-human-judgement-in-accounting-icas-study-shows

AI: models and OCR

- PaddleOCR PP-OCRv5 multilingual: https://www.paddleocr.ai/latest/en/version3.x/algorithm/PP-OCRv5/PP-OCRv5_multi_languages.html
- Tesseract tessdata_best (Apache-2.0): https://github.com/tesseract-ocr/tessdata_best
- Surya OCR (code Apache-2.0; weights modified OpenRAIL-M, revenue cap): https://github.com/datalab-to/surya
- Qwen3-4B model card (Apache-2.0): https://huggingface.co/Qwen/Qwen3-4B
- Sarvam 30B/105B under Apache 2.0: https://www.opensourceforu.com/?p=97113 ; https://www.businesstoday.in/tech-today/story/sarvam-ai-launches-30b-and-105b-open-source-models-516729-2026-02-18
