//! Card S9 (cold start): the refusals, shortcuts, tab strip and import entry a person meets with
//! no company open. Written before the code (see `design/REAL-PATH-AUDIT.md`, items 1 to 4).
//!
//! Provenance: the mock, `design/mock-v2/index.html`: the toast (CSS 290-292, `toast()` at 920:
//! a dark pill centred above the key bar, with an optional underlined action button) is how the
//! mock shows a message such as "F12 opens options for this screen." (1004). The mock's welcome
//! card "Import from another accounting app" (1864) is a choice like the other three. The wording
//! is plain words by the owner's brief for this round (no screen names in brackets, no "Home").

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use egui::epaint::{ClippedShape, Shape};
use egui::{Context, Event, Key, Modifiers, Pos2, RawInput, Rect, vec2};
use ledgerly_app::screens::setup::SetupScreen;
use ledgerly_app::shell::{Shell, unique_tab_labels};
use ledgerly_app::theme::{self, ThemeKind};

/// Every text a frame painted, with where it starts.
fn texts(shapes: &[ClippedShape]) -> Vec<(String, Pos2)> {
    fn walk(shape: &Shape, out: &mut Vec<(String, Pos2)>) {
        match shape {
            Shape::Text(t) => out.push((t.galley.text().to_owned(), t.pos)),
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            Shape::Noop
            | Shape::Circle(_)
            | Shape::Ellipse(_)
            | Shape::LineSegment { .. }
            | Shape::Path(_)
            | Shape::Rect(_)
            | Shape::Mesh(_)
            | Shape::QuadraticBezier(_)
            | Shape::CubicBezier(_)
            | Shape::Band(_)
            | Shape::Callback(_) => {}
        }
    }
    let mut out = Vec::new();
    for clipped in shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

fn key_event(k: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn frame_at(
    ctx: &Context,
    shell: &mut Shell,
    events: Vec<Event>,
    time: Option<f64>,
) -> Vec<(String, Pos2)> {
    let input = RawInput {
        events,
        time,
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 800.0))),
        ..RawInput::default()
    };
    let out = ctx.run_ui(input, |ui| shell.show(ui));
    let painted = texts(&out.shapes);
    out.drop_without_applying_deltas();
    painted
}

fn frame(ctx: &Context, shell: &mut Shell, events: Vec<Event>) -> Vec<(String, Pos2)> {
    frame_at(ctx, shell, events, None)
}

fn cold() -> (Context, Shell) {
    let mut shell = Shell::cold_start();
    let ctx = Context::default();
    theme::apply(&ctx, ThemeKind::Light);
    frame(&ctx, &mut shell, vec![]);
    frame(&ctx, &mut shell, vec![]);
    (ctx, shell)
}

fn press(ctx: &Context, shell: &mut Shell, k: Key) -> Vec<(String, Pos2)> {
    frame(ctx, shell, vec![key_event(k, Modifiers::NONE)]);
    frame(ctx, shell, vec![])
}

#[test]
fn a_shortcut_with_no_company_says_what_it_needs_in_plain_words() {
    // provenance: owner's brief for round s9: a function key or letter that needs a company must
    // answer, in plain words, naming the screen, not do nothing. Names are the sidebar's
    // (sidebar.rs labels) and the keys are the mock's (index.html:1003-1004, 1646).
    for (k, wanted) in [
        (
            Key::F8,
            "Sales (F8) needs a company. Create or open one first.",
        ),
        (
            Key::F9,
            "Purchase (F9) needs a company. Create or open one first.",
        ),
        (
            Key::F5,
            "Payment (F5) needs a company. Create or open one first.",
        ),
        (
            Key::F6,
            "Receipt (F6) needs a company. Create or open one first.",
        ),
        (
            Key::F4,
            "Contra (F4) needs a company. Create or open one first.",
        ),
        (
            Key::F7,
            "Journal (F7) needs a company. Create or open one first.",
        ),
        (
            Key::D,
            "Day book (D) needs a company. Create or open one first.",
        ),
        (
            Key::B,
            "Balance sheet (B) needs a company. Create or open one first.",
        ),
        (
            Key::M,
            "Masters (M) needs a company. Create or open one first.",
        ),
        (
            Key::G,
            "GST returns (G) needs a company. Create or open one first.",
        ),
    ] {
        let (ctx, mut shell) = cold();
        press(&ctx, &mut shell, k);
        assert_eq!(shell.notice(), Some(wanted), "{k:?}");
        assert_eq!(shell.notice_action(), Some("Create a company"), "{k:?}");
        assert_eq!(shell.tabs().active_stack().unwrap().depth(), 1);
    }
}

