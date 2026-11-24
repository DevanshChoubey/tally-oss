//! The sample company loads through the API with nothing unsupported, its books balance, and its
//! figures follow from the vouchers in the seed. No expected value here is copied from engine
//! output: each one is the mock's own table (`design/mock-v2/index.html`) or a hand working from
//! the seed's vouchers, written out beside it.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests stop on any failure"
)]

use std::collections::BTreeMap;

use ledgerly_api::{Api, Cell, ColumnSpec, Detail, Report, ReportFilter, ReportKind, ReportParams};
use ledgerly_core::{Clock, Date, Outcome, Paise, Period, SignedAmount};
use ledgerly_model::Scenario;
use time::OffsetDateTime;
use time::macros::date;

fn sample() -> (tempfile::TempDir, Api) {
    let dir = tempfile::tempdir_in("/media/devansh/Experiment/playground/claude-tmp").unwrap();
    let api = ledgerly_sample::create(
        &dir.path().join("sample.ledgerly"),
        Clock::Fixed(OffsetDateTime::UNIX_EPOCH),
    )
    .unwrap();
    (dir, api)
}

fn report(api: &Api, kind: ReportKind, as_of: Date) -> Report {
    let params = ReportParams {
        kind,
        period: Period::new(date!(2026 - 04 - 01), as_of).unwrap(),
        as_of,
        scenario: Scenario::Actual,
        detail: Detail::Detailed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    };
    match api.report(&params).unwrap() {
        Outcome::Value(r) => r,
        Outcome::Unsupported(u) => panic!("unsupported: {u:?}"),
    }
}

/// Paise, debit positive, of an amount cell.
fn signed(cell: &Cell) -> i64 {
    match cell {
        Cell::Amount(a) => to_signed(*a),
        Cell::Empty => 0,
        other @ (Cell::Signed(_)
        | Cell::Qty(_)
        | Cell::Text(_)
        | Cell::Date(_)
        | Cell::Provisional { .. }
        | Cell::Unsupported(_)) => panic!("not an amount: {other:?}"),
    }
}

fn to_signed(a: SignedAmount) -> i64 {
    a.to_signed().unwrap().get()
}

/// Every row of the trial balance by label, as paise with debit positive.
fn trial_balance(api: &Api, as_of: Date) -> (BTreeMap<String, i64>, Report) {
    let r = report(api, ReportKind::TrialBalance, as_of);
    let rows = r
        .rows
        .iter()
        .map(|row| {
            let dr = signed(&row.cells[0]);
            let cr = signed(&row.cells[1]);
            (row.label.clone(), dr + cr)
        })
        .collect();
    (rows, r)
}

fn paise(rupees: &str) -> i64 {
    let (whole, frac) = rupees.split_once('.').unwrap_or((rupees, "0"));
    let sign = if whole.starts_with('-') { -1 } else { 1 };
    let whole: i64 = whole.trim_start_matches('-').parse().unwrap();
    let frac = format!("{frac:0<2}");
    sign * (whole * 100 + frac.parse::<i64>().unwrap())
}

#[test]
fn the_seed_posts_with_nothing_unsupported_and_the_books_are_locked() {
    let (_dir, api) = sample();
    // provenance: mock index.html:489 (CO.lock = '31-10-2026').
    assert_eq!(api.lock_date(), Some(date!(2026 - 10 - 31)));
    // provenance: mock index.html:489 (CO.today = '21-11-2026').
    assert_eq!(ledgerly_sample::today().unwrap(), date!(2026 - 11 - 21));
    // The stored statement is in the file (mock STMT, index.html:745).
    let books = match api.books().unwrap() {
        Outcome::Value(b) => b,
        Outcome::Unsupported(u) => panic!("{u:?}"),
    };
    let bank = books
        .masters
        .ledgers
        .values()
        .find(|l| l.name == "Saptagiri Commercial Bank A/c")
        .unwrap()
        .id;
    assert_eq!(api.bank_statements(bank).len(), 1);
}

