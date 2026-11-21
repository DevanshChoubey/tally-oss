//! Card A2 tests: natural-language entry (spec 6.8). A recorded model returns fixed JSON (a
//! recording of model behaviour, not an expected value); every expected amount, span and side is
//! derived by hand in the test.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::sync::Mutex;

use ledgerly_assist::nl_entry::{
    AmountTextError, MasterLedger, ModelClient, ModelRequest, NlEntry, OUTPUT_GRAMMAR,
    PROMPT_TEMPLATE_VERSION, RecordedModel, parse_amount_text,
};
use ledgerly_assist::{AssistError, Proposal, ProposalInput, Proposer, SourceRef, VoucherDraft};
use ledgerly_core::{CancelToken, DrCr, LedgerId, Paise};
use ledgerly_model::BaseVoucherKind;
use time::macros::date;

const RENT: &str = "paid rent 25k to Sharma Properties by HDFC";

fn master(id: u64, name: &str) -> MasterLedger {
    MasterLedger {
        id: LedgerId(id),
        name: name.to_owned(),
        aliases: vec![],
    }
}

fn masters() -> Vec<MasterLedger> {
    vec![
        master(5, "Sharma Properties"),
        master(2, "HDFC Bank"),
        master(9, "Office Rent"),
    ]
}

fn json(
    kind: &str,
    amount: Option<&str>,
    date: Option<&str>,
    party: Option<&str>,
    account: Option<&str>,
) -> String {
    let q = |v: Option<&str>| v.map_or("null".to_owned(), |s| format!("\"{s}\""));
    format!(
        "{{\"kind\":\"{kind}\",\"amount_text\":{},\"date_text\":{},\"party_name\":{},\"account_name\":{},\"narration\":\"rent\",\"confidence_permille\":900}}",
        q(amount),
        q(date),
        q(party),
        q(account)
    )
}

fn draft_for(text: &str, model_json: &str, ledgers: Vec<MasterLedger>) -> VoucherDraft {
    let model = RecordedModel::new().with_recording(text, model_json);
    let entry = NlEntry::new(model, ledgers);
    match entry
        .propose(&ProposalInput::Text(text.to_owned()), &CancelToken::new())
        .unwrap()
    {
        Proposal::VoucherDraft(d) => *d,
        other => panic!("expected a voucher draft, got {other:?}"),
    }
}

fn error_for(text: &str, model_json: &str, ledgers: Vec<MasterLedger>) -> AssistError {
    let model = RecordedModel::new().with_recording(text, model_json);
    NlEntry::new(model, ledgers)
        .propose(&ProposalInput::Text(text.to_owned()), &CancelToken::new())
        .unwrap_err()
}

#[test]
fn rent_payment_becomes_a_payment_draft_with_resolved_ledgers() {
    let d = draft_for(
        RENT,
        &json(
            "payment",
            Some("25k"),
            None,
            Some("Sharma Properties"),
            Some("HDFC"),
        ),
        masters(),
    );
    assert_eq!(d.kind, BaseVoucherKind::Payment);
    // provenance: derived - "25k" means 25 x 1,000 = 25,000 rupees; x 100 paise = 2,500,000.
    let amount = Paise::new(2_500_000);
    assert_eq!(d.document_total.as_ref().unwrap().value, amount);
    // Payment: the party is debited, the bank is credited (a payment reduces the bank balance).
    assert_eq!(d.lines.len(), 2);
    assert_eq!(d.lines[0].side, DrCr::Dr);
    assert_eq!(d.lines[0].ledger, Some(LedgerId(5)));
    assert_eq!(d.lines[0].amount.value, amount);
    assert_eq!(d.lines[1].side, DrCr::Cr);
    // "HDFC" are the leading words of the one ledger "HDFC Bank" (id 2 in the masters above).
    assert_eq!(d.lines[1].ledger, Some(LedgerId(2)));
    assert_eq!(d.lines[1].amount.value, amount);
    assert_eq!(d.party, Some(LedgerId(5)));
    assert!(d.create_ledgers.is_empty());
    assert_eq!(d.date, None);
    assert_eq!(d.narration, "rent");
    assert_eq!(d.document_tax, None);
}

