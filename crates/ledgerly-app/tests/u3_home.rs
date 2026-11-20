//! Card U3: the home screen. Expected values are derived by hand and the derivation sits next to
//! each one; nothing is copied from this crate's output. The figures on a tile are handed in as
//! `Outcome<Paise>` (the host takes them from report outputs), so these tests check that the screen
//! shows exactly what it is given, formatted with Indian grouping (spec 7), and shows no number
//! for an unsupported report (engineering rules A1 and C4). The key list is the mock's
//! (`design/mock/index.html`, the "Open (when not typing)" help table).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_app::screens::home::{
    AttentionItem, AttentionKind, HomeData, HomeScreen, HomeTarget, Kpi, kpi_view, shortcut_target,
    value_text,
};
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{Outcome, Paise};

fn paise(v: i64) -> Paise {
    Paise::new(v)
}

fn kpi(label: &str, value: Outcome<Paise>, foot: &str, target: HomeTarget) -> Kpi {
    Kpi {
        label: label.to_owned(),
        value,
        footnote: foot.to_owned(),
        target,
    }
}

fn bs(node: &str) -> HomeTarget {
    HomeTarget::BalanceSheet {
        node: node.to_owned(),
    }
}

fn sample() -> HomeData {
    HomeData {
        company_name: "Example Traders LLP".to_owned(),
        subtitle: "Books are open.".to_owned(),
        kpis: vec![
            kpi(
                "Cash and bank",
                Outcome::Value(paise(123_456_700)),
                "2 bank accounts, 1 cash",
                bs("Cash-in-hand"),
            ),
            kpi(
                "Receivables",
                Outcome::Value(paise(4_500_000)),
                "4 parties",
                bs("Sundry debtors"),
            ),
            kpi(
                "GST payable",
                Outcome::unsupported("GST payable", "needs the GSTR-3B builder"),
                "after ITC",
                HomeTarget::GstReturns {
                    tab: "gstr3b".to_owned(),
                },
            ),
        ],
        attention: vec![
            AttentionItem {
                kind: AttentionKind::Pending,
                headline: "4 invoices in IMS are pending".to_owned(),
                detail: "They stay out of ITC until accepted.".to_owned(),
                target: Some(HomeTarget::PendingIms),
            },
            AttentionItem {
                kind: AttentionKind::Done,
                headline: "GSTR-3B for October filed".to_owned(),
                detail: String::new(),
                target: None,
            },
        ],
    }
}

fn key(key: Key, modifiers: Modifiers) -> RawInput {
    RawInput {
        events: vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..RawInput::default()
    }
}

fn frame(ctx: &egui::Context, screen: &mut HomeScreen, input: RawInput) -> ScreenAction {
    let mut app = AppCtx::default();
    let mut action = ScreenAction::Pop;
    let out = ctx.run_ui(input, |ui| {
        action = screen.ui(ui, &mut app);
    });
    out.drop_without_applying_deltas();
    action
}

// ---- figures ---------------------------------------------------------------------------------

#[test]
fn value_is_shown_with_indian_grouping() {
    // provenance: derived. Spec 7 gives the grouping `12,34,567.00`: the last three digits form a
    // group, then pairs. Rupees 12,34,567 = 1_234_567 rupees = 1_234_567 * 100 = 123_456_700 paise.
    assert_eq!(
        value_text(&Outcome::Value(paise(123_456_700))),
        "\u{20B9}12,34,567.00"
    );
}

#[test]
fn small_and_zero_values() {
    // provenance: derived. 1_500 paise = 15 rupees exactly; zero is `0.00` (two decimals, spec 7).
    assert_eq!(value_text(&Outcome::Value(paise(1_500))), "\u{20B9}15.00");
    assert_eq!(value_text(&Outcome::Value(paise(0))), "\u{20B9}0.00");
}

#[test]
fn paise_part_is_kept() {
    // provenance: derived. 100_000_05 paise = 1,00,000 rupees and 5 paise -> `1,00,000.05`.
    assert_eq!(
        value_text(&Outcome::Value(paise(10_000_005))),
        "\u{20B9}1,00,000.05"
    );
}

#[test]
fn unsupported_tile_shows_words_and_reason_never_a_number() {
    // Rule A1: a value, Unsupported, or an error; an unsupported report gives no figure.
    let k = kpi(
        "GST payable",
        Outcome::unsupported("GSTR-3B liability", "rate pack pending"),
        "after ITC",
        HomeTarget::GstReturns { tab: String::new() },
    );
    let v = kpi_view(&k);
    assert_eq!(v.value_text, "Not supported yet");
    assert!(!v.value_text.chars().any(|c| c.is_ascii_digit()));
    assert!(!v.value_text.contains('\u{20B9}'));
    assert_eq!(v.footnote_text, "GSTR-3B liability: rate pack pending");
}

#[test]
fn supported_tile_keeps_its_footnote_and_label() {
    let k = kpi(
        "Receivables",
        Outcome::Value(paise(4_500_000)),
        "4 parties, oldest 38 days",
        bs("Sundry debtors"),
    );
    let v = kpi_view(&k);
    assert_eq!(v.label, "Receivables");
    // provenance: derived. 4_500_000 paise = 45,000 rupees -> `45,000.00`.
    assert_eq!(v.value_text, "\u{20B9}45,000.00");
    assert_eq!(v.footnote_text, "4 parties, oldest 38 days");
}

