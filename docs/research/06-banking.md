# 06 - Banking and payments

Research date: 2026-11-20. Reference product: TallyPrime 7.x. Web research only, clean-room. Every
fact below is summarized in our own words. Statements that come only from Tally/vendor marketing are
marked **[vendor claim]**. Statements we could not confirm are marked **[unconfirmed]**.

## Summary

- **Bank reconciliation is the core of this area.** In TallyPrime every bank-side voucher line carries
  instrument data (transaction type, instrument number and date, and a "bank date"). Reconciliation
  means filling in the bank date. That can be done by hand, by importing a statement (Excel, CSV or
  MT940, with a per-bank format profile and **no PDF**), or by a direct bank feed (Connected Banking).
  The match types are Exact, Potential, Partial and manual "Find Match". Statement lines that are not
  in the books can be turned into vouchers.
- **Direct bank connectivity in Tally is narrow and runs through partnerships.** Connected Banking
  (statements and balances, from TallyPrime 6.0) covers Axis, SBI, ICICI and Kotak. Connected Payments
  (payment initiation from the app, from 7.0) covers Axis and SBI, with Kotak shown as live in one
  place and "coming soon" in another. Both need a TSS subscription and a Tally.NET ID, and the user
  approves the link on the bank's corporate portal. Tally's bank counts for statement-format support
  disagree from page to page (145+, 200+, 250+, 450+).
- **File-based e-payments still matter more than APIs.** Tally writes a bank-specific bulk payment
  file (NEFT/RTGS/IMPS/same-bank transfer), the user uploads it to the bank portal, and a "reverse
  file" is imported later to set each payment's status. About 16 to 18 banks are supported, and the
  user must ask the bank to enable bulk upload for the account.
- **Cheques have a full lifecycle**: cheque books with number ranges in the bank ledger, a cheque
  register with statuses, configurable cheque printing, blank cheques, post-dated vouchers that post
  on the cheque date, and deposit slips. Bounced cheques are handled by convention (a reversal entry),
  and we found no dedicated workflow.
- **Collections**: Tally can generate payment links and QR codes (Razorpay, PayU, or a plain UPI ID)
  for single or bulk invoices, track bills one by one, show ageing by bill date or due date, calculate
  interest (simple or compound) on overdue amounts, set credit limits with an overdue alert at
  invoice time, and send reminder letters by email or WhatsApp. WhatsApp goes through the official
  Business API and arrived in Release 4.0 or later **[vendor claim]**.
- **Account Aggregator (AA) is not a direct path for an open-source app.** Only entities regulated
  by RBI, SEBI, IRDAI or PFRDA can be a Financial Information User (FIU). Coverage of business current
  accounts is limited to sole-proprietor accounts at some banks. Partnership and company accounts are
  not supported by the current schema (Setu, 2023), and we found no evidence that this has changed.
  Fetches cost money (vendor quotes range from about Rs 2.5 to Rs 30 per fetch). GSTN is a provider
  (GSTR-1 and GSTR-3B), but only for the credit use case.
- **Recommendation**: the MVP should use offline, file-based statement import across many formats,
  including PDF via local text extraction, plus a strong matching engine with user-defined rules and
  AI suggestions. Add UPI QR on invoices with no integration at all (NPCI `upi://pay` URI), cheques,
  and bulk payment file export with a pluggable per-bank format registry. Leave AA and bank APIs to
  an optional plugin run by a regulated partner.

## Feature inventory