#[test]
fn sources_are_character_ranges_into_the_typed_text() {
    let d = draft_for(
        RENT,
        &json(
            "payment",
            Some("25k"),
            None,
            Some("Sharma Properties"),
            Some("HDFC"),
        ),
        masters(),
    );
    // provenance: derived - counting characters of "paid rent 25k to Sharma Properties by HDFC"
    // from 0: "25k" starts at 10 (after "paid rent ") and has 3 characters, so ends at 13;
    // "Sharma Properties" (17 characters) starts at 17 and ends at 34; "HDFC" is 38..42.
    assert_eq!(
        d.lines[0].amount.source,
        SourceRef::Typed { start: 10, end: 13 }
    );
    assert_eq!(
        d.party_name.as_ref().unwrap().source,
        SourceRef::Typed { start: 17, end: 34 }
    );
    assert_eq!(
        d.lines[1].ledger_name.source,
        SourceRef::Typed { start: 38, end: 42 }
    );
    assert_eq!(d.party_name.unwrap().confidence_permille, 900);
}

#[test]
fn unresolved_ledger_is_a_create_suggestion_never_an_invented_id() {
    let text = "paid rent 25k to Verma Traders by HDFC";
    let d = draft_for(
        text,
        &json(
            "payment",
            Some("25k"),
            None,
            Some("Verma Traders"),
            Some("HDFC"),
        ),
        masters(),
    );
    assert_eq!(d.party, None);
    assert_eq!(d.lines[0].ledger, None);
    assert_eq!(d.create_ledgers, vec!["Verma Traders".to_owned()]);
    // The bank still resolves; only the unknown name is suggested.
    assert_eq!(d.lines[1].ledger, Some(LedgerId(2)));
}

#[test]
fn with_no_masters_every_name_is_suggested_once() {
    let d = draft_for(
        RENT,
        &json(
            "payment",
            Some("25k"),
            None,
            Some("Sharma Properties"),
            Some("HDFC"),
        ),
        vec![],
    );
    assert!(d.lines.iter().all(|l| l.ledger.is_none()));
    assert_eq!(
        d.create_ledgers,
        vec!["Sharma Properties".to_owned(), "HDFC".to_owned()]
    );
}

#[test]
fn receipt_debits_the_bank_and_credits_the_party() {
    let text = "received 1.5 lakh from Sharma Properties in HDFC";
    let d = draft_for(
        text,
        &json(
            "receipt",
            Some("1.5 lakh"),
            None,
            Some("Sharma Properties"),
            Some("HDFC"),
        ),
        masters(),
    );
    assert_eq!(d.kind, BaseVoucherKind::Receipt);
    // provenance: derived - 1.5 lakh = 1.5 x 100,000 = 150,000 rupees = 15,000,000 paise.
    assert_eq!(d.document_total.unwrap().value, Paise::new(15_000_000));
    // A receipt increases the bank (Dr) and the party's payable/receivable is credited.
    assert_eq!(d.lines[0].side, DrCr::Cr);
    assert_eq!(d.lines[0].ledger, Some(LedgerId(5)));
    assert_eq!(d.lines[1].side, DrCr::Dr);
    assert_eq!(d.lines[1].ledger, Some(LedgerId(2)));
}

#[test]
fn exact_name_beats_a_longer_name_that_starts_with_it() {
    let ledgers = vec![master(1, "HDFC"), master(2, "HDFC Bank")];
    let d = draft_for(
        RENT,
        &json(
            "payment",
            Some("25k"),
            None,
            Some("Sharma Properties"),
            Some("HDFC"),
        ),
        ledgers,
    );
    assert_eq!(d.lines[1].ledger, Some(LedgerId(1)));
}

