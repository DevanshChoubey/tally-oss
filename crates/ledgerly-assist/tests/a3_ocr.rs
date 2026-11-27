//! Card A3 tests: reading recorded OCR output of a fictional bill into a `VoucherDraft` (spec 6.8,
//! rules A1 and A10).
//!
//! Provenance. The OCR output is a recorded-shape fixture of a fictional bill; the bill's own
//! figures are hand arithmetic written next to them, not engine output:
//!
//! - Taxable value Rs 15,000.00 = `1_500_000` paise.
//! - CGST at 9% of 15,000.00 = 1,350.00 (15,000 x 9 / 100) = `135_000` paise; SGST the same.
//! - Total = 15,000.00 + 1,350.00 + 1,350.00 = 17,700.00 = `1_770_000` paise; total tax 2,700.00 =
//!   `270_000` paise.
//!
//! Amount and date strings are expanded by hand where the case is about number format
//! (rupees x 100 + paise). The supplier GSTIN is `ledgerly-core`'s hand-derived test vector
//! `29ABCDE1234F1ZW`, whose check character is derived there with the mod-36 algorithm:
//! the 14-character prefix sums to 184, 184 mod 36 = 4 and (36 - 4) mod 36 = 32 = 'W'. Hence
//! `...ZX` fails its check and the checker names 'W' as the expected character. The buyer GSTIN
//! `07AAACL0000A1ZP` is another vector of the same file. Names and numbers are fictional.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::type_complexity,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_assist::ocr::{OcrLine, OcrOutput, ocr_to_draft, read_bill};
use ledgerly_assist::{AssistError, SourceRef, Transcribed, VoucherDraft};
use ledgerly_core::{Decimal, DrCr, GroupId, Gstin, LedgerId, Paise, Rate, SignedAmount};
use ledgerly_model::{
    Address, BaseVoucherKind, Ledger, Masters, PartyDetails, RegistrationType, TaxDetails,
    TaxLedgerKind,
};
use time::macros::date;

const HASH: &str = "ab00ab00ab00ab00ab00ab00ab00ab00ab00ab00ab00ab00ab00ab00ab00ab00";
const SUPPLIER_GSTIN: &str = "29ABCDE1234F1ZW";
const BUYER_GSTIN: &str = "07AAACL0000A1ZP";

const SUPPLIER: LedgerId = LedgerId(40);
const CGST: LedgerId = LedgerId(20);
const SGST: LedgerId = LedgerId(21);

fn line(text: &str, bbox: [u32; 4], confidence_permille: u16) -> OcrLine {
    OcrLine {
        page: 1,
        text: text.to_owned(),
        bbox,
        confidence_permille,
    }
}

/// The recorded output for the fictional bill, one line each, with a bbox and confidence.
fn bill_lines() -> Vec<OcrLine> {
    vec![
        line("TAX INVOICE", [200, 20, 300, 40], 990),
        line("Supplier: Kaveri Stationers", [40, 80, 420, 28], 940),
        line("GSTIN: 29ABCDE1234F1ZW", [40, 115, 380, 28], 930),
        line("Invoice No: KS/2026/0042", [40, 150, 360, 28], 960),
        line("Date: 15/03/2026", [40, 185, 250, 28], 970),
        line("Buyer GSTIN: 07AAACL0000A1ZP", [40, 220, 400, 28], 920),
        line("Item Qty Rate Amount", [40, 270, 500, 28], 900),
        line("Notebook A5 100 150.00 15,000.00", [40, 300, 500, 28], 910),
        line("Taxable Value 15,000.00", [40, 350, 400, 28], 950),
        line("CGST @ 9% 1,350.00", [40, 385, 400, 28], 945),
        line("SGST @ 9% 1,350.00", [40, 420, 400, 28], 944),
        line("Total Tax 2,700.00", [40, 455, 400, 28], 935),
        line("Total 17,700.00", [40, 490, 400, 28], 965),
    ]
}

