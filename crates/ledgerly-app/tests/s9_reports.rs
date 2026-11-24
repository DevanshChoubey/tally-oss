//! s9 report parity: the balance sheet's default view lists each group with its sub-groups, as
//! the mock does, over the sample company's books.
//!
//! Provenance: every figure is the mock's own (design/mock-v2/index.html `balanceSheet()`,
//! run with node over the mock's books, 2026-11-25) and is reproduced from the seed's vouchers in
//! `crates/ledgerly-sample/tests/sample.rs`. The words are the groups' names in the company's
//! chart (TallyPrime's reserved group names, `ledgerly-model` `seed_groups`).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;
use std::time::Duration;

use ledgerly_api::{ColumnSpec, Detail, ReportFilter, ReportKind, ReportParams};
use ledgerly_app::screens::report_view::{ReportSource, ReportViewScreen, ViewCell, ViewReport};
use ledgerly_app::worker::ApiWorker;
use ledgerly_core::{Clock, Period};
use ledgerly_model::Scenario;
use time::OffsetDateTime;
use time::macros::date;

fn sample_screen(name: &str, detail: Detail) -> (PathBuf, ApiWorker, ReportViewScreen) {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (api, _path) =
        ledgerly_sample::create_under(&dir, Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    let worker = ApiWorker::spawn(api).unwrap();
    let params = ReportParams {
        kind: ReportKind::BalanceSheet,
        period: Period::new(date!(2026 - 04 - 01), date!(2026 - 11 - 20)).unwrap(),
        as_of: date!(2026 - 11 - 20),
        scenario: Scenario::Actual,
        detail,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    };
    let screen = ReportViewScreen::worker(ReportSource::worker(params)).unwrap();
    (dir, worker, screen)
}

fn settle(screen: &mut ReportViewScreen, worker: &ApiWorker) {
    for _ in 0..4000 {
        screen.tick(Some(worker));
        if !screen.is_loading() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the report did not arrive");
}

/// The label, depth and the bare rupee text of the figure of each row.
fn lines(report: &ViewReport) -> Vec<(String, u8, String)> {
    report
        .rows
        .iter()
        .map(|r| {
            let cell = r
                .cells
                .iter()
                .find(|c| !matches!(c, ViewCell::Empty))
                .unwrap();
            let text = match cell {
                ViewCell::Amount(a) | ViewCell::Provisional { amount: a, .. } => {
                    format!("{}", a.amount.get())
                }
                other => panic!("{other:?}"),
            };
            (r.label.clone(), r.depth, text)
        })
        .collect()
}

fn paise(rupees: &str) -> String {
    let (w, f) = rupees.split_once('.').unwrap_or((rupees, "0"));
    format!(
        "{}",
        w.parse::<i64>().unwrap() * 100 + format!("{f:0<2}").parse::<i64>().unwrap()
    )
}

#[test]
fn the_default_balance_sheet_lists_groups_and_sub_groups_without_ledgers() {
    // provenance: mock index.html:1581-1589 (`rows(bs.liab,0)`, children filtered to
    // `depth===0&&c.kind!=='ledger'`): each top group, then its sub-groups and the closing stock,
    // never a ledger. Order and figures from the node run of `balanceSheet()` named above.
    let (_dir, worker, mut screen) = sample_screen("s9_bs_default", Detail::Condensed);
    settle(&mut screen, &worker);
    let want: Vec<(&str, u8, &str)> = vec![
        ("Capital Account", 0, "2000000"),
        ("Loans (Liability)", 0, "840000"),
        ("Secured Loans", 1, "840000"),
        ("Current Liabilities", 0, "1072967.30"),
        ("Sundry Creditors", 1, "707970"),
        ("Duties & Taxes", 1, "33997.30"),
        ("Provisions", 1, "331000"),
        ("Profit and Loss A/c", 0, "1696569.70"),
        ("Fixed Assets", 0, "2362846"),
        ("Current Assets", 0, "3246691"),
        ("Closing stock", 1, "1425539"),
        ("Sundry Debtors", 1, "618790"),
        ("Cash-in-Hand", 1, "78337.50"),
        ("Bank Accounts", 1, "941524.50"),
        ("Loans & Advances (Asset)", 1, "182500"),
    ];
    let got = lines(screen.report());
    let want: Vec<(String, u8, String)> = want
        .into_iter()
        .map(|(l, d, f)| (l.to_owned(), d, paise(f)))
        .collect();
    assert_eq!(got, want);
}

#[test]
fn alt_f1_adds_the_ledgers() {
    // provenance: mock index.html:1583 (`detailed||...`): the detailed view lists every child,
    // ledgers included. Ananya Kulkarni's capital is 15,00,000 (mock index.html:518).
    let (_dir, worker, mut screen) = sample_screen("s9_bs_alt_f1", Detail::Condensed);
    settle(&mut screen, &worker);
    screen.toggle_detail();
    settle(&mut screen, &worker);
    let got = lines(screen.report());
    assert!(
        got.contains(&("Ananya Kulkarni, capital".to_owned(), 1, paise("1500000"))),
        "{got:?}"
    );
}