#[test]
fn matching_ignores_case_spacing_and_uses_aliases() {
    let text = "paid rent 25k to sharma  properties by hdfc";
    let mut ledgers = masters();
    ledgers.push(MasterLedger {
        id: LedgerId(30),
        name: "State Bank".to_owned(),
        aliases: vec!["SBI".to_owned()],
    });
    let d = draft_for(
        text,
        &json(
            "payment",
            Some("25k"),
            None,
            Some("sharma  properties"),
            Some("hdfc"),
        ),
        ledgers.clone(),
    );
    assert_eq!(d.lines[0].ledger, Some(LedgerId(5)));
    assert_eq!(d.lines[1].ledger, Some(LedgerId(2)));

    let text = "paid rent 25k to Sharma Properties by SBI";
    let d = draft_for(
        text,
        &json(
            "payment",
            Some("25k"),
            None,
            Some("Sharma Properties"),
            Some("SBI"),
        ),
        ledgers,
    );
    assert_eq!(d.lines[1].ledger, Some(LedgerId(30)));
}

#[test]
fn a_name_matching_two_ledgers_is_unsupported_not_a_guess() {
    let ledgers = vec![master(2, "HDFC Bank"), master(3, "HDFC Credit Card")];
    let err = error_for(
        RENT,
        &json(
            "payment",
            Some("25k"),
            None,
            Some("Sharma Properties"),
            Some("HDFC"),
        ),
        ledgers,
    );
    match err {
        AssistError::Unsupported(u) => {
            assert!(u.what.contains("HDFC"));
            assert!(u.reason.contains("HDFC Bank") && u.reason.contains("HDFC Credit Card"));
        }
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

#[test]
fn a_name_the_user_did_not_type_is_rejected() {
    // The model invented "Gupta Estates"; it is not in the typed text.
    let err = error_for(
        RENT,
        &json(
            "payment",
            Some("25k"),
            None,
            Some("Gupta Estates"),
            Some("HDFC"),
        ),
        masters(),
    );
    assert!(matches!(err, AssistError::Output { .. }), "{err:?}");
}

#[test]
fn an_amount_the_user_did_not_type_is_rejected() {
    // The model "computed" 25000 where the user typed 25k: not a transcription.
    let err = error_for(
        RENT,
        &json(
            "payment",
            Some("25000"),
            None,
            Some("Sharma Properties"),
            Some("HDFC"),
        ),
        masters(),
    );
    assert!(matches!(err, AssistError::Output { .. }), "{err:?}");
}

#[test]
fn an_amount_that_is_only_the_tail_of_a_typed_token_is_rejected() {
    // The user typed "25k"; the model returned "5k", which is a substring of it but a different
    // amount (5,000 instead of 25,000). A transcription must match whole tokens.
    let err = error_for(
        RENT,
        &json(
            "payment",
            Some("5k"),
            None,
            Some("Sharma Properties"),
            Some("HDFC"),
        ),
        masters(),
    );
    assert!(matches!(err, AssistError::Output { .. }), "{err:?}");
}

#[test]
fn an_amount_that_is_part_of_a_decimal_is_rejected() {
    // "5" sits inside the typed "1.5 lakh" after a decimal point; it is not a separate token.
    let text = "received 1.5 lakh from Sharma Properties in HDFC";
    let err = error_for(
        text,
        &json(
            "receipt",
            Some("5 lakh"),
            None,
            Some("Sharma Properties"),
            Some("HDFC"),
        ),
        masters(),
    );
    assert!(matches!(err, AssistError::Output { .. }), "{err:?}");
}

#[test]
fn a_missing_or_unreadable_amount_is_an_error() {
    let none = error_for(
        RENT,
        &json(
            "payment",
            None,
            None,
            Some("Sharma Properties"),
            Some("HDFC"),
        ),
        masters(),
    );
    assert!(matches!(none, AssistError::Output { .. }));
    // "rent" is in the text but is not money.
    let word = error_for(
        RENT,
        &json(
            "payment",
            Some("rent"),
            None,
            Some("Sharma Properties"),
            Some("HDFC"),
        ),
        masters(),
    );
    assert!(matches!(word, AssistError::Output { .. }));
}

#[test]
fn dates_are_parsed_by_the_module_and_unreadable_dates_stay_blank() {
    let cases = [
        // provenance: derived - day-first numeric dates as written in India.
        ("31/03/2026", Some(date!(2026 - 03 - 31))),
        ("01-04-2026", Some(date!(2026 - 04 - 01))),
        ("05.11.2026", Some(date!(2026 - 11 - 05))),
        // provenance: derived - ISO 8601 year-month-day.
        ("2026-04-01", Some(date!(2026 - 04 - 01))),
        // 2026 is not a leap year, so 29 Feb does not exist.
        ("29/02/2026", None),
        // Two-digit years and words are not guessed.
        ("31/03/26", None),
        ("yesterday", None),
    ];
    for (typed, expected) in cases {
        let text = format!("paid rent 25k to Sharma Properties by HDFC on {typed}");
        let d = draft_for(
            &text,
            &json(
                "payment",
                Some("25k"),
                Some(typed),
                Some("Sharma Properties"),
                Some("HDFC"),
            ),
            masters(),
        );
        assert_eq!(d.date.map(|t| t.value), expected, "{typed}");
    }
}

#[test]
fn other_voucher_kinds_are_unsupported() {
    for kind in ["sales", "purchase", "journal", "contra"] {
        let err = error_for(
            RENT,
            &json(
                kind,
                Some("25k"),
                None,
                Some("Sharma Properties"),
                Some("HDFC"),
            ),
            masters(),
        );
        assert!(
            matches!(err, AssistError::Unsupported(_)),
            "{kind}: {err:?}"
        );
    }
    let err = error_for(
        RENT,
        &json(
            "banana",
            Some("25k"),
            None,
            Some("Sharma Properties"),
            Some("HDFC"),
        ),
        masters(),
    );
    assert!(matches!(err, AssistError::Output { .. }));
}

#[test]
fn malformed_model_output_is_an_error_never_a_default() {
    let bad = [
        "not json".to_owned(),
        "{}".to_owned(),
        // unknown field
        json("payment", Some("25k"), None, None, None).replace("\"kind\"", "\"extra\":1,\"kind\""),
        // confidence out of range
        json("payment", Some("25k"), None, None, None).replace("900", "1001"),
    ];
    for output in bad {
        let err = error_for(RENT, &output, masters());
        assert!(
            matches!(err, AssistError::Output { .. }),
            "{output}: {err:?}"
        );
    }
}

#[test]
fn only_typed_text_is_supported_and_the_model_is_not_called_otherwise() {
    let model = RecordedModel::new();
    let entry = NlEntry::new(model, masters());
    let cancel = CancelToken::new();
    let inputs = [
        ProposalInput::Question("what is my cash balance".to_owned()),
        ProposalInput::Columns {
            headers: vec!["Date".to_owned()],
            sample_rows: vec![],
        },
        ProposalInput::Document {
            bytes: vec![1],
            mime: "image/png".to_owned(),
            sha256: "00".repeat(32),
        },
        ProposalInput::Text("   ".to_owned()),
    ];
    for input in inputs {
        let err = entry.propose(&input, &cancel).unwrap_err();
        assert!(
            matches!(
                err,
                AssistError::Unsupported(_) | AssistError::Output { .. }
            ),
            "{err:?}"
        );
    }
}

#[test]
fn cancelled_before_the_call_never_reaches_the_model() {
    let model =
        RecordedModel::new().with_recording(RENT, json("payment", Some("25k"), None, None, None));
    let entry = NlEntry::new(model, masters());
    let cancel = CancelToken::new();
    cancel.cancel();
    let err = entry
        .propose(&ProposalInput::Text(RENT.to_owned()), &cancel)
        .unwrap_err();
    assert_eq!(err, AssistError::Cancelled);
    assert_eq!(entry.model().calls(), 0);
}

struct CancelsWhileRunning(String);

impl ModelClient for CancelsWhileRunning {
    fn complete(
        &self,
        _request: &ModelRequest,
        cancel: &CancelToken,
    ) -> Result<String, AssistError> {
        cancel.cancel();
        Ok(self.0.clone())
    }
}

#[test]
fn cancelled_during_the_call_returns_cancelled_and_no_draft() {
    let entry = NlEntry::new(
        CancelsWhileRunning(json("payment", Some("25k"), None, None, None)),
        masters(),
    );
    let err = entry
        .propose(&ProposalInput::Text(RENT.to_owned()), &CancelToken::new())
        .unwrap_err();
    assert_eq!(err, AssistError::Cancelled);
}

struct Capturing(Mutex<Vec<ModelRequest>>);

impl ModelClient for Capturing {
    fn complete(
        &self,
        request: &ModelRequest,
        _cancel: &CancelToken,
    ) -> Result<String, AssistError> {
        self.0.lock().unwrap().push(request.clone());
        Ok(json("payment", Some("25k"), None, None, None))
    }
}

#[test]
fn the_model_sees_the_text_template_version_and_grammar_but_not_the_masters() {
    let entry = NlEntry::new(Capturing(Mutex::new(vec![])), masters());
    entry
        .propose(&ProposalInput::Text(RENT.to_owned()), &CancelToken::new())
        .unwrap();
    let seen = entry_requests(&entry);
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].user_text, RENT);
    assert_eq!(seen[0].template_version, PROMPT_TEMPLATE_VERSION);
    assert_eq!(seen[0].grammar, OUTPUT_GRAMMAR);
    assert!(!seen[0].system.contains("Sharma") && !seen[0].system.contains("HDFC"));
}

