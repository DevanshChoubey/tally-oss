//! Card W4 tests: the Review queue screen and the Home card's list, over a real `Api` on a
//! company file, run on the API worker.
//!
//! Provenance. The company is the one of `c3_bank_worker.rs`: a bank ledger with a debit opening
//! of Rs 1,000.00, a customer, one receipt of Rs 500.00 on 5 Apr 2026 into the bank, and an MT940
//! statement with one credit of 500,00 on 5 Apr. The one proposal is therefore the exact match of
//! that line to that voucher, derived by hand (same amount, same day). The four not-connected
//! sources are named in `ledgerly-api/src/review.rs`.
//!
//! Tally citation, read 2026-11-23: the "Display Optional Vouchers" page under Exception
//! Reports (`help.tallysolutions.com/docs/te9rel66/Reports/MIS_Reports/Display_Optional_Vouchers.htm`)
//! is the nearest report of the product; it is not a queue of proposals, so the queue is not
//! built from it. See the PARITY-GAPS entry.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use ledgerly_api::{Api, ApiError, Command, CommandResult, ReviewSource};
use ledgerly_app::screens::home::HomeScreen;
use ledgerly_app::screens::review::{
    Decision, EmptyView, ReviewScreen, badge, empty_view, reject_notice, rows,
};
use ledgerly_app::worker::ApiWorker;
use ledgerly_core::{
    Clock, CompanyId, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode, VoucherId,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BankAllocation, BankDetails, BaseVoucherKind, Company, EntityKind, Features,
    InstrumentKind, Ledger, LedgerLine, NegativeStockPolicy, ReservedGroup, Voucher, VoucherMode,
    VoucherStatus,
};
use time::OffsetDateTime;
use time::macros::date;

const BANK: LedgerId = LedgerId(10);
const CUSTOMER: LedgerId = LedgerId(12);

const STATEMENT: &str = ":20:S1\r\n:25:0123456789\r\n:60F:C260401INR1000,00\r\n:61:260405C500,00NTRFNONREF//BK1\r\n:62F:C260430INR1500,00\r\n";

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

