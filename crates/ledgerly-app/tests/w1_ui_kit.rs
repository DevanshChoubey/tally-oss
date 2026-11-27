//! Card W1: the component kit. Expected values come from the mock v2 (`design/mock-v2/index.html`)
//! and the amount and date conventions in the mock (derived), not from this crate's output.

use std::cell::{Cell, RefCell};

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Ui, Vec2};
use ledgerly_app::i18n::Tr;
use ledgerly_app::theme::{self, ThemeKind, Tokens};
use ledgerly_app::ui::badge::{self, BadgeKind};
use ledgerly_app::ui::button::{self, ButtonKind};
use ledgerly_app::ui::conf::{self, Confidence};
use ledgerly_app::ui::empty;
use ledgerly_app::ui::format;
use ledgerly_app::ui::keycap;
use ledgerly_app::ui::note::{self, NoteKind};
use ledgerly_app::ui::page_head;
use ledgerly_app::ui::prop::{self, Prop, PropAction, PropState};
use ledgerly_app::ui::prov::{self, ProvKind};
use ledgerly_app::ui::tabs;
use ledgerly_core::{Date, Month, Paise};

/// Runs one frame of `body` in a context that already has the theme applied.
fn run(ctx: &egui::Context, events: Vec<Event>, body: impl FnMut(&mut Ui)) {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))),
        events,
        ..RawInput::default()
    };
    ctx.run_ui(input, body).drop_without_applying_deltas();
}

/// A context with `kind` applied and one warm-up frame, so fonts are bound.
fn themed(kind: ThemeKind) -> egui::Context {
    let ctx = egui::Context::default();
    theme::apply(&ctx, kind);
    run(&ctx, Vec::new(), |_ui| {});
    ctx
}

fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

#[test]
fn amounts_use_indian_grouping_and_two_decimals() {
    // derived: the mock's Indian grouping, `1,00,000.00`, with a leading minus for negatives.
    assert_eq!(format::amount(Paise::new(10_000_000)), "1,00,000.00");
    assert_eq!(format::amount(Paise::new(123_456_789)), "12,34,567.89");
    assert_eq!(format::amount(Paise::new(-5025)), "-50.25");
    assert_eq!(format::amount(Paise::new(0)), "0.00");
    assert_eq!(format::amount(Paise::new(99)), "0.99");
    assert_eq!(format::amount(Paise::new(100_000)), "1,000.00");
}

#[test]
fn dates_read_day_month_year_without_a_leading_zero() {
    // derived: the mock writes dates as `21 Nov 2026`; the day has no leading zero.
    let d = Date::from_calendar_date(2026, Month::November, 21).expect("valid date");
    assert_eq!(format::date(d), "21 Nov 2026");
    let d = Date::from_calendar_date(2026, Month::November, 5).expect("valid date");
    assert_eq!(format::date(d), "5 Nov 2026");
}

#[test]
fn badge_colours_pair_each_kind_with_its_soft_background() {
    // derived: the mock's `.badge` rules map each kind to a soft fill and an ink colour.
    for kind in ThemeKind::ALL {
        let t: Tokens = theme::tokens(kind);
        let cases = [
            (BadgeKind::Ok, (t.ok_soft, t.ok)),
            (BadgeKind::Warn, (t.warn_soft, t.warn)),
            (BadgeKind::Bad, (t.bad_soft, t.bad)),
            (BadgeKind::Accent, (t.accent_soft, t.accent_ink)),
            (BadgeKind::Ai, (t.ai_soft, t.ai_ink)),
            (BadgeKind::Neutral, (t.sunken, t.text_2)),
        ];
        for (badge_kind, want) in cases {
            assert_eq!(
                badge::colors(badge_kind, &t),
                want,
                "{kind:?} {badge_kind:?}"
            );
        }
    }
}

#[test]
fn confidence_bars_and_words_follow_the_mock() {
    // derived: the mock's `.conf` shows three bars for high, two for medium, one for low and none
    // for a deterministic rule.
    assert_eq!(conf::lit_bars(Confidence::High), 3);
    assert_eq!(conf::lit_bars(Confidence::Medium), 2);
    assert_eq!(conf::lit_bars(Confidence::Low), 1);
    assert_eq!(conf::lit_bars(Confidence::Rule), 0);
    assert_eq!(conf::word(Confidence::Rule), "Rule, deterministic");
    // provenance: index.html:962, `${CONF[l]} confidence` after the bars (CONF at :961).
    assert_eq!(conf::word(Confidence::High), "High confidence");
    assert_eq!(conf::word(Confidence::Medium), "Medium confidence");
    assert_eq!(conf::word(Confidence::Low), "Low confidence");
}

