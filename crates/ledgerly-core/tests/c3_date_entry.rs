//! Card C3 tests: the public date-entry interface, FY 2026-27.
#![allow(
    clippy::unwrap_used,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]
use ledgerly_core::FinancialYear;
use ledgerly_core::date_entry::{DateEntryError, parse_date_entry};
use time::macros::date;

#[test]
fn the_card_cases() {
    let fy = FinancialYear::april(2026).unwrap();
    let today = date!(2026 - 06 - 20);
    // provenance: derived — Indian FY 1 Apr–31 Mar; FY 2026-27 starts 1 Apr 2026.
    assert_eq!(
        parse_date_entry("1-4", fy, today),
        Ok(date!(2026 - 04 - 01))
    );
    // provenance: derived — January is in the FY's second calendar year, 2027.
    assert_eq!(
        parse_date_entry("15/1", fy, today),
        Ok(date!(2027 - 01 - 15))
    );
    // provenance: derived — February 2027 has 28 days; 31-2 is never valid.
    assert!(matches!(
        parse_date_entry("31-2", fy, today),
        Err(DateEntryError::InvalidDate { .. })
    ));
    // provenance: derived — an ISO date is the date it names.
    assert_eq!(
        parse_date_entry("2026-05-07", fy, today),
        Ok(date!(2026 - 05 - 07))
    );
}