fn ledger(
    id: LedgerId,
    name: &str,
    kind: ReservedGroup,
    bank: Option<BankDetails>,
    opening: i64,
) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: Some(group(kind)),
        opening: SignedAmount::dr(Paise::new(opening)),
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn receipt() -> Voucher {
    let type_id = default_voucher_types()
        .into_iter()
        .find(|t| t.base == BaseVoucherKind::Receipt)
        .unwrap()
        .id;
    let bank_line = LedgerLine {
        ineligible_itc: None,
        ledger: BANK,
        amount: SignedAmount::dr(Paise::new(50_000)),
        bills: Vec::new(),
        cost: Vec::new(),
        bank: Some(BankAllocation {
            instrument: InstrumentKind::Neft,
            instrument_number: None,
            instrument_date: None,
            bank_date: None,
            favouring: None,
        }),
    };
    let party_line = LedgerLine {
        ineligible_itc: None,
        ledger: CUSTOMER,
        amount: SignedAmount::cr(Paise::new(50_000)),
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    };
    Voucher {
        id: VoucherId(1),
        type_id,
        number: None,
        series: None,
        date: date!(2026 - 04 - 05),
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
        ledger_lines: vec![bank_line, party_line],
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn ok(out: Result<Outcome<CommandResult>, ApiError>) {
    match out {
        Ok(Outcome::Value(_)) => {}
        other => panic!("expected a committed value, got {other:?}"),
    }
}

/// A company file with the bank and customer ledgers. With `with_proposal`, the receipt is
/// posted and the statement imported, so the queue holds the one exact match.
fn setup(name: &str, with_proposal: bool) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let clock = Clock::Fixed(OffsetDateTime::UNIX_EPOCH);
    let mut api = Api::create(&path, &company(), clock).unwrap();
    let details = BankDetails {
        account_number: "0123456789".to_owned(),
        ifsc: "TEST0000001".to_owned(),
        bank_name: "Test Bank".to_owned(),
        branch: "Main".to_owned(),
    };
    ok(api.execute(Command::CreateLedger(ledger(
        BANK,
        "Bank Current Account",
        ReservedGroup::BankAccounts,
        Some(details),
        100_000,
    ))));
    ok(api.execute(Command::CreateLedger(ledger(
        CUSTOMER,
        "Customer A",
        ReservedGroup::SundryDebtors,
        None,
        0,
    ))));
    if with_proposal {
        ok(api.execute(Command::PostVoucher(receipt())));
        match api
            .import_bank_statement(STATEMENT.as_bytes(), "generic-mt940", BANK)
            .unwrap()
        {
            Outcome::Value(_) => {}
            other @ Outcome::Unsupported(_) => panic!("statement not imported: {other:?}"),
        }
    }
    api
}

/// Runs the Review screen's load on the worker until the answer is in. Bounded.
fn load_queue(screen: &mut ReviewScreen, worker: &ApiWorker) {
    screen.request_load();
    for _ in 0..2000 {
        screen.tick(Some(worker));
        if screen.panel().is_some() || screen.error().is_some() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the review queue did not arrive within the bound");
}

#[test]
fn the_rows_come_from_the_engine_proposal() {
    // provenance: derived. One exact match: statement line 4 (5 Apr, credit 500,00 = 50_000
    // paise) to voucher 1 (5 Apr). The row's kind, amount and ledger are written out here.
    let worker = ApiWorker::spawn(setup("w4_app_rows.ledgerly", true)).unwrap();
    let mut screen = ReviewScreen::default();
    load_queue(&mut screen, &worker);
    let panel = screen.panel().unwrap();
    assert_eq!(panel.rows.len(), 1);
    let row = &panel.rows[0];
    assert_eq!(row.kind, "Bank match");
    assert_eq!(row.amount, Some(Paise::new(50_000)));
    assert_eq!(row.ledger, Some(BANK));
    assert!(row.what.ends_with("voucher 1"), "what = {}", row.what);
    assert!(row.source.contains("line 4"), "source = {}", row.source);
    assert_eq!(screen.opens(), Some(BANK));
}

#[test]
fn the_count_is_withheld_and_matches_the_rows_when_every_source_is_listed() {
    // provenance: derived. While four sources are unlisted the count is `None` (never a partial
    // number). With those removed from the answer, the count is the row count: one.
    let api = setup("w4_app_count.ledgerly", true);
    let mut queue = api.review_queue().unwrap();
    let worker = ApiWorker::spawn(api).unwrap();
    let mut screen = ReviewScreen::default();
    load_queue(&mut screen, &worker);
    let panel = screen.panel().unwrap();
    assert_eq!(panel.count, None);
    assert_eq!(panel.not_connected.len(), 4);

    queue.unlisted.clear();
    assert_eq!(badge(&queue), Some(1));
    assert_eq!(rows(&queue).len(), 1);
}

#[test]
fn j_and_k_move_the_selection_within_the_rows() {
    // provenance: derived. One row: the selection stays on row 0 whichever way it moves.
    let worker = ApiWorker::spawn(setup("w4_app_keys.ledgerly", true)).unwrap();
    let mut screen = ReviewScreen::default();
    load_queue(&mut screen, &worker);
    screen.move_by(1);
    assert_eq!(screen.selected(), 0);
    screen.move_by(-1);
    assert_eq!(screen.selected(), 0);
    assert_eq!(screen.opens(), Some(BANK));
}

#[test]
fn a_company_with_nothing_to_review_names_the_sources_it_cannot_list() {
    // provenance: derived. No statement and no receipt: no rows. The four unlisted sources are
    // named, so the empty state does not claim the mock's "Nothing to review" wording.
    let api = setup("w4_app_empty.ledgerly", false);
    let queue = api.review_queue().unwrap();
    assert!(matches!(empty_view(&queue), EmptyView::NotConnected(_)));
    assert!(
        queue
            .unlisted
            .iter()
            .any(|u| u.source == ReviewSource::Gstr2bExplanation)
    );
    let worker = ApiWorker::spawn(api).unwrap();
    let mut screen = ReviewScreen::default();
    load_queue(&mut screen, &worker);
    let panel = screen.panel().unwrap();
    assert_eq!(panel.rows.len(), 0);
    assert_eq!(screen.opens(), None);
    assert_eq!(panel.count, None);
    assert!(panel.not_connected[0].starts_with("Bill photos:"));
}

#[test]
fn the_home_card_shows_the_same_rows_as_the_review_queue() {
    // provenance: derived. Both come from one `Api::review_queue` answer for the same company,
    // so the Home card's panel equals the Review queue's panel.
    let worker = ApiWorker::spawn(setup("w4_app_home.ledgerly", true)).unwrap();
    let mut screen = ReviewScreen::default();
    load_queue(&mut screen, &worker);
    let mut home = HomeScreen::default();
    home.request_refresh(date!(2026 - 04 - 30));
    for _ in 0..2000 {
        home.tick(Some(&worker));
        if home.review_panel().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(home.review_panel(), screen.panel());
}

/// Every text the frame drew, in drawing order.
fn drawn(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    fn walk(shape: &egui::epaint::Shape, out: &mut Vec<String>) {
        match shape {
            egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_owned()),
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            egui::epaint::Shape::Noop
            | egui::epaint::Shape::Circle(_)
            | egui::epaint::Shape::Ellipse(_)
            | egui::epaint::Shape::LineSegment { .. }
            | egui::epaint::Shape::Path(_)
            | egui::epaint::Shape::Band(_)
            | egui::epaint::Shape::Rect(_)
            | egui::epaint::Shape::Mesh(_)
            | egui::epaint::Shape::QuadraticBezier(_)
            | egui::epaint::Shape::CubicBezier(_)
            | egui::epaint::Shape::Callback(_) => {}
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, &mut out);
    }
    out
}

#[test]
fn the_queue_is_a_table_beside_the_how_decisions_work_card() {
    // provenance: the mock's `SCREENS.review` (design/mock-v2/index.html) and its capture
    // `captures/states/light/review.default.webp`: a table with the columns Proposal, Source,
    // Confidence and Amount beside a card titled "How decisions work". The keys listed there are
    // the ones this screen has. Since s11 Y, Ctrl+A, N and Alt+D are built (see the tests below),
    // so the card lists them as the mock does. Catches the plain stack of labels the screen drew
    // before 2026-11-25.
    use ledgerly_app::{AppCtx, Screen};
    let worker = ApiWorker::spawn(setup("w4_app_look.ledgerly", true)).unwrap();
    let mut screen = ReviewScreen::default();
    load_queue(&mut screen, &worker);
    let ctx = egui::Context::default();
    let mut app = AppCtx::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1100.0, 800.0),
        )),
        ..egui::RawInput::default()
    };
    let out = ctx.run_ui(input, |ui| {
        let _ = screen.ui(ui, &mut app);
    });
    let text = drawn(&out.shapes);
    out.drop_without_applying_deltas();
    for wanted in [
        "Proposal",
        "Source",
        "Confidence",
        "Amount",
        "How decisions work",
        "Bank match",
    ] {
        assert!(
            text.iter().any(|t| t == wanted),
            "missing {wanted:?} in {text:?}"
        );
    }
    for built in ["Accept", "Reject"] {
        assert!(
            text.iter().any(|t| t.starts_with(built)),
            "{built} is built, so the card must offer it: {text:?}"
        );
    }
}

