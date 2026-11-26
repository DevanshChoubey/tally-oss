//! Card S8 (first run): what a user sees when the app starts with no company open.
//!
//! The released 1.0.0 app opened on a raw developer form (a "path to a .ledgerly file" box and a
//! create form with tiny inputs). The designed first run is the setup wizard's Start step
//! (mock state `setup.welcome`). These tests build the shell with `Shell::cold_start`, the
//! constructor the real `main` runs (`shell::run`), and look at what it paints.
//!
//! Provenance: the mock, `design/mock-v2/index.html`: line 448 (`setup.welcome`, "First run,
//! welcome"), 1863 (the Start pane: "Welcome to Ledgerly", Create a company, Import, Open a
//! Ledgerly file, Try the sample company), 893-896 (the top bar on setup shows one "New company"
//! chip and no date chip), 1891 (key bar "Setup, step 1 of 7").

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unnecessary_wraps,
    reason = "test code: rule B2 lets tests unwrap and panic; the pickers must match the screen's fn() -> Option<PathBuf>"
)]

use std::path::PathBuf;
use std::time::Duration;

use egui::epaint::{ClippedShape, Shape};
use egui::{Context, Event, Key, Modifiers, Pos2, RawInput, Rect, vec2};
use ledgerly_api::Api;
use ledgerly_app::screens::setup::SetupScreen;
use ledgerly_app::shell::{Shell, company_from_form};
use ledgerly_app::theme::{self, ThemeKind};
use ledgerly_core::{Clock, OffsetDateTime};

