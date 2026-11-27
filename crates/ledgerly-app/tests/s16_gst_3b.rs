//! Round s16: the GSTR-3B tab draws the payment of tax (table 6.1) the engine worked out from the
//! credit accepted in IMS. Every amount is derived by hand from the sample's November vouchers and
//! 2B file, as written beside it, from CGST Act s.49 and s.170 (read 2026-11-26); none is copied
//! from engine output. The mock's rows are `design/mock-v2/index.html` `gst.gstr3b`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::time::{Duration, Instant};

use ledgerly_app::screens::gst::look::Line3b;
use ledgerly_app::screens::gst::{GstScreen, GstTab, ImsAction};
use ledgerly_app::worker::ApiWorker;
use ledgerly_core::{Clock, Gstin, Paise, Period};
use time::OffsetDateTime;
use time::macros::date;

fn gstin() -> Gstin {
    Gstin::parse("27ZZZZZ0000Z1ZQ").unwrap()
}

fn november() -> Period {
    Period::new(date!(2026 - 11 - 01), date!(2026 - 11 - 30)).unwrap()
}

fn rs(rupees: i64, paise: i64) -> Paise {
    Paise::new(rupees * 100 + paise)
}

fn open(with_2b: bool) -> (tempfile::TempDir, ApiWorker) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sample.ledgerly");
    let clock = Clock::Fixed(OffsetDateTime::UNIX_EPOCH);
    let api = if with_2b {
        ledgerly_sample::create(&path, clock).unwrap()
    } else {
        ledgerly_sample::create_without_2b(&path, clock).unwrap()
    };
    (dir, ApiWorker::spawn(api).unwrap())
}

