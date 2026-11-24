//! The Open bills card beside an altered bill-wise party comes from the engine's outstanding
//! report, run on the API worker (`design/mock-v2/index.html:1806`: each open bill's reference,
//! date, open amount and a "N days overdue" or "Not due" badge), and the form's Balance field is
//! the ledger's closing balance from the trial balance (index.html:1805).
//!
//! Provenance: every expected figure is `derived` by hand from the vouchers posted below, with the
//! arithmetic next to it. Ageing follows the outstanding report's own rule (ledgerly-reports
//! `outstanding.rs`: overdue days are the report date less the due date, in buckets of 0-30,
//! 31-60, 61-90 and over 90 days; a bill due after the report date is not yet due). The report
//! date is the last voucher date, as for the balances. The screen counts no days itself: the age
//! shown is the report's bucket.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;
use std::time::Duration;

use ledgerly_api::{
    Api, ApiError, Cell, ColumnHeader, ColumnSpec, Command, Detail, DrillKey, Report, ReportFilter,
    ReportKind, ReportParams, ReportRow, RowKind,
};
use ledgerly_app::screens::masters::bills::{
    BillAge, BillLine, BillsState, PartyBills, age_badge, from_answers, party_lines,
};
use ledgerly_app::screens::masters::{MasterKind, MasterRef, MastersScreen};
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{
    Clock, CompanyId, Date, GroupId, LedgerId, Outcome, Paise, Period, SignedAmount, StateCode,
    Unsupported, VoucherId,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, BillAllocation, BillRefKind, BillWiseConfig, Company, EntityKind,
    Features, Ledger, LedgerLine, NegativeStockPolicy, ReservedGroup, Scenario, Voucher,
    VoucherMode, VoucherStatus,
};
use time::OffsetDateTime;
use time::macros::date;

const CASH: LedgerId = LedgerId(1);
const SALES: LedgerId = LedgerId(2);
const CUSTOMER: LedgerId = LedgerId(3);
const QUIET: LedgerId = LedgerId(4);

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(29).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    }
}

fn rs(rupees: i64) -> Paise {
    Paise::new(rupees * 100)
}

fn group(kind: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(kind))
        .unwrap()
        .id
}

fn plain(id: LedgerId, name: &str, reserved: ReservedGroup) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: group(reserved),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn line(ledger: LedgerId, amount: SignedAmount) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger,
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    }
}

fn journal(id: u64, on: Date, lines: Vec<LedgerLine>) -> Voucher {
    let type_id = default_voucher_types()
        .into_iter()
        .find(|t| t.base == BaseVoucherKind::Journal)
        .unwrap()
        .id;
    Voucher {
        id: VoucherId(id),
        type_id,
        number: None,
        series: None,
        date: on,
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::AsVoucher,
        party: None,
        place_of_supply: None,
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: String::new(),
        ledger_lines: lines,
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: std::collections::BTreeMap::new(),
    }
}

fn committed(out: Result<Outcome<ledgerly_api::CommandResult>, ApiError>) {
    match out {
        Ok(Outcome::Value(_)) => {}
        other => panic!("expected a committed value, got {other:?}"),
    }
}

fn bill(kind: BillRefKind, name: &str, due: Option<Date>, amount: SignedAmount) -> BillAllocation {
    BillAllocation {
        kind,
        name: name.to_owned(),
        due_date: due,
        credit_days: None,
        amount,
    }
}

fn billed(ledger: LedgerId, amount: SignedAmount, bills: Vec<BillAllocation>) -> LedgerLine {
    let mut l = line(ledger, amount);
    l.bills = bills;
    l
}

fn bill_wise(id: LedgerId, name: &str) -> Ledger {
    let mut l = plain(id, name, ReservedGroup::SundryDebtors);
    l.bill_wise = Some(BillWiseConfig {
        default_credit_days: None,
    });
    l
}