/// Every text a frame painted.
fn texts(shapes: &[ClippedShape]) -> Vec<String> {
    fn walk(shape: &Shape, out: &mut Vec<String>) {
        match shape {
            Shape::Text(t) => out.push(t.galley.text().to_owned()),
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

fn key(k: Key) -> Event {
    Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

/// One frame of `shell` at the capture size, with `events`. Returns the painted texts.
fn frame(ctx: &Context, shell: &mut Shell, events: Vec<Event>) -> Vec<String> {
    let input = RawInput {
        events,
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 800.0))),
        ..RawInput::default()
    };
    let out = ctx.run_ui(input, |ui| shell.show(ui));
    let painted = texts(&out.shapes);
    out.drop_without_applying_deltas();
    painted
}

fn settled(shell: &mut Shell) -> (Context, Vec<String>) {
    let ctx = Context::default();
    theme::apply(&ctx, ThemeKind::Light);
    frame(&ctx, shell, vec![]);
    let painted = frame(&ctx, shell, vec![]);
    (ctx, painted)
}

#[test]
fn the_cold_start_shows_the_setup_welcome_not_the_raw_form() {
    // provenance: index.html:1863 (Start pane wording) and 1891 (key bar name).
    let mut shell = Shell::cold_start();
    let (_ctx, painted) = settled(&mut shell);
    assert_eq!(
        shell
            .tabs()
            .active_stack()
            .unwrap()
            .top_key_bar_name(&ledgerly_app::i18n::Tr::default()),
        "Setup, step 1 of 7"
    );
    for wanted in [
        "Welcome to Ledgerly",
        "Create a company",
        "Open a Ledgerly file",
    ] {
        assert!(painted.iter().any(|t| t == wanted), "{wanted}: {painted:?}");
    }
    // The raw form's words must not be on the first screen.
    for raw in [
        "Open a company file",
        "Create a company file",
        "path to a .ledgerly file",
    ] {
        assert!(!painted.iter().any(|t| t == raw), "{raw} is on screen");
    }
}

#[test]
fn the_cold_start_top_bar_says_new_company_and_has_no_date_chip() {
    // provenance: index.html:893-896. With no company on the setup screen the bar shows the one
    // "New company" chip, not "No company open" with F3, and no date chip.
    let mut shell = Shell::cold_start();
    let (_ctx, painted) = settled(&mut shell);
    assert!(painted.iter().any(|t| t == "New company"), "{painted:?}");
    assert!(!painted.iter().any(|t| t == "No company open"));
    assert!(!painted.iter().any(|t| t == "F2"), "date chip is shown");
}

#[test]
fn the_cold_start_has_no_breadcrumb_and_selects_company_setup_in_the_sidebar() {
    // provenance: index.html:1884 (no phead on setup) and the mock capture
    // `setup.welcome.webp`: "Company setup" is the selected sidebar entry.
    let mut shell = Shell::cold_start();
    let (_ctx, _painted) = settled(&mut shell);
    assert_eq!(shell.selected_sidebar(), Some("Company setup"));
    // The shell draws breadcrumbs only above a stack of two or more screens.
    assert_eq!(shell.tabs().active_stack().unwrap().depth(), 1);
}

/// A company file at `name` under the test temp dir, closed again.
fn company_file(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let company = company_from_form("Fictional Traders", "27", "2026").unwrap();
    drop(Api::create(&path, &company, Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap());
    path
}

fn picks_company() -> Option<PathBuf> {
    Some(company_file("s8-picked.ledgerly"))
}

fn picks_nothing() -> Option<PathBuf> {
    None
}

fn picks_junk() -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("s8-junk.ledgerly");
    std::fs::write(&path, b"this is not a company file").unwrap();
    Some(path)
}

/// Selects the "Open a Ledgerly file" card with the arrow keys and presses Enter.
fn choose_open(ctx: &Context, shell: &mut Shell) {
    frame(ctx, shell, vec![key(Key::ArrowDown)]);
    frame(ctx, shell, vec![]);
    let painted = frame(ctx, shell, vec![]);
    assert!(
        painted.iter().any(|t| t == "Open a Ledgerly file")
            && painted.iter().any(|t| t == "Welcome to Ledgerly"),
        "{painted:?}"
    );
    frame(ctx, shell, vec![key(Key::Enter)]);
}

fn cold_start_with(picker: fn() -> Option<PathBuf>) -> Shell {
    Shell::new(Box::new(
        SetupScreen::with_clock(Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).with_picker(picker),
    ))
}

#[test]
fn opening_a_file_from_the_welcome_opens_the_company_without_typing_a_path() {
    // provenance: index.html:1863 ("Open a Ledgerly file: From a backup or another computer").
    // The file comes from the picker; the company then opens and Home replaces the wizard.
    let mut shell = cold_start_with(picks_company);
    let (ctx, _) = settled(&mut shell);
    choose_open(&ctx, &mut shell);
    for _ in 0..2000 {
        if shell.app_ctx_mut().books.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
        frame(&ctx, &mut shell, vec![]);
    }
    assert!(
        shell.app_ctx_mut().books.is_some(),
        "the company did not open"
    );
    frame(&ctx, &mut shell, vec![]);
    let books = shell.app_ctx_mut().books.as_ref().unwrap();
    assert_eq!(books.company.name, "Fictional Traders");
    assert_ne!(
        shell
            .tabs()
            .active_stack()
            .unwrap()
            .top_key_bar_name(&ledgerly_app::i18n::Tr::default()),
        "Setup, step 1 of 7",
        "Home should have replaced the wizard"
    );
}

#[test]
fn cancelling_the_file_dialog_leaves_the_welcome_as_it_was() {
    let mut shell = cold_start_with(picks_nothing);
    let (ctx, _) = settled(&mut shell);
    choose_open(&ctx, &mut shell);
    for _ in 0..20 {
        frame(&ctx, &mut shell, vec![]);
    }
    assert!(shell.app_ctx_mut().api.is_none());
    assert_eq!(
        shell
            .tabs()
            .active_stack()
            .unwrap()
            .top_key_bar_name(&ledgerly_app::i18n::Tr::default()),
        "Setup, step 1 of 7"
    );
}

#[test]
fn a_file_that_is_not_a_company_says_so_and_stays_on_the_welcome() {
    let mut shell = cold_start_with(picks_junk);
    let (ctx, _) = settled(&mut shell);
    choose_open(&ctx, &mut shell);
    let mut painted = Vec::new();
    for _ in 0..2000 {
        std::thread::sleep(Duration::from_millis(2));
        painted = frame(&ctx, &mut shell, vec![]);
        if painted.iter().any(|t| t.contains("could not")) {
            break;
        }
    }
    assert!(
        painted.iter().any(|t| t.contains("could not")),
        "no reason shown: {painted:?}"
    );
    assert!(shell.app_ctx_mut().api.is_none());
    assert_eq!(
        shell
            .tabs()
            .active_stack()
            .unwrap()
            .top_key_bar_name(&ledgerly_app::i18n::Tr::default()),
        "Setup, step 1 of 7"
    );
}

#[test]
fn a_new_tab_with_no_company_opens_the_setup_welcome_too() {
    let mut shell = Shell::cold_start();
    let (ctx, _) = settled(&mut shell);
    frame(
        &ctx,
        &mut shell,
        vec![Event::Key {
            key: Key::T,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        }],
    );
    assert_eq!(shell.tabs().len(), 2);
    frame(&ctx, &mut shell, vec![]);
    assert_eq!(
        shell
            .tabs()
            .active_stack()
            .unwrap()
            .top_key_bar_name(&ledgerly_app::i18n::Tr::default()),
        "Setup, step 1 of 7"
    );
}

/// The notice the key bar shows after the palette opens `title` on the cold start.
fn palette_pick(shell: &mut Shell, ctx: &Context, query: &'static str) {
    frame(
        ctx,
        shell,
        vec![Event::Key {
            key: Key::K,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        }],
    );
    frame(ctx, shell, vec![]);
    shell.palette_mut().set_query(query);
    frame(ctx, shell, vec![]);
    frame(ctx, shell, vec![key(Key::Enter)]);
    frame(ctx, shell, vec![]);
}

#[test]
fn screens_that_only_show_a_companys_data_do_not_open_with_no_company() {
    // provenance: the audit of the real cold start (design/REAL-PATH-AUDIT.md): Home waited for
    // books forever ("Loading the company's books...") and Masters opened a page of empty tables. With no company open these opens must leave a notice and no new screen.
    for (query, name) in [("Home", "Home"), ("Masters", "Masters")] {
        let mut shell = Shell::cold_start();
        let (ctx, _) = settled(&mut shell);
        palette_pick(&mut shell, &ctx, query);
        assert_eq!(
            shell.tabs().active_stack().unwrap().depth(),
            1,
            "{name} opened with no company"
        );
        let notice = shell.notice().unwrap_or_default().to_owned();
        assert!(
            notice == format!("{name} needs a company. Create or open one first."),
            "{name}: {notice:?}"
        );
    }
}

#[test]
fn the_review_queue_with_no_company_says_so_without_also_saying_it_is_loading() {
    // provenance: the audit of the real cold start (design/REAL-PATH-AUDIT.md) showed the red
    // "No company is open" note and "Loading the queue from the engine." together.
    let mut shell = Shell::cold_start();
    let (ctx, _) = settled(&mut shell);
    palette_pick(&mut shell, &ctx, "Review queue");
    frame(&ctx, &mut shell, vec![]);
    let painted = frame(&ctx, &mut shell, vec![]);
    assert!(
        painted.iter().any(|t| t.contains("No company is open")),
        "{painted:?}"
    );
    assert!(
        !painted.iter().any(|t| t.contains("Loading the queue")),
        "{painted:?}"
    );
}