#[test]
fn the_trial_balance_balances_with_no_opening_difference_on_both_days() {
    let (_dir, api) = sample();
    for as_of in [date!(2026 - 10 - 31), date!(2026 - 11 - 21)] {
        let (rows, r) = trial_balance(&api, as_of);
        assert_eq!(signed(&r.totals[0]), -signed(&r.totals[1]), "{as_of}");
        assert!(
            rows.keys().all(|k| !k.contains("Difference")),
            "an opening difference row on {as_of}"
        );
    }
}

#[test]
fn balances_on_31_oct_are_the_mocks_opening_table() {
    let (_dir, api) = sample();
    let (rows, _) = trial_balance(&api, date!(2026 - 10 - 31));
    // provenance: mock index.html:517-549, the `ob` column (balance on 1 Nov 2026, debit
    // positive, rupees). The vouchers dated before 1 Nov in the seed are invented to reach it.
    // Capital (index.html:518-519), IGST, Round off (index.html:529-530, 544: all nil on 1 Nov) and
    // Opening stock (index.html:549, 9,12,000 Dr, an expense-nature ledger) are included.
    let mock: [(&str, &str); 41] = [
        ("Ananya Kulkarni, capital", "-1500000"),
        ("Rohan Deshpande, capital", "-500000"),
        ("IGST", "0"),
        ("Round off", "0"),
        ("Opening stock", "912000"),
        ("Mula Valley Co-op Bank, term loan", "-840000"),
        ("Kaveri Packaging Pvt Ltd", "-214300"),
        ("Nilgiri Steel Traders", "-200150"),
        ("Vardhaman Logistics", "-150000"),
        ("Rupa Polymers", "0"),
        ("CGST", "0"),
        ("SGST", "0"),
        ("TDS payable", "-12500"),
        ("Audit fee payable", "-45000"),
        ("Salary payable", "-286000"),
        ("Plant and machinery", "1840000"),
        ("Furniture and fixtures", "235000"),
        ("Computers", "303179"),
        ("Shree Ganesh Traders", "120000"),
        ("Mehta Agro Industries", "0"),
        ("Lakshmi Farm Supplies", "96250"),
        ("Patil Hardware Stores", "88420"),
        ("Kisan Seva Kendra", "147384.50"),
        ("Cash sale (unregistered)", "0"),
        ("Cash", "48215.50"),
        ("Saptagiri Commercial Bank A/c", "610000"),
        ("Mula Valley Co-op Bank, current A/c", "214000"),
        ("Rent deposit, Sharma Properties", "150000"),
        ("TDS receivable", "32500"),
        ("Sales", "-8240000"),
        ("Purchases", "5130000"),
        ("Freight inward", "184000"),
        ("Salaries", "1410000"),
        ("Rent", "175000"),
        ("Electricity", "96400"),
        ("Bank charges", "6820"),
        ("Depreciation", "107331"),
        ("Staff welfare", "38600"),
        ("Interest on term loan", "61250"),
        ("Discount received", "-18400"),
        ("Rupa Polymers", "0"),
    ];
    for (name, rupees) in mock {
        // A ledger with a zero balance is left out of the detailed trial balance.
        let got = rows.get(name).copied().unwrap_or(0);
        assert_eq!(got, paise(rupees), "{name}");
    }
}