fn bill() -> OcrOutput {
    OcrOutput {
        doc_hash: HASH.to_owned(),
        lines: bill_lines(),
    }
}

/// The bill with the lines whose text starts with `prefix` removed.
fn without(prefix: &str) -> OcrOutput {
    let mut out = bill();
    out.lines.retain(|l| !l.text.starts_with(prefix));
    out
}

/// The bill with one extra line.
fn with_extra(text: &str) -> OcrOutput {
    let mut out = bill();
    out.lines.push(line(text, [40, 530, 400, 28], 900));
    out
}

/// The bill with the lines starting with `prefix` replaced by `text`.
fn replacing(prefix: &str, text: &str) -> OcrOutput {
    let mut out = without(prefix);
    out.lines.push(line(text, [40, 560, 400, 28], 900));
    out
}

fn ledger(
    id: LedgerId,
    name: &str,
    party: Option<PartyDetails>,
    tax: Option<TaxDetails>,
) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
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

fn party(gstin: Option<&str>) -> PartyDetails {
    PartyDetails {
        registration: RegistrationType::Regular,
        gstin: gstin.map(|g| Gstin::parse(g).unwrap()),
        pan: None,
        state: None,
        address: Address::default(),
    }
}

fn tax(kind: TaxLedgerKind, percent: Option<i64>) -> TaxDetails {
    TaxDetails {
        kind,
        rate_percent: percent.map(|p| Rate(Decimal::new(p, 0))),
    }
}

fn masters(ledgers: Vec<Ledger>) -> Masters {
    let mut m = Masters::default();
    for l in ledgers {
        m.ledgers.insert(l.id, l);
    }
    m
}

/// Masters holding the supplier (with its GSTIN on file) and one CGST and one SGST ledger.
fn full_masters() -> Masters {
    masters(vec![
        ledger(
            SUPPLIER,
            "Kaveri Stationers",
            Some(party(Some(SUPPLIER_GSTIN))),
            None,
        ),
        ledger(
            CGST,
            "Input CGST",
            None,
            Some(tax(TaxLedgerKind::Cgst, None)),
        ),
        ledger(
            SGST,
            "Input SGST",
            None,
            Some(tax(TaxLedgerKind::Sgst, None)),
        ),
    ])
}

fn doc_source(bbox: [u32; 4]) -> SourceRef {
    SourceRef::Document {
        doc_hash: HASH.to_owned(),
        page: 1,
        bbox,
    }
}

fn read(ocr: &OcrOutput, m: &Masters) -> VoucherDraft {
    ocr_to_draft(ocr, m).unwrap()
}

#[test]
fn recorded_bill_becomes_a_draft_with_bbox_provenance() {
    let draft = read(&bill(), &full_masters());
    assert_eq!(draft.kind, BaseVoucherKind::Purchase);
    // Each value carries the bbox and confidence of the OCR line it came from.
    assert_eq!(
        draft.invoice_number,
        Some(Transcribed {
            value: "KS/2026/0042".to_owned(),
            source: doc_source([40, 150, 360, 28]),
            confidence_permille: 960,
        })
    );
    assert_eq!(
        draft.date,
        Some(Transcribed {
            value: date!(2026 - 03 - 15),
            source: doc_source([40, 185, 250, 28]),
            confidence_permille: 970,
        })
    );
    assert_eq!(
        draft.supplier_gstin,
        Some(Transcribed {
            value: SUPPLIER_GSTIN.to_owned(),
            source: doc_source([40, 115, 380, 28]),
            confidence_permille: 930,
        })
    );
    assert_eq!(
        draft.party_name,
        Some(Transcribed {
            value: "Kaveri Stationers".to_owned(),
            source: doc_source([40, 80, 420, 28]),
            confidence_permille: 940,
        })
    );
    assert_eq!(draft.party, Some(SUPPLIER));
    // Document figures are the ones printed: total 17,700.00 and tax 2,700.00 (see file header).
    assert_eq!(
        draft.document_total,
        Some(Transcribed {
            value: Paise::new(1_770_000),
            source: doc_source([40, 490, 400, 28]),
            confidence_permille: 965,
        })
    );
    assert_eq!(
        draft.document_tax,
        Some(Transcribed {
            value: Paise::new(270_000),
            source: doc_source([40, 455, 400, 28]),
            confidence_permille: 935,
        })
    );
    assert_eq!(draft.narration, "Purchase bill KS/2026/0042");
    assert_eq!(draft.items, [] as [ledgerly_assist::DraftItem; 0]);
    assert_eq!(draft.create_ledgers, [] as [std::string::String; 0]);
}