| Feature | What it does | How the user interacts (in TallyPrime) | Priority |
|---|---|---|---|
| Bank ledger setup | A ledger under Bank Accounts / Bank OD with account number, IFSC, branch, holder name, opening balance, reconciliation beginning date, cheque-book ranges, cheque printing config, e-payment and auto-recon flags | Ledger create/alter screen with bank-detail sub-screens | MVP |
| Bank allocation details per voucher line | Stores the transaction type (cheque, DD, e-fund transfer, inter-bank, same-bank, others), instrument number and date, favouring name, and payee bank and IFSC | Sub-screen that opens on the bank line of payment, receipt and contra vouchers | MVP |
| Manual BRS | User enters the bank date per transaction. Report shows balance per books, unreconciled items and expected bank balance | Bank Reconciliation report: type bank dates, set status | MVP |
| Opening BRS | Records items left unreconciled from before the reconciliation start date | Entered from the BRS report | MVP |
| Statement import | Imports a bank statement file with a per-bank format profile. No PDF, and no Excel/CSV converted from PDF. Duplicate re-import is blocked | Import menu, then bank statement: pick ledger, file and type, preview, accept | MVP |
| Auto-reconciliation | Matches statement lines to book entries by instrument number, date and amount. Results are grouped as Exact, Potential and Partial, with manual Find Match. User can modify a match, unlink it or set its status | In the BRS report: accept or modify suggestions | MVP |
| Create vouchers from statement | Turns unmatched bank lines (charges, interest, direct credits) into vouchers | Select lines in BRS, create voucher | MVP |
| BRS summary and banking activities reports | Counts per section, expected bank balance, books-versus-bank counts | Reports | MVP |
| Connected Banking (feeds) | Live balance and statement fetch through bank APIs. Axis, SBI, ICICI and Kotak only. Needs TSS and a Tally.NET ID with mobile OTP. Approval happens on the bank's corporate portal. Statements are fetched on request ("Get Statement") and can sit in a "pending at bank" state | Bank Connections report, then connect and approve on the bank portal | Later (plugin) |
| Connected Payments | Starts payments from the app to the bank. Maker sends with OTP and the checker approves on the bank portal. Status is tracked in the e-Payments report and failed payments can be reset. Axis and SBI, with Kotak live or "coming soon" depending on the page. TallyPrime 7.0+ | e-Payments report: send, refresh status | Later (plugin) |
| e-Payments file export | Writes a bank-specific bulk payment file from payment vouchers that carry full payee bank details. Needs a bank-side bulk-upload facility (CMS form). Reverse file import updates status | Exchange menu, Send Payments, then e-Payments report (ready, sent, failed). User uploads to the bank portal | v2 (one generic CSV in MVP) |
| Payment advice | Prints or emails a supplier advice with bill-wise breakup, deductions and TDS | Print from voucher or in bulk per ledger | MVP |
| Cheque books / ranges | Several cheque books per bank with number ranges. Next number is suggested automatically | Bank ledger, cheque book sub-screen | MVP |
| Cheque register | Every cheque with its status (issued, unused, cancelled, reconciled and so on). Drills to ranges and to BRS | Banking menu, Cheque Register | MVP |
| Cheque printing | Prints on pre-printed bank cheques with a per-bank layout. Amount in words in lakh/crore or million style. Batch printing of pending cheques | Print at voucher save, or from the Cheque Printing report | v2 |
| Blank / cancelled cheques | Records blank cheques handed out with no date or amount, and cancelled leaves | Payment voucher marked blank, or cancel action in the register | v2 |
| Post-dated cheques (PDC) | Voucher marked post-dated stays out of the books until its date, then posts automatically. Reports show PDCs received and issued | Voucher option "post-dated". Exception reports | MVP |
| Bounced / dishonoured cheque | No dedicated workflow found. Convention is a reversing voucher plus bank charges, and reopening the bill | Manual entries | MVP (guided "mark dishonoured" action, our improvement) |
| Deposit slip | Lists cheque/DD receipts for a bank deposit | Banking menu, Deposit Slip (receipts of type cheque/DD) | v2 |
| UPI QR / payment link on invoice | Link or QR per invoice, at save, or in bulk. Uses Razorpay, PayU, or a plain UPI ID. Shared by email, SMS or print, with WhatsApp "planned" or a copy-paste link. Partial payment allowed. Gateway report import updates status. India and INR only | F11 feature, then generate at save or in bulk | MVP for plain UPI QR. v2 for gateway links |
| Bill-wise outstanding and ageing | Tracks receivables and payables per bill (new ref, against ref, advance, on account). Ageing by bill date or due date with custom buckets | Outstanding reports, ageing toggle | MVP (likely covered by AR/AP research too) |
| Credit limit and credit period | Per-party limit and default credit days. Warns at invoice time if the party is over the limit or has overdue bills | Party ledger fields, warning at voucher entry | MVP |
| Interest calculation | Interest on balances or on each bill. Simple or compound, rate per period, rate slabs by date, grace days. Interest reports and interest vouchers (debit/credit notes) | F11 to enable, per-ledger parameters, interest reports | v2 |
| Reminder letters | Templated dunning letters from outstandings. Bulk send by email, or by WhatsApp via the official Business API (Release 4.0+) | Outstanding report, print/email/WhatsApp | v2 |
| Cash management | Cash ledgers, cash-in-hand and cash-flow reports, negative cash warnings, contra entries for deposits and withdrawals, denomination details **[unconfirmed for Tally]** | Contra voucher, cash reports | MVP (basic) |
| Funds flow / cash flow projection | Projection of receivables and payables by due date **[unconfirmed for Tally]** | Report | Later |

