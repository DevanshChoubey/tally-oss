//! Card U9: bank reconciliation screen. Spec 6.5 is the source of every expected value. The
//! statement, vouchers and suggestions are a small fictional fixture built here (no real account
//! numbers or names), modelled on the mock's "Bank matching" tab.
//!
//! Provenance, `derived`: spec 6.5 says "only user-confirmed matches set the bank date",
//! "many-to-many matches" and "kinds Exact | Rule | AiSuggested | Manual". Spec 7 says Esc goes
//! back. Amount text is derived by hand from the Indian digit grouping (last three digits, then
//! pairs), for example 14_738_450 paise = Rs 147384.50 = `1,47,384.50`.
//!
//! Pending (no provenance, so the screen offers nothing): creating a new voucher from a
//! statement line (the mock's "Create" suggestion) needs the engine to build and check the
//! voucher; the screen has no such action. A manual match over several statement lines needs a
//! rule for the bank date, which no source gives; manual matches take one line.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_app::screens::bank::{
    BankScreen, BankScreenError, BrsRow, BrsView, Confidence, Flow, LineStatus, OpenVoucherView,
    StatementLineView, SuggestionView, Tab, format_flow, format_line_date, kind_label,
};
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{BankStatementId, Date, Outcome, Paise, VoucherId};
use ledgerly_model::MatchKind;
use time::macros::date;

const STMT: BankStatementId = BankStatementId(7);

fn line(no: u32, d: Date, desc: &str, paise: i64, flow: Flow) -> StatementLineView {
    StatementLineView {
        line_no: no,
        date: d,
        description: desc.to_owned(),
        amount: Paise::new(paise),
        flow,
    }
}

fn lines() -> Vec<StatementLineView> {
    vec![
        line(
            1,
            date!(2026 - 11 - 20),
            "NEFT CR KISAN SEVA",
            14_738_450,
            Flow::In,
        ),
        line(
            2,
            date!(2026 - 11 - 20),
            "CHQ PAID 004512",
            5_000_000,
            Flow::Out,
        ),
        line(
            3,
            date!(2026 - 11 - 19),
            "NEFT CR MEHTA AGRO",
            20_000_000,
            Flow::In,
        ),
        line(4, date!(2026 - 11 - 18), "UPI RENT", 2_500_000, Flow::Out),
        line(
            5,
            date!(2026 - 11 - 17),
            "NEFT CR PART ONE",
            100_000,
            Flow::In,
        ),
        line(
            6,
            date!(2026 - 11 - 17),
            "NEFT CR PART TWO",
            200_000,
            Flow::In,
        ),
    ]
}

fn suggestion(
    ls: &[u32],
    vs: &[u64],
    kind: MatchKind,
    bank_date: Date,
    confidence: Confidence,
) -> SuggestionView {
    SuggestionView {
        lines: ls.to_vec(),
        vouchers: vs.iter().map(|v| VoucherId(*v)).collect(),
        voucher_label: format!("vouchers {vs:?}"),
        reason: "test reason".to_owned(),
        confidence,
        kind,
        bank_date,
    }
}

fn suggestions() -> Vec<SuggestionView> {
    vec![
        suggestion(
            &[1],
            &[219],
            MatchKind::Exact,
            date!(2026 - 11 - 20),
            Confidence::High,
        ),
        suggestion(
            &[2],
            &[318],
            MatchKind::Rule,
            date!(2026 - 11 - 20),
            Confidence::High,
        ),
        suggestion(
            &[4],
            &[315],
            MatchKind::AiSuggested,
            date!(2026 - 11 - 18),
            Confidence::Low,
        ),
        suggestion(
            &[5, 6],
            &[400, 401],
            MatchKind::Exact,
            date!(2026 - 11 - 17),
            Confidence::High,
        ),
    ]
}

fn open(v: u64, label: &str, paise: i64, flow: Flow) -> OpenVoucherView {
    OpenVoucherView {
        voucher: VoucherId(v),
        date: date!(2026 - 11 - 19),
        label: label.to_owned(),
        amount: Paise::new(paise),
        flow,
    }
}