#[test]
fn provenance_labels_match_the_mock() {
    // derived: the mock's `.prov` labels, in lower case.
    assert_eq!(prov::label(ProvKind::FromBill), "from bill");
    assert_eq!(prov::label(ProvKind::Ai), "AI");
    assert_eq!(prov::label(ProvKind::YourWords), "your words");
    assert_eq!(prov::label(ProvKind::Engine), "engine");
    assert_eq!(prov::label(ProvKind::Books), "books");
    assert_eq!(prov::label(ProvKind::Rule), "rule");
}

#[test]
fn every_kit_component_draws_in_every_theme() {
    for kind in ThemeKind::ALL {
        let ctx = themed(kind);
        run(&ctx, Vec::new(), |ui| {
            for badge_kind in [
                BadgeKind::Ok,
                BadgeKind::Warn,
                BadgeKind::Bad,
                BadgeKind::Accent,
                BadgeKind::Ai,
                BadgeKind::Neutral,
            ] {
                badge::badge(ui, badge_kind, "Posted");
            }
            button::button(ui, "Save", Some("Ctrl+S"), ButtonKind::Primary);
            button::button(ui, "Cancel", None, ButtonKind::Secondary);
            button::button(ui, "Delete", Some("Del"), ButtonKind::Danger);
            keycap::keycap(ui, "F8");
            for note_kind in [
                NoteKind::Info,
                NoteKind::Ok,
                NoteKind::Warn,
                NoteKind::Bad,
                NoteKind::Ai,
            ] {
                note::note(ui, note_kind, "Check the GSTIN");
            }
            for prov_kind in [
                ProvKind::FromBill,
                ProvKind::Ai,
                ProvKind::YourWords,
                ProvKind::Engine,
                ProvKind::Books,
                ProvKind::Rule,
            ] {
                prov::prov(ui, prov_kind);
            }
            conf::conf(ui, Confidence::High, &Tr::default());
            conf::conf(ui, Confidence::Rule, &Tr::default());
            page_head::page_head(ui, "Vouchers", Some("3 this month"), |ui| {
                button::button(ui, "New sale", Some("F8"), ButtonKind::Primary);
            });
            let clicked = empty::empty_state(
                ui,
                "No vouchers yet",
                "Vouchers you post appear here.",
                Some(("New sale", "F8")),
            );
            assert!(!clicked, "nothing was clicked");
        });
    }
}

#[test]
fn proposal_keys_map_to_the_mock_shortcuts() {
    // derived: the mock's proposal keys are Ctrl+A to accept, Enter to edit, Alt+D to reject.
    assert_eq!(PropAction::Accept.key_press(), (Modifiers::CTRL, Key::A));
    assert_eq!(PropAction::Edit.key_press(), (Modifiers::NONE, Key::Enter));
    assert_eq!(PropAction::Reject.key_press(), (Modifiers::ALT, Key::D));
    assert_eq!(PropAction::Accept.shortcut(), "Ctrl+A");
    assert_eq!(PropAction::Edit.shortcut(), "Enter");
    assert_eq!(PropAction::Reject.shortcut(), "Alt+D");
}

#[test]
fn a_blocked_accept_never_fires() {
    // derived: the design rule that a check which blocks a proposal blocks Accept.
    assert_eq!(prop::resolve(PropAction::Accept, false), None);
    assert_eq!(
        prop::resolve(PropAction::Accept, true),
        Some(PropAction::Accept)
    );
    assert_eq!(
        prop::resolve(PropAction::Edit, false),
        Some(PropAction::Edit)
    );
    assert_eq!(
        prop::resolve(PropAction::Reject, false),
        Some(PropAction::Reject)
    );
}

