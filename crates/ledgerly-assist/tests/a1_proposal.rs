//! Card A1 tests: proposal types (spec 6.8). Confidence is per-mille and the threshold test keeps
//! a value whose confidence equals it; proposals survive a serde round trip unchanged.

#![allow(clippy::unwrap_used, reason = "integration tests may unwrap")]

use ledgerly_assist::{
    DraftLine, MappingSuggestion, MatchSuggestion, Proposal, ReportQuery, SourceRef, Transcribed,
    VoucherDraft,
};
use ledgerly_core::{DrCr, LedgerId, Paise, VoucherId};
use ledgerly_model::BaseVoucherKind;
use time::macros::date;

fn typed<T>(value: T, confidence_permille: u16) -> Transcribed<T> {
    Transcribed {
        value,
        source: SourceRef::Typed { start: 3, end: 9 },
        confidence_permille,
    }
}

#[test]
fn confidence_at_the_threshold_is_kept_and_below_it_is_blank() {
    // Definition of the rule (spec 6.8: below threshold means blank): >= keeps, < blanks.
    let v = typed(1_i32, 700);
    assert!(v.is_confident(700));
    assert!(v.is_confident(0));
    assert!(!v.is_confident(701));
    assert!(typed(1_i32, 0).is_confident(0));
}

#[test]
fn every_proposal_round_trips_through_json() {
    let draft = VoucherDraft {
        kind: BaseVoucherKind::Payment,
        date: Some(typed(date!(2026 - 03 - 31), 900)),
        party_name: Some(typed("Sharma Properties".to_owned(), 800)),
        party: Some(LedgerId(5)),
        invoice_number: None,
        supplier_gstin: None,
        lines: vec![DraftLine {
            ledger_name: typed("Rent".to_owned(), 900),
            ledger: Some(LedgerId(7)),
            side: DrCr::Dr,
            amount: Transcribed {
                value: Paise::new(2_500_000),
                source: SourceRef::Document {
                    doc_hash: "ab".repeat(32),
                    page: 1,
                    bbox: [10, 20, 30, 40],
                },
                confidence_permille: 910,
            },
        }],
        items: vec![],
        document_total: Some(typed(Paise::new(2_500_000), 910)),
        document_tax: None,
        create_ledgers: vec![],
        narration: "rent".to_owned(),
    };
    let proposals = vec![
        Proposal::VoucherDraft(Box::new(draft)),
        Proposal::MatchSuggestion(MatchSuggestion {
            statement_row: 4,
            vouchers: vec![VoucherId(2)],
            ledger_name: None,
            reason: "same amount and reference".to_owned(),
            confidence_permille: 850,
        }),
        Proposal::ReportQuery(ReportQuery {
            report: "trial_balance".to_owned(),
            from: Some(date!(2025 - 04 - 01)),
            to: Some(date!(2026 - 03 - 31)),
            ledger_names: vec![],
        }),
        Proposal::MappingSuggestion(MappingSuggestion {
            columns: vec![("Dt".to_owned(), "date".to_owned())],
            confidence_permille: 700,
        }),
    ];
    for p in proposals {
        let json = serde_json::to_string(&p).unwrap();
        let back: Proposal = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }
}
