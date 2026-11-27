//! Round s21, plan round A: the assistant's drafts in the Review queue (mock `review.default`:
//! "Draft purchase · Rupa Polymers, bill RP/5521 · Bill photo `IMG_2041.jpg` · High confidence ·
//! 80,240.00").
//!
//! Provenance (derived). The rows are built from the literal drafts below. The amount is the
//! engine's figure handed in as `DraftFigure::Total` (the screen draws it, never computes it); a
//! blocked draft has no amount and is drawn "none". Confidence bands are our own documented rule
//! (`review::confidence_of`): 850 per-mille and up is High, the keep threshold (700) up to 849 is
//! Medium, below 700 is Low. The words are ours.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_api::{
    AiDraft, AiFeature, DraftFigure, Proposal, ReviewDraft, ReviewQueue, SourceRef, Transcribed,
    VoucherDraft,
};
use ledgerly_app::release::set_ai_enabled;
use ledgerly_app::screens::review::{RowOrigin, Verdict, accept_verdict, rows};
use ledgerly_app::ui::conf::Confidence;
use ledgerly_core::{EventSeq, Paise};
use ledgerly_model::BaseVoucherKind;
use time::OffsetDateTime;

fn doc<T>(value: T) -> Transcribed<T> {
    Transcribed {
        value,
        source: SourceRef::Document {
            doc_hash: "9f".repeat(32),
            page: 1,
            bbox: [0, 0, 10, 10],
        },
        confidence_permille: 920,
    }
}

fn bill_draft(id: u64, confidence_permille: u16, engine: DraftFigure) -> ReviewDraft {
    let draft = VoucherDraft {
        kind: BaseVoucherKind::Purchase,
        date: None,
        party_name: Some(doc("Rupa Polymers".to_owned())),
        party: None,
        invoice_number: Some(doc("RP/5521".to_owned())),
        supplier_gstin: None,
        lines: Vec::new(),
        items: Vec::new(),
        document_total: None,
        document_tax: None,
        create_ledgers: Vec::new(),
        narration: String::new(),
    };
    ReviewDraft {
        draft: AiDraft {
            id: EventSeq(id),
            at: OffsetDateTime::UNIX_EPOCH,
            feature: AiFeature::BillReading,
            provider: "Fixed answers (test provider)".to_owned(),
            model_version: "s21-1".to_owned(),
            prompt_template_version: "ocr/1".to_owned(),
            input_label: "IMG_2041.jpg".to_owned(),
            input_hash: "9f".repeat(32),
            proposal: Proposal::VoucherDraft(Box::new(draft)),
            confidence_permille,
        },
        engine,
    }
}

fn queue(drafts: Vec<ReviewDraft>) -> ReviewQueue {
    ReviewQueue {
        items: Vec::new(),
        duplicates: Vec::new(),
        drafts,
        unlisted: Vec::new(),
    }
}

#[test]
fn a_bill_draft_row_reads_as_the_mock_with_the_engine_amount() {
    set_ai_enabled(true);
    let shown = rows(&queue(vec![bill_draft(
        41,
        920,
        DraftFigure::Total(Paise::new(8_024_000)),
    )]));
    assert_eq!(shown.len(), 1);
    let row = &shown[0];
    assert_eq!(row.kind, "Draft purchase");
    assert_eq!(row.what, "Rupa Polymers, bill RP/5521");
    assert_eq!(row.source, "Bill photo\nIMG_2041.jpg");
    assert_eq!(row.confidence, Confidence::High);
    assert_eq!(row.amount, Some(Paise::new(8_024_000)));
    assert_eq!(
        row.origin,
        RowOrigin::Draft {
            id: EventSeq(41),
            feature: AiFeature::BillReading
        }
    );
    assert_eq!(
        accept_verdict(row),
        Verdict::AcceptDraft { id: EventSeq(41) }
    );
}

#[test]
fn a_blocked_draft_has_no_amount_and_bands_follow_the_threshold() {
    set_ai_enabled(true);
    let shown = rows(&queue(vec![
        bill_draft(1, 849, DraftFigure::Blocked("date missing".to_owned())),
        bill_draft(2, 700, DraftFigure::Total(Paise::new(100))),
        bill_draft(3, 699, DraftFigure::Total(Paise::new(100))),
    ]));
    assert_eq!(shown[0].amount, None);
    assert_eq!(shown[0].confidence, Confidence::Medium);
    assert_eq!(shown[1].confidence, Confidence::Medium);
    assert_eq!(shown[2].confidence, Confidence::Low);
}

#[test]
fn with_ai_out_of_the_release_no_draft_row_is_shown() {
    set_ai_enabled(false);
    let shown = rows(&queue(vec![bill_draft(
        41,
        920,
        DraftFigure::Total(Paise::new(8_024_000)),
    )]));
    assert_eq!(shown.len(), 0);
}
