//! Card P1: the built-in tax invoice template loads, so printing a voucher never fails on it.
//!
//! Provenance: the block list is `derived` from the template file itself (header, details, item
//! table, tax table, totals, amount in words, text), checked against the format rules in
//! `template.rs`. No output of the renderer is used as an expected value.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_print::TAX_INVOICE_A4;
use ledgerly_print::template::{BlockKind, load_template};

#[test]
fn the_built_in_tax_invoice_template_loads_with_its_blocks_in_order() {
    let t = load_template(TAX_INVOICE_A4).unwrap();
    let kinds: Vec<BlockKind> = t.blocks.iter().map(|b| b.body.kind()).collect();
    assert_eq!(
        kinds,
        vec![
            BlockKind::Header,
            BlockKind::Details,
            BlockKind::ItemTable,
            BlockKind::TaxTable,
            BlockKind::Totals,
            BlockKind::AmountInWords,
            BlockKind::Text,
        ]
    );
    assert!(
        kinds.iter().all(|k| *k != BlockKind::UpiQr),
        "the built-in template prints no UPI code: the books hold no payee address"
    );
}
