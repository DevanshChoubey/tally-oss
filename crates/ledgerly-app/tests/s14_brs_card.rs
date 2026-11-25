//! The card beside the reconciliation statement says Reconciled or Not yet reconciled from the
//! engine's own Difference line, and claims nothing when there is no such line.
//!
//! provenance: design/mock-v2/index.html:1819, `b.bank===b.expected ? 'Reconciled' : 'Not yet
//! reconciled'`: the statement's closing balance equals the expected bank balance exactly when the
//! Difference line (statement minus expected) is nil.

use ledgerly_app::screens::bank::{BrsRow, BrsRowKind, BrsVerdict, BrsView, brs_verdict};
use ledgerly_core::Paise;
use time::macros::date;

fn view(difference: Option<i64>) -> BrsView {
    let mut rows = vec![BrsRow {
        label: "Balance as per company books".to_owned(),
        amount: Paise::new(10_000),
        kind: BrsRowKind::Books,
    }];
    if let Some(d) = difference {
        rows.push(BrsRow {
            label: "Difference".to_owned(),
            amount: Paise::new(d),
            kind: BrsRowKind::Difference,
        });
    }
    BrsView {
        as_of: date!(2026 - 11 - 20),
        rows,
    }
}

#[test]
fn a_nil_difference_is_reconciled() {
    assert_eq!(brs_verdict(&view(Some(0))), BrsVerdict::Reconciled);
}

#[test]
fn a_difference_either_way_is_not_yet_reconciled() {
    assert_eq!(brs_verdict(&view(Some(21_330_550))), BrsVerdict::NotYet);
    assert_eq!(brs_verdict(&view(Some(-5))), BrsVerdict::NotYet);
}

#[test]
fn no_difference_line_claims_nothing() {
    assert_eq!(brs_verdict(&view(None)), BrsVerdict::Unknown);
}