#[test]
fn ctrl_a_takes_the_accept_key_when_the_card_handles_keys() {
    let ctx = themed(ThemeKind::Light);
    let got: Cell<Option<PropAction>> = Cell::new(None);
    run(&ctx, vec![key(Key::A, Modifiers::CTRL)], |ui| {
        let action =
            Prop::new("Voucher 1042")
                .handles_keys(true)
                .show(ui, |_ui| {}, |_ui| {}, |_ui| {});
        got.set(action);
    });
    assert_eq!(got.get(), Some(PropAction::Accept));
}

#[test]
fn ctrl_a_does_not_save_a_blocked_proposal() {
    let ctx = themed(ThemeKind::Light);
    let got: Cell<Option<PropAction>> = Cell::new(Some(PropAction::Edit));
    run(&ctx, vec![key(Key::A, Modifiers::CTRL)], |ui| {
        let action = Prop::new("Voucher 1042")
            .accept(false, Some("Debit and credit differ"))
            .handles_keys(true)
            .show(ui, |_ui| {}, |_ui| {}, |_ui| {});
        got.set(action);
    });
    assert_eq!(got.get(), None);
}

#[test]
fn keys_are_ignored_when_the_card_does_not_handle_them() {
    let ctx = themed(ThemeKind::Light);
    let got: Cell<Option<PropAction>> = Cell::new(Some(PropAction::Edit));
    run(&ctx, vec![key(Key::A, Modifiers::CTRL)], |ui| {
        let action = Prop::new("Voucher 1042").show(ui, |_ui| {}, |_ui| {}, |_ui| {});
        got.set(action);
    });
    assert_eq!(got.get(), None);
}

#[test]
fn a_settled_proposal_takes_no_keys() {
    let ctx = themed(ThemeKind::Light);
    let got: Cell<Option<PropAction>> = Cell::new(Some(PropAction::Edit));
    run(&ctx, vec![key(Key::Enter, Modifiers::NONE)], |ui| {
        let action = Prop::new("Voucher 1042")
            .state(PropState::Done)
            .handles_keys(true)
            .show(ui, |_ui| {}, |_ui| {}, |_ui| {});
        got.set(action);
    });
    assert_eq!(got.get(), None);
}

#[test]
fn tabs_report_the_tab_that_was_clicked() {
    // Frame 1 records where each tab sits. Frame 2 moves the pointer over tab 1 and presses; frame
    // 3 releases, which is the click.
    let ctx = themed(ThemeKind::Light);
    let labels = ["Ledger", "Stock", "Reports"];
    let rects: RefCell<Vec<Rect>> = RefCell::new(Vec::new());
    let clicked: Cell<Option<usize>> = Cell::new(None);
    let draw = |ui: &mut Ui| {
        let response = tabs::tabs(ui, &labels, 0);
        if !response.rects.is_empty() {
            rects.replace(response.rects.clone());
        }
        clicked.set(response.clicked);
    };
    run(&ctx, Vec::new(), draw);
    let target = rects.borrow().get(1).copied().expect("tab 1 was drawn");
    let at = target.center();
    run(
        &ctx,
        vec![
            Event::PointerMoved(at),
            Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
        draw,
    );
    run(
        &ctx,
        vec![Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
        draw,
    );
    assert_eq!(clicked.get(), Some(1));
}

#[test]
fn an_empty_state_without_an_action_draws_without_a_button() {
    let ctx = themed(ThemeKind::Dark);
    run(&ctx, Vec::new(), |ui| {
        let clicked = empty::empty_state(ui, "Nothing here", "Add a bill to start.", None);
        assert!(!clicked);
    });
}

#[test]
fn the_page_head_leaves_16_points_before_what_follows() {
    // provenance: design/mock-v2/index.html:121: `.phead` has `margin-bottom:16px`, so the
    // first thing under the head starts 16 px below it, on every screen. Derived: the label drawn
    // after the head starts 16 points below the bottom of everything the head drew.
    let ctx = themed(ThemeKind::Light);
    let gap = Cell::new(0.0_f32);
    run(&ctx, Vec::new(), |ui| {
        ui.vertical(|ui| {
            let head = page_head::page_head(ui, "Vouchers", Some("3 this month"), |ui| {
                button::button(ui, "New sale", Some("F8"), ButtonKind::Primary);
            });
            let head_bottom = head.bottom();
            let next = ui.label("first line").rect;
            gap.set(next.top() - head_bottom);
        });
    });
    assert!((gap.get() - 16.0).abs() < 0.5, "gap {}", gap.get());
}
