//! Card C5: the palette searches through the API worker, a pick opens the right screen, the
//! report menu opens worker-mode reports, and a voucher row of a report opens the voucher.
//!
//! Provenance. Every expected value is `derived`, with the arithmetic next to it. Nothing is
//! copied from this crate's output.
//!
//! Sources, all read 2026-11-21:
//! * Tally, keyboard shortcuts <https://help.tallysolutions.com/keyboard-shortcuts-tally-prime/>:
//!   Enter is "To drill-down and open a voucher or master from the last level details of a
//!   report"; Ctrl+Enter (TallyPrime) is "To drill-down and open a voucher for display".
//! * Tally, Day Book <https://help.tallysolutions.com/tally-prime/accounting-financial-reports/day-book-tally/>:
//!   "Now, you can Press Enter to open the transaction, and check the required details."
//! * Tally, Trial Balance <https://help.tallysolutions.com/tally-prime/accounting-financial-reports/trial-balance-tally/>:
//!   "By default, a group-wise view of the Trial Balance appears" and the report "will be generated
//!   as on the date of the last voucher entry".
//! * Spec section 7 (`docs/specs/2026-11-20-ledgerly-design.md`): the palette searches masters
//!   by name, alias and GSTIN, and vouchers by number, party, amount and date.
//! * GSTIN `29ABCDE1234F1ZW` is hand-derived valid in `ledgerly-core/src/gstin.rs` (its check
//!   character is worked out there).
//!
//! `pending`: Tally opens a voucher from a report for alteration. Ledgerly has no alteration
//! screen yet, so the drill opens a read-only display (see `voucher_view.rs`). No test claims
//! otherwise.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use ledgerly_api::{Api, ApiError, Command, Detail, ReportKind, SearchHit};
use ledgerly_app::palette::{
    Palette, PaletteChoice, PaletteEntry, PaletteMode, PaletteSearch, PaletteTarget,
    entries_from_hits, unavailable_reason,
};
use ledgerly_app::screens::report_view::{
    ReportViewScreen, ViewCell, ViewKey, VoucherViewError, voucher_view,
};
use ledgerly_app::shell::navigate::{
    self, NavError, default_params, menu_entries, open_target, report_from_key, report_key,
    report_screen,
};
use ledgerly_app::shell::{StackChange, Tabs};
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{
    Clock, CompanyId, Date, DrCr, GroupId, Gstin, LedgerId, Outcome, Paise, Period, SignedAmount,
    StateCode, VoucherId,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, Ledger, LedgerLine, Masters,
    NegativeStockPolicy, PartyDetails, RegistrationType, ReservedGroup, Voucher, VoucherMode,
    VoucherStatus,
};
use time::OffsetDateTime;
use time::macros::date;

const CASH: LedgerId = LedgerId(1);
const CAPITAL: LedgerId = LedgerId(2);
const CUSTOMER: LedgerId = LedgerId(3);
const CUSTOMER_GSTIN: &str = "29ABCDE1234F1ZW";

fn rs(rupees: i64) -> Paise {
    Paise::new(rupees * 100)
}

fn clock() -> Clock {
    Clock::Fixed(OffsetDateTime::UNIX_EPOCH)
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

/// Customer One: alias "Acme Stores", registered with `CUSTOMER_GSTIN`.
fn customer() -> Ledger {
    let mut ledger = plain(CUSTOMER, "Customer One", ReservedGroup::SundryDebtors);
    ledger.aliases = vec!["Acme Stores".to_owned()];
    ledger.party = Some(PartyDetails {
        registration: RegistrationType::Regular,
        gstin: Some(Gstin::parse(CUSTOMER_GSTIN).unwrap()),
        pan: None,
        state: Some(StateCode::new(29).unwrap()),
        address: Address::default(),
    });
    ledger
}

fn ledgers() -> Vec<Ledger> {
    vec![
        plain(CASH, "Cash", ReservedGroup::CashInHand),
        plain(CAPITAL, "Capital", ReservedGroup::CapitalAccount),
        customer(),
    ]
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
        extra: BTreeMap::new(),
    }
}