#[test]
fn spot_values_follow_from_the_vouchers() {
    let (_dir, api) = sample();
    let (rows, _) = trial_balance(&api, date!(2026 - 11 - 21));
    let get = |name: &str| rows.get(name).copied().unwrap();

    // provenance: derived. Saptagiri on 1 Nov is 6,10,000 (mock index.html:535). November
    // vouchers on it, from the seed: PY/0315 -25,000; RC/0215 +80,000; PY/0316 -14,860;
    // CT/0021 -20,000; RC/0219 +1,47,384.50; PY/0318 -50,000. Sum +1,17,524.50.
    assert_eq!(get("Saptagiri Commercial Bank A/c"), paise("727524.50"));

    // provenance: derived. Cash on 1 Nov is 48,215.50. CT/0021 brings in 20,000. SI/0415 is a
    // counter sale paid in cash: 4 x 1,450 + 6 x 640 = 5,800 + 3,840 = 9,640 at 5% GST
    // (both items are 5% in the seed) = 482, so 10,122.00 is received.
    // 48,215.50 + 20,000 + 10,122 = 78,337.50.
    assert_eq!(get("Cash"), paise("78337.50"));

    // provenance: derived. Output CGST in November: SI/0412 (Nashik, same state) is
    // 25 x 3,850 + 67 x 505 = 96,250 + 33,835 = 1,30,085 at 18% = 23,415.30, half 11,707.65;
    // SI/0415 is 482 at 5%, half 241; SI/0417 is 120 x 312.50 = 37,500 at 18% = 6,750, half
    // 3,375. The other three invoices go to Gujarat, Tamil Nadu and (cash) ... only SI/0401 and
    // SI/0405 are inter-state, so they carry IGST, not CGST.
    // 11,707.65 + 241 + 3,375 = 15,323.65 credit.
    assert_eq!(get("CGST"), -paise("15323.65"));
    assert_eq!(get("SGST"), -paise("15323.65"));

    // provenance: derived. IGST is one ledger for input and output here and is nil on 1 Nov
    // (mock index.html:529-531). Debits: PB/0141 11,520 and PB/0144 18,000 (mock index.html:569,
    // 575). Credits: SI/0401 89,000 at 18% = 16,020; SI/0405 87,000 at 5% = 4,350.
    // 11,520 + 18,000 - 16,020 - 4,350 = 9,150 debit. (The mock's own igstin 29,520 less igstout
    // 20,370 = 9,150, `node` over index.html's balances().)
    assert_eq!(get("IGST"), paise("9150"));

    // provenance: derived. SI/0372 was 1,20,000 (mock index.html:583-585) and RC/0215 settled
    // 80,000 of it; SI/0412's total is 1,30,085 + 23,415.30 = 1,53,500.30 rounded to the rupee
    // (mock index.html:635 `half(gross,100)*100`) = 1,53,500. Shree Ganesh owes 40,000 + 1,53,500.
    assert_eq!(get("Shree Ganesh Traders"), paise("193500"));

    // provenance: derived. Kisan Seva Kendra paid its one bill (SI/0390, 1,47,384.50) in full by
    // RC/0219, so the ledger is empty and leaves the detailed trial balance.
    assert_eq!(rows.get("Kisan Seva Kendra"), None);
}

#[test]
fn stock_after_the_november_sales_is_the_bought_quantity_less_what_was_sold() {
    let (_dir, api) = sample();
    let r = report(&api, ReportKind::StockSummary, date!(2026 - 11 - 21));
    let closing = |label: &str| -> String {
        let row = r.rows.iter().find(|row| row.label == label).unwrap();
        match &row.cells[6] {
            Cell::Qty(q) => q.0.to_string(),
            other @ (Cell::Amount(_)
            | Cell::Signed(_)
            | Cell::Text(_)
            | Cell::Date(_)
            | Cell::Empty
            | Cell::Provisional { .. }
            | Cell::Unsupported(_)) => panic!("{other:?}"),
        }
    };
    // provenance: derived. Bought: mock ITEMS qty (index.html:555-562). Sold in November, from the
    // seed's invoices: lateral 25 (SI/0412); nozzle sets 67; sprayers 60 + 4; ball valves 120;
    // pickaxes 6; pumps 10 (SI/0401); HDPE none.
    assert_eq!(closing("Drip lateral 16 mm, 400 m roll"), "187");
    assert_eq!(closing("Brass nozzle set (6 pc)"), "573");
    assert_eq!(closing("Hand sprayer 16 L"), "96");
    assert_eq!(closing("PVC ball valve 50 mm"), "1060");
    assert_eq!(closing("Steel pickaxe 2.5 kg"), "144");
    assert_eq!(closing("HDPE pipe coil 32 mm"), "58");
    assert_eq!(closing("Monoblock pump 1 HP"), "12");
}