#[test]
fn views_follow_the_given_order_and_count() {
    let screen = HomeScreen::new(sample());
    let views = screen.kpi_views();
    let labels: Vec<&str> = views.iter().map(|v| v.label.as_str()).collect();
    assert_eq!(labels, vec!["Cash and bank", "Receivables", "GST payable"]);
    assert_eq!(views.first().unwrap().value_text, "\u{20B9}12,34,567.00");
    assert_eq!(views.get(2).unwrap().value_text, "Not supported yet");
}

// ---- keys and requests ----------------------------------------------------------------------------

#[test]
fn shortcut_table_matches_the_mock() {
    // provenance: derived from the mock's help table ("Open (when not typing)"): Home H, Sales
    // invoice F8, Payment F5, Receipt F6, Purchase from bill F9, Balance sheet B, Day book D,
    // GST returns G, Masters M; and Ctrl+K opens the palette (spec 7).
    let none = Modifiers::NONE;
    assert_eq!(
        shortcut_target(Key::F8, none),
        Some(HomeTarget::SalesInvoice)
    );
    assert_eq!(shortcut_target(Key::F6, none), Some(HomeTarget::Receipt));
    assert_eq!(shortcut_target(Key::F5, none), Some(HomeTarget::Payment));
    assert_eq!(
        shortcut_target(Key::F9, none),
        Some(HomeTarget::PurchaseFromBill)
    );
    assert_eq!(shortcut_target(Key::B, none), Some(bs("")));
    assert_eq!(shortcut_target(Key::D, none), Some(HomeTarget::DayBook));
    assert_eq!(
        shortcut_target(Key::G, none),
        Some(HomeTarget::GstReturns { tab: String::new() })
    );
    assert_eq!(shortcut_target(Key::M, none), Some(HomeTarget::Masters));
    assert_eq!(
        shortcut_target(Key::K, Modifiers::COMMAND),
        Some(HomeTarget::Palette)
    );
}

#[test]
fn other_keys_and_modifier_combinations_open_nothing() {
    assert_eq!(shortcut_target(Key::X, Modifiers::NONE), None);
    assert_eq!(shortcut_target(Key::F8, Modifiers::CTRL), None);
    assert_eq!(shortcut_target(Key::K, Modifiers::NONE), None);
}

#[test]
fn f8_in_a_frame_requests_the_sales_invoice_once() {
    let ctx = egui::Context::default();
    let mut screen = HomeScreen::new(sample());
    let action = frame(&ctx, &mut screen, key(Key::F8, Modifiers::NONE));
    assert!(matches!(action, ScreenAction::Stay));
    assert_eq!(screen.take_request(), Some(HomeTarget::SalesInvoice));
    assert_eq!(screen.take_request(), None);
}

#[test]
fn letter_keys_request_their_screens() {
    let ctx = egui::Context::default();
    let mut screen = HomeScreen::new(sample());
    frame(&ctx, &mut screen, key(Key::D, Modifiers::NONE));
    assert_eq!(screen.take_request(), Some(HomeTarget::DayBook));
    frame(&ctx, &mut screen, key(Key::M, Modifiers::NONE));
    assert_eq!(screen.take_request(), Some(HomeTarget::Masters));
}

#[test]
fn ctrl_k_requests_the_palette() {
    let ctx = egui::Context::default();
    let mut screen = HomeScreen::new(sample());
    frame(&ctx, &mut screen, key(Key::K, Modifiers::COMMAND));
    assert_eq!(screen.take_request(), Some(HomeTarget::Palette));
}

#[test]
fn a_frame_without_keys_requests_nothing() {
    let ctx = egui::Context::default();
    let mut screen = HomeScreen::new(sample());
    frame(&ctx, &mut screen, RawInput::default());
    assert_eq!(screen.take_request(), None);
}

#[test]
fn latest_request_wins_and_set_data_keeps_it() {
    let mut screen = HomeScreen::new(sample());
    screen.request_open(HomeTarget::DayBook);
    screen.request_open(HomeTarget::Masters);
    screen.set_data(HomeData::default());
    assert_eq!(screen.take_request(), Some(HomeTarget::Masters));
    assert!(screen.data().kpis.is_empty());
}

// ---- drawing --------------------------------------------------------------------------------

#[test]
fn title_is_home() {
    assert_eq!(HomeScreen::default().title(), "Home");
}

#[test]
fn full_and_empty_screens_draw_without_panicking() {
    let ctx = egui::Context::default();
    let mut full = HomeScreen::new(sample());
    assert!(matches!(
        frame(&ctx, &mut full, RawInput::default()),
        ScreenAction::Stay
    ));
    let mut empty = HomeScreen::default();
    assert!(matches!(
        frame(&ctx, &mut empty, RawInput::default()),
        ScreenAction::Stay
    ));
}

#[test]
fn attention_kinds_carry_distinct_words() {
    // Spec 7: no colour-only signals, so each kind has its own word.
    assert_eq!(AttentionKind::Pending.word(), "Pending");
    assert_eq!(AttentionKind::Due.word(), "Due");
    assert_eq!(AttentionKind::Done.word(), "Done");
}
