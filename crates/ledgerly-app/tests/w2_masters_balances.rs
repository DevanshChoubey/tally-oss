//! The Balance column of the masters ledger list comes from the engine's trial balance, run on
//! the API worker (`design/mock-v2/index.html:1796`: `Balance ${prov('eng','engine')}`, each row
//! `inr(Math.abs(b[l.id])) Dr|Cr`).
//!
//! Provenance: every expected figure is `derived` by hand from the vouchers posted below, with the
//! arithmetic next to it. The figure is the ledger's closing balance as the trial balance gives
//! it (the trial balance's own provenance is in `w7_report_worker.rs`), as of the last voucher
//! date, the date the balance sheet opens on.
//!
//! The screen joins the engine's rows to ledgers by id only. A ledger the trial balance has no row
//! for (the engine leaves out a nil balance) shows nothing, never a made-up 0.00; a cell the
//! engine cannot compute shows a dash.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use ledgerly_api::{
    Api, ApiError, Cell, ColumnHeader, ColumnSpec, Command, Detail, DrillKey, Report, ReportFilter,
    ReportKind, ReportParams, ReportRow, RowKind,
};
use ledgerly_app::screens::masters::MastersScreen;
use ledgerly_app::screens::masters::balances::{
    BalanceCell, BalanceState, LedgerBalances, from_report,
};
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{
    Clock, CompanyId, Date, GroupId, LedgerId, Outcome, Paise, Period, SignedAmount, StateCode,
    Unsupported, VoucherId,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, Ledger, LedgerLine,
    NegativeStockPolicy, ReservedGroup, Scenario, Voucher, VoucherMode, VoucherStatus,
};
use time::OffsetDateTime;
use time::macros::date;

const CASH: LedgerId = LedgerId(1);
const CAPITAL: LedgerId = LedgerId(2);
const CUSTOMER: LedgerId = LedgerId(3);
const IDLE: LedgerId = LedgerId(4);

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

fn setup(name: &str) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let mut api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    for ledger in [
        plain(CASH, "Cash", ReservedGroup::CashInHand),
        plain(CAPITAL, "Capital", ReservedGroup::CapitalAccount),
        plain(CUSTOMER, "Customer", ReservedGroup::SundryDebtors),
        plain(IDLE, "Idle Debtor", ReservedGroup::SundryDebtors),
    ] {
        committed(api.execute(Command::CreateLedger(ledger)));
    }
    let on = date!(2026 - 04 - 10);
    for v in [
        journal(
            1,
            on,
            vec![
                line(CASH, SignedAmount::dr(rs(100_000))),
                line(CAPITAL, SignedAmount::cr(rs(100_000))),
            ],
        ),
        journal(
            2,
            on,
            vec![
                line(CASH, SignedAmount::dr(rs(5_000))),
                line(CUSTOMER, SignedAmount::cr(rs(5_000))),
            ],
        ),
    ] {
        committed(api.execute(Command::PostVoucher(v)));
    }
    api
}