fn entry_requests(entry: &NlEntry<Capturing>) -> Vec<ModelRequest> {
    entry.model().0.lock().unwrap().clone()
}

#[test]
fn amount_text_is_read_deterministically() {
    // provenance: derived - each row is hand arithmetic: rupees x 100 = paise. Units: k = 1,000;
    // lakh = 1,00,000; crore = 1,00,00,000 (Indian numbering).
    let cases: [(&str, i64); 11] = [
        ("25k", 2_500_000),            // 25 x 1,000 = 25,000 rupees
        ("25K", 2_500_000),            // case ignored
        ("25 thousand", 2_500_000),    // same
        ("1.5 lakh", 15_000_000),      // 1.5 x 100,000 = 150,000 rupees
        ("2 cr", 2_000_000_000),       // 2 x 10,000,000 = 20,000,000 rupees
        ("2.25 crore", 2_250_000_000), // 2.25 x 10,000,000 = 22,500,000 rupees
        ("₹1,25,000", 12_500_000),     // 125,000 rupees
        ("Rs. 25,000/-", 2_500_000),   // 25,000 rupees
        ("Rs 499.50", 49_950),         // 499.50 rupees
        ("25000", 2_500_000),          // plain rupees
        ("INR 0.05", 5),               // 5 paise
    ];
    for (text, paise) in cases {
        assert_eq!(parse_amount_text(text), Ok(Paise::new(paise)), "{text}");
    }
}