fn open_vouchers() -> Vec<OpenVoucherView> {
    vec![
        open(219, "Receipt/0219", 14_738_450, Flow::In),
        open(318, "Payment/0318", 5_000_000, Flow::Out),
        open(315, "Payment/0315", 2_500_000, Flow::Out),
        open(400, "Receipt/0400", 100_000, Flow::In),
        open(401, "Receipt/0401", 200_000, Flow::In),
        open(500, "Receipt/0500", 20_000_000, Flow::In),
    ]
}

fn screen() -> BankScreen {
    BankScreen::new(
        "HDFC Bank A/c",
        STMT,
        lines(),
        suggestions(),
        open_vouchers(),
    )
    .unwrap()
}

// ---- construction -----------------------------------------------------------------------------

#[test]
fn a_new_screen_has_every_line_waiting_and_nothing_confirmed() {
    let mut s = screen();
    assert_eq!(s.title(), "Bank reconciliation: HDFC Bank A/c");
    assert_eq!(s.pending_count(), 6);
    assert_eq!(s.focused_line_no(), Some(1));
    assert_eq!(s.tab(), Tab::Matching);
    // spec 6.5: only user-confirmed matches set the bank date, so nothing is confirmed on open.
    assert_eq!(s.take_confirmed(), Vec::new());
}

#[test]
fn construction_rejects_bad_input_with_a_typed_error() {
    let err = |ls, ss, vs| BankScreen::new("B", STMT, ls, ss, vs).err().unwrap();
    assert_eq!(
        err(Vec::new(), Vec::new(), Vec::new()),
        BankScreenError::NoLines
    );

    let mut dup = lines();
    dup.push(line(3, date!(2026 - 11 - 01), "x", 1, Flow::In));
    assert_eq!(
        err(dup, Vec::new(), Vec::new()),
        BankScreenError::DuplicateLine { line_no: 3 }
    );

    let unknown = vec![suggestion(
        &[99],
        &[1],
        MatchKind::Exact,
        date!(2026 - 11 - 01),
        Confidence::High,
    )];
    assert_eq!(
        err(lines(), unknown, Vec::new()),
        BankScreenError::UnknownLine { line_no: 99 }
    );

    for bad in [
        suggestion(
            &[],
            &[1],
            MatchKind::Exact,
            date!(2026 - 11 - 01),
            Confidence::High,
        ),
        suggestion(
            &[1],
            &[],
            MatchKind::Exact,
            date!(2026 - 11 - 01),
            Confidence::High,
        ),
        suggestion(
            &[1, 1],
            &[1],
            MatchKind::Exact,
            date!(2026 - 11 - 01),
            Confidence::High,
        ),
        suggestion(
            &[1],
            &[2, 2],
            MatchKind::Exact,
            date!(2026 - 11 - 01),
            Confidence::High,
        ),
    ] {
        assert_eq!(
            err(lines(), vec![bad], Vec::new()),
            BankScreenError::MalformedSuggestion
        );
    }

    // spec 6.5: Manual is the user's kind; a matcher may not claim it.
    let manual = vec![suggestion(
        &[1],
        &[1],
        MatchKind::Manual,
        date!(2026 - 11 - 01),
        Confidence::High,
    )];
    assert_eq!(
        err(lines(), manual, Vec::new()),
        BankScreenError::ManualSuggestion
    );

    let twice = vec![open(5, "a", 1, Flow::In), open(5, "b", 1, Flow::In)];
    assert_eq!(
        err(lines(), Vec::new(), twice),
        BankScreenError::DuplicateVoucher {
            voucher: VoucherId(5)
        }
    );
}

// ---- accepting --------------------------------------------------------------------------------