#[test]
fn a_shortcut_typed_into_a_field_is_not_taken() {
    // Typing "d" in a name field is typing. Walk to the Company step, focus the name, press D.
    let (ctx, mut shell) = cold();
    frame(
        &ctx,
        &mut shell,
        vec![key_event(Key::Enter, Modifiers::NONE)],
    );
    frame(&ctx, &mut shell, vec![]);
    frame(&ctx, &mut shell, vec![]);
    press(&ctx, &mut shell, Key::D);
    assert_eq!(shell.notice(), None);
}

#[test]
fn the_message_is_a_toast_above_the_key_bar_not_red_text_in_it() {
    // provenance: index.html:290 `.toast{bottom:48px; centred}`. The key bar is the bottom 32
    // points of the window (800 high here); the toast sits above it.
    let (ctx, mut shell) = cold();
    frame(&ctx, &mut shell, vec![key_event(Key::F8, Modifiers::NONE)]);
    // An area is laid out once before it is painted.
    frame(&ctx, &mut shell, vec![]);
    let painted = frame(&ctx, &mut shell, vec![]);
    let (_, at) = painted
        .iter()
        .find(|(t, _)| t == "Sales (F8) needs a company. Create or open one first.")
        .expect("the toast text is not painted");
    assert!(at.y < 760.0, "the message is in the key bar: y {}", at.y);
    assert!(
        at.y > 600.0,
        "the message is not near the bottom: y {}",
        at.y
    );
    let buttons = painted
        .iter()
        .filter(|(t, _)| t == "Create a company")
        .count();
    assert!(
        buttons >= 2,
        "the toast has no action button (the card title is the other \"Create a company\")"
    );
}

#[test]
fn the_toast_goes_away_by_itself() {
    // provenance: index.html:921 (`setTimeout(..., 2600)`); ours holds a little longer, 6 s.
    let (ctx, mut shell) = cold();
    frame_at(
        &ctx,
        &mut shell,
        vec![key_event(Key::F8, Modifiers::NONE)],
        Some(1.0),
    );
    frame_at(&ctx, &mut shell, vec![], Some(1.1));
    assert!(shell.notice().is_some());
    frame_at(&ctx, &mut shell, vec![], Some(3.0));
    assert!(shell.notice().is_some(), "gone too soon");
    frame_at(&ctx, &mut shell, vec![], Some(8.0));
    assert_eq!(shell.notice(), None);
    assert_eq!(shell.notice_action(), None);
}

#[test]
fn a_screen_that_needs_a_company_is_refused_in_plain_words() {
    // provenance: owner's brief: the refusal read "create one or open a file in Company setup
    // first (Home)". It now names what was asked for and what to do.
    let (ctx, mut shell) = cold();
    frame(
        &ctx,
        &mut shell,
        vec![key_event(Key::K, Modifiers::COMMAND)],
    );
    frame(&ctx, &mut shell, vec![]);
    shell.palette_mut().set_query("Masters");
    frame(&ctx, &mut shell, vec![]);
    frame(
        &ctx,
        &mut shell,
        vec![key_event(Key::Enter, Modifiers::NONE)],
    );
    frame(&ctx, &mut shell, vec![]);
    assert_eq!(
        shell.notice(),
        Some("Masters needs a company. Create or open one first.")
    );
    assert_eq!(shell.notice_action(), Some("Create a company"));
}

