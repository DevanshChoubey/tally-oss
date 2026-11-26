//! Card S15: the bill-wise popup takes the keyboard when the engine's open bills arrive.
//!
//! The popup opens on Enter on the amount of a bill-wise party's line, and its bills reach the
//! screen some frames later (a worker answers). The keyboard must be on the first bill's amount
//! once they have, so a user types the settled amount without reaching for the mouse (the mock's
//! popup, `design/mock-v2/index.html` lines 1398-1403, says "Enter accepts").
//!
//! derived: a field has the keyboard when the text typed reaches it; the offered default (to the paisa) for a
//! payment of 1,500 against a bill of 1,000 is 1000.00, so a typed digit changes it.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use egui::{Event, Pos2, RawInput, Rect, Vec2};
use ledgerly_app::screens::voucher_entry::VoucherEntryScreen;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{
    Date, FinancialYear, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode, VoucherTypeId,
};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, Group, Ledger, Masters,
    NegativeStockPolicy, NumberingConfig, NumberingMethod, ReservedGroup, RestartPeriod,
    VoucherType,
};
use time::macros::date;

const CASH: LedgerId = LedgerId(1);
const RENT: LedgerId = LedgerId(5);
const PARTY: LedgerId = LedgerId(6);
const T_PAYMENT: VoucherTypeId = VoucherTypeId(2);
const GROUP: GroupId = GroupId(1);
const CASH_GROUP: GroupId = GroupId(2);
const TODAY: Date = date!(2026 - 11 - 21);

fn ledger(id: LedgerId, name: &str) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: GROUP,
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

/// S22: a payment's or receipt's account lists only cash and bank ledgers, so the fixture puts
/// `ledgers` under a reserved group (Cash-in-Hand, Bank Accounts) inside Current Assets, as in a
/// real company.
fn reserve(
    masters: &mut Masters,
    id: GroupId,
    name: &str,
    reserved: ReservedGroup,
    ledgers: &[LedgerId],
) {
    masters.groups.insert(
        id,
        Group {
            ineligible_itc: None,
            id,
            name: name.to_owned(),
            aliases: Vec::new(),
            parent: Some(GROUP),
            reserved: Some(reserved),
            nature: None,
            affects_gross_profit: false,
            tally: None,
        },
    );
    for ledger in ledgers {
        if let Some(l) = masters.ledgers.get_mut(ledger) {
            l.group = id;
        }
    }
}

fn books() -> Books {
    let mut masters = Masters::default();
    masters.groups.insert(
        GROUP,
        Group {
            ineligible_itc: None,
            id: GROUP,
            name: "Current Assets".to_owned(),
            aliases: Vec::new(),
            parent: None,
            reserved: None,
            nature: None,
            affects_gross_profit: false,
            tally: None,
        },
    );
    let mut party = ledger(PARTY, "Vardhaman Logistics");
    party.bill_wise = Some(ledgerly_model::BillWiseConfig {
        default_credit_days: None,
    });
    for l in [ledger(CASH, "Cash"), ledger(RENT, "Rent"), party] {
        masters.ledgers.insert(l.id, l);
    }
    reserve(
        &mut masters,
        CASH_GROUP,
        "Cash-in-Hand",
        ReservedGroup::CashInHand,
        &[CASH],
    );
    masters.voucher_types.insert(
        T_PAYMENT,
        VoucherType {
            id: T_PAYMENT,
            name: "Payment".to_owned(),
            base: BaseVoucherKind::Payment,
            abbreviation: "Pay".to_owned(),
            numbering: NumberingConfig {
                method: NumberingMethod::Automatic,
                prevent_renumbering: false,
                prefix: String::new(),
                suffix: String::new(),
                width: 4,
                restart: RestartPeriod::Yearly,
                series: Vec::new(),
            },
            active: true,
            tally: None,
        },
    );
    Books {
        company: Company {
            ineligible_itc_default: None,
            id: ledgerly_core::CompanyId(1),
            name: "Test Traders".to_owned(),
            mailing_name: None,
            address: Address::default(),
            state: StateCode::new(27).unwrap(),
            pan: None,
            registrations: Vec::new(),
            fy_start: date!(2026 - 04 - 01),
            books_begin: date!(2026 - 04 - 01),
            entity_kind: EntityKind::Company,
            features: Features::default(),
            negative_stock: NegativeStockPolicy::Block,
        },
        masters,
        vouchers: BTreeMap::new(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

fn open(b: &Books) -> VoucherEntryScreen {
    let fy = FinancialYear::april(2026).unwrap();
    match VoucherEntryScreen::open(b, T_PAYMENT, fy, TODAY).unwrap() {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("unexpectedly unsupported: {u:?}"),
    }
}

/// One frame on a context that lives across calls, with `events` as the frame's input.
fn frame(ctx: &egui::Context, s: &mut VoucherEntryScreen, app: &mut AppCtx, events: Vec<Event>) {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
        events,
        ..RawInput::default()
    };
    let out = ctx.run_ui(input, |ui| {
        let _stay = s.ui(ui, app);
    });
    out.drop_without_applying_deltas();
}

#[test]
fn bills_that_arrive_late_still_take_the_keyboard() {
    let b = books();
    let m = &b.masters;
    let mut app = AppCtx {
        tr: ledgerly_app::i18n::Tr::default(),
        books: Some(b.clone()),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    let ctx = egui::Context::default();
    let mut s = open(&b);
    s.set_ledger_text(0, "cash");
    s.submit(m);
    s.set_ledger_text(1, "vardhaman");
    s.submit(m);
    s.set_amount_text(1, "1500");
    s.submit(m);
    // The popup is open and the worker has not answered: several frames draw "Reading the open bills".
    for _ in 0..5 {
        frame(&ctx, &mut s, &mut app, Vec::new());
    }
    s.set_open_bills(
        PARTY,
        vec![ledgerly_api::OpenBill {
            ledger: PARTY,
            name: "VL/861".to_owned(),
            date: date!(2026 - 11 - 03),
            due: None,
            open: SignedAmount::cr(Paise::new(100_000)),
        }],
    );
    for _ in 0..5 {
        frame(&ctx, &mut s, &mut app, Vec::new());
    }
    assert_eq!(s.draft().rows[1].bills[0].amount_text, "1000.00");
    frame(&ctx, &mut s, &mut app, vec![Event::Text("7".to_owned())]);
    frame(&ctx, &mut s, &mut app, Vec::new());
    assert_ne!(
        s.draft().rows[1].bills[0].amount_text,
        "1000.00",
        "the typed digit did not reach the first bill's amount"
    );
}
