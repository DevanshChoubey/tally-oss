//! Card U11: the assistant panel. Spec 6.8 and the card text give the contract; each test names
//! the sentence it checks. Expected values are derived by hand (written next to each) and none is
//! copied from this crate's output. The panel shows no derived number of its own: the only
//! numbers drawn are `Paise` handed in as "engine computed", formatted by the shared formatter.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use egui::{Event, Key, Modifiers, RawInput, Shape};
use ledgerly_app::screens::assist::{
    AssistScreen, BLANK_TEXT, ComputedFigure, DraftView, EngineCheck, LABEL_AI_PROPOSED,
    LABEL_ENGINE_COMPUTED, OFF_TEXT, PanelAction, ProposalView, ProposedField, ProposedLine,
    ReportAnswer, ReportView, SuggestionView, Tier,
};
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{DrCr, Paise};

// ---- fixtures -------------------------------------------------------------------------------

fn field(label: &str, value: Option<&str>, permille: u16) -> ProposedField {
    ProposedField {
        label: label.to_owned(),
        value: value.map(str::to_owned),
        confidence_permille: permille,
        source: "Typed text".to_owned(),
    }
}

fn draft(check: EngineCheck) -> DraftView {
    DraftView {
        kind: "Payment".to_owned(),
        fields: vec![
            field("Party", Some("Sharma Properties"), 950),
            field("Invoice number", None, 400),
        ],
        lines: vec![ProposedLine {
            ledger: field("Ledger", Some("Rent"), 900),
            side: DrCr::Dr,
            amount: field("Amount", Some("25,000.00"), 980),
        }],
        create_ledgers: vec!["Sharma Properties".to_owned()],
        check,
    }
}

fn ok_check() -> EngineCheck {
    EngineCheck::Ok {
        figures: vec![ComputedFigure {
            label: "Total".to_owned(),
            amount: Paise::new(25_000_00),
        }],
    }
}

fn blocked_check() -> EngineCheck {
    EngineCheck::Blocked {
        field: "document_total".to_owned(),
        reason: "the engine computed a different total".to_owned(),
    }
}

fn unsupported_check() -> EngineCheck {
    EngineCheck::Unsupported {
        what: "GST place of supply".to_owned(),
        reason: "not built yet".to_owned(),
    }
}

fn screen_with(check: EngineCheck) -> AssistScreen {
    let mut s = AssistScreen::new(Tier::Local);
    s.set_proposal(ProposalView::Voucher(draft(check)));
    s
}

// ---- egui helpers ---------------------------------------------------------------------------

fn collect_text(shape: &Shape, out: &mut Vec<String>) {
    if let Shape::Text(t) = shape {
        out.push(t.galley.text().to_owned());
    } else if let Shape::Vec(inner) = shape {
        for s in inner {
            collect_text(s, out);
        }
    }
}

/// Runs three frames (egui lays grids out invisibly on the first) and returns the last frame's
/// action and every text drawn in it.
fn frames(s: &mut AssistScreen, input: &RawInput) -> (ScreenAction, Vec<String>) {
    let ctx = egui::Context::default();
    let mut app = AppCtx::default();
    let mut action = ScreenAction::Stay;
    let mut texts = Vec::new();
    for i in 0..3 {
        let raw = if i == 0 {
            input.clone()
        } else {
            RawInput::default()
        };
        let out = ctx.run_ui(raw, |ui| {
            action = s.ui(ui, &mut app);
        });
        texts.clear();
        for clipped in &out.shapes {
            collect_text(&clipped.shape, &mut texts);
        }
        out.drop_without_applying_deltas();
    }
    (action, texts)
}

fn has(texts: &[String], needle: &str) -> bool {
    texts.iter().any(|t| t.contains(needle))
}

fn ctrl_enter() -> RawInput {
    RawInput {
        events: vec![Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        }],
        ..RawInput::default()
    }
}

// ---- labels ---------------------------------------------------------------------------------

#[test]
fn labels_are_the_words_the_card_names() {
    // Card U11: Labels "AI proposed" vs "engine computed".
    assert_eq!(LABEL_AI_PROPOSED, "AI proposed");
    assert_eq!(LABEL_ENGINE_COMPUTED, "Engine computed");
}

#[test]
fn a_draft_is_drawn_under_both_labels() {
    let mut s = screen_with(ok_check());
    let (_, texts) = frames(&mut s, &RawInput::default());
    assert!(has(&texts, LABEL_AI_PROPOSED));
    assert!(has(&texts, LABEL_ENGINE_COMPUTED));
    assert!(has(&texts, "Sharma Properties"));
}