#[test]
fn recorded_bill_lines_are_taxable_taxes_and_party_in_that_order() {
    let draft = read(&bill(), &full_masters());
    let seen: Vec<(String, Option<LedgerId>, DrCr, i64, [u32; 4])> = draft
        .lines
        .iter()
        .map(|l| {
            let SourceRef::Document { bbox, .. } = l.amount.source else {
                panic!("a line amount must come from the document");
            };
            (
                l.ledger_name.value.clone(),
                l.ledger,
                l.side,
                l.amount.value.get(),
                bbox,
            )
        })
        .collect();
    // 15,000.00 + 1,350.00 + 1,350.00 debit = 17,700.00 credit to the supplier (file header).
    assert_eq!(
        seen,
        vec![
            (
                "Taxable Value".to_owned(),
                None,
                DrCr::Dr,
                1_500_000,
                [40, 350, 400, 28]
            ),
            (
                "CGST".to_owned(),
                Some(CGST),
                DrCr::Dr,
                135_000,
                [40, 385, 400, 28]
            ),
            (
                "SGST".to_owned(),
                Some(SGST),
                DrCr::Dr,
                135_000,
                [40, 420, 400, 28]
            ),
            (
                "Kaveri Stationers".to_owned(),
                Some(SUPPLIER),
                DrCr::Cr,
                1_770_000,
                [40, 490, 400, 28]
            ),
        ]
    );
}

#[test]
fn a_clean_bill_has_no_rejected_fields() {
    let reading = read_bill(&bill(), &full_masters()).unwrap();
    assert!(reading.rejected.is_empty(), "{:?}", reading.rejected);
}

#[test]
fn gstin_failing_the_check_character_is_rejected() {
    // 29ABCDE1234F1ZX: the check character 'X' is wrong, 'W' is right (file header).
    let ocr = replacing("GSTIN", "GSTIN: 29ABCDE1234F1ZX");
    let reading = read_bill(&ocr, &full_masters()).unwrap();
    assert_eq!(reading.draft.supplier_gstin, None);
    assert_eq!(reading.rejected.len(), 1);
    let rejected = reading.rejected.first().unwrap();
    assert_eq!(rejected.field, "supplier_gstin");
    assert_eq!(rejected.text, "GSTIN: 29ABCDE1234F1ZX");
    assert!(
        rejected.reason.contains("should be W, not X"),
        "{}",
        rejected.reason
    );
    // The rest of the bill is still read, and the party still resolves by name.
    assert_eq!(reading.draft.party, Some(SUPPLIER));
    assert_eq!(
        reading.draft.document_total.map(|t| t.value),
        Some(Paise::new(1_770_000))
    );
}

#[test]
fn gstin_with_an_ocr_letter_for_digit_is_rejected_not_repaired() {
    // 'O' (letter) for '0' in the PAN digits: a GSTIN has digits at positions 8 to 11 of the PAN.
    // The reader must not turn it back into '0' (rule A1: no auto-fix of user data).
    let ocr = replacing("GSTIN", "GSTIN: 29ABCDE1O34F1ZW");
    let reading = read_bill(&ocr, &full_masters()).unwrap();
    assert_eq!(reading.draft.supplier_gstin, None);
    assert_eq!(
        reading.rejected.first().map(|r| r.field.as_str()),
        Some("supplier_gstin")
    );
}

