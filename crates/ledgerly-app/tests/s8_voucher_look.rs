//! Voucher sheet look (round S8): the payment and receipt screens against the mock
//! (`design/mock-v2/index.html`, `payScreen`, lines 1381-1411, and the `.vsheet`, `.vhead`,
//! `table.lines`, `.vfoot` and `.ta` rules, lines 233-271).
//!
//! Provenance: the mock's CSS and markup. The head is four equal columns with a 16 point gap, the
//! account spans two (`.fld.span2`, line 1394); the line table runs # Particulars Group Amount
//! (line 1395); the type-ahead is `position:fixed` under its input (`.ta`, line 263), so it
//! never moves what is under it.
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
    for l in [ledger(CASH, "Cash"), ledger(RENT, "Rent")] {
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

fn draw(s: &mut VoucherEntryScreen, b: &Books, frames: usize) {
    let mut app = AppCtx {
        tr: ledgerly_app::i18n::Tr::default(),
        books: Some(b.clone()),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    let ctx = egui::Context::default();
    for _ in 0..frames {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            ..RawInput::default()
        };
        let out = ctx.run_ui(input, |ui| {
            let _stay = s.ui(ui, &mut app);
        });
        out.drop_without_applying_deltas();
    }
}

fn rect(s: &VoucherEntryScreen, name: &str) -> Rect {
    s.field_rect(name)
        .unwrap_or_else(|| panic!("the field {name:?} was not drawn"))
}

#[test]
fn the_account_spans_two_columns_beside_the_date() {
    // provenance: index.html:1394 (`.fld.span2` for the account) on the four-column head of
    // line 234: the date is column one and the account follows it, two columns wide.
    let b = books();
    let mut s = open(&b);
    draw(&mut s, &b, 3);
    let (date, account) = (rect(&s, "Date"), rect(&s, "Account"));
    assert!(
        (account.width() - (2.0 * date.width() + 16.0)).abs() < 1.0,
        "account {account:?} spans two of {date:?}"
    );
    assert!(
        (account.min.x - (date.max.x + 16.0)).abs() < 1.0,
        "the account follows the date"
    );
    assert!(
        (account.min.y - date.min.y).abs() < 0.5,
        "date and account on one line"
    );
}

#[test]
fn the_particulars_table_runs_as_the_mock_has_it() {
    // provenance: index.html:1395: # Particulars Group Amount, the last one for figures.
    let b = books();
    let mut s = open(&b);
    draw(&mut s, &b, 3);
    let heads = s.heading_rects();
    let names: Vec<&str> = heads.iter().map(|(n, _)| *n).collect();
    assert_eq!(names, ["#", "Particulars", "Group", "Amount"]);
    for pair in heads.windows(2) {
        assert!(pair[0].1.min.x < pair[1].1.min.x, "{pair:?} out of order");
    }
}

#[test]
fn the_account_list_floats_and_moves_nothing() {
    // provenance: index.html:263, `.ta{position:fixed}`.
    let b = books();
    let mut s = open(&b);
    draw(&mut s, &b, 3);
    let account = rect(&s, "Account");
    let narration_open = rect(&s, "Narration");
    let list = s
        .picker_rect()
        .expect("the account list is open on an empty account");
    // provenance: index.html:1254, `ta.style.top=(r.bottom+4)+'px'` with `r` the input's rect: the
    // list hangs 4 points under the 32 point input (caption 20 + gap 4 above it), over the hint.
    let input_bottom = account.min.y + 20.0 + 4.0 + 32.0;
    assert!(
        (list.min.y - (input_bottom + 4.0)).abs() < 3.0,
        "the list {list:?} hangs under the account input (bottom {input_bottom}) of {account:?}"
    );
    assert!(
        list.max.y > narration_open.min.y - 200.0,
        "the list reaches down over the sheet"
    );
    s.draft_mut().rows[0].ledger_text = "Cash".to_owned();
    s.draft_mut().rows[0].ledger = Some(ledgerly_app::screens::voucher_entry::LedgerRef::Chosen(
        CASH,
    ));
    draw(&mut s, &b, 3);
    let narration_closed = rect(&s, "Narration");
    assert!(
        (narration_open.min.y - narration_closed.min.y).abs() < 0.5,
        "Narration moved from {narration_open:?} to {narration_closed:?}"
    );
}