#[test]
fn engine_figures_are_drawn_in_indian_grouping_as_given() {
    // Derived: Paise 25_000_00 is 25,000 rupees; Indian grouping of 25000 is "25,000", then ".00".
    let mut s = screen_with(ok_check());
    let (_, texts) = frames(&mut s, &RawInput::default());
    assert!(has(&texts, "25,000.00"));
}

#[test]
fn a_blocked_draft_shows_no_engine_figure() {
    // Rule A1 / spec 6.8: a mismatch blocks; no engine number is displayed for a blocked draft.
    let mut s = screen_with(blocked_check());
    let (_, texts) = frames(&mut s, &RawInput::default());
    assert!(has(&texts, "Blocked at document_total"));
    assert!(!has(&texts, "Checked: the engine reproduced"));
}

#[test]
fn a_report_answer_shows_the_query_with_it() {
    // Spec 6.8: "question -> ReportQuery (engine runs it; answer shows the query)".
    let mut s = AssistScreen::new(Tier::Local);
    s.set_proposal(ProposalView::Report(ReportView {
        query: "trial_balance, April 2025".to_owned(),
        answer: ReportAnswer::Computed(vec![ComputedFigure {
            label: "Debit total".to_owned(),
            // Derived: 1_00_000 rupees = 1_00_000_00 paise; Indian grouping "1,00,000.00".
            amount: Paise::new(1_00_000_00),
        }]),
    }));
    let (_, texts) = frames(&mut s, &RawInput::default());
    assert!(has(&texts, "Query: trial_balance, April 2025"));
    assert!(has(&texts, "1,00,000.00"));
}

// ---- blank below threshold ------------------------------------------------------------------

#[test]
fn a_low_confidence_field_is_blank_not_guessed() {
    // Spec 6.8: "below threshold -> blank".
    let f = field("Invoice number", None, 400);
    assert!(f.is_blank());
    assert_eq!(f.display_value(), BLANK_TEXT);
    let g = field("Party", Some("Sharma Properties"), 950);
    assert!(!g.is_blank());
    assert_eq!(g.display_value(), "Sharma Properties");
}

#[test]
fn a_blank_field_is_drawn_as_blank() {
    let mut s = screen_with(ok_check());
    let (_, texts) = frames(&mut s, &RawInput::default());
    assert!(has(&texts, BLANK_TEXT));
}

#[test]
fn confidence_text_is_per_mille_as_percent_without_floats() {
    // Derived: 925 per-mille = 92.5 percent; 1000 = 100.0; 5 = 0.5; 0 = 0.0.
    assert_eq!(field("x", None, 925).confidence_text(), "92.5%");
    assert_eq!(field("x", None, 1000).confidence_text(), "100.0%");
    assert_eq!(field("x", None, 5).confidence_text(), "0.5%");
    assert_eq!(field("x", None, 0).confidence_text(), "0.0%");
}

// ---- accept is disabled while the check blocks ----------------------------------------------

#[test]
fn accept_is_enabled_only_when_the_engine_check_is_ok() {
    // Card U11: Accept disabled while DraftCheck blocks.
    assert!(screen_with(ok_check()).can_accept());
    assert!(!screen_with(blocked_check()).can_accept());
    assert!(!screen_with(unsupported_check()).can_accept());
    assert!(!screen_with(EngineCheck::NotChecked).can_accept());
}

#[test]
fn accept_while_blocked_records_nothing() {
    let mut s = screen_with(blocked_check());
    assert!(!s.accept());
    assert_eq!(s.take_action(), None);
    let reason = s.accept_blocked_reason().unwrap();
    assert!(reason.contains("document_total"));
}

#[test]
fn accept_when_ok_is_recorded_once() {
    let mut s = screen_with(ok_check());
    assert!(s.accept());
    assert_eq!(s.take_action(), Some(PanelAction::Accept));
    assert_eq!(s.take_action(), None);
}

#[test]
fn ctrl_enter_accepts_only_when_ok() {
    let mut ok = screen_with(ok_check());
    frames(&mut ok, &ctrl_enter());
    assert_eq!(ok.take_action(), Some(PanelAction::Accept));

    let mut blocked = screen_with(blocked_check());
    frames(&mut blocked, &ctrl_enter());
    assert_eq!(blocked.take_action(), None);
}