## Data-model implications

- **BankAccount**: extends a ledger of the Bank or OD group. Fields: account number (plain local
  storage, masked in UI and logs), IFSC (validate format `^[A-Z]{4}0[A-Z0-9]{6}$`), bank (a registry
  ID that picks the statement and payment formats), branch, holder name, account type (CA, SA, CC, OD),
  OD/CC limit, `recon_start_date`, and an opening BRS item list.
- **BankAllocation**: one or more per bank voucher line. Fields: `txn_type` (an enum with no catch-all:
  Cheque, DD, ECS/NACH, NEFT, RTGS, IMPS, UPI, SameBankTransfer, InterBankTransfer, Card, Cash
  deposit, Other with a required note), instrument number and date, favouring name, payee account and
  IFSC, UTR/RRN, `bank_date` (nullable, and the reconciliation fact), `recon_status`
  (Unreconciled, Reconciled, Excluded, Disputed), and a link to the matched statement line(s).
- **BankStatement** and **StatementLine** are stored as **immutable imported facts**, separate from
  the books. Statement fields: source (file hash, format, importer version, fetch channel), period,
  and the opening and closing balance as printed. Line fields: value date, posting date, description,
  cheque or reference number, debit, credit, running balance, and a raw-row JSON for audit. A hash
  identifies duplicates so a re-import is a no-op. The running balance check (opening plus the sum of
  lines must equal closing) is a hard validation that rejects the file. A bad parse must never turn
  into a wrong reconciliation.
- **Match**: a many-to-many link between statement lines and allocations, because one deposit can
  cover many receipts and bank charges can be split. Fields: match kind (exact, rule, AI-suggested,
  manual), confidence, who confirmed it and when. Matches are proposals until the user confirms them.
- **MatchRule**: user-defined rules such as "description contains 'NEFT CHG' then Bank Charges".
  They are versioned and stored with the company data.
- **ChequeBook** (bank, from and to number, issue date) and **ChequeLeaf** (number, status: Unused,
  Issued, Blank, Cancelled, Presented, Cleared, Bounced, Stopped; with voucher link). A uniqueness
  constraint on bank plus number catches duplicate cheques.
- **PostDated flag** on vouchers: the voucher is stored but excluded from balances until its date.
  Reports must show "as of" both with and without PDCs.
- **PaymentBatch** (e-payment): payments, target bank format, status per line (Ready, Exported,
  Sent, Success, Failed, Reset), exported file hash, reverse-file import.
- **PaymentRequest** on invoices: provider (UPI URI, Razorpay, PayU and so on), amount, reference
  (`tr`), link or QR payload, status, and the gateway settlement ID used to reconcile gateway
  settlements, which net out fees and GST on fees, against the bank.
- **Party credit fields**: credit limit, credit days, interest parameters (rate, basis simple or
  compound, period, grace days, applicable-from slabs), and reminder preferences (channel and
  contact).
- **Bank format registry**: data-driven (TOML/JSON) definitions of the statement columns and payment
  file layout for each bank, versioned, contributed by the community and tested against **anonymized
  sample files with expected parsed outputs**. Expected outputs need provenance in the form of a hand
  check against the bank's PDF. This is how we beat "450 banks" without a big team.
