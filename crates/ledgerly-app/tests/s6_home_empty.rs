//! The line under "Welcome to your new company" (mock state `home.empty`): the company, then its
//! financial year with the year's first and last day.
//!
//! Expected values are `derived` from the mock's line (design/mock-v2/index.html, `SCREENS.home`,
//! the `empty` state): "Kaveri Valley Traders · FY 2026-27 (1 Apr 2026 to 31 Mar 2027)".

use ledgerly_app::screens::home::new_company_line;
use ledgerly_core::FinancialYear;
use time::macros::date;

#[test]
fn the_new_company_line_names_the_financial_year_and_its_days() {
    let fy = FinancialYear {
        start: date!(2026 - 04 - 01),
    };
    // provenance: derived, the mock's line with our capture company's name; a year starting
    // 1 Apr 2026 ends the day before 1 Apr 2027.
    assert_eq!(
        new_company_line("Fictional Traders", fy),
        "Fictional Traders \u{b7} FY 2026-27 (1 Apr 2026 to 31 Mar 2027)"
    );
}
