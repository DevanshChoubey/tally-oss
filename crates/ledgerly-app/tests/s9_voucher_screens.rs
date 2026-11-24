//! Voucher entry screens against the mock (round s9): the sidebar entry that stays selected.
//!
//! Provenance: the mock's `NAV` (`design/mock-v2/index.html` line 888) has Payment and Receipt
//! rows, and `renderShell` (line 904) marks the row of the open screen with `aria-current`.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_app::Screen;
use ledgerly_app::screens::voucher_entry::VoucherEntryScreen;
use ledgerly_core::{
    Date, FinancialYear, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode, VoucherTypeId,
};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, Group, Ledger, Masters,
    NegativeStockPolicy, NumberingConfig, NumberingMethod, RestartPeriod, VoucherType,
};
use time::macros::date;

const CASH: LedgerId = LedgerId(1);
const RENT: LedgerId = LedgerId(5);
const T_PAYMENT: VoucherTypeId = VoucherTypeId(2);
const GROUP: GroupId = GroupId(1);
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

#[test]
fn the_payment_screen_keeps_the_payment_row_selected() {
    // derived: mock line 904 marks the row of the open screen; the payment screen is the
    // mock's `payment` screen, whose row is "Payment" (line 888).
    let b = books();
    let screen = open(&b);
    assert_eq!(
        ledgerly_app::shell::sidebar::selected_label(&screen.sidebar_title()),
        Some("Payment")
    );
}
