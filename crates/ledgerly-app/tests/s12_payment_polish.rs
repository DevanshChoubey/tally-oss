//! Payment and receipt polish (round s12): a committed amount reads with Indian grouping, and the
//! Mode is chosen from the instruments the engine knows.
//!
//! Provenance: the mock's `payScreen` (`design/mock-v2/index.html` line 1396 draws the line's
//! amount as `inr(l[1])`, which groups the Indian way: 6,500.00), and the mock's Mode field
//! (line 1394) naming NEFT / UPI. derived: 6500 rupees is 6,500.00 (one group of three digits
//! after the thousands); 150000 rupees is 1,50,000.00 (lakh grouping, 3 then 2 digits).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use egui::{Pos2, RawInput, Rect, Vec2};
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

use egui::epaint::{ClippedShape, Shape};
use ledgerly_model::InstrumentKind;

fn texts(shapes: &[ClippedShape]) -> Vec<String> {
    fn walk(shape: &Shape, out: &mut Vec<String>) {
        match shape {
            Shape::Text(t) => out.push(t.galley.text().to_owned()),
            Shape::Vec(v) => {
                for s in v {
                    walk(s, out);
                }
            }
            Shape::Noop
            | Shape::Circle(_)
            | Shape::Ellipse(_)
            | Shape::LineSegment { .. }
            | Shape::Path(_)
            | Shape::Band(_)
            | Shape::Rect(_)
            | Shape::Mesh(_)
            | Shape::QuadraticBezier(_)
            | Shape::CubicBezier(_)
            | Shape::Callback(_) => {}
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, &mut out);
    }
    out
}

fn frame(s: &mut VoucherEntryScreen, b: &Books) -> Vec<String> {
    let mut app = AppCtx {
        tr: ledgerly_app::i18n::Tr::default(),
        books: Some(b.clone()),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    let ctx = egui::Context::default();
    let mut last = Vec::new();
    for _ in 0..4 {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            ..RawInput::default()
        };
        let out = ctx.run_ui(input, |ui| {
            let _stay = s.ui(ui, &mut app);
        });
        last = texts(&out.shapes);
        out.drop_without_applying_deltas();
    }
    last
}

#[test]
fn a_committed_amount_reads_with_indian_grouping_and_an_edited_one_stays_raw() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b);
    s.set_ledger_text(0, "cash");
    s.submit(m);
    s.set_ledger_text(1, "rent");
    s.submit(m);
    // The keyboard is on the amount: the typed text stays as typed.
    s.set_amount_text(1, "150000.00");
    let editing = frame(&mut s, &b);
    assert!(editing.iter().any(|t| t == "150000.00"), "{editing:?}");
    assert!(!editing.iter().any(|t| t == "1,50,000.00"), "{editing:?}");
    // Enter commits the line and the keyboard leaves it.
    s.submit(m);
    let committed = frame(&mut s, &b);
    assert!(
        committed.iter().any(|t| t == "1,50,000.00"),
        "{committed:?}"
    );
    assert!(!committed.iter().any(|t| t == "150000.00"), "{committed:?}");
    // The draft still holds what was typed: the display is only a view of it.
    assert_eq!(s.draft().rows[1].amount_text, "150000.00");
}

#[test]
fn choosing_a_mode_stores_the_instrument_the_engine_expects() {
    let b = books();
    let mut s = open(&b);
    s.choose_mode(InstrumentKind::Upi);
    assert_eq!(s.draft().mode_text, "UPI");
    assert_eq!(
        ledgerly_app::screens::voucher_entry::parse_mode("UPI"),
        Some(InstrumentKind::Upi)
    );
}

/// derived: the bill-wise popup of a payment of 1,50,000 against a bill of 1,50,000 offers the
/// bill's whole open amount, and the amount cell reads as every other amount does, 1,50,000.00
/// (lakh grouping: 3 digits, then pairs), even while the keyboard is on it and the text is the
/// offered default. What the user types stays as typed; the draft holds the plain digits.
#[test]
fn the_bill_wise_amount_reads_with_indian_grouping() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b);
    s.set_ledger_text(0, "cash");
    s.submit(m);
    s.set_ledger_text(1, "vardhaman");
    s.submit(m);
    s.set_amount_text(1, "150000");
    s.submit(m);
    s.set_open_bills(
        PARTY,
        vec![ledgerly_api::OpenBill {
            ledger: PARTY,
            name: "VL/861".to_owned(),
            date: date!(2026 - 11 - 03),
            due: None,
            open: SignedAmount::cr(Paise::new(15_000_000)),
        }],
    );
    let shown = frame(&mut s, &b);
    // The line's amount, the bill's open amount and the amount cell: three cells, all grouped.
    let grouped = shown.iter().filter(|t| *t == "1,50,000.00").count();
    assert_eq!(grouped, 3, "{shown:?}");
    assert!(!shown.iter().any(|t| t == "150000.00"), "{shown:?}");
    assert_eq!(s.draft().rows[1].bills[0].amount_text, "150000.00");
}
