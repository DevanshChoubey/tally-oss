//! s9 GST returns: the GSTR-1 tiles count sections, not vouchers.
//!
//! Provenance: derived from the design mock (`design/mock-v2/index.html`, `SCREENS.gst`, the
//! `cnt` function: a tile counts the sections whose status is its name). With the mock's six
//! sections (4A Ready, 5 Not relevant, 7 Ready, 9B Not relevant, 12 Needs correction, 13 Ready)
//! the tiles read Ready 3, Not relevant 2, Mismatch 0, Needs correction 1.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_api::{Bucket as ApiBucket, Gstr1Section};
use ledgerly_app::screens::gst::{Bucket, GstScreen, GstTab};
use ledgerly_core::Paise;

fn section(code: &'static str, status: ApiBucket) -> Gstr1Section {
    Gstr1Section {
        code,
        name: "x",
        records: 1,
        taxable: Some(Paise::ZERO),
        tax: None,
        status,
        note: None,
    }
}

#[test]
fn the_gstr1_tiles_count_sections_by_status() {
    let mut s = GstScreen::new("November 2026", None);
    assert_eq!(s.section_count(Bucket::Ready), None, "nothing loaded yet");
    s.set_sections(vec![
        section("4A", ApiBucket::Ready),
        section("5", ApiBucket::NotRelevant),
        section("7", ApiBucket::Ready),
        section("9B", ApiBucket::NotRelevant),
        section("12", ApiBucket::NeedsCorrection),
        section("13", ApiBucket::Ready),
    ]);
    assert_eq!(s.section_count(Bucket::Ready), Some(3));
    assert_eq!(s.section_count(Bucket::NotRelevant), Some(2));
    assert_eq!(s.section_count(Bucket::Mismatch), Some(0));
    assert_eq!(s.section_count(Bucket::NeedsCorrection), Some(1));
    assert_eq!(s.tab(), GstTab::Reconciliation);
}