#[test]
fn tabs_with_the_same_title_are_told_apart() {
    // provenance: owner's brief item 4: "two tabs both titled Company setup".
    let titles = vec![
        "Company setup".to_owned(),
        "Company setup".to_owned(),
        "Day book".to_owned(),
    ];
    assert_eq!(
        unique_tab_labels(&titles),
        vec!["Company setup", "Company setup 2", "Day book"]
    );
    let (ctx, mut shell) = cold();
    frame(
        &ctx,
        &mut shell,
        vec![key_event(Key::T, Modifiers::COMMAND)],
    );
    let painted = frame(&ctx, &mut shell, vec![]);
    assert!(
        painted.iter().any(|(t, _)| t == "Company setup 2"),
        "{painted:?}"
    );
    assert!(
        !painted.iter().any(|(t, _)| t == "+"),
        "the plain + button is drawn"
    );
}

#[test]
fn the_import_card_on_the_welcome_opens_the_import_screen() {
    // provenance: owner's brief item 1: the card said "Not available in this wizard yet" while
    // the palette opened a working import. index.html:1864 draws it as a choice like the others.
    let (ctx, mut shell) = cold();
    let painted = frame(&ctx, &mut shell, vec![]);
    assert!(
        !painted
            .iter()
            .any(|(t, _)| t.contains("Not available in this wizard")),
        "the card still says it is unavailable"
    );
    press(&ctx, &mut shell, Key::ArrowRight);
    frame(
        &ctx,
        &mut shell,
        vec![key_event(Key::Enter, Modifiers::NONE)],
    );
    frame(&ctx, &mut shell, vec![]);
    let stack = shell.tabs().active_stack().unwrap();
    assert_eq!(stack.depth(), 2);
    assert_eq!(stack.top_title(), "Import from another app");
}

#[test]
fn the_import_screen_with_no_company_says_what_it_needs() {
    // provenance: the one-step import writes into the open, empty company (import/direct.rs:
    // "no company file is open to import into"); with none open the screen says so up front.
    let (ctx, mut shell) = cold();
    press(&ctx, &mut shell, Key::ArrowRight);
    frame(
        &ctx,
        &mut shell,
        vec![key_event(Key::Enter, Modifiers::NONE)],
    );
    frame(&ctx, &mut shell, vec![]);
    let painted = frame(&ctx, &mut shell, vec![]);
    assert!(
        painted
            .iter()
            .any(|(t, _)| t.starts_with("No company is open yet.")),
        "{painted:?}"
    );
}

static PICKED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn counting_picker() -> Option<std::path::PathBuf> {
    PICKED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    None
}