- Money is fixed-point (paise as i64, or a decimal type). Never use floats for amounts or interest.

## Statement formats & bank integration options (incl. account aggregator)

### File formats (offline, MVP)

| Format | Notes |
|---|---|
| CSV / Excel (xls, xlsx) | Most common download from Indian net-banking portals. Layout differs by bank and by account type, with header rows, merged cells, Dr/Cr in one column or two, and dd/mm/yy dates. Needs per-bank profiles plus a column-mapping wizard for unknown banks |
| MT940 (SWIFT customer statement) | Offered by corporate/CMS portals of larger banks. A structured, stable standard, so a good first-class importer. MT942 (intraday) is optional |
| ISO 20022 camt.053 / camt.052 | Not mentioned by Tally. Some corporate CMS portals may offer it **[unconfirmed for Indian banks]**. Cheap to support next to MT940 |
| PDF | Tally explicitly refuses PDF and PDF-converted files. In practice many small businesses only have PDFs (emailed e-statements, sometimes password-protected). Local text extraction with a per-bank table template, plus an AI fallback that only proposes rows, would be a differentiator. The rule: the parsed lines must reproduce the printed opening and closing balance and the running balance, or the import is rejected |
| OFX / QIF | Rare in India **[unconfirmed]**. Low priority |

### Bank-side payment files

- Each bank has its own bulk-upload layout (CSV, fixed-width or Excel). Some require encryption or
  a specific product code, and some require the ERP to be registered as the vendor on the bank side.
  Tally lists about 16 to 18 supported banks with contact steps per bank. Our design should use a
  generic payment-batch model plus a per-bank format plugin. The MVP ships a "generic CSV" export
  that the user maps.
- Reverse files (status or UTR returned by the bank) are imported to close the loop.

### Direct bank APIs

- Large banks run corporate API suites. ICICI's offers a statement API (2,000 records per call,
  paginated at 200), balance checks and payments. Eligible users include corporates, fintechs and
  ERP providers. "Connected Banking" at ICICI needs a current account, CIB credentials and a
  **Connected Banking partner platform**, meaning the software vendor must be an onboarded partner.
  A processing fee may apply **[vendor claim]**.
- Consequence: an open-source, local-first app has no central entity to sign partner agreements, hold
  API keys or receive whitelisted callbacks. Options: (a) a community or commercial "connector
  service" run by a company that signs bank partnerships, as an optional plugin; (b) users who have
  their own corporate API access (large firms) plug in their credentials locally; (c) stay with files.
  Zoho Books gets broad Indian bank feeds through an aggregator (said to be Yodlee) plus direct
  partnerships, and gates automatic feeds to paid plans **[third-party claim, unconfirmed]**.

### Account Aggregator (RBI NBFC-AA framework)

- **Roles**: the FIP (bank or other data holder) sends data to the FIU (data user) through an
  RBI-licensed AA, based on the customer's digital consent. The AA cannot read the data.
- **Eligibility**: only entities regulated by RBI, SEBI, IRDAI or PFRDA can be FIUs. An unregulated
  software company cannot be an FIU, so it must work through a regulated entity. Onboarding means
  agreements with each AA, Sahamati participation terms, KYC, five named contact persons (SPOCs), and
  certification by a Sahamati-empanelled auditor before go-live.
- **Coverage for accounting**: savings accounts are broadly live. For current accounts, only
  **sole-proprietor current accounts (including CC/OD) at some banks** are live. The FI schema did not
  support non-proprietor (partnership or company) current accounts (Setu, 2023), and as of the
  Sahamati activation table (May 2026) we found no evidence that this has changed **[unconfirmed]**.
  So AA does not cover most SMB bank accounts that need reconciliation today.
- **GSTN as FIP**: GSTR-1 and GSTR-3B data can be shared, but the stated purpose is cash-flow-based
  lending to MSMEs. Use for reconciling one's own books is not an obvious fit for the purpose code.
- **Cost**: there is no public rate card. Vendor and third-party figures range from about Rs 2.5 per
  fetch plus Rs 3 to 6 per account, up to Rs 20 to 30 per fetch, plus 18% GST **[unconfirmed]**. A
  third-party source says a standard AA pricing model is due from 1 Nov 2026 **[unconfirmed]**. Fair
  Use v2.0 and client-standard deadlines also fall around Oct to Dec 2026 **[third-party claim]**.