#[test]
fn amount_text_that_is_not_one_clean_amount_is_an_error() {
    assert_eq!(parse_amount_text("  "), Err(AmountTextError::Empty));
    assert_eq!(parse_amount_text("0"), Err(AmountTextError::Zero));
    assert_eq!(parse_amount_text("0 lakh"), Err(AmountTextError::Zero));
    // 10.555 rupees is 1055.5 paise: not a whole number of paise.
    assert_eq!(
        parse_amount_text("10.555"),
        Err(AmountTextError::TooPrecise)
    );
    // 0.000555 x 1,000 = 0.555 rupee: three decimal places, not a whole number of paise.
    assert_eq!(
        parse_amount_text("0.000555k"),
        Err(AmountTextError::TooPrecise)
    );
    assert!(matches!(
        parse_amount_text("12abc3"),
        Err(AmountTextError::UnknownUnit(_))
    ));
    assert!(matches!(
        parse_amount_text("1,,0"),
        Err(AmountTextError::NotANumber(_))
    ));
    assert!(matches!(
        parse_amount_text("1.2.3"),
        Err(AmountTextError::NotANumber(_))
    ));
    assert!(matches!(
        parse_amount_text(",5"),
        Err(AmountTextError::NotANumber(_))
    ));
    assert!(matches!(
        parse_amount_text("-25k"),
        Err(AmountTextError::NotANumber(_))
    ));
    assert!(matches!(
        parse_amount_text("rent"),
        Err(AmountTextError::NotANumber(_))
    ));
    assert!(matches!(
        parse_amount_text("25 dollars"),
        Err(AmountTextError::UnknownUnit(_))
    ));
    // i64::MAX paise is about 9.22e16 rupees; 99999999999999 cr is 9.9999e21 rupees.
    assert_eq!(
        parse_amount_text("99999999999999 cr"),
        Err(AmountTextError::Overflow)
    );
}