#[test]
fn lower_case_gstin_is_rejected_not_upper_cased() {
    let ocr = replacing("GSTIN", "GSTIN: 29abcde1234f1zw");
    let reading = read_bill(&ocr, &full_masters()).unwrap();
    assert_eq!(reading.draft.supplier_gstin, None);
    assert_eq!(reading.rejected.len(), 1);
}

#[test]
fn only_the_buyer_gstin_on_the_bill_is_not_taken_as_the_supplier() {
    let reading = read_bill(&without("GSTIN"), &full_masters()).unwrap();
    assert_eq!(reading.draft.supplier_gstin, None);
    assert_eq!(
        reading.rejected,
        [] as [ledgerly_assist::ocr::RejectedField; 0]
    );
    // The buyer line is still on the page; it is the buyer's, not the supplier's.
    assert!(bill_lines().iter().any(|l| l.text.contains(BUYER_GSTIN)));
}

#[test]
fn a_low_confidence_value_is_kept_with_its_confidence() {
    // Blanking below the threshold is the validator's job (A1); the reader reports the OCR
    // confidence unchanged.
    let mut ocr = bill();
    for l in &mut ocr.lines {
        if l.text.starts_with("Total 17") {
            l.confidence_permille = 310;
        }
    }
    let draft = read(&ocr, &full_masters());
    assert_eq!(
        draft
            .document_total
            .map(|t| (t.value.get(), t.confidence_permille)),
        Some((1_770_000, 310))
    );
}

#[test]
fn no_derived_number_is_ever_supplied() {
    // Without a printed Total Tax line the draft has no document tax: the reader never sums the
    // CGST and SGST lines (rule A10). Without a Total line there is no document total and no
    // party line.
    let draft = read(&without("Total"), &full_masters());
    assert_eq!(draft.document_tax, None);
    assert_eq!(draft.document_total, None);
    assert!(draft.lines.iter().all(|l| l.side == DrCr::Dr));
    assert_eq!(draft.lines.len(), 3);
}

#[test]
fn party_resolves_by_gstin_even_when_the_name_differs() {
    let ocr = replacing("Supplier:", "Supplier: Kaveri Stationery Mart");
    let draft = read(&ocr, &full_masters());
    assert_eq!(draft.party, Some(SUPPLIER));
    assert_eq!(draft.create_ledgers, [] as [std::string::String; 0]);
}

#[test]
fn party_resolves_by_name_ignoring_case_and_spacing_and_by_alias() {
    let m = masters(vec![ledger(
        SUPPLIER,
        "KAVERI   stationers",
        Some(party(None)),
        None,
    )]);
    assert_eq!(read(&without("GSTIN"), &m).party, Some(SUPPLIER));

    let mut aliased = ledger(SUPPLIER, "KS Traders", Some(party(None)), None);
    aliased.aliases.push("Kaveri Stationers".to_owned());
    let m = masters(vec![aliased]);
    assert_eq!(read(&without("GSTIN"), &m).party, Some(SUPPLIER));
}

#[test]
fn unknown_party_is_suggested_for_creation_never_given_an_id() {
    let m = masters(vec![
        ledger(
            LedgerId(41),
            "Other Traders",
            Some(party(Some(BUYER_GSTIN))),
            None,
        ),
        ledger(
            CGST,
            "Input CGST",
            None,
            Some(tax(TaxLedgerKind::Cgst, None)),
        ),
        ledger(
            SGST,
            "Input SGST",
            None,
            Some(tax(TaxLedgerKind::Sgst, None)),
        ),
    ]);
    let draft = read(&bill(), &m);
    assert_eq!(draft.party, None);
    assert_eq!(draft.create_ledgers, vec!["Kaveri Stationers".to_owned()]);
    let party_line = draft.lines.last().unwrap();
    assert_eq!(party_line.ledger, None);
}