- **Fit for us**: AA is a later, optional plugin through a regulated partner, mainly for
  proprietorships, and it costs money for each fetch. Recurring consents (for example monthly fetches)
  exist, but fair-use rules limit frequency **[unconfirmed details]**. Watch for enterprise
  current-account FI types going live.

### UPI collection without integration

- NPCI's UPI linking spec defines `upi://pay?pa=<vpa>&pn=<name>&am=<amount>&cu=INR&tr=<ref>&tn=<note>`.
  When this URI is encoded as a QR code on an invoice, any UPI app can pay it, with no gateway, fee
  or account. The spec says the parameters of a dynamic QR (one with an amount) should not be
  editable. A static QR lets the payer type the amount. App behaviour varies across bank apps
  (developer reports).
- Limit: no automatic confirmation of payment. Status comes from statement reconciliation, where the
  `tr`/`tn` reference may appear in the narration **[unconfirmed for all banks]**. Gateway links
  (Razorpay, PayU, Cashfree and so on) give webhooks but need an internet-facing callback or polling.
  For a desktop app, polling with the user's own API keys is the local-first route.
- Check whether the B2C dynamic-QR rules for e-invoices apply (covered by the GST researcher).

## Where AI helps

1. **Statement-line to ledger matching.** Rank candidate book entries using amount, date window,
   the UTR or cheque number found in the narration, the party name in the narration (fuzzy and
   transliterated), and past behaviour. Output is always a *suggestion* with an explanation such as
   "amount equal, NEFT UTR matches the voucher reference". The user confirms. The engine never
   creates a number, only links.
2. **Narration parsing.** Indian narrations are dense (`NEFT/N123456789/ACME TRADERS/HDFC0000123`,
   `UPI/412345678901/rahul@okicici/Payment`, `ACH D- TP ACH ...EMI`, `CHQ DEP 000123`). An
   extraction step pulls out the channel, counterparty, VPA, IFSC and reference. Deterministic regex
   per bank runs first. The LLM is a fallback for unknown patterns and proposes new regex rules for
   review.