/// Customer: INV-1 for 10,000 on 10 Apr due 20 Apr, INV-2 for 5,000 on 30 Jun due 30 Jul,
/// 4,000 received against INV-1 and 1,000 on account, both on 30 Jun. Quiet: bill-wise, nothing
/// posted.
fn setup(name: &str) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let mut api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    for ledger in [
        plain(CASH, "Cash", ReservedGroup::CashInHand),
        plain(SALES, "Sales", ReservedGroup::SalesAccounts),
        bill_wise(CUSTOMER, "Customer"),
        bill_wise(QUIET, "Quiet Debtor"),
    ] {
        committed(api.execute(Command::CreateLedger(ledger)));
    }
    let june = date!(2026 - 06 - 30);
    for v in [
        journal(
            1,
            date!(2026 - 04 - 10),
            vec![
                billed(
                    CUSTOMER,
                    SignedAmount::dr(rs(10_000)),
                    vec![bill(
                        BillRefKind::New,
                        "INV-1",
                        Some(date!(2026 - 04 - 20)),
                        SignedAmount::dr(rs(10_000)),
                    )],
                ),
                line(SALES, SignedAmount::cr(rs(10_000))),
            ],
        ),
        journal(
            2,
            june,
            vec![
                billed(
                    CUSTOMER,
                    SignedAmount::dr(rs(5_000)),
                    vec![bill(
                        BillRefKind::New,
                        "INV-2",
                        Some(date!(2026 - 07 - 30)),
                        SignedAmount::dr(rs(5_000)),
                    )],
                ),
                line(SALES, SignedAmount::cr(rs(5_000))),
            ],
        ),
        journal(
            3,
            june,
            vec![
                line(CASH, SignedAmount::dr(rs(5_000))),
                billed(
                    CUSTOMER,
                    SignedAmount::cr(rs(5_000)),
                    vec![
                        bill(
                            BillRefKind::Against,
                            "INV-1",
                            None,
                            SignedAmount::cr(rs(4_000)),
                        ),
                        bill(
                            BillRefKind::OnAccount,
                            "",
                            None,
                            SignedAmount::cr(rs(1_000)),
                        ),
                    ],
                ),
            ],
        ),
    ] {
        committed(api.execute(Command::PostVoucher(v)));
    }
    api
}

fn period() -> Period {
    Period::new(date!(2026 - 04 - 01), date!(2026 - 06 - 30)).unwrap()
}

