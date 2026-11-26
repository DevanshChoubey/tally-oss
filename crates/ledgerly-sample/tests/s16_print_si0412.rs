//! Round s16 fix 3: the sample's SI/0412 (two items, two HSN codes, both at 18%) prints.
//!
//! Every expected value is `derived` by hand from the sample's vouchers (the same data as
//! `design/mock-v2/index.html` `VOUCHERS`) and the 18% slab (CGST 9% + SGST 9%, GST rate pack
//! 2026.1-slabs), written next to it. The tax table is by HSN code, as the print help page says
//! the product's tax invoice lists tax by HSN/SAC
//! (<https://help.tallysolutions.com/print-documents/>, read 2026-11-26).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests stop on any failure"
)]

use ledgerly_api::{Api, PageItem};
use ledgerly_core::{Clock, Outcome};
use time::OffsetDateTime;

fn texts(api: &Api, number: &str) -> Vec<String> {
    let Outcome::Value(books) = api.books().unwrap() else {
        panic!("books")
    };
    let id = books
        .vouchers
        .values()
        .find(|v| v.number.as_deref() == Some(number))
        .unwrap()
        .id;
    let doc = match api.print_voucher_document(id).unwrap() {
        Outcome::Value(d) => d,
        Outcome::Unsupported(u) => panic!("unsupported: {} ({})", u.what, u.reason),
    };
    doc.pages
        .iter()
        .flat_map(|p| p.items.iter())
        .filter_map(|i| match i {
            PageItem::Text { text, .. } => Some(text.clone()),
            PageItem::Rule { .. } | PageItem::Fill { .. } => None,
        })
        .collect()
}

#[test]
fn si_0412_prints_with_one_tax_row_per_hsn_code() {
    // provenance: derived. 25 x 3,850.00 = 96,250.00 (HSN 3917) and 67 x 505.00 = 33,835.00
    // (HSN 7419), both 18% intra-state: CGST 9% of 96,250.00 = 8,662.50 and of 33,835.00 =
    // 3,045.15 (SGST the same), so the total tax of the two rows is 17,325.00 and 6,090.30 and
    // of the invoice 23,415.30 (= 2 x 11,707.65); 1,30,085.00 + 23,415.30 = 1,53,500.30 prints
    // as 1,53,500.00 with round-off -0.30.
    let dir = tempfile::tempdir().unwrap();
    let api = ledgerly_sample::create(
        &dir.path().join("s.ledgerly"),
        Clock::Fixed(OffsetDateTime::UNIX_EPOCH),
    )
    .unwrap();
    let t = texts(&api, "SI/0412");
    for want in [
        "3917",
        "7419",
        "96,250.00",
        "33,835.00",
        "8,662.50",
        "3,045.15",
        "17,325.00",
        "6,090.30",
        "23,415.30",
        "-0.30",
        "1,30,085.00",
        "1,53,500.00",
    ] {
        assert!(
            t.iter().any(|s| s.contains(want)),
            "{want} not printed in {t:?}"
        );
    }
}
