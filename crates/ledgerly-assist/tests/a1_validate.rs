//! Card A1 tests: `validate_draft` recomputes a draft through the model and GST crates (spec 6.8,
//! rule A10).
//!
//! Provenance. Every expected figure is hand arithmetic written next to it. Intra-state supply is
//! taxed CGST + SGST (CGST Act s.9) and inter-state supply IGST (IGST Act s.5); the 18% slab is
//! 9% + 9% (research 04-gst 2.2 row "GST 2.0 slabs", via `packs/rates-slabs.toml`); the invoice
//! value is rounded to the whole rupee under the policy documented in card G4 (CGST Act s.170),
//! with the difference as an explicit round-off line. Payment totals are plain sums of the debit
//! lines. No expected value is copied from the engine's output. Names, GSTINs and amounts are
//! fictional. GSTINs come from `ledgerly-core`'s hand-derived test vectors.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "integration-test helpers outside #[test] fns may panic on a broken fixture"
)]

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use ledgerly_assist::validate::{DraftCheck, validate_draft, validate_draft_with_place};
use ledgerly_assist::{AssistError, DraftItem, DraftLine, SourceRef, Transcribed, VoucherDraft};
use ledgerly_core::{
    CompanyId, Date, Decimal, DrCr, GroupId, Gstin, LedgerId, Outcome, Paise, Rate, SignedAmount,
    StateCode, VoucherId, VoucherTypeId,
};
use ledgerly_gst::pack::{RuleSet, load_pack};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, GstClassification,
    GstRegistration, Ledger, Masters, NegativeStockPolicy, NumberingConfig, NumberingMethod,
    PartyDetails, RegistrationType, RestartPeriod, TaxDetails, TaxLedgerKind, Taxability, Voucher,
    VoucherMode, VoucherStatus, VoucherType,
};
use time::macros::date;

const THRESHOLD: u16 = 700;
const HIGH: u16 = 950;

const SALES_LEDGER: LedgerId = LedgerId(10);
const PARTY: LedgerId = LedgerId(11);
const PURCHASE_LEDGER: LedgerId = LedgerId(12);
const CGST: LedgerId = LedgerId(20);
const SGST: LedgerId = LedgerId(21);
const IGST: LedgerId = LedgerId(22);
const ROUND_OFF: LedgerId = LedgerId(23);
const BANK: LedgerId = LedgerId(30);
const RENT: LedgerId = LedgerId(31);

const SALES_TYPE: VoucherTypeId = VoucherTypeId(1);
const PAYMENT_TYPE: VoucherTypeId = VoucherTypeId(2);
const PURCHASE_TYPE: VoucherTypeId = VoucherTypeId(4);

const KARNATAKA: u8 = 29;
const MAHARASHTRA: u8 = 27;

const VOUCHER_DATE: Date = date!(2026 - 01 - 15);

fn state(code: u8) -> StateCode {
    StateCode::new(code).unwrap()
}

fn rules_on(d: Date) -> RuleSet {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("ledgerly-gst")
        .join("packs")
        .join("rates-slabs.toml");
    let pack = load_pack(&fs::read_to_string(path).unwrap()).unwrap();
    match pack.for_time_of_supply(d) {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("no rules on {d}: {u:?}"),
    }
}

fn voucher_type(id: VoucherTypeId, name: &str, base: BaseVoucherKind) -> VoucherType {
    VoucherType {
        id,
        name: name.to_owned(),
        base,
        abbreviation: name.chars().take(3).collect(),
        numbering: NumberingConfig {
            method: NumberingMethod::Automatic,
            prevent_renumbering: false,
            prefix: String::new(),
            suffix: String::new(),
            width: 0,
            restart: RestartPeriod::Yearly,
            series: vec![],
        },
        active: true,
        tally: None,
    }
}

fn ledger(
    id: LedgerId,
    name: &str,
    party: Option<PartyDetails>,
    tax: Option<TaxDetails>,
    gst: Option<GstClassification>,
) -> Ledger {
    Ledger {
        id,
        name: name.to_owned(),
        aliases: vec![],
        group: GroupId(1),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party,
        bank: None,
        tax,
        gst,
        reserved: None,
        tally: None,
    }
}

