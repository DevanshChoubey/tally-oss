//! Card C4: the home KPIs come from `Api::report` through the worker.
//!
//! Provenance: every expected figure is `derived` by hand from the vouchers posted below, with the
//! arithmetic next to it. Tally reference for the tile set: the TallyPrime dashboard help page
//! lists Cash/Bank Accounts and Receivables/Payables among its default tiles (read 2026-11-21,
//! <https://help.tallysolutions.com/tally-prime/reports-in-tallyprime/dashboard/>). The page does
//! not define the figures, so the tiles here are the closing balance of one reserved group each
//! (the group's own net balance, our choice, not Tally's wording).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;
use std::time::Duration;

use ledgerly_api::{Api, ApiError, Command};
use ledgerly_app::screens::home::{HomeData, HomeScreen, KpiView};
use ledgerly_app::worker::ApiWorker;
use ledgerly_core::{
    Clock, CompanyId, Date, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode, VoucherId,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, Ledger, LedgerLine,
    NegativeStockPolicy, ReservedGroup, Voucher, VoucherMode, VoucherStatus,
};
use time::OffsetDateTime;
use time::macros::date;

const CASH: LedgerId = LedgerId(1);
const CAPITAL: LedgerId = LedgerId(2);
const CUSTOMER_ONE: LedgerId = LedgerId(3);
const CUSTOMER_TWO: LedgerId = LedgerId(4);
const SUPPLIER: LedgerId = LedgerId(5);

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

/// Creates the company file and posts the vouchers; `extra` adds the second customer and the
/// supplier with their vouchers.
fn setup(name: &str, extra: bool) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let mut api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    let mut ledgers = vec![
        plain(CASH, "Cash", ReservedGroup::CashInHand),
        plain(CAPITAL, "Capital", ReservedGroup::CapitalAccount),
        plain(CUSTOMER_ONE, "Customer One", ReservedGroup::SundryDebtors),
    ];
    if extra {
        ledgers.push(plain(
            CUSTOMER_TWO,
            "Customer Two",
            ReservedGroup::SundryDebtors,
        ));
        ledgers.push(plain(SUPPLIER, "Supplier", ReservedGroup::SundryCreditors));
    }
    for ledger in ledgers {
        committed(api.execute(Command::CreateLedger(ledger)));
    }
    let on = date!(2026 - 04 - 10);
    let mut vouchers = vec![
        // Capital of 1,00,000 paid into Cash.
        journal(
            1,
            on,
            vec![
                line(CASH, SignedAmount::dr(rs(100_000))),
                line(CAPITAL, SignedAmount::cr(rs(100_000))),
            ],
        ),
        // An advance of 5,000 from Customer One into Cash.
        journal(
            2,
            on,
            vec![
                line(CASH, SignedAmount::dr(rs(5_000))),
                line(CUSTOMER_ONE, SignedAmount::cr(rs(5_000))),
            ],
        ),
    ];
    if extra {
        vouchers.push(journal(
            3,
            on,
            vec![
                line(CUSTOMER_TWO, SignedAmount::dr(rs(30_000))),
                line(CAPITAL, SignedAmount::cr(rs(30_000))),
            ],
        ));
        vouchers.push(journal(
            4,
            on,
            vec![
                line(CAPITAL, SignedAmount::dr(rs(8_000))),
                line(SUPPLIER, SignedAmount::cr(rs(8_000))),
            ],
        ));
    }
    for v in vouchers {
        committed(api.execute(Command::PostVoucher(v)));
    }
    api
}

/// Asks for the tiles as of 30 April 2026 and waits (bounded) for the worker's answer.
fn refreshed(worker: &ApiWorker) -> Vec<KpiView> {
    let mut screen = HomeScreen::new(HomeData::default());
    screen.request_refresh(date!(2026 - 04 - 30));
    for _ in 0..2000 {
        screen.tick(Some(worker));
        if !screen.is_loading() {
            return screen.kpi_views();
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the figures did not arrive within the bound");
}

fn view<'a>(views: &'a [KpiView], label: &str) -> &'a KpiView {
    views.iter().find(|v| v.label == label).unwrap()
}

#[test]
fn home_kpis_are_the_closing_group_balances_derived_by_hand() {
    // provenance: derived. Posted: V1 Cash Dr 1,00,000 / Capital Cr 1,00,000; V2 Cash Dr 5,000 /
    // Customer One Cr 5,000; V3 Customer Two Dr 30,000 / Capital Cr 30,000; V4 Capital Dr 8,000 /
    // Supplier Cr 8,000.
    // Cash in hand: only Cash, Dr 1,00,000 + 5,000 = 1,05,000.00.
    // Bank accounts: no ledger in the group, so a nil balance: 0.00.
    // Receivables (Sundry Debtors): Customer One Cr 5,000 + Customer Two Dr 30,000 = Dr 25,000.00.
    // Payables (Sundry Creditors): Supplier Cr 8,000.00, the usual side for creditors.
    // GST payable: the engine has no payable-after-set-off figure, so the tile is not supported.
    // Indian grouping: 1,05,000.00 (last three digits, then pairs).
    let worker = ApiWorker::spawn(setup("c4_home_kpis.ledgerly", true)).unwrap();
    let views = refreshed(&worker);

    assert_eq!(
        view(&views, "Cash in hand").value_text,
        "\u{20B9}1,05,000.00"
    );
    assert_eq!(view(&views, "Bank accounts").value_text, "\u{20B9}0.00");
    assert_eq!(view(&views, "Receivables").value_text, "\u{20B9}25,000.00");
    assert_eq!(view(&views, "Payables").value_text, "\u{20B9}8,000.00");
    assert_eq!(view(&views, "GST payable").value_text, "Not supported yet");
}

#[test]
fn a_balance_on_the_unusual_side_is_not_shown_as_a_plain_figure() {
    // provenance: derived. Only V1 and V2: Customer One is the only debtor and stands Cr 5,000,
    // the opposite of the usual debit side for Sundry Debtors. The tile has no side marker, so it
    // must not show 5,000.00 as if it were owed to us (rule A1): it is "Not supported yet".
    let worker = ApiWorker::spawn(setup("c4_home_opposite.ledgerly", false)).unwrap();
    let views = refreshed(&worker);

    let receivables = view(&views, "Receivables");
    assert_eq!(receivables.value_text, "Not supported yet");
    assert!(
        receivables.footnote_text.contains("opposite"),
        "{}",
        receivables.footnote_text
    );
    // The cash tile is still its own hand-derived figure: Dr 1,00,000 + 5,000.
    assert_eq!(
        view(&views, "Cash in hand").value_text,
        "\u{20B9}1,05,000.00"
    );
}

#[test]
fn a_refresh_with_no_company_open_shows_no_figures_and_says_so() {
    // provenance: derived. No worker means no company file: nothing can be computed, so the
    // screen keeps no tiles and gives the reason.
    let mut screen = HomeScreen::new(HomeData::default());
    screen.request_refresh(date!(2026 - 04 - 30));
    screen.tick(None);
    assert!(!screen.is_loading());
    assert_eq!(screen.kpi_views(), Vec::<KpiView>::new());
    assert!(screen.notice().is_some());
}