fn loaded(worker: &ApiWorker, ledger: LedgerId) -> PartyBills {
    let mut bills = PartyBills::new();
    bills.request(ledger, period());
    for _ in 0..2000 {
        bills.tick(Some(worker));
        if !bills.is_loading() {
            return bills;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the open bills did not arrive within the bound");
}

#[test]
fn the_card_lists_the_partys_open_bills_from_the_engine() {
    // provenance: derived, as of 30 Jun 2026 (the last voucher date).
    // INV-1: 10,000 Dr less 4,000 received against it = 6,000.00 Dr open. Due 20 Apr, so
    // 30 Jun - 20 Apr = 10 (rest of April) + 31 (May) + 30 (June) = 71 days overdue: the
    // 61-90 days bucket.
    // INV-2: 5,000.00 Dr open, due 30 Jul, after the report date: not yet due.
    // On account: the 1,000 received with no bill, 1,000.00 Cr, never aged.
    let worker = ApiWorker::spawn(setup("w2_masters_bills.ledgerly")).unwrap();
    let b = loaded(&worker, CUSTOMER);
    let BillsState::Loaded(lines) = b.state() else {
        panic!("no bills: {:?}", b.state());
    };
    assert_eq!(
        lines,
        &vec![
            BillLine {
                name: "INV-1".to_owned(),
                date: Some(date!(2026 - 04 - 10)),
                pending: Ok(SignedAmount::dr(rs(6_000))),
                age: BillAge::Overdue("61-90 days".to_owned()),
            },
            BillLine {
                name: "INV-2".to_owned(),
                date: Some(date!(2026 - 06 - 30)),
                pending: Ok(SignedAmount::dr(rs(5_000))),
                age: BillAge::NotDue,
            },
            BillLine {
                name: "On account".to_owned(),
                date: None,
                pending: Ok(SignedAmount::cr(rs(1_000))),
                age: BillAge::OnAccount,
            },
        ]
    );
}

#[test]
fn a_party_with_nothing_open_has_an_empty_card() {
    // provenance: derived. Quiet Debtor is bill-wise with nothing posted, so the outstanding
    // report does not list it: nothing is open.
    let worker = ApiWorker::spawn(setup("w2_masters_bills_quiet.ledgerly")).unwrap();
    let b = loaded(&worker, QUIET);
    assert_eq!(b.state(), &BillsState::Loaded(Vec::new()));
}

fn headings() -> Vec<ColumnHeader> {
    [
        "Bill date",
        "Due date",
        "Pending",
        "Not yet due",
        "0-30 days",
        "31-60 days",
        "61-90 days",
        "Over 90 days",
        "No due date",
        "On account",
        "Net outstanding",
    ]
    .iter()
    .map(|label| ColumnHeader {
        label: (*label).to_owned(),
        period: None,
    })
    .collect()
}

fn report(rows: Vec<ReportRow>) -> Report {
    Report {
        title: "Bills receivable".to_owned(),
        params: ReportParams {
            kind: ReportKind::Outstanding { receivable: true },
            period: period(),
            as_of: period().to(),
            scenario: Scenario::Actual,
            detail: Detail::Detailed,
            columns: ColumnSpec::Single,
            filter: ReportFilter::default(),
            exceptions_only: false,
        },
        columns: headings(),
        rows,
        totals: Vec::new(),
        notes: Vec::new(),
    }
}

fn party_row(id: LedgerId) -> ReportRow {
    ReportRow {
        key: Some(DrillKey::Ledger(id)),
        label: String::new(),
        depth: 0,
        kind: RowKind::Ledger,
        cells: vec![Cell::Empty; 11],
    }
}

fn bill_row(id: LedgerId, name: &str, buckets: &[usize]) -> ReportRow {
    // Column 0 is the bill date, 2 the pending amount; `buckets` are the age columns filled.
    let cells = (0..11)
        .map(|i| match i {
            0 => Cell::Date(date!(2026 - 04 - 10)),
            2 => Cell::Amount(SignedAmount::dr(rs(100))),
            i if buckets.contains(&i) => Cell::Amount(SignedAmount::dr(rs(100))),
            _ => Cell::Empty,
        })
        .collect();
    ReportRow {
        key: Some(DrillKey::Bill {
            ledger: id,
            name: name.to_owned(),
        }),
        label: name.to_owned(),
        depth: 1,
        kind: RowKind::Bill,
        cells,
    }
}

#[test]
fn only_the_partys_own_rows_are_kept() {
    // provenance: derived. Another party's ledger and bill rows are skipped; the party's bill is
    // kept with its bucket.
    let r = report(vec![
        party_row(QUIET),
        bill_row(QUIET, "Q-1", &[4]),
        party_row(CUSTOMER),
        bill_row(CUSTOMER, "C-1", &[5]),
    ]);
    let lines = party_lines(&r, CUSTOMER).unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].name, "C-1");
    assert_eq!(lines[0].age, BillAge::Overdue("31-60 days".to_owned()));
    assert_eq!(party_lines(&r, CASH), None);
}

#[test]
fn a_bill_in_no_age_column_or_in_two_is_not_given_an_age() {
    // provenance: derived (rule A1). A bill must sit in exactly one of the report's age columns;
    // otherwise its age is unknown and the badge is a dash, never a guess.
    let r = report(vec![
        party_row(CUSTOMER),
        bill_row(CUSTOMER, "none", &[]),
        bill_row(CUSTOMER, "two", &[3, 7]),
    ]);
    let lines = party_lines(&r, CUSTOMER).unwrap();
    for line in &lines {
        assert!(matches!(line.age, BillAge::Unknown(_)), "{line:?}");
        assert_eq!(age_badge(&line.age), Some(("\u{2014}".to_owned(), false)));
    }
}