#[test]
fn accepting_records_the_suggestions_kind_date_and_vouchers() {
    let mut s = screen();
    s.accept_focused().unwrap();
    let confirmed = s.take_confirmed();
    assert_eq!(confirmed.len(), 1);
    let m = confirmed.first().unwrap();
    // spec 6.5: statement id, the statement line number and the voucher come from the suggestion
    // the user accepted; the kind is the suggestion's kind; the bank date is the one it carried.
    assert_eq!(m.statement, STMT);
    assert_eq!(m.statement_lines, vec![1]);
    assert_eq!(m.vouchers, vec![VoucherId(219)]);
    assert_eq!(m.kind, MatchKind::Exact);
    assert_eq!(m.bank_date, date!(2026 - 11 - 20));
    assert_eq!(s.status(1), Some(LineStatus::Matched));
    // The outbox is drained: the same match is never handed over twice.
    assert_eq!(s.take_confirmed(), Vec::new());
}

#[test]
fn accepting_moves_focus_to_the_next_waiting_line() {
    let mut s = screen();
    s.accept_focused().unwrap();
    assert_eq!(s.focused_line_no(), Some(2));
    s.accept_focused().unwrap();
    // Line 3 has no suggestion, so focus stops there; it is still waiting.
    assert_eq!(s.focused_line_no(), Some(3));
}

#[test]
fn a_line_without_a_suggestion_cannot_be_accepted_and_records_nothing() {
    let mut s = screen();
    s.focus_next();
    s.focus_next();
    assert_eq!(s.focused_line_no(), Some(3));
    assert_eq!(
        s.accept_focused(),
        Err(BankScreenError::NoSuggestion { line_no: 3 })
    );
    assert_eq!(
        s.last_error(),
        Some(&BankScreenError::NoSuggestion { line_no: 3 })
    );
    assert_eq!(s.take_confirmed(), Vec::new());
    assert_eq!(s.status(3), Some(LineStatus::Pending));
}

#[test]
fn a_matched_line_cannot_be_matched_again() {
    let mut s = screen();
    s.accept_focused().unwrap();
    s.focus_prev();
    assert_eq!(s.focused_line_no(), Some(1));
    assert_eq!(
        s.accept_focused(),
        Err(BankScreenError::LineNotPending { line_no: 1 })
    );
    assert_eq!(s.take_confirmed().len(), 1);
}

#[test]
fn a_many_to_many_suggestion_matches_all_its_lines_and_vouchers_at_once() {
    let mut s = screen();
    for _ in 0..4 {
        s.focus_next();
    }
    assert_eq!(s.focused_line_no(), Some(5));
    s.accept_focused().unwrap();
    let m = s.take_confirmed().pop().unwrap();
    // spec 6.5: many-to-many matches. Both statement lines and both vouchers are in one match.
    assert_eq!(m.statement_lines, vec![5, 6]);
    assert_eq!(m.vouchers, vec![VoucherId(400), VoucherId(401)]);
    assert_eq!(s.status(5), Some(LineStatus::Matched));
    assert_eq!(s.status(6), Some(LineStatus::Matched));
}

#[test]
fn a_voucher_in_a_confirmed_match_is_not_suggested_again() {
    let mut ss = suggestions();
    ss.push(suggestion(
        &[3],
        &[219],
        MatchKind::AiSuggested,
        date!(2026 - 11 - 19),
        Confidence::Low,
    ));
    let mut s = BankScreen::new("B", STMT, lines(), ss, open_vouchers()).unwrap();
    assert!(s.suggestion_for(3).is_some());
    s.accept_focused().unwrap();
    // Voucher 219 now belongs to a confirmed match, so line 3's suggestion is withdrawn.
    assert!(s.suggestion_for(3).is_none());
    assert!(
        s.manual_candidates()
            .iter()
            .all(|v| v.voucher != VoucherId(219))
    );
}

#[test]
fn an_ai_suggestion_keeps_its_kind_and_is_labelled_as_proposed() {
    let mut s = screen();
    for _ in 0..3 {
        s.focus_next();
    }
    assert_eq!(s.focused_line_no(), Some(4));
    assert_eq!(s.suggestion_for(4).unwrap().kind, MatchKind::AiSuggested);
    assert_eq!(kind_label(MatchKind::AiSuggested), "AI proposed");
    s.accept_focused().unwrap();
    assert_eq!(
        s.take_confirmed().first().unwrap().kind,
        MatchKind::AiSuggested
    );
}

