//! Round s21: values read by ocrs need review (coordinator decision 2026-11-27: ocrs gives no
//! confidence, so every value it reads is "needs review", the lowest confidence; the person
//! confirms each field before the draft can post).
//!
//! Provenance. `fixtures/ocrs/bill-left.png` was drawn for this test (`DejaVu Sans` 26 px, one
//! `Label value` line each) and `bill-left.ocrs.json` is what ocrs 0.14.0 returned for it with the
//! owner-fetched models on 2026-11-27 (observed; the models are not kept). The expected values are
//! the drawn text: invoice KS/2026/0042, date 15/03/2026, total 17,700.00 = 1,770,000 paise, tax
//! 2,700.00 = 270,000 paise, taxable 15,000.00 = 1,500,000 paise, CGST and SGST 1,350.00 =
//! 135,000 paise each, supplier Kaveri Stationers, GSTIN 29ABCDE1234F1ZW (the a3 test vector).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_assist::ocr::{OcrOutput, read_bill};
use ledgerly_assist::ocr_models::OCRS_CONFIDENCE_PERMILLE;
use ledgerly_assist::review::{DraftField, NEEDS_REVIEW_PERMILLE, confirm, needs_review};
use ledgerly_core::{Decimal, GroupId, Gstin, LedgerId, Paise, Rate, SignedAmount};
use ledgerly_model::{
    Address, Ledger, Masters, PartyDetails, RegistrationType, TaxDetails, TaxLedgerKind,
};
use time::macros::date;

fn ocrs_bill() -> OcrOutput {
    serde_json::from_str(include_str!("fixtures/ocrs/bill-left.ocrs.json")).unwrap()
}

fn ledger(id: u64, name: &str, party: Option<PartyDetails>, tax: Option<TaxDetails>) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id: LedgerId(id),
        name: name.to_owned(),
        aliases: vec![],
        group: Some(GroupId(1)),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party,
        bank: None,
        tax,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn masters() -> Masters {
    let supplier = PartyDetails {
        registration: RegistrationType::Regular,
        gstin: Some(Gstin::parse("29ABCDE1234F1ZW").unwrap()),
        pan: None,
        state: None,
        address: Address::default(),
    };
    let tax = |kind| TaxDetails {
        kind,
        rate_percent: Some(Rate(Decimal::new(9, 0))),
    };
    let mut m = Masters::default();
    for l in [
        ledger(40, "Kaveri Stationers", Some(supplier), None),
        ledger(20, "Input CGST", None, Some(tax(TaxLedgerKind::Cgst))),
        ledger(21, "Input SGST", None, Some(tax(TaxLedgerKind::Sgst))),
    ] {
        m.ledgers.insert(l.id, l);
    }
    m
}

#[test]
fn needs_review_is_the_lowest_confidence_and_ocrs_values_carry_it() {
    assert_eq!(NEEDS_REVIEW_PERMILLE, 0);
    assert_eq!(OCRS_CONFIDENCE_PERMILLE, NEEDS_REVIEW_PERMILLE);
    assert!(
        ocrs_bill()
            .lines
            .iter()
            .all(|l| l.confidence_permille == NEEDS_REVIEW_PERMILLE)
    );
}

#[test]
fn the_ocrs_reading_of_the_drawn_bill_has_the_drawn_values_each_needing_review() {
    let draft = read_bill(&ocrs_bill(), &masters()).unwrap().draft;
    assert_eq!(draft.invoice_number.as_ref().unwrap().value, "KS/2026/0042");
    assert_eq!(draft.date.as_ref().unwrap().value, date!(2026 - 03 - 15));
    assert_eq!(
        draft.supplier_gstin.as_ref().unwrap().value,
        "29ABCDE1234F1ZW"
    );
    assert_eq!(
        draft.party_name.as_ref().unwrap().value,
        "Kaveri Stationers"
    );
    assert_eq!(
        draft.document_total.as_ref().unwrap().value,
        Paise::new(1_770_000)
    );
    assert_eq!(
        draft.document_tax.as_ref().unwrap().value,
        Paise::new(270_000)
    );
    let mut amounts: Vec<i64> = draft.lines.iter().map(|l| l.amount.value.get()).collect();
    amounts.sort_unstable();
    assert_eq!(amounts, [135_000, 135_000, 1_500_000, 1_770_000]);

    let fields = needs_review(&draft);
    for f in [
        DraftField::Date,
        DraftField::PartyName,
        DraftField::InvoiceNumber,
        DraftField::SupplierGstin,
        DraftField::DocumentTotal,
        DraftField::DocumentTax,
    ] {
        assert!(fields.contains(&f), "{f:?} in {fields:?}");
    }
    for i in 0..draft.lines.len() {
        assert!(fields.contains(&DraftField::LineAmount(i)), "{fields:?}");
        assert!(fields.contains(&DraftField::LineLedger(i)), "{fields:?}");
    }
}

#[test]
fn confirming_every_field_leaves_nothing_to_review_and_changes_no_value() {
    let read = read_bill(&ocrs_bill(), &masters()).unwrap().draft;
    let mut draft = read.clone();
    for f in needs_review(&read) {
        assert!(confirm(&mut draft, f), "{f:?}");
    }
    assert_eq!(needs_review(&draft), Vec::<DraftField>::new());
    assert_eq!(
        draft.document_total.as_ref().unwrap().value,
        Paise::new(1_770_000)
    );
    assert_eq!(
        draft.document_total.as_ref().unwrap().source,
        read.document_total.unwrap().source
    );
    assert_eq!(draft.date.as_ref().unwrap().confidence_permille, 1000);
}

#[test]
fn a_field_the_draft_does_not_have_cannot_be_confirmed() {
    let mut draft = read_bill(&ocrs_bill(), &masters()).unwrap().draft;
    let n = draft.lines.len();
    assert!(!confirm(&mut draft, DraftField::LineAmount(n)));
    draft.invoice_number = None;
    assert!(!confirm(&mut draft, DraftField::InvoiceNumber));
}

#[test]
fn field_names_are_the_validators() {
    assert_eq!(DraftField::Date.name(), "date");
    assert_eq!(DraftField::LineAmount(1).name(), "lines[1].amount");
    assert_eq!(DraftField::LineLedger(0).name(), "lines[0].ledger");
    assert_eq!(DraftField::DocumentTotal.name(), "document_total");
}
