//! Card C6 tests: the rebuild check over company files written through the API.
//!
//! Provenance. Expected values are `derived`:
//! - A file whose projections were never switched on has no writer for them, so the rebuild
//!   check reports `NotMaintained` (store `backup` module doc; the card text: old files are
//!   "not maintained", never a mismatch with figures).
//! - After `Api::enable_projections`, `commit` maintains the tables, so the check is a match
//!   and stays one after further postings.
//! - Counts: the file holds two ledger masters (Cash, Rent) plus the seed groups and voucher
//!   types; each payment adds 1 voucher and 2 postings (Dr Rent, Cr Cash), so after two
//!   payments the stored vouchers are 2 and the stored postings 4.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::{Path, PathBuf};

use ledgerly_api::{Api, ApiError, Command, CommandResult, Voucher};
use ledgerly_core::{
    Clock, CompanyId, Date, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode, VoucherId,
    VoucherTypeId,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, Ledger, LedgerLine,
    NegativeStockPolicy, ReservedGroup, VoucherMode, VoucherStatus,
};
use time::OffsetDateTime;
use time::macros::date;

const CASH: LedgerId = LedgerId(1);
const RENT: LedgerId = LedgerId(2);

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

fn clock() -> Clock {
    Clock::Fixed(OffsetDateTime::UNIX_EPOCH)
}

fn reserved(group: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(group))
        .unwrap()
        .id
}

fn payment_type() -> VoucherTypeId {
    default_voucher_types()
        .into_iter()
        .find(|t| t.base == BaseVoucherKind::Payment)
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

/// A payment of `rupees`: Dr Rent, Cr Cash.
fn payment(id: u64, on: Date, rupees: i64) -> Voucher {
    let paise = Paise::new(rupees * 100);
    Voucher {
        id: VoucherId(id),
        type_id: payment_type(),
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
        ledger_lines: vec![
            line(RENT, SignedAmount::dr(paise)),
            line(CASH, SignedAmount::cr(paise)),
        ],
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: std::collections::BTreeMap::new(),
    }
}

fn value(out: Result<Outcome<CommandResult>, ApiError>) -> CommandResult {
    match out {
        Ok(Outcome::Value(result)) => result,
        other => panic!("expected a committed value, got {other:?}"),
    }
}

/// A company file with the Cash and Rent ledgers, and no vouchers.
fn setup(dir: &Path) -> (PathBuf, Api) {
    let path = dir.join("company.ledgerly");
    let mut api = Api::create(&path, &company(), clock()).unwrap();
    value(api.execute(Command::CreateLedger(ledger(
        CASH,
        "Cash",
        reserved(ReservedGroup::CashInHand),
    ))));
    value(api.execute(Command::CreateLedger(ledger(
        RENT,
        "Rent",
        reserved(ReservedGroup::IndirectExpenses),
    ))));
    (path, api)
}

#[test]
fn rebuild_check_matches_on_a_file_the_api_created() {
    let dir = tempfile::tempdir().unwrap();
    let (_path, mut api) = setup(dir.path());
    value(api.execute(Command::PostVoucher(payment(
        1,
        date!(2026 - 06 - 10),
        1000,
    ))));
    // Updated with owner approval (2026-11-21). This test used to be
    // `rebuild_check_matches_after_projections_are_switched_on` and first asserted NotMaintained,
    // because C6 shipped the switch as opt-in. The owner then had `Api::create` switch it on, so
    // the file already maintains its tables and switching it on again changes nothing.
    assert!(!api.enable_projections().unwrap());
    let built = api.rebuild_check().unwrap();
    assert!(built.is_match(), "{built:?}");

    value(api.execute(Command::PostVoucher(payment(2, date!(2026 - 06 - 11), 250))));
    let after = api.rebuild_check().unwrap();
    assert!(after.is_match(), "{after:?}");
    // Derived: 2 payments, 2 postings each.
    assert_eq!(after.stored.vouchers, 2);
    assert_eq!(after.stored.postings, 4);
    assert_eq!(after.stored, after.rebuilt);
}

#[test]
fn the_switch_survives_reopening_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut api) = setup(dir.path());
    value(api.execute(Command::PostVoucher(payment(
        1,
        date!(2026 - 06 - 10),
        1000,
    ))));
    api.enable_projections().unwrap();
    drop(api);
    let mut reopened = Api::open(&path).unwrap();
    value(reopened.execute(Command::PostVoucher(payment(2, date!(2026 - 06 - 11), 250))));
    let report = reopened.rebuild_check().unwrap();
    assert!(report.is_match(), "{report:?}");
    assert_eq!(report.stored.vouchers, 2);
}