#[test]
fn the_balance_sheet_carries_the_mocks_capital_profit_and_current_liabilities() {
    let (_dir, api) = sample();
    let r = report(&api, ReportKind::BalanceSheet, date!(2026 - 11 - 21));
    let net = |needle: &str| -> i64 {
        let row = r
            .rows
            .iter()
            .find(|row| row.depth == 0 && row.label.to_lowercase().contains(needle))
            .unwrap_or_else(|| panic!("no top row for {needle}"));
        row.cells.iter().map(signed_or_provisional).sum()
    };
    // provenance: mock index.html:664-681 (balanceSheet) over the mock's own books, run with
    // node: Capital account 20,00,000.00 (15,00,000 + 5,00,000, index.html:518-519; the mock
    // has no drawings).
    assert_eq!(net("capital"), -paise("2000000"));
    // provenance: derived. Profit = - (net debit of income and expense ledgers) + closing stock
    // (mock index.html:667-668). The P&L ledgers on 1 Nov sum to 1,36,999 credit (mock
    // index.html:540-549, with Opening stock 9,12,000 as an expense). November adds sales
    // 89,000 + 87,000 + 1,30,085 + 9,640 + 37,500 = 3,53,225 credit and debits purchases
    // 64,000 + 1,00,000, rent 25,000, electricity 14,860, depreciation 15,333 and round off 0.30
    // (mock `balances()`), so the net credit is 1,36,999 + 3,53,225 - 2,19,193.30 = 2,71,030.70.
    // Closing stock 14,25,539.00 (mock stockPosition). 2,71,030.70 + 14,25,539.00 = 16,96,569.70.
    assert_eq!(net("profit"), -paise("1696569.70"));
    // provenance: mock index.html:681 over the same books: Current liabilities 10,72,967.30 =
    // creditors 7,07,970 + duties 33,997.30 + provisions 3,31,000.
    assert_eq!(net("current liabilities"), -paise("1072967.30"));
}

/// Debit-positive paise of an amount or provisional amount, zero for an empty cell.
fn signed_or_provisional(cell: &Cell) -> i64 {
    match cell {
        Cell::Provisional { amount, .. } => to_signed(*amount),
        other @ (Cell::Amount(_) | Cell::Empty) => signed(other),
        other @ (Cell::Signed(_)
        | Cell::Qty(_)
        | Cell::Text(_)
        | Cell::Date(_)
        | Cell::Unsupported(_)) => panic!("not an amount: {other:?}"),
    }
}

#[test]
fn the_day_book_says_who_entered_each_november_voucher_and_how() {
    // provenance: the mock's VOUCHERS list (design/mock-v2/index.html:567-580): `by` is who
    // entered it and `ai` marks an accepted draft. Bank rule (PY/0316) and Image attached
    // (PB/0144) are not recorded in the company file, so they read as Typed here (see
    // design/PARITY-GAPS.md). The rest of the 14 are Typed by the named user.
    let (_dir, api) = sample();
    let r = report(&api, ReportKind::DayBook, date!(2026 - 11 - 21));
    let col = |name: &str| r.columns.iter().position(|c| c.label == name).unwrap();
    let text = |row: &ledgerly_api::ReportRow, c: usize| match &row.cells[c] {
        Cell::Text(t) => t.clone(),
        other @ (Cell::Amount(_)
        | Cell::Signed(_)
        | Cell::Qty(_)
        | Cell::Date(_)
        | Cell::Empty
        | Cell::Provisional { .. }
        | Cell::Unsupported(_)) => panic!("{other:?}"),
    };
    let (no, by, origin, narr) = (
        col("Voucher no."),
        col("Entered by"),
        col("Origin"),
        col("Narration"),
    );
    let find = |number: &str| {
        let row = r
            .rows
            .iter()
            .find(|row| matches!(&row.cells[no], Cell::Text(t) if t == number))
            .unwrap_or_else(|| panic!("no {number}"));
        (text(row, by), text(row, origin), text(row, narr))
    };
    let t = |a: &str, b: &str, c: &str| (a.to_owned(), b.to_owned(), c.to_owned());
    assert_eq!(
        find("SI/0401"),
        t("Ananya K.", "Typed", "Against PO MAI/77")
    );
    assert_eq!(
        find("PY/0315"),
        t(
            "Ananya K.",
            "AI draft",
            "Office rent, November, Sharma Properties"
        )
    );
    assert_eq!(find("SI/0405"), t("Rohan D.", "Typed", ""));
    assert_eq!(
        find("PB/0141"),
        t("Ananya K.", "AI draft", "Cartons, bill KP/2231")
    );
    assert_eq!(
        find("RC/0215"),
        t("Rohan D.", "Typed", "NEFT, part against SI/0372")
    );
    assert_eq!(
        find("JV/0032"),
        t("Ananya K.", "Typed", "Monthly depreciation, November")
    );
}