#[test]
fn a_ledger_that_is_not_a_party_never_matches_the_party_name() {
    // A ledger named like the supplier but without party details (say an expense head) is not a
    // party ledger.
    let m = masters(vec![ledger(LedgerId(50), "Kaveri Stationers", None, None)]);
    let draft = read(&without("GSTIN"), &m);
    assert_eq!(draft.party, None);
    assert_eq!(
        draft.create_ledgers.first().map(String::as_str),
        Some("Kaveri Stationers")
    );
}

#[test]
fn two_party_ledgers_with_the_same_name_are_left_for_the_person() {
    let m = masters(vec![
        ledger(LedgerId(60), "Kaveri Stationers", Some(party(None)), None),
        ledger(LedgerId(61), "kaveri stationers", Some(party(None)), None),
    ]);
    let draft = read(&without("GSTIN"), &m);
    assert_eq!(draft.party, None);
    // Ambiguous is not "missing": no ledger is suggested for the party (tax heads still are,
    // since these masters hold no tax ledgers).
    assert!(
        !draft
            .create_ledgers
            .contains(&"Kaveri Stationers".to_owned())
    );
}

#[test]
fn name_and_gstin_matching_different_ledgers_is_reported() {
    let m = masters(vec![
        ledger(
            LedgerId(60),
            "Kaveri Stationers",
            Some(party(Some(BUYER_GSTIN))),
            None,
        ),
        ledger(
            LedgerId(61),
            "Other Name",
            Some(party(Some(SUPPLIER_GSTIN))),
            None,
        ),
    ]);
    let reading = read_bill(&bill(), &m).unwrap();
    assert_eq!(reading.draft.party, None);
    assert_eq!(reading.rejected.len(), 1);
    assert_eq!(reading.rejected.first().unwrap().field, "party");
}

#[test]
fn tax_ledger_fixed_to_the_bills_rate_is_preferred_and_other_rates_never_fit() {
    let m = masters(vec![
        ledger(SUPPLIER, "Kaveri Stationers", Some(party(None)), None),
        ledger(
            LedgerId(22),
            "CGST 6%",
            None,
            Some(tax(TaxLedgerKind::Cgst, Some(6))),
        ),
        ledger(
            CGST,
            "CGST 9%",
            None,
            Some(tax(TaxLedgerKind::Cgst, Some(9))),
        ),
        ledger(
            SGST,
            "Input SGST",
            None,
            Some(tax(TaxLedgerKind::Sgst, None)),
        ),
    ]);
    let draft = read(&bill(), &m);
    let ledgers: Vec<Option<LedgerId>> = draft.lines.iter().map(|l| l.ledger).collect();
    // taxable (none), CGST 9% -> 20, SGST (rate-less ledger fits) -> 21, party -> 40.
    assert_eq!(ledgers, vec![None, Some(CGST), Some(SGST), Some(SUPPLIER)]);

    // A bill at 2.5% fits neither CGST ledger fixed to 6% or 9%: suggest creating one.
    let ocr = replacing("CGST", "CGST @ 2.5% 375.00");
    let draft = read(&ocr, &m);
    let cgst_line = draft
        .lines
        .iter()
        .find(|l| l.ledger_name.value == "CGST")
        .unwrap();
    assert_eq!(cgst_line.ledger, None);
    // 2.5% of 15,000.00 is 375.00 (hand arithmetic); the amount is read as printed.
    assert_eq!(cgst_line.amount.value, Paise::new(37_500));
    assert_eq!(draft.create_ledgers, vec!["CGST @ 2.5%".to_owned()]);
}

#[test]
fn two_tax_ledgers_of_one_kind_are_left_unresolved_without_a_suggestion() {
    let m = masters(vec![
        ledger(SUPPLIER, "Kaveri Stationers", Some(party(None)), None),
        ledger(CGST, "CGST A", None, Some(tax(TaxLedgerKind::Cgst, None))),
        ledger(
            LedgerId(22),
            "CGST B",
            None,
            Some(tax(TaxLedgerKind::Cgst, None)),
        ),
        ledger(
            SGST,
            "Input SGST",
            None,
            Some(tax(TaxLedgerKind::Sgst, None)),
        ),
    ]);
    let draft = read(&bill(), &m);
    let cgst_line = draft
        .lines
        .iter()
        .find(|l| l.ledger_name.value == "CGST")
        .unwrap();
    assert_eq!(cgst_line.ledger, None);
    assert_eq!(draft.create_ledgers, [] as [std::string::String; 0]);
}

