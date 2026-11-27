//! s9: the day book says who entered each voucher and how, and carries its narration.
//!
//! Provenance: derived. Each voucher is posted under a stated actor, and the expected cell is what
//! that actor means by the rules written in `day_book_meta.rs`: a user is "Typed", the assistant
//! is an "AI draft" accepted by the user who confirmed it. The mock's day book columns
//! (design/mock-v2/index.html:1648: Date, Type, No., Particulars, Amount, Entered by, Origin; the
//! narration sits under the particulars) are the reference for what the screen needs.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_api::{Api, Command};
use ledgerly_core::{
    Clock, CompanyId, Date, GroupId, LedgerId, Paise, Period, SignedAmount, StateCode, UserId,
    VoucherId, VoucherTypeId,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, Ledger, LedgerLine,
    NegativeStockPolicy, ReservedGroup, Scenario, Voucher, VoucherMode, VoucherStatus,
};
use ledgerly_reports::{Cell, ColumnSpec, Detail, Report, ReportFilter, ReportKind, ReportParams};
use ledgerly_store::Actor;
use time::OffsetDateTime;
use time::macros::date;

const CASH: LedgerId = LedgerId(1);
const RENT: LedgerId = LedgerId(2);

fn reserved(group: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(group))
        .unwrap()
        .id
}

fn ledger(id: LedgerId, name: &str, group: GroupId) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: Some(group),
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

fn payment(id: u64, on: Date, narration: &str) -> Voucher {
    let paise = Paise::new(100_000);
    let type_id: VoucherTypeId = default_voucher_types()
        .into_iter()
        .find(|t| t.base == BaseVoucherKind::Payment)
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
        narration: narration.to_owned(),
        ledger_lines: vec![
            line(RENT, SignedAmount::dr(paise)),
            line(CASH, SignedAmount::cr(paise)),
        ],
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
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
    }
}

fn day_book(api: &Api) -> Report {
    let p = ReportParams {
        kind: ReportKind::DayBook,
        period: Period::new(date!(2026 - 11 - 01), date!(2026 - 11 - 30)).unwrap(),
        as_of: date!(2026 - 11 - 30),
        scenario: Scenario::Actual,
        detail: Detail::Detailed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    };
    api.report(&p).unwrap().value().unwrap()
}

fn text(cell: &Cell) -> &str {
    match cell {
        Cell::Text(t) => t,
        other @ (Cell::Amount(_)
        | Cell::Signed(_)
        | Cell::Qty(_)
        | Cell::Date(_)
        | Cell::Empty
        | Cell::Provisional { .. }
        | Cell::Unsupported(_)) => panic!("{other:?}"),
    }
}

#[test]
fn the_day_book_names_who_entered_each_voucher_and_how() {
    // provenance: derived. Voucher 1 is posted by user 1 (named "Ananya K." in the directory),
    // voucher 2 by user 2 ("Rohan D."), voucher 3 by the assistant on user 1's confirmation and
    // voucher 4 by user 7, who has no name in the directory. The rules: a user is Typed, the
    // assistant is an "AI draft" accepted by whoever confirmed it, a user without a name is
    // "User <id>". Narration is the voucher's own text.
    let dir = tempfile::tempdir().unwrap();
    let mut api = Api::create(
        &dir.path().join("c.ledgerly"),
        &company(),
        Clock::Fixed(OffsetDateTime::UNIX_EPOCH),
    )
    .unwrap();
    api.execute(Command::CreateLedger(ledger(
        CASH,
        "Cash",
        reserved(ReservedGroup::CashInHand),
    )))
    .unwrap();
    api.execute(Command::CreateLedger(ledger(
        RENT,
        "Rent",
        reserved(ReservedGroup::IndirectExpenses),
    )))
    .unwrap();
    api.set_user_name(UserId(1), "Ananya K.").unwrap();
    api.set_user_name(UserId(2), "Rohan D.").unwrap();
    let post = |api: &mut Api, actor: Actor, v: Voucher| {
        api.set_actor(actor);
        api.execute(Command::PostVoucher(v)).unwrap();
    };
    post(
        &mut api,
        Actor::User(UserId(1)),
        payment(1, date!(2026 - 11 - 02), "Office rent, November"),
    );
    post(
        &mut api,
        Actor::User(UserId(2)),
        payment(2, date!(2026 - 11 - 03), ""),
    );
    post(
        &mut api,
        Actor::Assistant {
            model: "assistant".to_owned(),
            on_behalf_of: UserId(1),
        },
        payment(3, date!(2026 - 11 - 04), "Plain-words draft"),
    );
    post(
        &mut api,
        Actor::User(UserId(7)),
        payment(4, date!(2026 - 11 - 05), "No directory entry"),
    );

    let r = day_book(&api);
    let col = |name: &str| r.columns.iter().position(|c| c.label == name).unwrap();
    let (narr, by, origin) = (col("Narration"), col("Entered by"), col("Origin"));
    let got: Vec<(String, String, String)> = r
        .rows
        .iter()
        .map(|row| {
            (
                text(&row.cells[narr]).to_owned(),
                text(&row.cells[by]).to_owned(),
                text(&row.cells[origin]).to_owned(),
            )
        })
        .collect();
    let want = [
        ("Office rent, November", "Ananya K.", "Typed"),
        ("", "Rohan D.", "Typed"),
        ("Plain-words draft", "Ananya K.", "AI draft"),
        ("No directory entry", "User 7", "Typed"),
    ]
    .map(|(a, b, c)| (a.to_owned(), b.to_owned(), c.to_owned()));
    assert_eq!(got, want);
    assert_eq!(r.totals.len(), r.columns.len());
}