#[test]
fn gstr1_sections_count_what_the_return_holds() {
    // provenance: the mock's `gstr1()` run with node over its November sales
    // (design/mock-v2/index.html:698-715): 4A B2B invoices 4 / 3,43,585.00 taxable / 50,535.30
    // tax; 7 B2C others one state row, 9,640.00 taxable (SI/0415: 4 x 1,450.00 spare + 6 x 640.00
    // pickaxe) at 5 percent = 482.00 tax; 12 HSN summary 3,49,385.00 = 3,43,585.00 + 5,800.00,
    // the pickaxe line left out because the item has no HSN, so Table 12 is Needs correction; the
    // spare's HSN 8424 at 5 percent is already a row, so 12 keeps 5 rows; 13 counts 5 invoices.
    // HSN is a Table 12 field only (GSTN GSTR-1 user guide, read 2026-11-25), so SI/0415 stays in
    // Table 7 and there is no held-back row.
    let (_dir, api) = sample();
    let books = api.books().unwrap().value().unwrap();
    let gstin = books.company.registrations[0].gstin.clone();
    let period = Period::new(date!(2026 - 11 - 01), date!(2026 - 11 - 30)).unwrap();
    let ret = api.gstr1(&gstin, period).unwrap().value().unwrap();
    let rows: Vec<String> = ret
        .sections()
        .unwrap()
        .into_iter()
        .map(|s| {
            let (taxable, tax) = (s.taxable.map(Paise::get), s.tax.map(Paise::get));
            format!(
                "{} {} {taxable:?} {tax:?} {:?}",
                s.code, s.records, s.status
            )
        })
        .collect();
    let want = vec![
        "4A 4 Some(34358500) Some(5053530) Ready",
        "5 0 Some(0) Some(0) NotRelevant",
        "7 1 Some(964000) Some(48200) Ready",
        "9B 0 Some(0) Some(0) NotRelevant",
        "12 5 Some(34938500) None NeedsCorrection",
        "13 5 None None Ready",
    ];
    assert_eq!(rows, want);
    let sections = ret.sections().unwrap();
    let hsn = sections.iter().find(|s| s.code == "12").unwrap();
    assert!(hsn.note.as_deref().unwrap_or("").contains("Steel pickaxe"));
}

#[test]
fn gstr3b_computes_on_the_sample_and_matches_the_mocks_tables() {
    // provenance: design/mock-v2/index.html `gstr3b()` over the same books (run with node on
    // 2026-11-25): table 3.1(a) taxable 3,53,225.00 with IGST 20,370.00, CGST 15,323.65 and SGST
    // 15,323.65 (screenshot design/mock-v2/captures/states/light/gst.gstr3b.webp), and "In your
    // books" ITC of IGST 29,520.00 with no CGST or SGST.
    let (_dir, api) = sample();
    let books = api.books().unwrap().value().unwrap();
    let gstin = books.company.registrations[0].gstin.clone();
    let period = Period::new(date!(2026 - 11 - 01), date!(2026 - 11 - 30)).unwrap();
    let w = match api.gstr3b(&gstin, period).unwrap() {
        Outcome::Value(w) => w,
        Outcome::Unsupported(u) => panic!("{u:?}"),
    };
    assert_eq!(w.outward_taxable_value.get(), 35_322_500);
    assert_eq!(w.outward_tax.integrated.get(), 2_037_000);
    assert_eq!(w.outward_tax.central.get(), 1_532_365);
    assert_eq!(w.outward_tax.state_ut.get(), 1_532_365);
    assert_eq!(w.itc_all_other.integrated.get(), 2_952_000);
    assert_eq!(w.itc_all_other.central.get(), 0);
    assert_eq!(w.itc_all_other.state_ut.get(), 0);
}