#[test]
fn no_tax_ledgers_at_all_suggests_creating_one_per_head() {
    let m = masters(vec![ledger(
        SUPPLIER,
        "Kaveri Stationers",
        Some(party(None)),
        None,
    )]);
    let draft = read(&bill(), &m);
    assert_eq!(
        draft.create_ledgers,
        vec!["CGST @ 9%".to_owned(), "SGST @ 9%".to_owned()]
    );
}

#[test]
fn a_label_on_two_lines_is_rejected_not_chosen_or_added() {
    // A tax table with one CGST row per rate: 9% and 2.5% rows. The reader must neither pick one
    // nor add them (no derived number).
    let ocr = with_extra("CGST @ 2.5% 100.00");
    let reading = read_bill(&ocr, &full_masters()).unwrap();
    assert!(
        reading
            .draft
            .lines
            .iter()
            .all(|l| l.ledger_name.value != "CGST")
    );
    assert_eq!(reading.rejected.len(), 1);
    let rejected = reading.rejected.first().unwrap();
    assert_eq!(rejected.field, "cgst");
    assert!(rejected.reason.contains('2'), "{}", rejected.reason);
}

#[test]
fn amount_formats_are_read_exactly_or_rejected() {
    // (text after "Total ", expected paise). Expansions by hand: rupees x 100 + paise.
    let ok: [(&str, i64); 8] = [
        ("17,700.00", 1_770_000),     // 17,700 x 100
        ("17700", 1_770_000),         // no grouping, no decimals
        ("Rs. 17,700.00", 1_770_000), // currency words are fillers
        ("\u{20b9}17,700.50", 1_770_050),
        ("INR 1,23,456.50", 12_345_650), // Indian grouping: 123,456 x 100 + 50
        ("1,234,567.8", 123_456_780),    // Western grouping; .8 is 80 paise
        (": 17,700.05", 1_770_005),
        ("0.00", 0),
    ];
    for (text, paise) in ok {
        let ocr = replacing("Total 17", &format!("Total {text}"));
        let draft = read(&ocr, &full_masters());
        assert_eq!(
            draft.document_total.map(|t| t.value),
            Some(Paise::new(paise)),
            "{text}"
        );
    }
    let bad = [
        "17,700.123", // a third decimal would need rounding
        "-500.00",    // negative
        "- 500.00",   // minus separated by a space is still negative
        ": - 500.00", // minus after a colon filler
        "- 17,700.00",
        "1,2",        // bad grouping
        "12,34",      // last group must have 3 digits
        "1,2345,678", // bad grouping
        "17,700.",    // dangling point
        "seventeen",
        "17,700.00 only",
        "99999999999999999999.00", // overflows i64 paise
        "",
    ];
    for text in bad {
        let ocr = replacing("Total 17", &format!("Total {text}"));
        let reading = read_bill(&ocr, &full_masters()).unwrap();
        assert_eq!(reading.draft.document_total, None, "{text}");
        assert_eq!(reading.rejected.len(), 1, "{text}");
        assert_eq!(
            reading.rejected.first().map(|r| r.field.as_str()),
            Some("document_total"),
            "{text}"
        );
    }
}

#[test]
fn total_tax_is_not_mistaken_for_total() {
    // "Total Tax" must go to document_tax only (longest label wins).
    let draft = read(&bill(), &full_masters());
    assert_eq!(
        draft.document_tax.map(|t| t.value),
        Some(Paise::new(270_000))
    );
    assert_eq!(
        draft.document_total.map(|t| t.value),
        Some(Paise::new(1_770_000))
    );
}