// ---- skipping ---------------------------------------------------------------------------------

#[test]
fn skip_sets_a_line_aside_and_a_second_skip_puts_it_back() {
    let mut s = screen();
    s.skip_focused().unwrap();
    assert_eq!(s.status(1), Some(LineStatus::Skipped));
    assert_eq!(s.pending_count(), 5);
    assert_eq!(s.focused_line_no(), Some(2));
    s.focus_prev();
    s.skip_focused().unwrap();
    assert_eq!(s.status(1), Some(LineStatus::Pending));
    assert_eq!(s.take_confirmed(), Vec::new());
}

#[test]
fn a_matched_line_cannot_be_skipped() {
    let mut s = screen();
    s.accept_focused().unwrap();
    s.focus_prev();
    assert_eq!(
        s.skip_focused(),
        Err(BankScreenError::CannotSkipMatched { line_no: 1 })
    );
    assert_eq!(s.status(1), Some(LineStatus::Matched));
}

// ---- manual match -----------------------------------------------------------------------------

#[test]
fn a_manual_match_uses_the_statement_lines_date_and_the_manual_kind() {
    let mut s = screen();
    s.focus_next();
    s.focus_next();
    assert_eq!(s.focused_line_no(), Some(3));
    s.toggle_manual();
    assert!(s.is_manual_open());
    s.toggle_voucher(VoucherId(500)).unwrap();
    assert_eq!(s.selected_vouchers(), vec![VoucherId(500)]);
    s.confirm_manual().unwrap();
    let m = s.take_confirmed().pop().unwrap();
    // spec 6.5: kind Manual; the bank date is the statement line's own date (2026-11-19).
    assert_eq!(m.kind, MatchKind::Manual);
    assert_eq!(m.statement_lines, vec![3]);
    assert_eq!(m.vouchers, vec![VoucherId(500)]);
    assert_eq!(m.bank_date, date!(2026 - 11 - 19));
    assert!(!s.is_manual_open());
    assert_eq!(s.status(3), Some(LineStatus::Matched));
}

#[test]
fn a_manual_match_needs_a_voucher_and_refuses_unknown_or_used_ones() {
    let mut s = screen();
    assert_eq!(s.confirm_manual(), Err(BankScreenError::NoVoucherSelected));
    assert_eq!(s.take_confirmed(), Vec::new());
    assert_eq!(
        s.toggle_voucher(VoucherId(9999)),
        Err(BankScreenError::UnknownVoucher {
            voucher: VoucherId(9999)
        })
    );
    s.accept_focused().unwrap();
    assert_eq!(
        s.toggle_voucher(VoucherId(219)),
        Err(BankScreenError::VoucherAlreadyMatched {
            voucher: VoucherId(219)
        })
    );
}

#[test]
fn choosing_a_voucher_twice_takes_it_back() {
    let mut s = screen();
    s.toggle_voucher(VoucherId(500)).unwrap();
    s.toggle_voucher(VoucherId(500)).unwrap();
    assert_eq!(s.selected_vouchers(), Vec::new());
}

// ---- statement (BRS) --------------------------------------------------------------------------

#[test]
fn the_statement_is_unsupported_until_the_engine_supplies_it() {
    let mut s = screen();
    assert!(s.brs().is_unsupported());
    s.set_brs(Outcome::Value(BrsView {
        as_of: date!(2026 - 11 - 20),
        rows: vec![BrsRow {
            label: "Balance as per books".to_owned(),
            amount: Paise::new(123_456_700),
        }],
    }));
    assert!(matches!(s.brs(), Outcome::Value(v) if v.rows.len() == 1));
}

// ---- text -------------------------------------------------------------------------------------

#[test]
fn amounts_and_dates_are_formatted_the_indian_way() {
    // 14_738_450 paise = Rs 147384.50; grouped 3 then 2s: 1,47,384.50.
    assert_eq!(
        format_flow(Paise::new(14_738_450), Flow::In),
        "+ 1,47,384.50"
    );
    // 5_000_000 paise = Rs 50000.00: 50,000.00.
    assert_eq!(format_flow(Paise::new(5_000_000), Flow::Out), "- 50,000.00");
    assert_eq!(format_line_date(date!(2026 - 11 - 05)), "05-11-2026");
}