/// The two vouchers of the fixture. Voucher 1: capital of 1,00,000 paid into Cash. Voucher 2: an
/// advance of 5,000 from Customer One into Cash. Both on 10 April 2026.
fn vouchers() -> Vec<Voucher> {
    vec![
        journal(
            1,
            date!(2026 - 04 - 10),
            vec![
                line(CASH, SignedAmount::dr(rs(100_000))),
                line(CAPITAL, SignedAmount::cr(rs(100_000))),
            ],
        ),
        journal(
            2,
            date!(2026 - 04 - 10),
            vec![
                line(CASH, SignedAmount::dr(rs(5_000))),
                line(CUSTOMER, SignedAmount::cr(rs(5_000))),
            ],
        ),
    ]
}

/// The books a shell would hold in `AppCtx::books` for the fixture.
fn books() -> Books {
    let mut masters = Masters::default();
    for ledger in ledgers() {
        masters.ledgers.insert(ledger.id, ledger);
    }
    for voucher_type in default_voucher_types() {
        masters.voucher_types.insert(voucher_type.id, voucher_type);
    }
    Books {
        company: company(),
        masters,
        vouchers: vouchers().into_iter().map(|v| (v.id, v)).collect(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

fn ctx_with_books() -> AppCtx {
    AppCtx {
        books: Some(books()),
        ..AppCtx::default()
    }
}

fn committed(out: Result<Outcome<ledgerly_api::CommandResult>, ApiError>) {
    match out {
        Ok(Outcome::Value(_)) => {}
        other => panic!("expected a committed value, got {other:?}"),
    }
}

/// A company file with the three ledgers and two vouchers of the fixture.
fn api(name: &str) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let mut api = Api::create(&path, &company(), clock()).unwrap();
    for ledger in ledgers() {
        committed(api.execute(Command::CreateLedger(ledger)));
    }
    for voucher in vouchers() {
        committed(api.execute(Command::PostVoucher(voucher)));
    }
    api
}

fn settle_search(search: &mut PaletteSearch, palette: &mut Palette, worker: &ApiWorker) {
    search.tick(palette, Some(worker));
    for _ in 0..2000 {
        if !search.is_pending() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
        search.tick(palette, Some(worker));
    }
    panic!("the search did not answer within the bound");
}

fn settle_report(screen: &mut ReportViewScreen, worker: &ApiWorker) {
    for _ in 0..2000 {
        screen.tick(Some(worker));
        if !screen.is_loading() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the report did not arrive within the bound");
}

fn searched(file: &str, query: &str, host: Vec<PaletteEntry>) -> (Palette, ApiWorker) {
    let worker = ApiWorker::spawn(api(file)).unwrap();
    let mut palette = Palette::new();
    palette.set_entries(host);
    palette.open(PaletteMode::OnTop);
    palette.set_query(query);
    let mut search = PaletteSearch::new();
    settle_search(&mut search, &mut palette, &worker);
    (palette, worker)
}

fn targets(palette: &Palette) -> Vec<PaletteTarget> {
    palette.results().iter().map(|e| e.target.clone()).collect()
}

struct Root;

impl Screen for Root {
    fn title(&self) -> String {
        "Home".to_owned()
    }

    fn ui(&mut self, _ui: &mut egui::Ui, _ctx: &mut AppCtx) -> ScreenAction {
        ScreenAction::Stay
    }
}

fn choice(target: PaletteTarget, mode: PaletteMode) -> PaletteChoice {
    PaletteChoice {
        target,
        title: "picked".to_owned(),
        mode,
    }
}

// ---- palette through the worker --------------------------------------------------------------

#[test]
fn a_ledger_is_found_by_its_alias_through_the_worker() {
    // provenance: derived. Spec section 7: masters are searched by name, alias and GSTIN; the API
    // rule (search.rs): "a substring of an alias matches". "acme" is a substring of the alias
    // "Acme Stores" of Customer One (ledger 3) and of no other ledger name or alias, and of no
    // voucher number, party, amount or date, report title or setting. One row, for ledger 3.
    // The row stays in the palette: its alias is the query, an exact match (1000 - 50).
    let (palette, _worker) = searched("c5_alias.ledgerly", "acme", Vec::new());
    assert_eq!(
        targets(&palette),
        vec![PaletteTarget::Master("ledger:3".to_owned())]
    );
    assert_eq!(palette.results()[0].title, "Customer One");
    assert_eq!(palette.note(), None);
}

#[test]
fn a_ledger_is_found_by_its_gstin_through_the_worker() {
    // provenance: derived. Spec section 7 (GSTIN). The API rule: a GSTIN matches when it equals
    // the query. Customer One holds 29ABCDE1234F1ZW; the other ledgers have no GSTIN. The query
    // is no amount and no date, so no voucher matches.
    let (palette, _worker) = searched("c5_gstin.ledgerly", CUSTOMER_GSTIN, Vec::new());
    assert_eq!(
        targets(&palette),
        vec![PaletteTarget::Master("ledger:3".to_owned())]
    );
    assert_eq!(palette.results()[0].title, "Customer One");
}

#[test]
fn a_voucher_is_found_by_its_amount_through_the_worker() {
    // provenance: derived. API rule: the query read as a rupee amount matches when a ledger line
    // has that amount. 5000 rupees is 500000 paise: voucher 2 has Cash Dr 5,000 and Customer One
    // Cr 5,000. Voucher 1 has lines of 1,00,000. No ledger name, alias, report or setting holds
    // "5000".
    let (palette, _worker) = searched("c5_amount.ledgerly", "5000", Vec::new());
    assert_eq!(
        targets(&palette),
        vec![PaletteTarget::Voucher("voucher:2".to_owned())]
    );
}

#[test]
fn a_report_the_host_lists_is_not_shown_twice() {
    // provenance: derived. The host lists the report menu, whose "Trial Balance" row has the key
    // "trial-balance". The API search for "trial" also returns the report "Trial balance" (the
    // only title containing "trial"), which has the same key, so only the host's row stays.
    let (palette, _worker) = searched("c5_dedupe.ledgerly", "trial", menu_entries());
    assert_eq!(
        targets(&palette),
        vec![PaletteTarget::Report("trial-balance".to_owned())]
    );
    assert_eq!(palette.results()[0].title, "Trial Balance");
}

#[test]
fn without_a_company_the_palette_says_ledgers_are_not_searched() {
    // provenance: derived. No worker means no API search: the palette keeps only the host's rows
    // and says so, rather than showing an empty list as if nothing matched.
    let mut palette = Palette::new();
    palette.open(PaletteMode::OnTop);
    palette.set_query("acme");
    let mut search = PaletteSearch::new();
    search.tick(&mut palette, None);
    assert_eq!(targets(&palette), Vec::<PaletteTarget>::new());
    assert!(palette.note().is_some());
    assert!(!search.is_pending());
}

#[test]
fn hits_make_rows_and_an_unavailable_hit_makes_a_note_not_a_row() {
    // provenance: derived. One hit of each kind that has a row: ledger 7 "Ghee", voucher 9
    // without a number. The Unavailable hit has no row; its reason is returned for the note.
    let hits = vec![
        SearchHit::Ledger {
            id: LedgerId(7),
            name: "Ghee".to_owned(),
            matched: ledgerly_api::MatchedOn::Name,
        },
        SearchHit::Voucher {
            id: VoucherId(9),
            number: None,
            date: date!(2026 - 04 - 10),
            matched: ledgerly_api::MatchedOn::Amount,
        },
        SearchHit::Unavailable {
            reason: "books blocked".to_owned(),
        },
    ];
    let entries = entries_from_hits("gh", &hits);
    let got: Vec<(PaletteTarget, &str)> = entries
        .iter()
        .map(|e| (e.target.clone(), e.title.as_str()))
        .collect();
    assert_eq!(
        got,
        vec![
            (PaletteTarget::Master("ledger:7".to_owned()), "Ghee"),
            (PaletteTarget::Voucher("voucher:9".to_owned()), "Voucher #9"),
        ]
    );
    assert_eq!(unavailable_reason(&hits), Some("books blocked"));
}

// ---- opening screens from picks ---------------------------------------------------------------

#[test]
fn a_picked_ledger_opens_its_report_on_top_of_the_screen() {
    // provenance: derived. Card C5: ledger -> ledger report. On top: the stack grows from 1 to 2
    // and the new screen is a worker-mode report that has not run yet, so it still carries the
    // placeholder title "Report".
    let ctx = ctx_with_books();
    let mut tabs = Tabs::new(Box::new(Root));
    let change = tabs
        .open_choice(
            &choice(
                PaletteTarget::Master("ledger:1".to_owned()),
                PaletteMode::OnTop,
            ),
            &ctx,
        )
        .unwrap();
    assert_eq!(change, StackChange::Pushed);
    let stack = tabs.active_stack().unwrap();
    assert_eq!(stack.depth(), 2);
    assert_eq!(stack.top_title(), "Report");
}

#[test]
fn a_replace_pick_replaces_the_screen() {
    // provenance: derived. Palette mode Replace (spec section 7: "the choice replaces the
    // current screen"): the stack keeps one screen and it is no longer the root.
    let ctx = ctx_with_books();
    let mut tabs = Tabs::new(Box::new(Root));
    let change = tabs
        .open_choice(
            &choice(
                PaletteTarget::Voucher("voucher:1".to_owned()),
                PaletteMode::Replace,
            ),
            &ctx,
        )
        .unwrap();
    assert_eq!(change, StackChange::Replaced);
    let stack = tabs.active_stack().unwrap();
    assert_eq!(stack.depth(), 1);
    assert_eq!(stack.top_title(), "Voucher #1");
}

#[test]
fn picks_that_open_no_screen_say_why_and_change_nothing() {
    // provenance: derived. A setting has no screen; a voucher that is not in the books is not
    // opened; a command is the host's, not a screen; with no books nothing opens.
    let ctx = ctx_with_books();
    let mut tabs = Tabs::new(Box::new(Root));
    let setting = tabs.open_choice(
        &choice(
            PaletteTarget::Setting("Inventory".to_owned()),
            PaletteMode::OnTop,
        ),
        &ctx,
    );
    assert!(matches!(setting, Err(NavError::Unsupported { .. })));
    let missing = tabs.open_choice(
        &choice(
            PaletteTarget::Voucher("voucher:99".to_owned()),
            PaletteMode::OnTop,
        ),
        &ctx,
    );
    assert_eq!(missing, Err(NavError::VoucherNotFound(99)));
    let command = tabs.open_choice(
        &choice(
            PaletteTarget::AiIntent("describe".to_owned()),
            PaletteMode::OnTop,
        ),
        &ctx,
    );
    assert_eq!(command, Err(NavError::NotNavigation));
    let bad = open_target(&PaletteTarget::Master("item:4".to_owned()), &ctx);
    assert!(matches!(bad, Err(NavError::BadKey(_))));
    let no_books = tabs.open_choice(
        &choice(
            PaletteTarget::Report("trial-balance".to_owned()),
            PaletteMode::OnTop,
        ),
        &AppCtx::default(),
    );
    assert_eq!(no_books, Err(NavError::NoCompany));
    assert_eq!(tabs.active_stack().unwrap().depth(), 1);
}

#[test]
fn report_keys_round_trip_and_unopenable_reports_have_no_key() {
    // provenance: derived. Every menu entry has a key that names the same report again. A
    // register has no key yet.
    for entry in menu_entries() {
        let PaletteTarget::Report(key) = &entry.target else {
            panic!("a menu entry is a report");
        };
        let kind = report_from_key(key).unwrap();
        assert_eq!(report_key(&kind).as_deref(), Some(key.as_str()));
    }
    assert_eq!(
        report_key(&ReportKind::Register(BaseVoucherKind::Journal)),
        None
    );
    assert_eq!(
        report_from_key("ledger:12"),
        Some(ReportKind::LedgerVouchers(LedgerId(12)))
    );
    assert_eq!(report_from_key("ledger:x"), None);
}

// ---- the report menu runs in worker mode ------------------------------------------------------

#[test]
fn trial_balance_from_the_menu_runs_on_the_worker_with_tallys_defaults() {
    // provenance: derived. Tally Trial Balance page (read 2026-11-21): group-wise by default, "as
    // on the date of the last voucher entry". Both vouchers are dated 10 April 2026 and the year
    // starts 1 April 2026, so the period is 1 to 10 April 2026 and the detail is condensed.
    // Figures: Cash Dr 1,00,000 + 5,000 = 1,05,000. Customer One Cr 5,000. Capital Cr 1,00,000.
    // Total Dr 1,05,000; total Cr 5,000 + 1,00,000 = 1,05,000 (Tally Trial Balance page: total
    // debits equal total credits).
    let books = books();
    let params = default_params(ReportKind::TrialBalance, &books).unwrap();
    assert_eq!(
        params.period,
        Period::new(date!(2026 - 04 - 01), date!(2026 - 04 - 10)).unwrap()
    );
    assert_eq!(params.as_of, date!(2026 - 04 - 10));
    assert_eq!(params.detail, Detail::Condensed);

    let worker = ApiWorker::spawn(api("c5_menu_tb.ledgerly")).unwrap();
    let mut screen = report_screen(ReportKind::TrialBalance, &books).unwrap();
    assert!(
        screen.is_loading(),
        "a worker-mode screen asks the worker on its first tick"
    );
    settle_report(&mut screen, &worker);
    assert_eq!(screen.report().title, "Trial Balance");
    let dr = ViewCell::Amount(SignedAmount::dr(rs(105_000)));
    let cr = ViewCell::Amount(SignedAmount::cr(rs(105_000)));
    assert_eq!(screen.report().totals, vec![dr, cr]);
}

#[test]
fn the_menu_opens_a_report_on_the_active_tab() {
    // provenance: derived. Opening a menu entry pushes one worker-mode report screen.
    let ctx = ctx_with_books();
    let mut tabs = Tabs::new(Box::new(Root));
    let change = tabs
        .open_report(ReportKind::DayBook, &ctx, PaletteMode::OnTop)
        .unwrap();
    assert_eq!(change, StackChange::Pushed);
    assert_eq!(tabs.active_stack().unwrap().depth(), 2);
    let none = tabs.open_report(ReportKind::DayBook, &AppCtx::default(), PaletteMode::OnTop);
    assert_eq!(none, Err(NavError::NoCompany));
    assert_eq!(tabs.active_stack().unwrap().depth(), 2);
    let _ = navigate::report_menu();
}

// ---- voucher drill ----------------------------------------------------------------------------

#[test]
fn a_voucher_row_of_a_ledger_report_drills_to_the_voucher() {
    // provenance: derived. Tally (shortcuts page, read 2026-11-21): Enter drills "from the last
    // level details of a report" to a voucher. The Cash ledger report lists the two vouchers that
    // touch Cash: 1 and 2 (both have a Cash line). Drilling the row of voucher 2 opens a screen
    // for voucher 2. Display only (pending: alteration, see the module header).
    let books = books();
    let worker = ApiWorker::spawn(api("c5_drill.ledgerly")).unwrap();
    let mut screen = report_screen(ReportKind::LedgerVouchers(CASH), &books).unwrap();
    settle_report(&mut screen, &worker);

    let voucher_rows: Vec<(usize, VoucherId)> = screen
        .report()
        .rows
        .iter()
        .enumerate()
        .filter_map(|(i, row)| match &row.key {
            Some(ViewKey::Voucher(id)) => Some((i, *id)),
            Some(
                ViewKey::Group(_) | ViewKey::Ledger(_) | ViewKey::Item(_) | ViewKey::Bill { .. },
            )
            | None => None,
        })
        .collect();
    let ids: Vec<VoucherId> = voucher_rows.iter().map(|(_, id)| *id).collect();
    assert_eq!(ids, vec![VoucherId(1), VoucherId(2)]);

    screen.select(voucher_rows[1].0);
    let ScreenAction::Push(opened) = screen.drill() else {
        panic!("a voucher row opens a screen");
    };
    assert_eq!(opened.title(), "Voucher #2");
    assert_eq!(screen.notice(), None);
}

#[test]
fn the_voucher_display_shows_the_stored_lines_and_no_total() {
    // provenance: derived. Voucher 1 as posted in the fixture: a Journal, no number, Regular,
    // lines Cash Dr 1,00,000.00 then Capital Cr 1,00,000.00, in that order, as stored.
    let view = voucher_view(&books(), VoucherId(1)).unwrap();
    assert_eq!(view.type_name, "Journal");
    assert_eq!(view.number, None);
    assert_eq!(view.status, "Regular");
    assert_eq!(view.date, date!(2026 - 04 - 10));
    let lines: Vec<(&str, DrCr, i64)> = view
        .lines
        .iter()
        .map(|l| (l.ledger.as_str(), l.amount.side, l.amount.amount.get()))
        .collect();
    assert_eq!(
        lines,
        vec![
            ("Cash", DrCr::Dr, 10_000_000),
            ("Capital", DrCr::Cr, 10_000_000)
        ]
    );
    assert_eq!(view.not_shown, Vec::<String>::new());
}

#[test]
fn a_voucher_that_is_not_in_the_books_is_an_error_not_a_blank_screen() {
    // provenance: derived. No voucher 99 exists in the fixture.
    assert_eq!(
        voucher_view(&books(), VoucherId(99)),
        Err(VoucherViewError::NotFound { id: 99 })
    );
}

// ---- the mock's screens: palette rows, stubs and the sidebar (card U4) ------------------------

/// The fixture books plus one bank ledger and one GST registration, so the bank reconciliation
/// and the GST returns have a screen to open.
fn books_with_bank() -> Books {
    let mut fixture = books();
    fixture.company.registrations = vec![ledgerly_model::GstRegistration {
        gstin: Gstin::parse(CUSTOMER_GSTIN).unwrap(),
        state: StateCode::new(29).unwrap(),
        effective_from: date!(2026 - 04 - 01),
        composition: false,
    }];
    let mut bank = plain(LedgerId(4), "HDFC Current", ReservedGroup::BankAccounts);
    bank.bank = Some(ledgerly_model::BankDetails {
        account_number: "000123456789".to_owned(),
        ifsc: "FIC0000001".to_owned(),
        bank_name: "Fictional Bank".to_owned(),
        branch: "Test Branch".to_owned(),
    });
    fixture.masters.ledgers.insert(bank.id, bank);
    fixture
}

fn books_ctx(books: Books) -> AppCtx {
    AppCtx {
        books: Some(books),
        ..AppCtx::default()
    }
}

fn report(key: &str) -> PaletteTarget {
    PaletteTarget::Report(key.to_owned())
}

#[test]
fn the_mock_screens_have_palette_rows_that_open_them_with_their_titles() {
    // provenance: derived. The titles are the mock's (design/mock-v2/index.html, SCREENS[...].title
    // and the NAV list): "Review queue", "Ask your books", "Plain-words entry", "Purchase from bill
    // photo", "Keyboard map", "AI and privacy", "Company setup". The masters screen titles itself
    // with the kind it shows, "Masters: Ledgers" by default (masters.rs). The bank screen titles
    // itself "Bank reconciliation: <ledger>", so with the one bank ledger of the fixture (HDFC
    // Current) the row opens it. The import row opens the import screen, titled "Import from
    // another app" too (owner decision 3 in docs/status/2026-11-22-ui-parity-report.md).
    let ctx = books_ctx(books_with_bank());
    let rows = navigate::screen_entries(ctx.books.as_ref());
    let cases = [
        ("Review queue", "Review queue"),
        ("Ask your books", "Ask your books"),
        ("Plain-words entry", "Plain-words entry"),
        ("Purchase from bill photo", "Purchase from bill photo"),
        ("Keyboard map", "Keyboard map"),
        ("AI and privacy", "AI and privacy"),
        ("Company setup", "Company setup"),
        ("Masters", "Masters: Ledgers"),
        ("Bank reconciliation", "Bank reconciliation: HDFC Current"),
        ("Import from another app", "Import from another app"),
    ];
    for (row_title, expected) in cases {
        let row = rows
            .iter()
            .find(|e| e.title == row_title)
            .unwrap_or_else(|| panic!("no palette row \"{row_title}\""));
        match open_target(&row.target, &ctx) {
            Ok(screen) => assert_eq!(screen.title(), expected, "row \"{row_title}\""),
            Err(err) => panic!("row \"{row_title}\" did not open: {err}"),
        }
    }
}

#[test]
fn the_stubs_open_with_their_titles_and_need_no_company() {
    // provenance: derived. A stub reads no books, so its key opens with no company open.
    let cases = [
        ("review", "Review queue"),
        ("ask", "Ask your books"),
        ("quick-entry", "Plain-words entry"),
        ("purchase-bill", "Purchase from bill photo"),
        ("keys", "Keyboard map"),
        ("ai-settings", "AI and privacy"),
        ("setup", "Company setup"),
    ];
    for (key, expected) in cases {
        match open_target(&report(key), &AppCtx::default()) {
            Ok(screen) => assert_eq!(screen.title(), expected, "stub \"{key}\""),
            Err(err) => panic!("stub \"{key}\" did not open: {err}"),
        }
    }
}

#[test]
fn the_bank_row_says_why_it_cannot_open_without_one_bank_ledger() {
    // provenance: derived. The fixture has no bank ledger, so the row names the reason rather
    // than opening a screen for a ledger it cannot choose (spec 6.5 screens are per bank ledger).
    match open_target(&report("bank-reconciliation"), &books_ctx(books())) {
        Err(NavError::Unsupported { what, reason }) => {
            assert_eq!(what, "Bank reconciliation");
            assert_eq!(reason, "the company has no bank ledger");
        }
        other => panic!("expected the reason, got {:?}", other.map(|s| s.title())),
    }
}

#[test]
fn the_sidebar_has_the_mock_groups_and_entries_in_order() {
    // provenance: derived. design/mock-v2/DESIGN.md, "Side bar groups", in order: Overview (Home,
    // Review queue), Enter (Sales, Purchase, Payment, Receipt, Plain-words entry), Books (Day
    // book, Balance sheet, Masters), Compliance (GST returns, Bank reconciliation), Settings (AI
    // and privacy, Keyboard map, Company setup). 2 + 5 + 3 + 2 + 3 = 15 rows.
    let groups: Vec<(&str, Vec<&str>)> = navigate::sidebar()
        .iter()
        .map(|g| {
            (
                g.title,
                g.entries.iter().map(|e| e.label).collect::<Vec<_>>(),
            )
        })
        .collect();
    assert_eq!(
        groups,
        vec![
            ("Overview", vec!["Home", "Review queue"]),
            (
                "Enter",
                vec![
                    "Sales",
                    "Purchase",
                    "Payment",
                    "Receipt",
                    "Plain-words entry"
                ]
            ),
            ("Books", vec!["Day book", "Balance sheet", "Masters"]),
            ("Compliance", vec!["GST returns", "Bank reconciliation"]),
            (
                "Settings",
                vec!["AI and privacy", "Keyboard map", "Company setup"]
            ),
        ]
    );
}

#[test]
fn the_sidebar_shows_the_mock_key_hints_and_no_guessed_count() {
    // provenance: derived. The mock's key hints (DESIGN.md): Sales F8, Purchase F9, Payment F5,
    // Receipt F6, Keyboard map ?. The classic keymap binds the same keys (keymap-classic.toml:
    // new-sales F8, new-purchase F9, new-payment F5, new-receipt F6). The Review queue count is
    // not guessed: it stays None until the shell reads it from the API.
    let groups = navigate::sidebar();
    let flat: Vec<&navigate::SidebarEntry> = groups.iter().flat_map(|g| g.entries.iter()).collect();
    let hints: Vec<(&str, &str)> = flat
        .iter()
        .filter_map(|e| e.hint.map(|h| (e.label, h)))
        .collect();
    assert_eq!(
        hints,
        vec![
            ("Sales", "F8"),
            ("Purchase", "F9"),
            ("Payment", "F5"),
            ("Receipt", "F6"),
            ("Keyboard map", "?"),
        ]
    );
    assert!(flat.iter().all(|e| e.count.is_none()));
}

#[test]
fn every_sidebar_target_resolves_to_a_screen_or_a_keymap_command() {
    // provenance: derived. A command target is a keymap action (the host opens it); every other
    // target opens a screen with the fixture books plus one bank ledger.
    let groups = navigate::sidebar();
    let ctx = books_ctx(books_with_bank());
    for entry in groups.iter().flat_map(|g| g.entries.iter()) {
        match &entry.target {
            PaletteTarget::Command(action) => assert!(
                ledgerly_app::keymap::Action::ALL.contains(action),
                "\"{}\" is not a keymap action",
                entry.label
            ),
            target @ (PaletteTarget::Report(_)
            | PaletteTarget::Master(_)
            | PaletteTarget::Voucher(_)
            | PaletteTarget::Setting(_)
            | PaletteTarget::AiIntent(_)) => {
                if let Err(err) = open_target(target, &ctx) {
                    panic!("sidebar row \"{}\" did not open: {err}", entry.label);
                }
            }
        }
    }
}

#[test]
fn every_sidebar_row_has_the_same_target_as_a_palette_row() {
    // provenance: derived. The sidebar and the palette share each target value (card U4).
    let groups = navigate::sidebar();
    let keymap = ledgerly_app::keymap::classic();
    let mut rows = navigate::host_entries(&keymap);
    rows.extend(navigate::screen_entries(Some(&books_with_bank())));
    for entry in groups.iter().flat_map(|g| g.entries.iter()) {
        assert!(
            rows.iter().any(|r| r.target == entry.target),
            "\"{}\" has no palette row with its target",
            entry.label
        );
    }
}

#[test]
fn the_reports_the_mock_does_not_show_stay_in_the_palette() {
    // provenance: derived. The report menu keeps all eight reports (Trial Balance, Balance Sheet,
    // Profit and Loss, Day Book, Stock Summary, Receivables and Payables Outstanding, Integrity
    // Certificate: the menu has eight entries). The sidebar names none of the five that the mock
    // does not show.
    let titles: Vec<String> = menu_entries().into_iter().map(|e| e.title).collect();
    assert_eq!(
        titles,
        vec![
            "Trial Balance",
            "Balance Sheet",
            "Profit and Loss",
            "Day Book",
            "Stock Summary",
            "Receivables Outstanding",
            "Payables Outstanding",
            "Integrity Certificate",
        ]
    );
    let groups = navigate::sidebar();
    let labels: Vec<&str> = groups
        .iter()
        .flat_map(|g| g.entries.iter())
        .map(|e| e.label)
        .collect();
    for hidden in [
        "Trial Balance",
        "Profit and Loss",
        "Stock Summary",
        "Receivables Outstanding",
        "Payables Outstanding",
        "Integrity Certificate",
    ] {
        assert!(!labels.contains(&hidden), "\"{hidden}\" is in the sidebar");
    }
}

#[test]
fn the_import_row_has_the_new_label_and_no_row_names_another_product() {
    // provenance: derived. Owner decision (U4 item): the palette row "Import from Tally" becomes
    // "Import from another app". No palette row or sidebar label names another accounting
    // product (engineering rules F5 allow the product name only as a factual interop label).
    let keymap = ledgerly_app::keymap::classic();
    let mut rows = navigate::host_entries(&keymap);
    rows.extend(navigate::screen_entries(Some(&books_with_bank())));
    let import = rows
        .iter()
        .find(|e| e.target == report("import-tally"))
        .unwrap_or_else(|| panic!("no import row"));
    assert_eq!(import.title, "Import from another app");
    assert!(rows.iter().all(|e| e.title != "Import from Tally"));

    let mut titles: Vec<String> = rows.iter().map(|e| e.title.clone()).collect();
    for group in navigate::sidebar() {
        titles.extend(group.entries.iter().map(|e| e.label.to_owned()));
    }
    for title in &titles {
        let lower = title.to_lowercase();
        for product in ["tally", "busy", "zoho", "vyapar", "quickbooks"] {
            assert!(!lower.contains(product), "\"{title}\" names {product}");
        }
    }
}