#[test]
fn dates_are_day_first_and_impossible_dates_are_rejected() {
    let ok = [
        ("15/03/2026", date!(2026 - 03 - 15)),
        ("15-03-2026", date!(2026 - 03 - 15)),
        ("15.03.2026", date!(2026 - 03 - 15)),
        ("5.3.2026", date!(2026 - 03 - 05)),
        ("15-Mar-2026", date!(2026 - 03 - 15)),
        ("15 March 2026", date!(2026 - 03 - 15)),
        ("29/02/2028", date!(2028 - 02 - 29)), // 2028 is a leap year (divisible by 4, not 100)
    ];
    for (text, expected) in ok {
        let draft = read(
            &replacing("Date", &format!("Date: {text}")),
            &full_masters(),
        );
        assert_eq!(draft.date.map(|d| d.value), Some(expected), "{text}");
    }
    let bad = [
        "31/02/2026", // February has 28 days in 2026
        "29/02/2027", // 2027 is not a leap year
        "03/15/2026", // month 15
        "15/03/26",   // two-digit year is ambiguous
        "15/03",
        "yesterday",
        "15/03/2026 10:30",
    ];
    for text in bad {
        let reading = read_bill(
            &replacing("Date", &format!("Date: {text}")),
            &full_masters(),
        )
        .unwrap();
        assert_eq!(reading.draft.date, None, "{text}");
        assert_eq!(
            reading.rejected.first().map(|r| r.field.as_str()),
            Some("date"),
            "{text}"
        );
    }
}

#[test]
fn credit_notes_and_similar_documents_are_unsupported() {
    for heading in [
        "CREDIT NOTE",
        "Debit Note",
        "Proforma Invoice",
        "Quotation",
        "ESTIMATE",
    ] {
        let mut ocr = bill();
        ocr.lines[0].text = heading.to_owned();
        match ocr_to_draft(&ocr, &full_masters()) {
            Err(AssistError::Unsupported(u)) => {
                let first_word = heading
                    .split_whitespace()
                    .next()
                    .unwrap()
                    .to_ascii_lowercase();
                assert!(u.what.contains(&first_word), "{}", u.what);
            }
            other => panic!("{heading}: expected Unsupported, got {other:?}"),
        }
    }
}

#[test]
fn a_word_that_merely_starts_like_a_heading_is_not_unsupported() {
    // "Estimated delivery" is not the heading "Estimate".
    let ocr = with_extra("Estimated delivery 3 days");
    assert!(ocr_to_draft(&ocr, &full_masters()).is_ok());
}

#[test]
fn malformed_ocr_output_is_an_error() {
    let empty = OcrOutput {
        doc_hash: HASH.to_owned(),
        lines: vec![],
    };
    assert!(matches!(
        ocr_to_draft(&empty, &full_masters()),
        Err(AssistError::Output { .. })
    ));
    let mut blank = bill();
    for l in &mut blank.lines {
        l.text = "   ".to_owned();
    }
    assert!(matches!(
        ocr_to_draft(&blank, &full_masters()),
        Err(AssistError::Output { .. })
    ));
    let mut no_hash = bill();
    no_hash.doc_hash = String::new();
    assert!(matches!(
        ocr_to_draft(&no_hash, &full_masters()),
        Err(AssistError::Output { .. })
    ));
    let mut over = bill();
    over.lines[3].confidence_permille = 1001;
    assert!(matches!(
        ocr_to_draft(&over, &full_masters()),
        Err(AssistError::Output { .. })
    ));
    let mut at_limit = bill();
    at_limit.lines[3].confidence_permille = 1000;
    assert!(ocr_to_draft(&at_limit, &full_masters()).is_ok());
}

#[test]
fn ocr_to_draft_is_the_draft_of_read_bill() {
    let m = full_masters();
    assert_eq!(
        ocr_to_draft(&bill(), &m).unwrap(),
        read_bill(&bill(), &m).unwrap().draft
    );
}

#[test]
fn reading_is_deterministic_and_ignores_line_order() {
    let m = full_masters();
    let mut reversed = bill();
    reversed.lines.reverse();
    // Reversing changes the order of the draft lines only through the fixed head order, so the
    // two drafts are equal field for field.
    assert_eq!(read(&bill(), &m), read(&reversed, &m));
}