fn settle(s: &mut GstScreen, w: &ApiWorker) {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        s.tick(Some(w));
        if !s.is_loading() && !s.has_pending_changes() {
            return;
        }
        assert!(Instant::now() < until, "the worker did not finish");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn screen(w: &ApiWorker) -> GstScreen {
    let mut s = GstScreen::new("November 2026", Some(gstin()));
    s.switch_tab(GstTab::Gstr3b);
    s.request_load(gstin(), november());
    settle(&mut s, w);
    s
}

/// `(label, tax heads)` of every figure row, in order.
fn rows(s: &GstScreen) -> Vec<(&'static str, Option<Paise>, [Paise; 4])> {
    s.gstr3b_lines()
        .iter()
        .filter_map(|l| match l {
            Line3b::Row {
                label,
                taxable,
                tax,
                ..
            } => Some((*label, *taxable, *tax)),
            Line3b::Total {
                label,
                taxable,
                tax,
            } => Some((*label, Some(*taxable), *tax)),
            Line3b::ValueOnly { label, value, .. } => {
                Some((*label, Some(*value), [Paise::ZERO; 4]))
            }
            Line3b::Sub(_) | Line3b::SubSourced { .. } => None,
        })
        .collect()
}

fn find(
    rows: &[(&'static str, Option<Paise>, [Paise; 4])],
    label: &str,
) -> (Option<Paise>, [Paise; 4]) {
    let r = rows.iter().find(|r| r.0 == label).unwrap_or_else(|| {
        panic!(
            "no row {label:?} in {:?}",
            rows.iter().map(|r| r.0).collect::<Vec<_>>()
        )
    });
    (r.1, r.2)
}

#[test]
fn the_tab_draws_the_mocks_set_off_rows_in_the_mocks_order() {
    // provenance: the mock's gst.gstr3b rows, in order: 3.1(a), 3.1(c), 3.1(d), 3.1(e), "4
    // Eligible ITC" with Reverse charge 4(A)(3), In your books, Accepted in IMS, No action yet; "6.1 Payment of tax" with Left after using accepted
    // credit and Payable in cash, rounded to rupees. The blocked row is ours and stays last in 4.
    let (_d, w) = open(true);
    let s = screen(&w);
    let labels: Vec<&str> = s
        .gstr3b_lines()
        .iter()
        .map(|l| match l {
            Line3b::Sub(label)
            | Line3b::SubSourced { label, .. }
            | Line3b::Row { label, .. }
            | Line3b::ValueOnly { label, .. }
            | Line3b::Total { label, .. } => *label,
        })
        .collect();
    assert_eq!(
        labels,
        vec![
            "3.1(a) Outward taxable supplies",
            "3.1(c) Nil rated and exempted",
            "3.1(d) Inward supplies liable to reverse charge",
            "3.1(e) Non-GST outward supplies",
            "4 Eligible ITC",
            "Reverse charge, 4(A)(3)",
            "In your books",
            "Accepted in IMS",
            "No action yet: deemed accepted when 2B is generated",
            "Blocked or ineligible, not claimed",
            "6.1 Payment of tax",
            "Left after using accepted credit",
            "Payable in cash, rounded to rupees",
        ]
    );
}

#[test]
fn the_rows_carry_the_hand_derived_november_figures() {
    // provenance: derived (see crates/ledgerly-sample/tests/s16_payment.rs). Accepted: only G1,
    // IGST 11,520.00. No action: G2 and G4 IGST 18,450.00 + 12,240.00 = 30,690.00 and G3 CGST and
    // SGST 1,080.00 each. Left: IGST 20,370.00 - 11,520.00 = 8,850.00, CGST and SGST 15,323.65.
    // Cash (s.170 rounds each head to the rupee): 8,850.00, 15,324.00, 15,324.00; total 39,498.00.
    let (_d, w) = open(true);
    let s = screen(&w);
    let r = rows(&s);
    let z = Paise::ZERO;
    assert_eq!(find(&r, "Accepted in IMS").1, [rs(11_520, 0), z, z, z]);
    assert_eq!(
        find(&r, "No action yet: deemed accepted when 2B is generated").1,
        [rs(30_690, 0), rs(1_080, 0), rs(1_080, 0), z]
    );
    assert_eq!(
        find(&r, "Left after using accepted credit").1,
        [rs(8_850, 0), rs(15_323, 65), rs(15_323, 65), z]
    );
    let (total, cash) = find(&r, "Payable in cash, rounded to rupees");
    assert_eq!(cash, [rs(8_850, 0), rs(15_324, 0), rs(15_324, 0), z]);
    assert_eq!(total, Some(rs(39_498, 0)));
}

#[test]
fn rejecting_the_accepted_record_changes_the_cash_payable() {
    // provenance: derived: with G1 rejected no credit is accepted, so the cash is the whole
    // liability: 20,370.00 + 15,324.00 + 15,324.00 = 51,018.00.
    let (_d, w) = open(true);
    let mut s = screen(&w);
    s.decide(0, ImsAction::Reject).unwrap();
    settle(&mut s, &w);
    s.request_load(gstin(), november());
    settle(&mut s, &w);
    let r = rows(&s);
    assert_eq!(
        find(&r, "Payable in cash, rounded to rupees").0,
        Some(rs(51_018, 0))
    );
}

#[test]
fn without_a_2b_file_the_payment_rows_are_left_out_and_the_reason_is_said() {
    // provenance: derived: the engine refuses the payment without a 2B file ("Needs GSTR-2B"),
    // so no cash figure is drawn and a line says why; the working's own rows stay.
    let (_d, w) = open(false);
    let s = screen(&w);
    let labels: Vec<&str> = rows(&s).iter().map(|r| r.0).collect();
    assert!(
        !labels.contains(&"Payable in cash, rounded to rupees"),
        "{labels:?}"
    );
    assert!(labels.contains(&"In your books"));
    let notes = &s.figures(GstTab::Gstr3b).unwrap().notes;
    assert!(
        notes
            .iter()
            .any(|n| n.starts_with("Payment of tax not worked out: Needs GSTR-2B")),
        "{notes:?}"
    );
}
