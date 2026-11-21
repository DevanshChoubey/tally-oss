//! Card E4 tests: voucher number allocation.
//!
//! Provenance for every expected value is written next to it. All are `derived`:
//! - the format is the frozen `NumberingConfig` contract (`prefix` + counter zero-padded to
//!   `width` minimum digits + `suffix`; counter starts at 1 and is one more than the highest
//!   counter used in the same (type, series, numbering period));
//! - spec section 5 invariant 7 (uniqueness per type, series, numbering period);
//! - the Indian financial year runs 1 April to 31 March;
//! - CGST Rules 2017, rule 46(b): the invoice serial number is unique for a financial year, at
//!   most 16 characters, made of letters, digits, hyphen and slash.

#![allow(clippy::unwrap_used, reason = "test fixtures")]

use std::collections::BTreeMap;

use ledgerly_core::{CompanyId, Date, StateCode, VoucherId, VoucherTypeId};
use ledgerly_engine::EngineError;
use ledgerly_engine::numbering::next_number;
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, Masters, NegativeStockPolicy,
    NumberingConfig, NumberingMethod, RestartPeriod, Voucher, VoucherMode, VoucherStatus,
    VoucherType,
};
use time::macros::date;

const TYPE: VoucherTypeId = VoucherTypeId(1);

fn vtype(base: BaseVoucherKind, prefix: &str, width: u8, restart: RestartPeriod) -> VoucherType {
    VoucherType {
        id: TYPE,
        name: "Test".to_string(),
        base,
        abbreviation: "T".to_string(),
        numbering: NumberingConfig {
            method: NumberingMethod::Automatic,
            prevent_renumbering: false,
            prefix: prefix.to_string(),
            suffix: String::new(),
            width,
            restart,
            series: Vec::new(),
        },
        active: true,
        tally: None,
    }
}