#[test]
fn the_badges_name_the_reports_bucket() {
    // provenance: index.html:1806: `${x.overdue} days overdue` in the warning style, or "Not due"
    // in the neutral style. Derived: the report gives the bucket, not the day count, so the badge
    // names the bucket; a bill with no due date says so; the on-account line has no badge.
    assert_eq!(
        age_badge(&BillAge::Overdue("61-90 days".to_owned())),
        Some(("61-90 days overdue".to_owned(), true))
    );
    assert_eq!(
        age_badge(&BillAge::NotDue),
        Some(("Not due".to_owned(), false))
    );
    assert_eq!(
        age_badge(&BillAge::NoDueDate),
        Some(("No due date".to_owned(), false))
    );
    assert_eq!(age_badge(&BillAge::OnAccount), None);
}

#[test]
fn a_report_the_engine_cannot_give_is_said_unless_the_other_lists_the_party() {
    // provenance: derived. Receivable and payable are both asked. When the party is in one, its
    // lines are shown even if the other failed; when it is in neither and one failed, the party
    // may be in the failed one, so the reason is shown instead of "nothing open".
    let nope = || {
        Ok(Outcome::Unsupported(Unsupported {
            what: "outstanding".to_owned(),
            reason: "a party has an opening balance".to_owned(),
        }))
    };
    let listed = || Ok(Outcome::Value(report(vec![party_row(CUSTOMER)])));
    assert_eq!(
        from_answers(CUSTOMER, [nope(), listed()]),
        BillsState::Loaded(Vec::new())
    );
    let state = from_answers(QUIET, [nope(), listed()]);
    let BillsState::Unsupported(why) = state else {
        panic!("{state:?}");
    };
    assert!(why.contains("opening balance"), "{why}");
}

fn frame(ctx: &egui::Context, screen: &mut MastersScreen, app: &mut AppCtx) {
    let out = ctx.run_ui(egui::RawInput::default(), |ui| {
        let _action = screen.ui(ui, app);
    });
    out.drop_without_applying_deltas();
}

#[test]
fn altering_a_bill_wise_party_asks_for_its_bills() {
    // provenance: derived, as in the first test: INV-1 is the first open bill, 6,000.00 Dr. The
    // list asks for nothing; the alter form of a bill-wise party asks for its bills.
    let api = setup("w2_masters_bills_screen.ledgerly");
    let Ok(Outcome::Value(books)) = api.books() else {
        panic!("the books did not load");
    };
    let mut app = AppCtx {
        tr: ledgerly_app::i18n::Tr::default(),
        books: Some(books.clone()),
        api: Some(ApiWorker::spawn(api).unwrap()),
        lock_date: None,
        ai_tier: None,
    };
    let ctx = egui::Context::default();
    let mut screen = MastersScreen::new();
    frame(&ctx, &mut screen, &mut app);
    assert_eq!(screen.bills().state(), &BillsState::NotAsked);
    assert!(!screen.bills().is_loading());
    screen.select(MasterRef {
        kind: MasterKind::Ledger,
        id: CUSTOMER.0,
    });
    screen.begin_alter(&books).unwrap();
    for _ in 0..2000 {
        frame(&ctx, &mut screen, &mut app);
        if matches!(screen.bills().state(), BillsState::Loaded(_)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let BillsState::Loaded(lines) = screen.bills().state() else {
        panic!("no bills: {:?}", screen.bills().state());
    };
    assert_eq!(lines[0].name, "INV-1");
    assert_eq!(lines[0].pending, Ok(SignedAmount::dr(rs(6_000))));
}