3. **Rule suggestion.** After the user categorizes similar lines a few times (for example "SMS
   charges" or a "GST payment" via the CIN in the narration), the app proposes a saved rule.
4. **Creating vouchers for unmatched lines.** Draft payment/receipt/journal vouchers (bank charges
   with GST on charges, interest credited, TDS on interest, EMIs split into principal and interest
   from a loan schedule). Drafts are shown for approval.
5. **PDF and unknown-format statement import.** Infer the column mapping, or table structure from
   PDF text. This must pass the deterministic balance checks before acceptance.
6. **Many-to-one and one-to-many matching.** Subset-sum search over open receipts for one deposit,
   or for a gateway settlement net of fees. The solver is deterministic and AI ranks the plausible
   combinations.
7. **Collections.** Draft reminder text (tone by ageing bucket, in English, Hindi or a regional
   language), pick who to chase, predict likely late payers from history. The amounts in the message
   come from the engine, not the model.
8. **Anomaly flags.** Duplicate payments, a cheque number reused, unusual payee bank details (fraud
   on vendor account change), round-tripping.

## What users love / hate

Sources here are mostly Tally's own help and blog pages plus third-party support articles. We found
no Reddit or forum threads in this pass, so the "hate" items are **[unconfirmed / inferred from
troubleshooting docs]**.

Love:
- The bank date model is simple, and manual BRS from the keyboard is fast.
- Cheque printing and PDC handling are mature and familiar to Indian accountants **[vendor claim]**.
- Payment links and QR from invoices, plus WhatsApp reminders, cut collection effort **[vendor claim]**.
- Bill-wise outstandings with ageing are a long-standing strength.

Hate / friction:
- Statement import works only for banks and formats Tally supports. Unsupported ones need an email to
  support, and **PDF is refused**, though it is often the only format small businesses have.
- An opening-balance mismatch throws off every later BRS figure. A wrong instrument or transaction
  type on a voucher (for example "e-Fund transfer" against "inter-bank") can hide entries from BRS.
- Connected Banking covers only 4 banks, and payments 2 to 3. It needs TSS (an annual fee) and
  corporate-portal approval steps. Statements are fetched on request, not pushed.
- e-Payments need bank-side enablement (RM, CMS form, ERP registration), which is slow for small
  businesses.
- Bank counts on Tally's own pages contradict each other, which is confusing.
- No guided bounced-cheque workflow was found.

## Open questions

1. Exact current Tally bank lists for statement import, e-payments and Connected Banking. Their pages
   disagree (145+, 200+, 250+, 450+ for import; 16 or 18 for e-payments).
2. Is Kotak live for Connected Payments in 7.x? Is ICICI supported for payments at all?
3. Does Tally handle UPI narrations specially, such as extracting the RRN for matching? It was not
   found in the docs.
4. Have AA enterprise current-account FI types (partnership, LLP, company) gone live at any bank in
   2026? Has the standard AA pricing from 1 Nov 2026 been published?
5. Could a regulated partner (an NBFC or bank) offer an "AA-as-a-service" for accounting
   reconciliation under an acceptable purpose code? This needs a legal opinion.
6. Which Indian banks offer camt.053, and which require MT940 only through CMS?
7. Legal check: storing users' bank account numbers locally is fine, but are there DPDP Act
   obligations if the app syncs or backs up to the cloud?
8. Bounced-cheque handling: should we model statutory notice timelines (NI Act s.138, a notice within
   30 days of the dishonour memo) as reminders? This needs legal review.
9. Interest calculation specifics (day-count basis 365 or 360, leap years, slab changes inside a
   bill's life). Each rule needs a derivation-backed test before we ship v2 interest.

## Sources

- Tally help: Bank Reconciliation - https://help.tallysolutions.com/tally-prime/banking-utilities/bank-recon/
- Tally help: Banking Utilities overview - https://help.tallysolutions.com/banking/
- Tally help: Import bank statements - https://help.tallysolutions.com/?p=178947
- Tally help: Auto bank reconciliation - https://help.tallysolutions.com/bank-recon/auto-bank-reconciliation/
- Tally blog: Common BRS problems - https://tallysolutions.com/tally/common-bank-reconciliation-problems-and-how-to-resolve-them-in-tallyprime/
- Tally guide: Auto-import bank statements - https://tallysolutions.com/business-guides/how-to-auto-import-bank-statements-for-faster-reconciliation-in-tallyprime/
- Tally guide: Banking integration, auto feeds - https://tallysolutions.com/business-guides/tallyprime-banking-integration-auto-bank-feeds-auto-brs-payments/
- Tally: PrimeBanking 6.0 quick start - https://help.tallysolutions.com/wp-content/uploads/2025/04/TallyPrime_6.0_PrimeBanking_Quick_Start_Guide.pdf
- Tally help: Connected Banking - https://help.tallysolutions.com/tally-prime/banking-utilities/connected-banking-feature/
- Tally help: Connected Banking FAQ - https://help.tallysolutions.com/connected-banking-faq
- Tally help: ICICI / Axis / SBI connection - https://help.tallysolutions.com/connection-icici/ , https://help.tallysolutions.com/connection-axis/ , https://help.tallysolutions.com/connection-sbi/
- Tally help: Send payments from SBI - https://help.tallysolutions.com/send-payments-from-your-sbi-account-using-connected-banking
- Tally: TallyPrime 7.0 brochure - https://live.tallysolutions.com/wp-content/themes/tally/assets/doc/TP7.0_Brochure_For_Web.pdf
- Tally: Connected Banking 2.0 blog - https://tallysolutions.com/tally/connected-banking-tallyprime-automate-vendor-payments/ (403 on fetch, used via search snippet)
- Tally help: e-Payments - https://help.tallysolutions.com/e-payments-tally/
- Tally help: Contact details of banks for e-payment - https://help.tallysolutions.com/contact-details-of-banks-for-e-payment
- Tally help: Cheque payments / printing - https://help.tallysolutions.com/tally-prime/banking-utilities/banking-cheque-printing-tally/
- Tally help: Cheque register - https://help.tallysolutions.com/cheque-register/
- Tally help: PDC management - https://help.tallysolutions.com/tally-prime/banking-utilities/post-dated-cheque-managment-tally
- Tally help: Deposit slips / payment advice - https://help.tallysolutions.com/deposit-slips/ , https://help.tallysolutions.com/payment-advice/
- Tally help: Payment request and FAQ - https://help.tallysolutions.com/tally-prime/banking-utilities/payment-request/ , https://help.tallysolutions.com/tally-prime/banking-utilities/payment-request-faq/
- Tally guide: UPI/QR payments - https://tallysolutions.com/business-guides/upi-qr-payments-auto-link-receipts-tallyprime/
- Razorpay: TallyPrime integration - https://razorpay.com/blog/announcing-razorpay-tallyprime-integration/
- Tally help: Managing receivables and payables - https://help.tallysolutions.com/managing-receivables-and-payables-in-tallyprime/
- Tally guide: Receivables ageing and reminders - https://tallysolutions.com/business-guides/how-to-track-receivables-aging-and-send-payment-reminders-in-tallyprime/
- Tally: Credit and cashflow features - https://tallysolutions.com/features/credit-and-cashflow-management/
- Third-party: BRS entries not appearing - https://shwetasoftwares.freshdesk.com/support/solutions/articles/19000066149-entries-do-not-appear-in-brs-bank-reconciliation-statement-
- Sahamati: Account types activated by banks on AAs - https://sahamati.org.in/account-types-activated-by-banks-on-aas/
- Sahamati: FAQ - https://sahamati.org.in/faq/
- Sahamati: Onboarding for participants - https://sahamati.org.in/onboarding-for-participants
- Sahamati: Economic model (pricing) committee - https://sahamati.org.in/economic-model-committee/
- Sahamati: GST as FIP deep dive - https://sahamati.org.in/wp-content/uploads/2023/08/Refresher-session-18th-Aug-__-GST-as-FIP-on-AA_Deep-Dive_Sahamati-v2-August.pdf
- RBI: GSTN as FIP notification - https://website.rbi.org.in/web/rbi/-/notifications/inclusion-of-goods-and-service-tax-network-gstn-as-a-financial-information-provider-under-account-aggregator-framework-12412
- DFS: AA framework status - https://financialservices.gov.in/account-aggregator-framework
- Setu: GST data on AA (current account schema gap) - https://setu.co/blog/the-gst-data-tale-on-account-aggregator
- Setu: FIU go-live - https://docs.setu.co/data/account-aggregator/licenses-and-go-live/go-live
- Finanjo: AA 2026 standards (third-party) - https://finanjo.com/founders-office/rebit-aa-client-standards-2026
- Pricing (third-party, unconfirmed): https://productgrowth.in/tools/banking-api/finvu/ , https://digiqt.com/api-details/india/fintech/account-aggregator-api-finvu , https://javajack.github.io/b2b-lending-india/04-integrations/account-aggregator/
- ICICI Corporate API suite - https://www.icici.bank.in/business-banking/cms/corporate-api-suite
- ICICI Connected Banking - https://www.icici.bank.in/business-banking/cms/connected-banking
- Zoho: ICICI Connected Banking help - https://www.zoho.com/en-in/erp/help/vendor-payments/icici.html
- Zoho Books: transaction rules - https://www.zoho.com/books/help/banking/transaction-rules.html
- Third-party Zoho India bank feeds guide - https://www.patronaccounting.com/blog/zoho-books-bank-reconciliation-india-guide
- NPCI UPI linking spec 1.6 (mirror) - https://www.labnol.org/files/linking.pdf
- NPCI Circular 18 (QR parameters) - https://www.npci.org.in/PDF/npci/upi/circular/2017/Circular18_BankCompliances_to_enbaleUPIMerchantecosystem_0.pdf
- UPI deep-link inconsistency (developer report) - https://medium.com/@thinksantosh/upi-deep-linking-specification-app-developers-mustnt-expect-consistency-of-implementation-across-90b919e077cb