/// Ticks the screen until `done` holds, bounded.
fn tick_until(screen: &mut ReviewScreen, worker: &ApiWorker, done: impl Fn(&ReviewScreen) -> bool) {
    for _ in 0..2000 {
        screen.tick(Some(worker));
        if done(screen) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the screen did not reach the wanted state within the bound");
}

#[test]
fn accepting_the_exact_match_confirms_it_and_the_row_leaves_the_queue() {
    // provenance: derived. The one proposal is the exact match of statement line 4 (50_000 paise,
    // 5 Apr) to voucher 1. The engine confirms it (sets the bank date on the voucher's bank
    // line), after which the matcher has nothing left to suggest, so the reloaded queue is empty.
    let worker = ApiWorker::spawn(setup("s11_review_accept.ledgerly", true)).unwrap();
    let mut screen = ReviewScreen::default();
    load_queue(&mut screen, &worker);
    assert_eq!(screen.panel().unwrap().rows.len(), 1);
    screen.decide(Decision::Accept, Some(&worker));
    tick_until(&mut screen, &worker, |s| {
        s.panel().is_some_and(|p| p.rows.is_empty())
    });
    let notice = screen.notice().unwrap();
    assert!(notice.starts_with("Accepted"), "notice = {notice}");
}

#[test]
fn rejecting_hides_the_row_for_the_session_and_says_so() {
    // provenance: derived. N removes the one row and nothing is sent to the engine (so the
    // statement is still unmatched). A reload of the queue still lists the proposal, because the
    // engine keeps no rejection, but the screen keeps it hidden for the session.
    let worker = ApiWorker::spawn(setup("s11_review_reject.ledgerly", true)).unwrap();
    let mut screen = ReviewScreen::default();
    load_queue(&mut screen, &worker);
    let row = screen.panel().unwrap().rows[0].clone();
    screen.decide(Decision::Reject, Some(&worker));
    assert_eq!(screen.panel().unwrap().rows.len(), 0);
    assert_eq!(screen.notice(), Some(reject_notice(&row).as_str()));
    screen.request_load();
    for _ in 0..50 {
        screen.tick(Some(&worker));
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(screen.panel().unwrap().rows.len(), 0);
}