#[test]
fn ocr_output_round_trips_through_json() {
    let json = serde_json::to_string(&bill()).unwrap();
    let back: OcrOutput = serde_json::from_str(&json).unwrap();
    assert_eq!(back, bill());
}

#[test]
fn tax_line_without_a_rate_on_the_bill_never_takes_a_ledger_fixed_to_a_rate() {
    // The bill's CGST line states no rate ("CGST 1,350.00"). The only CGST ledger is fixed to 9%,
    // which the bill never states, so it cannot be shown to fit: no ledger is chosen and the
    // head is suggested for creation. Before the fix, the 9% ledger was taken for any rate.
    let m = masters(vec![
        ledger(SUPPLIER, "Kaveri Stationers", Some(party(None)), None),
        ledger(
            CGST,
            "CGST 9%",
            None,
            Some(tax(TaxLedgerKind::Cgst, Some(9))),
        ),
        ledger(
            SGST,
            "Input SGST",
            None,
            Some(tax(TaxLedgerKind::Sgst, None)),
        ),
    ]);
    let ocr = replacing("CGST", "CGST 1,350.00");
    let draft = read(&ocr, &m);
    let cgst_line = draft
        .lines
        .iter()
        .find(|l| l.ledger_name.value == "CGST")
        .unwrap();
    assert_eq!(cgst_line.ledger, None);
    assert_eq!(cgst_line.amount.value, Paise::new(135_000));
    assert_eq!(draft.create_ledgers, vec!["CGST".to_owned()]);
}

#[test]
fn tax_line_without_a_rate_takes_the_one_ledger_with_no_fixed_rate() {
    // provenance: derived - the rate-less CGST ledger is the only one that can fit a line with no
    // rate, so it is chosen (same rule as the rate-less SGST ledger in the other tests).
    let m = masters(vec![
        ledger(SUPPLIER, "Kaveri Stationers", Some(party(None)), None),
        ledger(
            CGST,
            "Input CGST",
            None,
            Some(tax(TaxLedgerKind::Cgst, None)),
        ),
        ledger(
            LedgerId(22),
            "CGST 9%",
            None,
            Some(tax(TaxLedgerKind::Cgst, Some(9))),
        ),
    ]);
    let ocr = replacing("CGST", "CGST 1,350.00");
    let draft = read(&ocr, &m);
    let cgst_line = draft
        .lines
        .iter()
        .find(|l| l.ledger_name.value == "CGST")
        .unwrap();
    assert_eq!(cgst_line.ledger, Some(CGST));
}

#[test]
fn an_unrecognised_tax_label_is_never_given_a_tax_ledger() {
    // "Surcharge" is not a tax head the reader knows, so its line must not be mapped to any tax
    // ledger. This is a regression guard: the unknown label never reaches the tax-ledger match,
    // so it passes before and after the rate fix; the rate-less case above is the bug.
    let m = masters(vec![
        ledger(SUPPLIER, "Kaveri Stationers", Some(party(None)), None),
        ledger(
            CGST,
            "Input CGST",
            None,
            Some(tax(TaxLedgerKind::Cgst, None)),
        ),
        ledger(
            SGST,
            "Input SGST",
            None,
            Some(tax(TaxLedgerKind::Sgst, None)),
        ),
    ]);
    let draft = read(&with_extra("Surcharge 5% 750.00"), &m);
    assert!(
        draft
            .lines
            .iter()
            .all(|l| l.ledger_name.value != "Surcharge"),
        "{:?}",
        draft.lines
    );
    let tax_ledgers: Vec<Option<LedgerId>> = draft
        .lines
        .iter()
        .filter(|l| l.ledger_name.value.contains("GST"))
        .map(|l| l.ledger)
        .collect();
    assert_eq!(tax_ledgers, vec![Some(CGST), Some(SGST)]);
}
