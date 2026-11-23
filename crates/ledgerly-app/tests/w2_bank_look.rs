//! The bank screen drawn to the mock (`design/mock-v2/index.html`, the `bank` screen at about
//! lines 1733-1746 and the `table.t` rules at lines 148-158): the reconciliation statement's
//! lines.
#![allow(clippy::unwrap_used, reason = "test code: rule B2 lets tests unwrap")]

use ledgerly_app::screens::bank::{BrsRow, BrsRowKind, BrsView, brs_line_heights};
use ledgerly_core::Paise;
use time::macros::date;

fn row(kind: BrsRowKind) -> BrsRow {
    BrsRow {
        label: "Line".to_owned(),
        amount: Paise::new(100),
        kind,
    }
}

#[test]
fn each_statement_line_has_the_mocks_height() {
    // provenance: derived from index.html:149-150 and 156 (13 px text at line-height 1.45 on
    // whole points is 19), as for the balance sheet: a `td` line is 7 + 19 + 7 + 1 px rule = 34;
    // the `tr.total` line (the expected bank balance, index.html:1743) is 7 + 19 + 7 and the 3 px
    // double rule = 36. The statement has no heading line (`<table class="t"><tbody>`).
    let view = BrsView {
        as_of: date!(2026 - 11 - 20),
        rows: vec![
            row(BrsRowKind::Books),
            row(BrsRowKind::Uncleared),
            row(BrsRowKind::Uncleared),
            row(BrsRowKind::Expected),
            row(BrsRowKind::Statement),
            row(BrsRowKind::Difference),
        ],
    };
    assert_eq!(
        brs_line_heights(&view),
        [34.0, 34.0, 34.0, 36.0, 34.0, 34.0]
    );
}