#[test]
fn a_new_check_result_re_enables_or_disables_accept() {
    let mut s = screen_with(blocked_check());
    assert!(!s.can_accept());
    s.set_check(ok_check());
    assert!(s.can_accept());
    s.set_check(unsupported_check());
    assert!(!s.can_accept());
}

#[test]
fn a_disabled_accept_says_why_on_screen() {
    // Spec 7: no colour-only signals; the reason is words.
    let mut s = screen_with(blocked_check());
    let (_, texts) = frames(&mut s, &RawInput::default());
    assert!(has(&texts, "Accept is disabled."));
    assert!(has(&texts, "the engine computed a different total"));
}

#[test]
fn a_report_answer_has_nothing_to_accept() {
    let mut s = AssistScreen::new(Tier::Local);
    s.set_proposal(ProposalView::Report(ReportView {
        query: "trial_balance".to_owned(),
        answer: ReportAnswer::Pending,
    }));
    assert!(!s.can_accept());
    assert!(!s.accept());
}

#[test]
fn a_bank_match_and_a_mapping_can_be_accepted() {
    let view = SuggestionView {
        heading: "Row 4 matches".to_owned(),
        rows: vec![("Statement row".to_owned(), "4".to_owned())],
        reason: Some("amount and reference agree".to_owned()),
        confidence_permille: 870,
    };
    let mut s = AssistScreen::new(Tier::Cloud);
    s.set_proposal(ProposalView::BankMatch(view.clone()));
    assert!(s.can_accept());
    s.set_proposal(ProposalView::Mapping(view));
    assert!(s.can_accept());
}

// ---- tiers, requests, reject ----------------------------------------------------------------

#[test]
fn nothing_is_sent_while_the_assistant_is_off() {
    let mut s = AssistScreen::new(Tier::Off);
    s.set_input("paid rent 25k to Sharma Properties by HDFC");
    s.submit_input();
    assert_eq!(s.take_request(), None);
    assert_eq!(s.error_text(), Some(OFF_TEXT));
    assert!(!s.is_waiting());
}

#[test]
fn off_tier_cannot_accept_even_an_ok_draft() {
    let mut s = screen_with(ok_check());
    s.set_tier(Tier::Off);
    assert!(!s.can_accept());
}

#[test]
fn a_typed_request_is_sent_once_trimmed() {
    let mut s = AssistScreen::new(Tier::Local);
    s.set_input("  paid rent 25k to Sharma Properties by HDFC  ");
    s.submit_input();
    assert!(s.is_waiting());
    assert_eq!(
        s.take_request().as_deref(),
        Some("paid rent 25k to Sharma Properties by HDFC")
    );
    assert_eq!(s.take_request(), None);
}

#[test]
fn blank_input_sends_nothing() {
    let mut s = AssistScreen::new(Tier::Local);
    s.set_input("   ");
    s.submit_input();
    assert_eq!(s.take_request(), None);
    assert!(!s.is_waiting());
}

#[test]
fn an_answer_or_an_error_ends_the_wait() {
    let mut s = AssistScreen::new(Tier::Local);
    s.set_input("what is my cash balance");
    s.submit_input();
    s.set_error("network error: offline");
    assert!(!s.is_waiting());
    assert_eq!(s.error_text(), Some("network error: offline"));
    s.set_input("again");
    s.submit_input();
    s.set_proposal(ProposalView::Voucher(draft(EngineCheck::NotChecked)));
    assert!(!s.is_waiting());
    assert_eq!(s.error_text(), None);
}

#[test]
fn reject_records_the_decision_drops_the_proposal_and_closes() {
    let mut s = screen_with(ok_check());
    s.reject();
    assert_eq!(s.take_action(), Some(PanelAction::Reject));
    assert!(s.proposal().is_none());
    let (action, _) = frames(&mut s, &RawInput::default());
    assert!(matches!(action, ScreenAction::Pop));
}

// ---- smoke ----------------------------------------------------------------------------------

#[test]
fn every_state_draws_and_stays() {
    let mut empty = AssistScreen::new(Tier::Off);
    assert_eq!(empty.title(), "Assistant");
    let (action, texts) = frames(&mut empty, &RawInput::default());
    assert!(matches!(action, ScreenAction::Stay));
    assert!(has(&texts, "Nothing proposed yet."));

    for check in [
        EngineCheck::NotChecked,
        ok_check(),
        blocked_check(),
        unsupported_check(),
    ] {
        let mut s = screen_with(check);
        let (action, _) = frames(&mut s, &RawInput::default());
        assert!(matches!(action, ScreenAction::Stay));
    }
}