#[test]
fn the_open_card_asks_the_injected_picker() {
    // provenance: coordinator's order for round s9: Enter on the Open card must reach a picker
    // the caller injects, so a test never raises a native dialog.
    let mut shell = Shell::new(Box::new(
        SetupScreen::default().with_picker(counting_picker),
    ));
    let ctx = Context::default();
    theme::apply(&ctx, ThemeKind::Light);
    frame(&ctx, &mut shell, vec![]);
    press(&ctx, &mut shell, Key::ArrowDown);
    frame(
        &ctx,
        &mut shell,
        vec![key_event(Key::Enter, Modifiers::NONE)],
    );
    frame(&ctx, &mut shell, vec![]);
    assert_eq!(PICKED.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn the_default_picker_raises_no_dialog_unless_the_real_window_switched_it_on() {
    // Nothing in a test or capture calls `enable_system_dialogs`; the default answers "cancelled".
    assert!(!ledgerly_app::screens::setup::system_dialogs_enabled());
    assert_eq!(ledgerly_app::screens::setup::default_file_dialog(), None);
    let (ctx, mut shell) = cold();
    press(&ctx, &mut shell, Key::ArrowDown);
    frame(
        &ctx,
        &mut shell,
        vec![key_event(Key::Enter, Modifiers::NONE)],
    );
    frame(&ctx, &mut shell, vec![]);
    assert!(shell.app_ctx_mut().api.is_none());
}

#[test]
fn the_company_is_made_with_the_filing_frequency_chosen() {
    use ledgerly_api::{AiTier, GstFiling};
    use ledgerly_app::screens::setup::form::{build_company, create_company_with_filing};
    use ledgerly_core::{Clock, OffsetDateTime};
    // provenance: derived; the wizard's GST filing choice must be what the file stores, read back
    // by `Api::gst_filing` (round S9, the mock's "GST filing" field on the company step).
    let path =
        std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("s9-filing-wizard.ledgerly");
    let _gone = std::fs::remove_file(&path);
    let form = ledgerly_app::screens::setup::form::CompanyForm {
        name: "Fictional Traders".to_owned(),
        kind: ledgerly_model::EntityKind::Partnership,
        city: "Pune".to_owned(),
        gstin: String::new(),
        state: ledgerly_core::StateCode::new(27).unwrap(),
        fy_start: time::macros::date!(2026 - 04 - 01),
        books_begin: time::macros::date!(2026 - 04 - 01),
    };
    let company = build_company(&form, ledgerly_model::Features::default()).unwrap();
    let clock = Clock::Fixed(OffsetDateTime::UNIX_EPOCH);
    let (api, _note) =
        create_company_with_filing(&path, &company, clock, AiTier::Off, GstFiling::Quarterly)
            .unwrap();
    assert_eq!(api.gst_filing().unwrap(), Some(GstFiling::Quarterly));
}

#[test]
fn the_company_step_offers_a_filing_frequency_and_keeps_it() {
    let (ctx, mut shell) = cold();
    press(&ctx, &mut shell, Key::Enter);
    let painted = frame(&ctx, &mut shell, vec![]);
    // provenance: mock index.html:1869 shows a "GST filing" field with Monthly; the old "Not
    // saved yet" hint is true no more, since the frequency is stored with the company.
    assert!(
        painted.iter().any(|(t, _)| t == "GST filing"),
        "{painted:?}"
    );
    assert!(painted.iter().any(|(t, _)| t == "Monthly"), "{painted:?}");
    assert!(
        !painted
            .iter()
            .any(|(t, _)| t.contains("no filing frequency is kept")),
        "{painted:?}"
    );
}

#[test]
fn the_default_company_file_is_named_from_the_company_name() {
    use ledgerly_app::screens::setup::default_file_label;
    // provenance: derived by hand from the wizard's rule (file_path): Documents/Ledgerly, the
    // name in lower case, each run of other characters one hyphen, no hyphen at either end.
    assert_eq!(
        default_file_label("Godavari Drip & Pump Works"),
        "~/Documents/Ledgerly/godavari-drip-pump-works.ledgerly"
    );
    assert_eq!(
        default_file_label("  "),
        "~/Documents/Ledgerly/<company name>.ledgerly"
    );
}

#[test]
fn bank_contra_and_journal_say_which_screen_needs_a_company() {
    // provenance: coordinator's order for round s9 (item 4): the refusal for these three read
    // "That screen needs a company"; it must name the screen, as Masters and Home do.
    for (query, name) in [
        ("Bank reconciliation", "Bank reconciliation"),
        ("Contra", "Contra"),
        ("Journal", "Journal"),
    ] {
        let (ctx, mut shell) = cold();
        frame(
            &ctx,
            &mut shell,
            vec![key_event(Key::K, Modifiers::COMMAND)],
        );
        frame(&ctx, &mut shell, vec![]);
        shell.palette_mut().set_query(query);
        frame(&ctx, &mut shell, vec![]);
        frame(
            &ctx,
            &mut shell,
            vec![key_event(Key::Enter, Modifiers::NONE)],
        );
        frame(&ctx, &mut shell, vec![]);
        assert_eq!(
            shell.notice(),
            Some(format!("{name} needs a company. Create or open one first.").as_str()),
            "{query}"
        );
    }
}

#[test]
fn only_one_module_may_raise_a_native_file_dialog_and_it_is_off_in_tests() {
    // provenance: coordinator's order for round s9 (item 5): no test can ever open a native
    // dialog. Every call to the `rfd` crate lives in `ui/dialogs.rs`, which answers "cancelled"
    // until the real window switches dialogs on; this scan fails if another file calls rfd.
    fn scan(dir: &std::path::Path, found: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                scan(&path, found);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                if text.contains("rfd::") && !path.ends_with("ui/dialogs.rs") {
                    found.push(path.display().to_string());
                }
            }
        }
    }
    let mut found = Vec::new();
    scan(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut found,
    );
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(
        ledgerly_app::ui::dialogs::save_file("Export", Some("a.csv"), &[("CSV", &["csv"])]),
        None
    );
    assert_eq!(
        ledgerly_app::ui::dialogs::open_file("Open", &[("CSV", &["csv"])]),
        None
    );
}