fn loaded(worker: &ApiWorker, as_of: Date) -> LedgerBalances {
    let mut balances = LedgerBalances::new();
    balances.request(as_of);
    for _ in 0..2000 {
        balances.tick(Some(worker));
        if !balances.is_loading() {
            return balances;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the balances did not arrive within the bound");
}

#[test]
fn each_ledger_shows_its_closing_balance_from_the_engine() {
    // provenance: derived. Posted on 10 Apr 2026: V1 Cash Dr 1,00,000 / Capital Cr 1,00,000;
    // V2 Cash Dr 5,000 / Customer Cr 5,000. Openings are nil.
    // Cash: Dr 1,00,000 + Dr 5,000 = 1,05,000.00 Dr.
    // Capital: 1,00,000.00 Cr. Customer: 5,000.00 Cr.
    // Idle Debtor: no posting and no opening, a nil balance. The trial balance has no row for it
    // (ledgerly-reports trial_balance.rs skips a zero balance), so the cell is left empty.
    let worker = ApiWorker::spawn(setup("w2_masters_balances.ledgerly")).unwrap();
    let b = loaded(&worker, date!(2026 - 04 - 30));
    assert_eq!(b.text(CASH), "1,05,000.00 Dr");
    assert_eq!(b.text(CAPITAL), "1,00,000.00 Cr");
    assert_eq!(b.text(CUSTOMER), "5,000.00 Cr");
    assert_eq!(b.text(IDLE), "");
    assert_eq!(b.heading_hint(), None);
}

#[test]
fn before_the_vouchers_every_balance_is_nil_and_left_empty() {
    // provenance: derived. As of 1 Apr 2026 nothing is posted yet (both vouchers are dated
    // 10 Apr) and openings are nil, so the trial balance has no ledger rows at all.
    let worker = ApiWorker::spawn(setup("w2_masters_balances_early.ledgerly")).unwrap();
    let b = loaded(&worker, date!(2026 - 04 - 01));
    assert_eq!(b.text(CASH), "");
}

fn report(rows: Vec<ReportRow>) -> Report {
    let on = date!(2026 - 04 - 30);
    Report {
        title: "Trial balance".to_owned(),
        params: ReportParams {
            kind: ReportKind::TrialBalance,
            period: Period::new(date!(2026 - 04 - 01), on).unwrap(),
            as_of: on,
            scenario: Scenario::Actual,
            detail: Detail::Detailed,
            columns: ColumnSpec::Single,
            filter: ReportFilter::default(),
            exceptions_only: false,
        },
        columns: vec![
            ColumnHeader {
                label: "Debit".to_owned(),
                period: None,
            },
            ColumnHeader {
                label: "Credit".to_owned(),
                period: None,
            },
        ],
        rows,
        totals: Vec::new(),
        notes: Vec::new(),
    }
}

fn row(key: DrillKey, kind: RowKind, cells: Vec<Cell>) -> ReportRow {
    ReportRow {
        key: Some(key),
        label: String::new(),
        depth: 1,
        kind,
        cells,
    }
}

#[test]
fn rows_are_joined_to_ledgers_by_id_and_group_rows_are_skipped() {
    // provenance: derived from the trial balance's layout (ledgerly-reports trial_balance.rs):
    // a ledger row has a Debit and a Credit cell, one of them filled. A group row is a total of
    // its ledgers and belongs to no ledger. A cell the engine could not compute stays unsupported,
    // and a cell that is not an amount is never read as one.
    let gap = Unsupported {
        what: "closing balance".to_owned(),
        reason: "test".to_owned(),
    };
    let r = report(vec![
        row(
            DrillKey::Group(GroupId(1)),
            RowKind::Group,
            vec![Cell::Amount(SignedAmount::dr(rs(9))), Cell::Empty],
        ),
        row(
            DrillKey::Ledger(CASH),
            RowKind::Ledger,
            vec![Cell::Amount(SignedAmount::dr(rs(7))), Cell::Empty],
        ),
        row(
            DrillKey::Ledger(CAPITAL),
            RowKind::Ledger,
            vec![Cell::Empty, Cell::Amount(SignedAmount::cr(rs(2)))],
        ),
        row(
            DrillKey::Ledger(CUSTOMER),
            RowKind::Ledger,
            vec![Cell::Unsupported(gap), Cell::Empty],
        ),
        row(
            DrillKey::Ledger(IDLE),
            RowKind::Ledger,
            vec![Cell::Text("7".to_owned()), Cell::Empty],
        ),
    ]);
    let map = from_report(&r);
    assert_eq!(map.len(), 4);
    assert_eq!(map[&CASH], BalanceCell::Amount(SignedAmount::dr(rs(7))));
    assert_eq!(map[&CAPITAL], BalanceCell::Amount(SignedAmount::cr(rs(2))));
    assert!(matches!(map[&CUSTOMER], BalanceCell::Unsupported(_)));
    assert!(matches!(map[&IDLE], BalanceCell::Unsupported(_)));
}

#[test]
fn what_the_column_shows_in_each_state() {
    // provenance: derived. Until the engine answers, and when it cannot answer, the column shows
    // a dash and the heading says why on hover; nothing is estimated (rule C4). A loaded cell
    // the engine could not compute is a dash too.
    let mut b = LedgerBalances::new();
    assert_eq!(b.text(CASH), "\u{2014}");
    assert!(b.heading_hint().is_some());
    let mut map = BTreeMap::new();
    map.insert(CASH, BalanceCell::Amount(SignedAmount::dr(rs(1_000))));
    map.insert(CAPITAL, BalanceCell::Unsupported("no".to_owned()));
    b.set(BalanceState::Loaded(map));
    assert_eq!(b.text(CASH), "1,000.00 Dr");
    assert_eq!(b.text(CAPITAL), "\u{2014}");
    assert_eq!(b.text(CUSTOMER), "");
    b.set(BalanceState::Unsupported("the report failed".to_owned()));
    assert_eq!(b.text(CASH), "\u{2014}");
    assert!(b.heading_hint().is_some());
}

#[test]
fn without_a_worker_nothing_is_asked_and_nothing_is_shown() {
    // provenance: derived. With no company file open there is no worker; the request waits and
    // the column keeps its dashes.
    let mut b = LedgerBalances::new();
    b.request(date!(2026 - 04 - 30));
    b.tick(None);
    assert_eq!(b.text(CASH), "\u{2014}");
}

fn frame(ctx: &egui::Context, screen: &mut MastersScreen, app: &mut AppCtx) {
    let out = ctx.run_ui(egui::RawInput::default(), |ui| {
        let _stay = screen.ui(ui, app);
    });
    out.drop_without_applying_deltas();
}

#[test]
fn the_screen_asks_on_open_and_again_after_a_save() {
    // provenance: derived. The screen asks for the balances as of the last voucher date
    // (10 Apr 2026, the date the balance sheet opens on), when it is first drawn; Cash is
    // 1,05,000.00 Dr as derived in `each_ledger_shows_its_closing_balance_from_the_engine`.
    // After anything is saved (the shell's change count moves) it asks again, so a posting never
    // leaves an old balance on screen.
    let api = setup("w2_masters_balances_screen.ledgerly");
    let Ok(Outcome::Value(books)) = api.books() else {
        panic!("the books did not load");
    };
    let mut app = AppCtx {
        books: Some(books),
        api: Some(ApiWorker::spawn(api).unwrap()),
        lock_date: None,
        ai_tier: None,
    };
    let ctx = egui::Context::default();
    let mut screen = MastersScreen::new();
    for _ in 0..2000 {
        frame(&ctx, &mut screen, &mut app);
        if !screen.balances().is_loading() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(screen.balances().text(CASH), "1,05,000.00 Dr");
    frame(&ctx, &mut screen, &mut app);
    assert!(
        !screen.balances().is_loading(),
        "nothing changed, nothing is asked"
    );
    ledgerly_app::shell::submit::note_changed();
    frame(&ctx, &mut screen, &mut app);
    assert!(screen.balances().is_loading(), "a save asks again");
}
