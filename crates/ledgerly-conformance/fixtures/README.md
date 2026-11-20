# Fixture company

All names, GSTINs and amounts are fictional (rule E7). GSTIN check characters are computed with
the algorithm in `ledgerly-core` (`gstin_check_character`).

**Neelgiri Traders (fictional)** — FY 2025-26, books begin 1 April 2025, trading company.

- Registrations: Maharashtra `27ABCDE1234F1Z?` and Karnataka `29ABCDE1234F1ZW` (same PAN `ABCDE1234F`).
  Card Q1 computes and fills the Maharashtra check character with `gstin_check_character`.
- Ledgers (12): Cash; HDFC Current A/c (bank); Capital; Sales – Local; Sales – Interstate;
  Purchase; CGST; SGST; IGST; Rent; Shree Ganesh Stores (debtor, registered, state 27);
  Coastal Supplies (creditor, registered, state 29).
- Stock items (3): Widget A (pcs, 18%); Widget B (box of 12 pcs, 5%); Item R3 (the valuation item
  from `docs/research/03-inventory.md`, average cost).
- Godowns: Main, Pune.
- Vouchers (20), including:
  - opening Capital ₹1,00,000 Cr / Cash ₹1,00,000 Dr;
  - the R3 movements: purchase 50 @ 13 on 10 Apr, sale 120 on 15 Apr, purchase 40 @ 15 on
    20 May, sale 30 on 31 May (opening 100 @ 10);
  - an intra-state sale of ₹10,000 @ 18% and an inter-state sale of ₹10,000 @ 18%;
  - one sale with supply on 21 Sep 2025 and invoice on 23 Sep 2025, and one with both on
    23 Sep 2025 (either side of the 22 Sep 2025 rate change);
  - a receipt against a bill, a payment by cheque, a credit note, a cancelled sales voucher,
    and an optional voucher.

## Capturing observed values (owner; plan prerequisite 4)

In TallyPrime Educational mode (dates limited to the 1st, 2nd and last day of a month, so shift
voucher dates to those days and record the mapping), create this company and post the vouchers,
then export as XML and Excel: Day Book, Trial Balance, Balance Sheet, Profit & Loss, Stock
Summary, Bills Receivable, Bills Payable, GSTR-1. Save them under `fixtures/observed/` with a
`SOURCE.md` naming the TallyPrime release and the date. Each file promotes `pending` cases to
`observed`.
