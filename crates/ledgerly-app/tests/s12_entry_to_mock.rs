//! Journal, contra and the remaining payment differences from the mock (round s12).
//!
//! Provenance: the mock's `SCREENS.journal`, `SCREENS.contra` and `payScreen`
//! (`design/mock-v2/index.html`, lines 2043-2090 and 1395-1436). The journal's hand check is in
//! the comment above `SCREENS.journal`: freight paid recorded as Purchases and moved to Freight
//! inward, 1,840.00 on both sides; the validation state credits 1,480.00 against 1,840.00, so
//! the difference is 1,840.00 - 1,480.00 = 360.00 (derived by hand: 184000 - 148000 = 36000
//! paise). The table's columns are `#, Ledger, Dr / Cr, Group, Amount` (index.html:2058).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use egui::epaint::{ClippedShape, Shape};
use egui::{Pos2, RawInput, Rect, Vec2};
use ledgerly_app::screens::voucher_entry::VoucherEntryScreen;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{
    Date, FinancialYear, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode, VoucherTypeId,
};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, Group, Ledger, Masters,
    NegativeStockPolicy, NumberingConfig, NumberingMethod, RestartPeriod, VoucherType,
};
use time::macros::date;

const FREIGHT: LedgerId = LedgerId(1);
const PURCHASES: LedgerId = LedgerId(2);
const CASH: LedgerId = LedgerId(3);
const T_JOURNAL: VoucherTypeId = VoucherTypeId(1);
const T_PAYMENT: VoucherTypeId = VoucherTypeId(2);
const T_CONTRA: VoucherTypeId = VoucherTypeId(3);
const DIRECT: GroupId = GroupId(1);
const PURCHASE_ACCOUNTS: GroupId = GroupId(2);
const TODAY: Date = date!(2026 - 11 - 21);

fn ledger(id: LedgerId, name: &str, group: GroupId) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group,
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

fn group(id: GroupId, name: &str) -> Group {
    Group {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        parent: None,
        reserved: None,
        nature: None,
        affects_gross_profit: false,
        tally: None,
    }
}

fn voucher_type(id: VoucherTypeId, name: &str, base: BaseVoucherKind) -> VoucherType {
    VoucherType {
        id,
        name: name.to_owned(),
        base,
        abbreviation: name.chars().take(3).collect(),
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
    }
}

fn books() -> Books {
    let mut masters = Masters::default();
    masters
        .groups
        .insert(DIRECT, group(DIRECT, "Direct expenses"));
    masters.groups.insert(
        PURCHASE_ACCOUNTS,
        group(PURCHASE_ACCOUNTS, "Purchase accounts"),
    );
    for l in [
        ledger(FREIGHT, "Freight inward", DIRECT),
        ledger(PURCHASES, "Purchases", PURCHASE_ACCOUNTS),
        ledger(CASH, "Cash", DIRECT),
    ] {
        masters.ledgers.insert(l.id, l);
    }
    for t in [
        voucher_type(T_JOURNAL, "Journal", BaseVoucherKind::Journal),
        voucher_type(T_PAYMENT, "Payment", BaseVoucherKind::Payment),
        voucher_type(T_CONTRA, "Contra", BaseVoucherKind::Contra),
    ] {
        masters.voucher_types.insert(t.id, t);
    }
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

fn open(b: &Books, t: VoucherTypeId) -> VoucherEntryScreen {
    let fy = FinancialYear::april(2026).unwrap();
    match VoucherEntryScreen::open(b, t, fy, TODAY).unwrap() {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("unexpectedly unsupported: {u:?}"),
    }
}

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

/// Dr Freight inward 1,840.00, Cr Purchases `credit`, the keyboard on the narration.
fn journal(b: &Books, credit: &str) -> VoucherEntryScreen {
    let m = &b.masters;
    let mut s = open(b, T_JOURNAL);
    s.set_ledger_text(0, "freight");
    s.submit(m);
    s.set_amount_text(0, "1840");
    s.submit(m);
    s.set_ledger_text(1, "purchases");
    s.submit(m);
    s.set_amount_text(1, credit);
    s.submit(m);
    s
}

#[test]
fn a_journal_lists_ledger_then_dr_cr_then_group_then_amount() {
    let b = books();
    let mut s = journal(&b, "1840");
    let _ = frame(&mut s, &b);
    let names: Vec<&str> = s.heading_rects().iter().map(|(n, _)| *n).collect();
    assert_eq!(names, ["#", "Ledger", "Dr / Cr", "Group", "Amount"]);
    let shown = frame(&mut s, &b);
    assert!(shown.iter().any(|t| t == "Direct expenses"), "{shown:?}");
    assert!(shown.iter().any(|t| t == "Purchase accounts"), "{shown:?}");
}

#[test]
fn a_journal_foot_shows_debit_credit_and_the_difference() {
    let b = books();
    let mut s = journal(&b, "1480");
    s.delete_line(); // the prefilled third line is not part of the entry
    let shown = frame(&mut s, &b);
    for want in [
        "Debit total",
        "1,840.00",
        "Credit total",
        "1,480.00",
        "360.00",
    ] {
        assert!(shown.iter().any(|t| t == want), "{want}: {shown:?}");
    }
    assert!(
        shown.iter().any(|t| t.starts_with("Difference")),
        "{shown:?}"
    );
}

#[test]
fn a_balanced_journal_shows_a_difference_of_nil() {
    let b = books();
    let mut s = journal(&b, "1840");
    let shown = frame(&mut s, &b);
    assert!(shown.iter().any(|t| t == "0.00"), "{shown:?}");
}

#[test]
fn saving_a_journal_that_does_not_balance_says_how_much_and_on_which_side() {
    let b = books();
    let mut s = journal(&b, "1480");
    // Delete the prefilled third line, as a user who means to stop there does.
    s.delete_line();
    s.save(&b.masters);
    assert!(s.take_save_request().is_none());
    assert_eq!(
        s.error_text(),
        Some("Debit and credit differ by 360.00. Enter 360.00 more on the credit side to save.")
    );
}

#[test]
fn the_list_ends_with_the_hint_row_and_the_payment_has_it_too() {
    let b = books();
    let mut s = journal(&b, "1840");
    s.submit(&b.masters); // balanced: the keyboard goes to the narration
    let shown = frame(&mut s, &b);
    assert!(
        shown
            .iter()
            .any(|t| t == "Next ledger, or Enter on empty to finish"),
        "{shown:?}"
    );
    let mut p = open(&b, T_PAYMENT);
    p.set_ledger_text(0, "cash");
    p.submit(&b.masters);
    p.set_ledger_text(1, "freight");
    p.submit(&b.masters);
    p.set_amount_text(1, "6500");
    p.submit(&b.masters);
    p.submit(&b.masters); // Enter on the empty second line finishes the list
    let shown = frame(&mut p, &b);
    assert!(
        shown
            .iter()
            .any(|t| t == "Next ledger, or Enter on empty to finish"),
        "{shown:?}"
    );
}

// provenance: index.html:2082 (`SCREENS.contra`, validation-error): with no account picked and an
// amount of zero the notes are "Pick the cash or bank account first." and "An amount of zero
// cannot be posted.", one under the other. derived: nothing typed means no account and no amount.
#[test]
fn saving_an_empty_contra_names_both_problems() {
    let b = books();
    let mut s = open(&b, T_CONTRA);
    s.save(&b.masters);
    assert!(s.take_save_request().is_none());
    assert_eq!(
        s.error_text(),
        Some("Pick the cash or bank account first.\nAn amount of zero cannot be posted.")
    );
}
