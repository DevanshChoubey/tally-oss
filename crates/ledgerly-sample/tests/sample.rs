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
use ledgerly_core::{Clock, Date, Outcome, Period, SignedAmount};
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
        other => panic!("not an amount: {other:?}"),
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
    // Left out on purpose, each listed in design/PARITY-GAPS.md ("Sample company"):
    //   - the two partners' capital: the balancing figure of the opening trial balance;
    //   - IGST: the stock bought in October carries IGST the mock leaves at 0;
    //   - Round off: the same invoice's round-off of Rs 0.36;
    //   - Opening stock: the engine takes opening stock from the stock ledger, not a ledger.
    let mock: [(&str, &str); 36] = [
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

    // provenance: derived. IGST is one ledger for input and output here. Debits: the October
    // stock purchase, 18% on 14,55,552 of lateral, nozzles, valves, HDPE and pumps (cost x qty
    // from mock index.html:555-562) = 2,61,999.36 plus 5% on 2,43,300 of sprayers and pickaxes
    // = 12,165, so 2,74,164.36; PB/0141 11,520; PB/0144 18,000. Credits: SI/0401 89,000 at 18%
    // = 16,020; SI/0405 87,000 at 5% = 4,350. 2,74,164.36 + 29,520 - 20,370 = 2,83,314.36 debit.
    assert_eq!(get("IGST"), paise("283314.36"));

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
            other => panic!("{other:?}"),
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