// ---- keys and drawing -------------------------------------------------------------------------

fn frame(ctx: &egui::Context, s: &mut BankScreen, input: RawInput) -> (ScreenAction, bool) {
    let mut app = AppCtx::default();
    let mut action = ScreenAction::Stay;
    let mut esc_left = false;
    let out = ctx.run_ui(input, |ui| {
        action = s.ui(ui, &mut app);
        esc_left = ui.input(|i| i.key_pressed(Key::Escape));
    });
    out.drop_without_applying_deltas();
    (action, esc_left)
}

fn key(key: Key) -> RawInput {
    RawInput {
        events: vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        ..RawInput::default()
    }
}

#[test]
fn the_screen_draws_on_both_tabs() {
    let ctx = egui::Context::default();
    let mut s = screen();
    let (action, _) = frame(&ctx, &mut s, RawInput::default());
    assert!(matches!(action, ScreenAction::Stay));
    s.set_tab(Tab::Statement);
    let (action, _) = frame(&ctx, &mut s, RawInput::default());
    assert!(matches!(action, ScreenAction::Stay));
}

#[test]
fn enter_accepts_and_s_skips_and_j_k_move() {
    let ctx = egui::Context::default();
    let mut s = screen();
    frame(&ctx, &mut s, RawInput::default());
    frame(&ctx, &mut s, key(Key::Enter));
    assert_eq!(s.status(1), Some(LineStatus::Matched));
    assert_eq!(s.take_confirmed().len(), 1);
    frame(&ctx, &mut s, key(Key::S));
    assert_eq!(s.status(2), Some(LineStatus::Skipped));
    assert_eq!(s.focused_line_no(), Some(3));
    frame(&ctx, &mut s, key(Key::J));
    assert_eq!(s.focused_line_no(), Some(4));
    frame(&ctx, &mut s, key(Key::K));
    assert_eq!(s.focused_line_no(), Some(3));
}

#[test]
fn b_switches_tabs_and_m_opens_the_manual_panel() {
    let ctx = egui::Context::default();
    let mut s = screen();
    frame(&ctx, &mut s, RawInput::default());
    frame(&ctx, &mut s, key(Key::B));
    assert_eq!(s.tab(), Tab::Statement);
    frame(&ctx, &mut s, key(Key::B));
    assert_eq!(s.tab(), Tab::Matching);
    frame(&ctx, &mut s, key(Key::M));
    assert!(s.is_manual_open());
}

#[test]
fn escape_is_left_to_the_shell_unless_the_manual_panel_is_open() {
    // spec 7: Esc goes back. The shell pops on an Esc no screen consumed.
    let ctx = egui::Context::default();
    let mut s = screen();
    frame(&ctx, &mut s, RawInput::default());
    let (action, esc_left) = frame(&ctx, &mut s, key(Key::Escape));
    assert!(matches!(action, ScreenAction::Stay));
    assert!(esc_left);
    s.toggle_manual();
    let (_, esc_left) = frame(&ctx, &mut s, key(Key::Escape));
    assert!(!esc_left);
    assert!(!s.is_manual_open());
}

#[test]
fn in_the_manual_panel_space_chooses_and_enter_confirms() {
    let ctx = egui::Context::default();
    let mut s = screen();
    frame(&ctx, &mut s, RawInput::default());
    s.toggle_manual();
    // The first candidate is voucher 219 (open vouchers are listed in the order given).
    frame(&ctx, &mut s, key(Key::Space));
    assert_eq!(s.selected_vouchers(), vec![VoucherId(219)]);
    frame(&ctx, &mut s, key(Key::Enter));
    let m = s.take_confirmed().pop().unwrap();
    assert_eq!(m.kind, MatchKind::Manual);
    assert_eq!(m.statement_lines, vec![1]);
}