fn tax_ledger(id: LedgerId, name: &str, kind: TaxLedgerKind) -> Ledger {
    ledger(
        id,
        name,
        None,
        Some(TaxDetails {
            kind,
            rate_percent: None,
        }),
        None,
    )
}

fn eighteen_percent() -> GstClassification {
    GstClassification {
        hsn: None,
        taxability: Taxability::Taxable,
        rate_percent: Some(Rate(Decimal::new(18, 0))),
    }
}

fn party_details() -> PartyDetails {
    PartyDetails {
        registration: RegistrationType::Regular,
        gstin: Some(Gstin::parse("07AAACL0000A1ZP").unwrap()),
        pan: None,
        state: None,
        address: Address::default(),
    }
}

fn books() -> Books {
    let company = Company {
        id: CompanyId(1),
        name: "Test Co".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: state(KARNATAKA),
        pan: None,
        registrations: vec![GstRegistration {
            gstin: Gstin::parse("29ABCDE1234F1ZW").unwrap(),
            state: state(KARNATAKA),
            effective_from: date!(2017 - 07 - 01),
            composition: false,
        }],
        fy_start: date!(2025 - 04 - 01),
        books_begin: date!(2025 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features {
            gst: true,
            ..Features::default()
        },
        negative_stock: NegativeStockPolicy::Warn,
    };
    let mut m = Masters::default();
    for t in [
        voucher_type(SALES_TYPE, "Sales", BaseVoucherKind::Sales),
        voucher_type(PAYMENT_TYPE, "Payment", BaseVoucherKind::Payment),
        voucher_type(PURCHASE_TYPE, "Purchase", BaseVoucherKind::Purchase),
    ] {
        m.voucher_types.insert(t.id, t);
    }
    for l in [
        ledger(SALES_LEDGER, "Sales", None, None, Some(eighteen_percent())),
        ledger(
            PURCHASE_LEDGER,
            "Purchases",
            None,
            None,
            Some(eighteen_percent()),
        ),
        ledger(PARTY, "Acme Traders", Some(party_details()), None, None),
        tax_ledger(CGST, "CGST", TaxLedgerKind::Cgst),
        tax_ledger(SGST, "SGST", TaxLedgerKind::Sgst),
        tax_ledger(IGST, "IGST", TaxLedgerKind::Igst),
        ledger(ROUND_OFF, "Round off", None, None, None),
        ledger(BANK, "Bank", None, None, None),
        ledger(RENT, "Rent", None, None, None),
    ] {
        m.ledgers.insert(l.id, l);
    }
    Books {
        company,
        masters: m,
        vouchers: BTreeMap::new(),
        postings: vec![],
        stock_moves: vec![],
    }
}

fn t<T>(value: T, confidence_permille: u16) -> Transcribed<T> {
    Transcribed {
        value,
        source: SourceRef::Typed { start: 0, end: 0 },
        confidence_permille,
    }
}

fn line(name: &str, ledger: Option<LedgerId>, side: DrCr, paise: i64) -> DraftLine {
    DraftLine {
        ledger_name: t(name.to_owned(), HIGH),
        ledger,
        side,
        amount: t(Paise::new(paise), HIGH),
    }
}

fn draft(kind: BaseVoucherKind, lines: Vec<DraftLine>) -> VoucherDraft {
    VoucherDraft {
        kind,
        date: Some(t(VOUCHER_DATE, HIGH)),
        party_name: None,
        party: None,
        invoice_number: None,
        supplier_gstin: None,
        lines,
        items: vec![],
        document_total: None,
        document_tax: None,
        create_ledgers: vec![],
        narration: String::new(),
    }
}

/// Sales of 10,000.00 at 18%, intra-state: party Dr 11,800.00; sales Cr 10,000.00; CGST and SGST
/// Cr 900.00 each. Totals as printed on the (fictional) document.
fn sales_draft() -> VoucherDraft {
    VoucherDraft {
        party_name: Some(t("Acme Traders".to_owned(), HIGH)),
        party: Some(PARTY),
        invoice_number: Some(t("INV-7".to_owned(), HIGH)),
        document_total: Some(t(Paise::new(1_180_000), HIGH)),
        document_tax: Some(t(Paise::new(180_000), HIGH)),
        ..draft(
            BaseVoucherKind::Sales,
            vec![
                line("Acme Traders", Some(PARTY), DrCr::Dr, 1_180_000),
                line("Sales", Some(SALES_LEDGER), DrCr::Cr, 1_000_000),
                line("CGST", Some(CGST), DrCr::Cr, 90_000),
                line("SGST", Some(SGST), DrCr::Cr, 90_000),
            ],
        )
    }
}

fn payment_draft() -> VoucherDraft {
    VoucherDraft {
        document_total: Some(t(Paise::new(2_500_000), HIGH)),
        narration: "March rent".to_owned(),
        ..draft(
            BaseVoucherKind::Payment,
            vec![
                line("Rent", Some(RENT), DrCr::Dr, 2_500_000),
                line("Bank", Some(BANK), DrCr::Cr, 2_500_000),
            ],
        )
    }
}

fn check_in(d: &VoucherDraft, b: &Books, place: Option<u8>) -> DraftCheck {
    validate_draft_with_place(d, b, &rules_on(d_date(d)), THRESHOLD, place.map(state)).unwrap()
}

fn d_date(d: &VoucherDraft) -> Date {
    d.date.as_ref().map_or(VOUCHER_DATE, |x| x.value)
}

fn ok(check: DraftCheck) -> Voucher {
    match check {
        DraftCheck::Ok(v) => v,
        DraftCheck::Blocked { field, reason } => panic!("blocked on {field}: {reason}"),
    }
}

fn blocked_field(check: DraftCheck) -> String {
    match check {
        DraftCheck::Blocked { field, .. } => field,
        DraftCheck::Ok(v) => panic!("expected Blocked, got a voucher: {v:?}"),
    }
}

// ---- a clean draft becomes a voucher ------------------------------------------------------

#[test]
fn clean_intra_state_sales_draft_becomes_a_voucher() {
    // provenance: derived — CGST Act s.9; slab.eighteen = 9% + 9%.
    // ₹10,000.00 x 9% = ₹900.00 CGST and the same SGST; invoice value 10,000 + 900 + 900 =
    // ₹11,800.00 (already whole rupees, round-off 0). The draft's lines and totals agree.
    let b = books();
    let v = ok(check_in(&sales_draft(), &b, Some(KARNATAKA)));
    assert_eq!(v.id, VoucherId(1));
    assert_eq!(v.type_id, SALES_TYPE);
    assert_eq!(v.mode, VoucherMode::AccountingInvoice);
    assert_eq!(v.status, VoucherStatus::Regular);
    assert_eq!(v.date, VOUCHER_DATE);
    assert_eq!(v.party, Some(PARTY));
    assert_eq!(v.place_of_supply, Some(state(KARNATAKA)));
    assert_eq!(v.reference.as_deref(), Some("INV-7"));
    assert_eq!(v.number, None);
    assert_eq!(v.ledger_lines.len(), 4);
    assert_eq!(v.ledger_lines.first().unwrap().ledger, PARTY);
    assert_eq!(
        v.ledger_lines.first().unwrap().amount,
        SignedAmount::dr(Paise::new(1_180_000))
    );
}

#[test]
fn clean_inter_state_sales_draft_with_igst_becomes_a_voucher() {
    // provenance: derived — IGST Act s.5 (inter-state: IGST); slab.eighteen igst_percent = 18.
    // IGST = ₹10,000.00 x 18% = ₹1,800.00 = 180,000 paise; invoice value ₹11,800.00.
    let b = books();
    let d = VoucherDraft {
        lines: vec![
            line("Acme Traders", Some(PARTY), DrCr::Dr, 1_180_000),
            line("Sales", Some(SALES_LEDGER), DrCr::Cr, 1_000_000),
            line("IGST", Some(IGST), DrCr::Cr, 180_000),
        ],
        ..sales_draft()
    };
    let v = ok(check_in(&d, &b, Some(MAHARASHTRA)));
    assert_eq!(v.ledger_lines.len(), 3);
}

#[test]
fn clean_purchase_draft_has_tax_on_the_side_opposite_the_party() {
    // provenance: derived — same arithmetic as the sales case: taxable ₹10,000.00, CGST and SGST
    // ₹900.00 each, supplier payable ₹11,800.00. A purchase debits purchases and tax and credits
    // the supplier. (Card G4 takes the company's registration state as the supplying state, so
    // this intra-state case is the one it computes for a purchase.)
    let b = books();
    let d = purchase_draft();
    let v = ok(check_in(&d, &b, Some(KARNATAKA)));
    assert_eq!(v.type_id, PURCHASE_TYPE);
}

#[test]
fn round_off_line_is_accepted_when_it_matches_the_engine() {
    // provenance: derived — taxable ₹1,000.50 = 100,050 paise. CGST 9% = 9,004.5 paise, half away
    // from zero = 9,005; SGST the same. Before round-off: 100,050 + 9,005 + 9,005 = 118,060 paise
    // = ₹1,180.60, which rounds to ₹1,181.00 = 118,100 paise (policy: card G4, CGST Act s.170).
    // Round-off = 118,100 - 118,060 = 40 paise. Credits: 100,050 + 9,005 + 9,005 + 40 = 118,100.
    let b = books();
    let d = VoucherDraft {
        document_total: Some(t(Paise::new(118_100), HIGH)),
        document_tax: Some(t(Paise::new(18_010), HIGH)),
        lines: vec![
            line("Acme Traders", Some(PARTY), DrCr::Dr, 118_100),
            line("Sales", Some(SALES_LEDGER), DrCr::Cr, 100_050),
            line("CGST", Some(CGST), DrCr::Cr, 9_005),
            line("SGST", Some(SGST), DrCr::Cr, 9_005),
            line("Round off", Some(ROUND_OFF), DrCr::Cr, 40),
        ],
        ..sales_draft()
    };
    let v = ok(check_in(&d, &b, Some(KARNATAKA)));
    assert_eq!(v.ledger_lines.len(), 5);
}

#[test]
fn missing_round_off_is_blocked_on_the_debit_total() {
    // provenance: derived — as above the engine's invoice value is 118,100 paise. This draft
    // leaves the round-off out, so its party line is 118,060 and the lines still balance, but the
    // debit total is 40 paise short of the engine's invoice value.
    let b = books();
    let d = VoucherDraft {
        document_total: None,
        document_tax: None,
        lines: vec![
            line("Acme Traders", Some(PARTY), DrCr::Dr, 118_060),
            line("Sales", Some(SALES_LEDGER), DrCr::Cr, 100_050),
            line("CGST", Some(CGST), DrCr::Cr, 9_005),
            line("SGST", Some(SGST), DrCr::Cr, 9_005),
        ],
        ..sales_draft()
    };
    assert_eq!(blocked_field(check_in(&d, &b, Some(KARNATAKA))), "lines");
}

// ---- the engine's figures against the draft's -----------------------------------------------

#[test]
fn document_total_different_from_the_engine_total_is_blocked() {
    // provenance: derived — the engine total is 10,000.00 + 900.00 + 900.00 = ₹11,800.00
    // = 1,180,000 paise. The document says ₹11,810.00 (a misread digit), so the draft is blocked
    // on `document_total`.
    let b = books();
    let d = VoucherDraft {
        document_total: Some(t(Paise::new(1_181_000), HIGH)),
        ..sales_draft()
    };
    assert_eq!(
        blocked_field(check_in(&d, &b, Some(KARNATAKA))),
        "document_total"
    );
}

#[test]
fn document_tax_different_from_the_engine_tax_is_blocked() {
    // provenance: derived — engine tax = 900.00 + 900.00 = ₹1,800.00 = 180,000 paise; the document
    // says ₹1,700.00.
    let b = books();
    let d = VoucherDraft {
        document_tax: Some(t(Paise::new(170_000), HIGH)),
        ..sales_draft()
    };
    assert_eq!(
        blocked_field(check_in(&d, &b, Some(KARNATAKA))),
        "document_tax"
    );
}

#[test]
fn draft_without_tax_lines_is_blocked_on_the_first_missing_head() {
    // provenance: derived — the engine computes CGST ₹900.00 on a ₹10,000.00 intra-state sale; the
    // draft books the sale with no tax lines (it balances: Dr 10,000.00 = Cr 10,000.00).
    let b = books();
    let d = VoucherDraft {
        document_total: None,
        document_tax: None,
        lines: vec![
            line("Acme Traders", Some(PARTY), DrCr::Dr, 1_000_000),
            line("Sales", Some(SALES_LEDGER), DrCr::Cr, 1_000_000),
        ],
        ..sales_draft()
    };
    assert_eq!(
        blocked_field(check_in(&d, &b, Some(KARNATAKA))),
        "lines.cgst"
    );
}

#[test]
fn wrong_cgst_amount_is_blocked_on_that_head() {
    // provenance: derived — correct CGST is ₹900.00; the draft has ₹800.00. The lines balance:
    // Dr 11,700.00 = Cr 10,000.00 + 800.00 + 900.00.
    let b = books();
    let d = VoucherDraft {
        document_total: None,
        document_tax: None,
        lines: vec![
            line("Acme Traders", Some(PARTY), DrCr::Dr, 1_170_000),
            line("Sales", Some(SALES_LEDGER), DrCr::Cr, 1_000_000),
            line("CGST", Some(CGST), DrCr::Cr, 80_000),
            line("SGST", Some(SGST), DrCr::Cr, 90_000),
        ],
        ..sales_draft()
    };
    assert_eq!(
        blocked_field(check_in(&d, &b, Some(KARNATAKA))),
        "lines.cgst"
    );
}

#[test]
fn igst_drafted_for_an_intra_state_supply_is_blocked() {
    // provenance: derived — a Karnataka supplier supplying within Karnataka charges CGST + SGST
    // (CGST Act s.9), so IGST must be nil; the draft carries ₹1,800.00 of IGST.
    let b = books();
    let d = VoucherDraft {
        lines: vec![
            line("Acme Traders", Some(PARTY), DrCr::Dr, 1_180_000),
            line("Sales", Some(SALES_LEDGER), DrCr::Cr, 1_000_000),
            line("IGST", Some(IGST), DrCr::Cr, 180_000),
        ],
        ..sales_draft()
    };
    assert_eq!(
        blocked_field(check_in(&d, &b, Some(KARNATAKA))),
        "lines.cgst"
    );
}

#[test]
fn rules_for_another_date_block_with_the_gst_field() {
    // provenance: derived — card G4 refuses a rule set that is not the one in force on the
    // voucher's time of supply; the draft is dated 2026-01-15 and the rules are for 2026-02-01.
    let b = books();
    let r = validate_draft_with_place(
        &sales_draft(),
        &b,
        &rules_on(date!(2026 - 02 - 01)),
        THRESHOLD,
        Some(state(KARNATAKA)),
    )
    .unwrap();
    assert_eq!(blocked_field(r), "gst");
}

#[test]
fn gst_draft_without_a_place_of_supply_is_unsupported_not_guessed() {
    // The frozen `validate_draft` takes no place of supply and none is guessed from the party or the
    // company: GST cannot be recomputed, so the answer is Unsupported (rule A1).
    let b = books();
    let r = validate_draft(&sales_draft(), &b, &rules_on(VOUCHER_DATE), THRESHOLD);
    assert!(
        matches!(r, Err(AssistError::Unsupported(_))),
        "expected Unsupported, got {r:?}"
    );
}

// ---- vouchers the engine does not tax --------------------------------------------------------

#[test]
fn payment_draft_with_matching_total_becomes_a_voucher() {
    // provenance: derived — Σ Dr = ₹25,000.00 = 2,500,000 paise = Σ Cr, and the document says the
    // same. A payment carries no GST in this engine.
    let b = books();
    let v = ok(check_in(&payment_draft(), &b, None));
    assert_eq!(v.type_id, PAYMENT_TYPE);
    assert_eq!(v.mode, VoucherMode::AsVoucher);
    assert_eq!(v.narration, "March rent");
    assert_eq!(v.place_of_supply, None);
}

#[test]
fn payment_document_total_different_from_the_debit_total_is_blocked() {
    // provenance: derived — Σ Dr = ₹25,000.00; the document says ₹25,001.00.
    let b = books();
    let d = VoucherDraft {
        document_total: Some(t(Paise::new(2_500_100), HIGH)),
        ..payment_draft()
    };
    assert_eq!(blocked_field(check_in(&d, &b, None)), "document_total");
}

#[test]
fn payment_with_a_document_tax_is_blocked_because_no_tax_is_computed() {
    // provenance: derived — a payment has no GST computed (tax nil), so a document tax of ₹18.00
    // cannot be confirmed.
    let b = books();
    let d = VoucherDraft {
        document_tax: Some(t(Paise::new(1_800), HIGH)),
        ..payment_draft()
    };
    assert_eq!(blocked_field(check_in(&d, &b, None)), "document_tax");
}

#[test]
fn payment_with_a_nil_document_tax_is_accepted() {
    // provenance: derived — tax nil equals a document tax of 0.00.
    let b = books();
    let d = VoucherDraft {
        document_tax: Some(t(Paise::ZERO, HIGH)),
        ..payment_draft()
    };
    ok(check_in(&d, &b, None));
}

#[test]
fn unbalanced_payment_is_blocked_on_lines() {
    // provenance: derived — spec invariant 1: Σ Dr = Σ Cr. Dr 25,000.00 against Cr 24,000.00.
    let b = books();
    let d = VoucherDraft {
        document_total: None,
        lines: vec![
            line("Rent", Some(RENT), DrCr::Dr, 2_500_000),
            line("Bank", Some(BANK), DrCr::Cr, 2_400_000),
        ],
        ..payment_draft()
    };
    assert_eq!(blocked_field(check_in(&d, &b, None)), "lines");
}

#[test]
fn negative_line_amount_is_blocked_on_that_line() {
    // provenance: derived — model invariant: no line amount is negative. Line 0 is -₹1.00.
    let b = books();
    let d = VoucherDraft {
        document_total: None,
        lines: vec![
            line("Rent", Some(RENT), DrCr::Dr, -100),
            line("Bank", Some(BANK), DrCr::Cr, 100),
        ],
        ..payment_draft()
    };
    assert_eq!(blocked_field(check_in(&d, &b, None)), "lines[0].amount");
}

// ---- confidence ------------------------------------------------------------------------------

#[test]
fn required_date_below_threshold_is_blank_and_blocks() {
    // threshold 700: 699 is blank, 700 is kept.
    let b = books();
    let low = VoucherDraft {
        date: Some(t(VOUCHER_DATE, THRESHOLD - 1)),
        ..payment_draft()
    };
    let r = validate_draft(&low, &b, &rules_on(VOUCHER_DATE), THRESHOLD).unwrap();
    assert_eq!(blocked_field(r), "date");
    let at = VoucherDraft {
        date: Some(t(VOUCHER_DATE, THRESHOLD)),
        ..payment_draft()
    };
    ok(check_in(&at, &b, None));
}

#[test]
fn missing_date_blocks() {
    let b = books();
    let d = VoucherDraft {
        date: None,
        ..payment_draft()
    };
    let r = validate_draft(&d, &b, &rules_on(VOUCHER_DATE), THRESHOLD).unwrap();
    assert_eq!(blocked_field(r), "date");
}

#[test]
fn invoice_number_below_threshold_is_left_blank() {
    let b = books();
    let low = VoucherDraft {
        invoice_number: Some(t("INV-7".to_owned(), THRESHOLD - 1)),
        ..sales_draft()
    };
    assert_eq!(ok(check_in(&low, &b, Some(KARNATAKA))).reference, None);
    let at = VoucherDraft {
        invoice_number: Some(t("INV-7".to_owned(), THRESHOLD)),
        ..sales_draft()
    };
    assert_eq!(
        ok(check_in(&at, &b, Some(KARNATAKA))).reference.as_deref(),
        Some("INV-7")
    );
}

#[test]
fn blank_document_total_is_not_compared() {
    // A wrong total read with confidence below the threshold is blank: it is not compared and does
    // not block (the lines themselves are right).
    let b = books();
    let d = VoucherDraft {
        document_total: Some(t(Paise::new(9_999_999), 100)),
        document_tax: Some(t(Paise::new(1), 100)),
        ..sales_draft()
    };
    ok(check_in(&d, &b, Some(KARNATAKA)));
}

#[test]
fn line_ledger_name_below_threshold_blocks_that_line() {
    let b = books();
    let mut d = sales_draft();
    d.lines.get_mut(1).unwrap().ledger_name = t("Sales".to_owned(), 100);
    assert_eq!(
        blocked_field(check_in(&d, &b, Some(KARNATAKA))),
        "lines[1].ledger"
    );
}

#[test]
fn line_amount_below_threshold_blocks_that_line() {
    let b = books();
    let mut d = sales_draft();
    d.lines.get_mut(2).unwrap().amount = t(Paise::new(90_000), 100);
    assert_eq!(
        blocked_field(check_in(&d, &b, Some(KARNATAKA))),
        "lines[2].amount"
    );
}

#[test]
fn party_name_below_threshold_drops_the_party_and_blocks_a_sale() {
    let b = books();
    let d = VoucherDraft {
        party_name: Some(t("Acme Traders".to_owned(), 100)),
        ..sales_draft()
    };
    assert_eq!(blocked_field(check_in(&d, &b, Some(KARNATAKA))), "party");
}

#[test]
fn confidence_above_1000_is_malformed_and_blocks() {
    let b = books();
    let d = VoucherDraft {
        date: Some(t(VOUCHER_DATE, 1001)),
        ..payment_draft()
    };
    assert_eq!(blocked_field(check_in(&d, &b, None)), "date");
}

// ---- masters and kinds ------------------------------------------------------------------------

#[test]
fn line_without_a_resolved_ledger_blocks_and_no_id_is_invented() {
    let b = books();
    let mut d = payment_draft();
    d.lines.get_mut(0).unwrap().ledger = None;
    assert_eq!(blocked_field(check_in(&d, &b, None)), "lines[0].ledger");
}

#[test]
fn line_naming_a_ledger_that_does_not_exist_blocks() {
    let b = books();
    let mut d = payment_draft();
    d.lines.get_mut(1).unwrap().ledger = Some(LedgerId(999));
    assert_eq!(blocked_field(check_in(&d, &b, None)), "lines[1].ledger");
}

#[test]
fn ledgers_the_user_would_need_to_create_block_the_draft() {
    let b = books();
    let d = VoucherDraft {
        create_ledgers: vec!["Sharma Properties".to_owned()],
        ..payment_draft()
    };
    assert_eq!(blocked_field(check_in(&d, &b, None)), "create_ledgers");
}

#[test]
fn party_that_does_not_exist_blocks() {
    let b = books();
    let d = VoucherDraft {
        party: Some(LedgerId(999)),
        ..payment_draft()
    };
    assert_eq!(blocked_field(check_in(&d, &b, None)), "party");
}

#[test]
fn missing_voucher_type_blocks_on_kind() {
    let mut b = books();
    b.masters.voucher_types.remove(&PAYMENT_TYPE);
    assert_eq!(blocked_field(check_in(&payment_draft(), &b, None)), "kind");
}

#[test]
fn several_active_voucher_types_of_the_kind_block_on_kind() {
    let mut b = books();
    let second = voucher_type(VoucherTypeId(9), "Payment 2", BaseVoucherKind::Payment);
    b.masters.voucher_types.insert(second.id, second);
    assert_eq!(blocked_field(check_in(&payment_draft(), &b, None)), "kind");
}

#[test]
fn inactive_voucher_type_is_not_chosen() {
    let mut b = books();
    let mut second = voucher_type(VoucherTypeId(9), "Old payment", BaseVoucherKind::Payment);
    second.active = false;
    b.masters.voucher_types.insert(second.id, second);
    let v = ok(check_in(&payment_draft(), &b, None));
    assert_eq!(v.type_id, PAYMENT_TYPE);
}

#[test]
fn new_voucher_id_follows_the_highest_existing_id() {
    let mut b = books();
    let first = ok(check_in(&payment_draft(), &b, None));
    let mut existing = first.clone();
    existing.id = VoucherId(41);
    b.vouchers.insert(existing.id, existing);
    let next = ok(check_in(&payment_draft(), &b, None));
    assert_eq!(next.id, VoucherId(42));
}

#[test]
fn draft_with_item_lines_is_unsupported() {
    let b = books();
    let d = VoucherDraft {
        items: vec![DraftItem {
            name: t("Widget".to_owned(), HIGH),
            qty: None,
            rate: None,
            amount: Some(t(Paise::new(1_000), HIGH)),
        }],
        ..payment_draft()
    };
    let r = validate_draft(&d, &b, &rules_on(VOUCHER_DATE), THRESHOLD);
    assert!(matches!(r, Err(AssistError::Unsupported(_))), "got {r:?}");
}

#[test]
fn kinds_the_assistant_does_not_draft_are_unsupported() {
    let b = books();
    let d = VoucherDraft {
        kind: BaseVoucherKind::StockJournal,
        ..payment_draft()
    };
    let r = validate_draft(&d, &b, &rules_on(VOUCHER_DATE), THRESHOLD);
    assert!(matches!(r, Err(AssistError::Unsupported(_))), "got {r:?}");
}

// ---- supplier GSTIN ----------------------------------------------------------------------------

fn purchase_draft() -> VoucherDraft {
    VoucherDraft {
        kind: BaseVoucherKind::Purchase,
        lines: vec![
            line("Acme Traders", Some(PARTY), DrCr::Cr, 1_180_000),
            line("Purchases", Some(PURCHASE_LEDGER), DrCr::Dr, 1_000_000),
            line("CGST", Some(CGST), DrCr::Dr, 90_000),
            line("SGST", Some(SGST), DrCr::Dr, 90_000),
        ],
        ..sales_draft()
    }
}

#[test]
fn purchase_supplier_gstin_must_match_the_party_ledger() {
    // The party ledger in the fixture holds 07AAACL0000A1ZP; on a purchase the party is the
    // supplier. Both strings are hand-derived valid test vectors in ledgerly-core.
    let b = books();
    let same = VoucherDraft {
        supplier_gstin: Some(t("07AAACL0000A1ZP".to_owned(), HIGH)),
        ..purchase_draft()
    };
    ok(check_in(&same, &b, Some(KARNATAKA)));
    let other = VoucherDraft {
        supplier_gstin: Some(t("29ABCDE1234F1ZW".to_owned(), HIGH)),
        ..purchase_draft()
    };
    assert_eq!(
        blocked_field(check_in(&other, &b, Some(KARNATAKA))),
        "supplier_gstin"
    );
}

#[test]
fn sales_supplier_gstin_must_match_the_company_registration() {
    // On a sale the supplier is the company, registered as 29ABCDE1234F1ZW in the fixture.
    let b = books();
    let same = VoucherDraft {
        supplier_gstin: Some(t("29ABCDE1234F1ZW".to_owned(), HIGH)),
        ..sales_draft()
    };
    ok(check_in(&same, &b, Some(KARNATAKA)));
    let other = VoucherDraft {
        supplier_gstin: Some(t("07AAACL0000A1ZP".to_owned(), HIGH)),
        ..sales_draft()
    };
    assert_eq!(
        blocked_field(check_in(&other, &b, Some(KARNATAKA))),
        "supplier_gstin"
    );
}

#[test]
fn malformed_supplier_gstin_blocks_when_confident_and_is_ignored_when_blank() {
    // The last character of 07AAACL0000A1ZP is its check character; Z in its place is wrong.
    let b = books();
    let bad = VoucherDraft {
        supplier_gstin: Some(t("07AAACL0000A1ZZ".to_owned(), HIGH)),
        ..sales_draft()
    };
    assert_eq!(
        blocked_field(check_in(&bad, &b, Some(KARNATAKA))),
        "supplier_gstin"
    );
    let blank = VoucherDraft {
        supplier_gstin: Some(t("07AAACL0000A1ZZ".to_owned(), 100)),
        ..sales_draft()
    };
    ok(check_in(&blank, &b, Some(KARNATAKA)));
}

#[test]
fn bill_wise_party_line_is_blocked_because_drafts_carry_no_bill_references() {
    let mut b = books();
    b.masters.ledgers.get_mut(&PARTY).unwrap().bill_wise = Some(ledgerly_model::BillWiseConfig {
        default_credit_days: Some(30),
    });
    let r = check_in(&sales_draft(), &b, Some(KARNATAKA));
    assert_eq!(blocked_field(r), "lines[0].bills");
}