fn books(gst: bool, vouchers: Vec<Voucher>) -> Books {
    let company = Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_string(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(27).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Proprietorship,
        features: Features {
            gst,
            ..Features::default()
        },
        negative_stock: NegativeStockPolicy::Block,
    };
    Books {
        company,
        masters: Masters::default(),
        vouchers: vouchers
            .into_iter()
            .map(|v| (v.id, v))
            .collect::<BTreeMap<_, _>>(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

fn voucher(
    id: u64,
    type_id: VoucherTypeId,
    number: &str,
    series: Option<&str>,
    date: Date,
    status: VoucherStatus,
) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id,
        number: Some(number.to_string()),
        series: series.map(str::to_string),
        date,
        supply_date: None,
        payment_date: None,
        status,
        mode: VoucherMode::AsVoucher,
        party: None,
        place_of_supply: None,
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: String::new(),
        ledger_lines: Vec::new(),
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn reg(id: u64, number: &str, d: Date) -> Voucher {
    voucher(id, TYPE, number, None, d, VoucherStatus::Regular)
}

#[test]
fn first_number_uses_prefix_and_width() {
    // derived: prefix `INV/26-27/` + counter 1 zero-padded to width 4 = `INV/26-27/0001`.
    let vt = vtype(
        BaseVoucherKind::Sales,
        "INV/26-27/",
        4,
        RestartPeriod::Yearly,
    );
    let n = next_number(&vt, None, date!(2026 - 04 - 10), &books(false, vec![])).unwrap();
    assert_eq!(n, "INV/26-27/0001");
}

#[test]
fn next_is_one_more_than_highest_used() {
    // derived: highest used counter is 3 (gap at 2 is not refilled), so next is 4.
    let vt = vtype(
        BaseVoucherKind::Sales,
        "INV/26-27/",
        4,
        RestartPeriod::Yearly,
    );
    let b = books(
        false,
        vec![
            reg(1, "INV/26-27/0001", date!(2026 - 04 - 10)),
            reg(2, "INV/26-27/0003", date!(2026 - 04 - 11)),
        ],
    );
    assert_eq!(
        next_number(&vt, None, date!(2026 - 05 - 01), &b).unwrap(),
        "INV/26-27/0004"
    );
}

#[test]
fn yearly_restart_starts_again_each_financial_year() {
    // derived: FY 2026-27 is 1 Apr 2026 to 31 Mar 2027. Counter 5 used in 2026-27.
    let vt = vtype(BaseVoucherKind::Sales, "INV/", 4, RestartPeriod::Yearly);
    let b = books(false, vec![reg(1, "INV/0005", date!(2026 - 06 - 01))]);
    // 31 Mar 2027 is still FY 2026-27: 5 + 1 = 6.
    assert_eq!(
        next_number(&vt, None, date!(2027 - 03 - 31), &b).unwrap(),
        "INV/0006"
    );
    // 1 Apr 2027 starts FY 2027-28: restarts at 1.
    assert_eq!(
        next_number(&vt, None, date!(2027 - 04 - 01), &b).unwrap(),
        "INV/0001"
    );
    // 31 Mar 2026 is FY 2025-26, which has no vouchers: 1.
    assert_eq!(
        next_number(&vt, None, date!(2026 - 03 - 31), &b).unwrap(),
        "INV/0001"
    );
}

#[test]
fn never_restart_continues_across_years() {
    // derived: RestartPeriod::Never means one counter for all dates; highest 5 -> 6.
    let vt = vtype(BaseVoucherKind::Journal, "J-", 3, RestartPeriod::Never);
    let b = books(false, vec![reg(1, "J-005", date!(2026 - 06 - 01))]);
    assert_eq!(
        next_number(&vt, None, date!(2030 - 01 - 01), &b).unwrap(),
        "J-006"
    );
}

#[test]
fn monthly_restart_starts_again_each_calendar_month() {
    // derived: RestartPeriod::Monthly restarts the counter each calendar month.
    let vt = vtype(BaseVoucherKind::Journal, "", 2, RestartPeriod::Monthly);
    let b = books(false, vec![reg(1, "07", date!(2026 - 06 - 30))]);
    assert_eq!(
        next_number(&vt, None, date!(2026 - 06 - 01), &b).unwrap(),
        "08"
    );
    assert_eq!(
        next_number(&vt, None, date!(2026 - 07 - 01), &b).unwrap(),
        "01"
    );
    // Same month number, a year later, is a different month.
    assert_eq!(
        next_number(&vt, None, date!(2027 - 06 - 15), &b).unwrap(),
        "01"
    );
}

#[test]
fn number_of_cancelled_voucher_is_not_reused() {
    // derived: spec 5 "Cancelled: number kept, no amounts". Counter 2 belongs to a cancelled
    // voucher and stays used, so the next is 3, not 2.
    let vt = vtype(
        BaseVoucherKind::Sales,
        "INV/26-27/",
        4,
        RestartPeriod::Yearly,
    );
    let b = books(
        false,
        vec![
            reg(1, "INV/26-27/0001", date!(2026 - 04 - 10)),
            voucher(
                2,
                TYPE,
                "INV/26-27/0002",
                None,
                date!(2026 - 04 - 11),
                VoucherStatus::Cancelled,
            ),
        ],
    );
    assert_eq!(
        next_number(&vt, None, date!(2026 - 04 - 12), &b).unwrap(),
        "INV/26-27/0003"
    );
}

#[test]
fn cancelled_highest_number_is_not_reused() {
    // derived: the highest number is cancelled; it still counts. 1 used, 2 cancelled -> 3.
    let vt = vtype(BaseVoucherKind::Sales, "", 1, RestartPeriod::Never);
    let b = books(
        false,
        vec![
            reg(1, "1", date!(2026 - 04 - 10)),
            voucher(
                2,
                TYPE,
                "2",
                None,
                date!(2026 - 04 - 11),
                VoucherStatus::Cancelled,
            ),
        ],
    );
    assert_eq!(
        next_number(&vt, None, date!(2026 - 04 - 12), &b).unwrap(),
        "3"
    );
}

#[test]
fn optional_and_post_dated_numbers_are_used_too() {
    // derived: invariant 7 is about numbers, not status; every stored voucher keeps its number.
    let vt = vtype(BaseVoucherKind::Journal, "", 2, RestartPeriod::Never);
    let b = books(
        false,
        vec![
            voucher(
                1,
                TYPE,
                "01",
                None,
                date!(2026 - 04 - 10),
                VoucherStatus::Optional,
            ),
            voucher(
                2,
                TYPE,
                "02",
                None,
                date!(2026 - 04 - 11),
                VoucherStatus::PostDated,
            ),
        ],
    );
    assert_eq!(
        next_number(&vt, None, date!(2026 - 04 - 12), &b).unwrap(),
        "03"
    );
}

#[test]
fn other_types_do_not_share_a_counter() {
    // derived: uniqueness is per (type, series, period); a voucher of another type is ignored.
    let vt = vtype(BaseVoucherKind::Journal, "J-", 2, RestartPeriod::Never);
    let b = books(
        false,
        vec![voucher(
            1,
            VoucherTypeId(2),
            "J-09",
            None,
            date!(2026 - 04 - 10),
            VoucherStatus::Regular,
        )],
    );
    assert_eq!(
        next_number(&vt, None, date!(2026 - 04 - 12), &b).unwrap(),
        "J-01"
    );
}

#[test]
fn series_have_separate_counters() {
    // derived: uniqueness is per (type, series, period). Series A has 2 used, series B has none.
    let mut vt = vtype(BaseVoucherKind::Sales, "", 2, RestartPeriod::Never);
    vt.numbering.series = vec!["A".to_string(), "B".to_string()];
    let b = books(
        false,
        vec![voucher(
            1,
            TYPE,
            "02",
            Some("A"),
            date!(2026 - 04 - 10),
            VoucherStatus::Regular,
        )],
    );
    assert_eq!(
        next_number(&vt, Some("A"), date!(2026 - 04 - 12), &b).unwrap(),
        "03"
    );
    assert_eq!(
        next_number(&vt, Some("B"), date!(2026 - 04 - 12), &b).unwrap(),
        "01"
    );
}

#[test]
fn unknown_or_missing_series_is_an_error() {
    // derived: a series not configured on the type, or none given when the type has several,
    // cannot be allocated without guessing; both are rejected.
    let mut vt = vtype(BaseVoucherKind::Sales, "", 2, RestartPeriod::Never);
    vt.numbering.series = vec!["A".to_string()];
    let b = books(false, vec![]);
    assert!(matches!(
        next_number(&vt, Some("Z"), date!(2026 - 04 - 12), &b),
        Err(EngineError::NotFound { .. })
    ));
    assert!(matches!(
        next_number(&vt, None, date!(2026 - 04 - 12), &b),
        Err(EngineError::Invalid { .. })
    ));
    let plain = vtype(BaseVoucherKind::Sales, "", 2, RestartPeriod::Never);
    assert!(matches!(
        next_number(&plain, Some("A"), date!(2026 - 04 - 12), &b),
        Err(EngineError::NotFound { .. })
    ));
}

#[test]
fn suffix_is_appended_and_stripped_when_counting() {
    // derived: number = prefix + counter + suffix; `P7-` + `/X` around `0007` gives counter 7.
    let mut vt = vtype(BaseVoucherKind::Journal, "P7-", 4, RestartPeriod::Never);
    vt.numbering.suffix = "/X".to_string();
    let b = books(false, vec![reg(1, "P7-0007/X", date!(2026 - 04 - 10))]);
    assert_eq!(
        next_number(&vt, None, date!(2026 - 04 - 12), &b).unwrap(),
        "P7-0008/X"
    );
}

#[test]
fn counter_wider_than_width_is_not_truncated() {
    // derived: `width` is a minimum number of digits; counter 10000 has 5 digits.
    let vt = vtype(BaseVoucherKind::Journal, "", 4, RestartPeriod::Never);
    let b = books(false, vec![reg(1, "9999", date!(2026 - 04 - 10))]);
    assert_eq!(
        next_number(&vt, None, date!(2026 - 04 - 12), &b).unwrap(),
        "10000"
    );
}

#[test]
fn manual_and_none_methods_are_not_allocated() {
    // derived: only NumberingMethod::Automatic is allocated by the engine.
    let b = books(false, vec![]);
    for method in [NumberingMethod::Manual, NumberingMethod::None] {
        let mut vt = vtype(BaseVoucherKind::Journal, "", 4, RestartPeriod::Never);
        vt.numbering.method = method;
        assert!(matches!(
            next_number(&vt, None, date!(2026 - 04 - 12), &b),
            Err(EngineError::Invalid { .. })
        ));
    }
}

#[test]
fn gst_invoice_number_over_16_characters_errors() {
    // derived: CGST Rule 46(b): serial number at most 16 characters.
    // `INVOICE/26-27/` is 14 characters; + 4 digits = 18 > 16.
    let vt = vtype(
        BaseVoucherKind::Sales,
        "INVOICE/26-27/",
        4,
        RestartPeriod::Yearly,
    );
    let b = books(true, vec![]);
    let err = next_number(&vt, None, date!(2026 - 04 - 10), &b).unwrap_err();
    assert_eq!(
        err,
        EngineError::NumberTooLong {
            number: "INVOICE/26-27/0001".to_string(),
            max: 16
        }
    );
}

#[test]
fn gst_invoice_number_boundary_16_ok_17_errors() {
    // derived: Rule 46(b) limit is 16 inclusive. `INV/26-27/` (10) + 6 digits = 16 passes;
    // + 7 digits = 17 fails.
    let b = books(true, vec![]);
    let ok = vtype(
        BaseVoucherKind::Sales,
        "INV/26-27/",
        6,
        RestartPeriod::Yearly,
    );
    assert_eq!(
        next_number(&ok, None, date!(2026 - 04 - 10), &b).unwrap(),
        "INV/26-27/000001"
    );
    let long = vtype(
        BaseVoucherKind::Sales,
        "INV/26-27/",
        7,
        RestartPeriod::Yearly,
    );
    assert!(matches!(
        next_number(&long, None, date!(2026 - 04 - 10), &b),
        Err(EngineError::NumberTooLong { max: 16, .. })
    ));
}

#[test]
fn gst_invoice_number_with_disallowed_character_errors() {
    // derived: Rule 46(b) allows letters, digits, hyphen and slash only; `#` is not allowed.
    let vt = vtype(BaseVoucherKind::Sales, "INV#", 4, RestartPeriod::Yearly);
    let b = books(true, vec![]);
    assert!(matches!(
        next_number(&vt, None, date!(2026 - 04 - 10), &b),
        Err(EngineError::Invalid { .. })
    ));
}

#[test]
fn length_limit_applies_only_to_gst_sales_documents() {
    // derived: Rule 46 governs tax invoices issued by the supplier. With GST off, or for a
    // journal, the 16 character limit does not apply.
    let long = "A-VERY-LONG-PREFIX/";
    let sales = vtype(BaseVoucherKind::Sales, long, 4, RestartPeriod::Never);
    assert!(next_number(&sales, None, date!(2026 - 04 - 10), &books(false, vec![])).is_ok());
    let journal = vtype(BaseVoucherKind::Journal, long, 4, RestartPeriod::Never);
    assert!(next_number(&journal, None, date!(2026 - 04 - 10), &books(true, vec![])).is_ok());
}

#[test]
fn financial_year_not_starting_in_april_is_unsupported_for_yearly_restart() {
    // derived: FinancialYear in ledgerly-core is April-start only; a yearly counter for another
    // start month cannot be computed, so it is rejected rather than guessed.
    let vt = vtype(BaseVoucherKind::Sales, "I", 3, RestartPeriod::Yearly);
    let mut b = books(false, vec![]);
    b.company.fy_start = date!(2026 - 01 - 01);
    assert!(matches!(
        next_number(&vt, None, date!(2026 - 04 - 10), &b),
        Err(EngineError::Invalid { .. })
    ));
}

#[test]
fn counter_is_never_a_number_already_present_unparsed() {
    // derived: format is prefix + digits + suffix; `X-0001-old` has trailing text, so it is
    // not a counter-bearing number and is ignored; counter 1 is free, `X-0001` is unused.
    let vt = vtype(BaseVoucherKind::Journal, "X-", 4, RestartPeriod::Never);
    let b = books(false, vec![reg(1, "X-0001-old", date!(2026 - 04 - 10))]);
    assert_eq!(
        next_number(&vt, None, date!(2026 - 04 - 12), &b).unwrap(),
        "X-0001"
    );
}
