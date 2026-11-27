//! S22: importing masters or transactions from an XML file into the open company, from the import
//! screen, from Alt+O and from the palette.
//!
//! Provenance. The file is a fictional, hand-built fixture in the import form of
//! <https://help.tallysolutions.com/sample-xml/> (read 2026-11-27). Expected values are
//! `derived`:
//! - Alt+O opens import ("Alt+O (Import) > Masters", "Alt+O (Import) > Transactions":
//!   <https://help.tallysolutions.com/import-data-from-xml-or-json/>, read 2026-11-27);
//! - the user chooses Masters or Transactions, the file, and for masters what to do with a master
//!   that exists (Combine Opening Balance, Ignore Duplicates, Modify with New Data:
//!   <https://help.tallysolutions.com/import-data-faq/>). No choice is made for the user, so a
//!   missing choice is refused in words and nothing runs;
//! - the counts are the objects of the fixture, counted by hand: two new ledgers, one ledger whose
//!   group is not in the company or the file (an exception).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_arithmetic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::{Event, Key, Modifiers, Pos2, RawInput, Rect, vec2};
use ledgerly_api::{Api, MasterBehaviour};
use ledgerly_app::screens::import::{FileImportState, FileKind, ImportScreen};
use ledgerly_app::shell::navigate::screen_entries;
use ledgerly_app::shell::submit;
use ledgerly_app::shell::{GatewayScreen, Shell};
use ledgerly_app::worker::ApiWorker;
use ledgerly_core::Clock;
use time::OffsetDateTime;
use time::macros::date;

const MASTERS: &str = r#"<ENVELOPE><HEADER><TALLYREQUEST>Import Data</TALLYREQUEST></HEADER><BODY><IMPORTDATA>
<REQUESTDESC><REPORTNAME>All Masters</REPORTNAME></REQUESTDESC><REQUESTDATA>
<TALLYMESSAGE><LEDGER NAME="Meena Fabrics" ACTION="Create"><NAME>Meena Fabrics</NAME><PARENT>Sundry Debtors</PARENT></LEDGER></TALLYMESSAGE>
<TALLYMESSAGE><LEDGER NAME="Hamali Charges" ACTION="Create"><NAME>Hamali Charges</NAME><PARENT>Indirect Expenses</PARENT></LEDGER></TALLYMESSAGE>
<TALLYMESSAGE><LEDGER NAME="Lost Ledger" ACTION="Create"><NAME>Lost Ledger</NAME><PARENT>No Such Group</PARENT></LEDGER></TALLYMESSAGE>
</REQUESTDATA></IMPORTDATA></BODY></ENVELOPE>"#;

fn sample(name: &str) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    ledgerly_sample::create(&path, Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap()
}

fn fixture(name: &str, body: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::write(&path, body).unwrap();
    path
}

fn screen() -> ImportScreen {
    ImportScreen::new(date!(2026 - 11 - 27)).unwrap()
}

fn finished(screen: &mut ImportScreen) {
    let start = Instant::now();
    while *screen.file_state() == FileImportState::Running {
        screen.tick();
        assert!(start.elapsed() < Duration::from_secs(20), "import finishes");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn failed(screen: &ImportScreen) -> String {
    match screen.file_state() {
        FileImportState::Failed(text) => text.clone(),
        other @ (FileImportState::Idle
        | FileImportState::Running
        | FileImportState::Imported(_)
        | FileImportState::Refused(_)) => panic!("expected a refusal in words, got {other:?}"),
    }
}

#[test]
fn masters_from_a_file_go_into_the_open_company_with_every_exception_listed() {
    let worker = ApiWorker::spawn(sample("s22-file-masters.ledgerly")).unwrap();
    let path = fixture("s22-file-masters.xml", MASTERS);
    let mut screen = screen();
    screen.set_file_form(
        Some(FileKind::Masters),
        &path.display().to_string(),
        Some(MasterBehaviour::IgnoreDuplicates),
    );
    screen.start_file_import(Some(&worker));
    finished(&mut screen);
    let FileImportState::Imported(summary) = screen.file_state() else {
        panic!("imported, got {:?}", screen.file_state());
    };
    assert_eq!(
        [
            summary.created,
            summary.altered,
            summary.combined,
            summary.ignored,
            summary.errors
        ],
        [2, 0, 0, 0, 1]
    );
    assert_eq!(summary.exceptions.len(), 1);
    assert_eq!(summary.exceptions[0].object, "ledger 'Lost Ledger'");
    let lines = screen.file_lines();
    assert_eq!(
        lines[0],
        "Masters: 2 created, 0 altered, 0 combined, 0 ignored, 1 not imported."
    );
    assert!(
        lines.iter().any(|l| l.contains("Lost Ledger")),
        "the exception is shown: {lines:?}"
    );
}

#[test]
fn the_form_says_what_is_missing_and_runs_nothing() {
    let worker = ApiWorker::spawn(sample("s22-file-form.ledgerly")).unwrap();
    let path = fixture("s22-file-form.xml", MASTERS);
    let path = path.display().to_string();
    let mut screen = screen();

    screen.set_file_form(None, &path, Some(MasterBehaviour::IgnoreDuplicates));
    screen.start_file_import(Some(&worker));
    assert_eq!(failed(&screen), "choose Masters or Transactions");

    screen.set_file_form(
        Some(FileKind::Masters),
        "  ",
        Some(MasterBehaviour::IgnoreDuplicates),
    );
    screen.start_file_import(Some(&worker));
    assert_eq!(
        failed(&screen),
        "enter the path of the XML file, or choose it"
    );

    screen.set_file_form(Some(FileKind::Masters), &path, None);
    screen.start_file_import(Some(&worker));
    assert_eq!(
        failed(&screen),
        "choose what to do with a master that is already in the company"
    );

    // Transactions need no choice about masters.
    screen.set_file_form(Some(FileKind::Transactions), &path, None);
    screen.start_file_import(None);
    assert_eq!(
        failed(&screen),
        "no company is open: create or open a company, then import into it"
    );
}

#[test]
fn a_file_that_cannot_be_read_says_so() {
    let worker = ApiWorker::spawn(sample("s22-file-bad.ledgerly")).unwrap();
    let path = fixture("s22-file-bad.xml", "this is not xml");
    let mut screen = screen();
    screen.set_file_form(
        Some(FileKind::Transactions),
        &path.display().to_string(),
        None,
    );
    screen.start_file_import(Some(&worker));
    finished(&mut screen);
    let text = failed(&screen);
    assert!(text.starts_with("cannot import "), "{text}");
}

fn input(events: Vec<Event>) -> RawInput {
    RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1366.0, 768.0))),
        events,
        ..RawInput::default()
    }
}

fn frame(ctx: &egui::Context, shell: &mut Shell, events: Vec<Event>) {
    let mut out = ctx.run_ui(input(events), |ui| shell.show(ui));
    out.textures_delta.clear();
}

#[test]
fn alt_o_opens_the_import_screen_on_an_open_company() {
    let ctx = egui::Context::default();
    let mut shell = Shell::new(Box::new(GatewayScreen::new()));
    shell.app_ctx_mut().api = Some(ApiWorker::spawn(sample("s22-file-alt-o.ledgerly")).unwrap());
    submit::note_changed();
    let start = Instant::now();
    while shell.app_ctx_mut().books.is_none() {
        frame(&ctx, &mut shell, Vec::new());
        assert!(start.elapsed() < Duration::from_secs(10), "books arrive");
        std::thread::sleep(Duration::from_millis(5));
    }
    let alt_o = Event::Key {
        key: Key::O,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::ALT,
    };
    frame(&ctx, &mut shell, vec![alt_o]);
    frame(&ctx, &mut shell, Vec::new());
    assert_eq!(shell.top_key_bar_name(), "Import from another app");
}

#[test]
fn the_palette_offers_import_from_a_file() {
    let titles: Vec<String> = screen_entries(None)
        .into_iter()
        .map(|e| e.title.clone())
        .collect();
    assert!(
        titles
            .iter()
            .any(|t| t == "Import masters or vouchers from a file"),
        "{titles:?}"
    );
}

/// Every painted text with its rect.
fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, Rect)> {
    fn walk(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
        match shape {
            egui::Shape::Vec(items) => items.iter().for_each(|s| walk(s, out)),
            egui::Shape::Text(t) => {
                out.push((t.galley.text().to_owned(), t.visual_bounding_rect()));
            }
            egui::Shape::Noop
            | egui::Shape::Circle(_)
            | egui::Shape::Ellipse(_)
            | egui::Shape::LineSegment { .. }
            | egui::Shape::Rect(_)
            | egui::Shape::Path(_)
            | egui::Shape::Band(_)
            | egui::Shape::Mesh(_)
            | egui::Shape::QuadraticBezier(_)
            | egui::Shape::CubicBezier(_)
            | egui::Shape::Callback(_) => {}
        }
    }
    let mut out = Vec::new();
    for clipped in shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

#[test]
fn the_result_of_a_file_import_comes_into_view_on_a_short_window() {
    // derived: the minimum window is 800 x 520 points (`shell::run`); the result sits under the
    // one-step form and the file form, so it is below the fold unless the screen scrolls. Every
    // exception must be readable, so the summary and the first exception must be painted inside
    // the window once the import finishes.
    use ledgerly_app::Screen;
    let worker = ApiWorker::spawn(sample("s22-file-view.ledgerly")).unwrap();
    let path = fixture("s22-file-view.xml", MASTERS);
    let mut screen = screen();
    screen.set_file_form(
        Some(FileKind::Masters),
        &path.display().to_string(),
        Some(MasterBehaviour::IgnoreDuplicates),
    );
    let ctx = egui::Context::default();
    let mut app = ledgerly_app::AppCtx {
        api: Some(worker),
        ..ledgerly_app::AppCtx::default()
    };
    let window = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 520.0));
    // A clock that moves a tenth of a second a frame, so egui's scroll animation finishes.
    let mut clock = 0.0_f64;
    let mut run = |screen: &mut ImportScreen, app: &mut ledgerly_app::AppCtx| {
        clock += 0.1;
        let raw = RawInput {
            screen_rect: Some(window),
            time: Some(clock),
            ..RawInput::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            let _action = screen.ui(ui, app);
        });
        out.textures_delta.clear();
        out.shapes
    };
    let _shapes = run(&mut screen, &mut app);
    screen.start_file_import(app.api.as_ref());
    finished(&mut screen);
    let mut shown = Vec::new();
    for _ in 0..20 {
        shown = texts(&run(&mut screen, &mut app));
    }
    for needle in ["Masters: 2 created", "ledger 'Lost Ledger'"] {
        let (text, rect) = shown
            .iter()
            .find(|(t, _)| t.contains(needle))
            .unwrap_or_else(|| panic!("'{needle}' is painted: {shown:?}"));
        assert!(
            window.contains_rect(*rect),
            "'{text}' is painted at {rect:?}, outside the {window:?} window"
        );
    }
}

#[test]
fn the_path_row_sits_under_its_label_with_no_gap() {
    // derived: the mock's file form (`design/mock-v2/index.html`, `impFileCard`) puts the path field
    // directly under the "XML file" label, as every other labelled field; seen in the real app
    // (s22, 2026-11-27) with no kind chosen, the row was centred in a tall box with about 100 points
    // of blank space above and below it (about 120 points from the field's text to the Import
    // button's). Measured between painted texts: the field's text may sit at most one button height
    // (32 points) under the label, and the Import button's text at most one button height plus the
    // field's and the button's own vertical padding (10 points each, `ui::button`'s 32-point box
    // around a 12-point line) under the field's text, so at most 52 points.
    use ledgerly_app::Screen;
    let mut screen = ImportScreen::for_file(date!(2026 - 11 - 27)).unwrap();
    let ctx = egui::Context::default();
    let mut app = ledgerly_app::AppCtx::default();
    let mut shown = Vec::new();
    for i in 0..10 {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1366.0, 768.0))),
            time: Some(f64::from(i) * 0.1),
            ..RawInput::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            let _action = screen.ui(ui, &mut app);
        });
        out.textures_delta.clear();
        shown = texts(&out.shapes);
    }
    let find = |needle: &str| {
        shown
            .iter()
            .find(|(t, _)| t == needle)
            .map_or_else(|| panic!("'{needle}' is painted: {shown:?}"), |(_, r)| *r)
    };
    let label = find("XML file");
    let hint = find("path of the XML file");
    let run = find("Import the file");
    assert!(
        hint.top() - label.bottom() <= 32.0,
        "field {hint:?} is far under its label {label:?}"
    );
    assert!(
        run.top() - hint.bottom() <= 52.0,
        "Import button {run:?} is far under the field {hint:?}"
    );
}

#[test]
fn enter_says_import_the_file_while_the_file_path_has_focus() {
    // derived: with the file path focused Enter runs the import from the file
    // (`ImportScreen::import_keys`), so the key bar says so; elsewhere Enter is the one-step
    // import and keeps the mock's "Import the company" (index.html:2455). Opened as the palette
    // opens it (not Alt+O), nothing has the keyboard until the path is clicked.
    use ledgerly_app::Screen;
    let mut screen = ImportScreen::new(date!(2026 - 11 - 27)).unwrap();
    let ctx = egui::Context::default();
    // The form takes the keyboard only with a company open.
    let mut app = ledgerly_app::AppCtx {
        api: Some(ApiWorker::spawn(sample("s22-enter-hint.ledgerly")).unwrap()),
        ..ledgerly_app::AppCtx::default()
    };
    let tr = ledgerly_app::i18n::Tr::default();
    let enter = |screen: &ImportScreen| {
        screen
            .keys(&tr)
            .into_iter()
            .find(|h| h.keys == "Enter")
            .map(|h| h.label)
    };
    let mut run = |events: Vec<Event>, i: u32, screen: &mut ImportScreen| {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1366.0, 768.0))),
            time: Some(f64::from(i) * 0.1),
            events,
            ..RawInput::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            let _action = screen.ui(ui, &mut app);
        });
        out.textures_delta.clear();
        texts(&out.shapes)
    };
    let mut shown = Vec::new();
    for i in 0..10 {
        shown = run(Vec::new(), i, &mut screen);
    }
    assert_eq!(enter(&screen).as_deref(), Some("Import the company"));
    let hint = shown
        .iter()
        .find(|(t, _)| t == "path of the XML file")
        .map(|(_, r)| *r)
        .unwrap();
    let at = hint.center();
    let click = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    run(vec![Event::PointerMoved(at)], 10, &mut screen);
    run(vec![click(true)], 11, &mut screen);
    run(vec![click(false)], 12, &mut screen);
    run(Vec::new(), 13, &mut screen);
    assert_eq!(enter(&screen).as_deref(), Some("Import the file"));
}

#[test]
fn enter_stays_on_the_file_after_a_choice_is_clicked_and_goes_back_with_the_one_step_form() {
    // derived: a click on a choice of the file form takes the keyboard out of the path field, but
    // the user is still filling in the file form, so Enter must not start the one-step company
    // import (`ImportScreen::import_keys`); clicking a field of the one-step form hands Enter
    // back to it. The key bar says which import Enter runs.
    use ledgerly_app::Screen;
    let mut screen = ImportScreen::new(date!(2026 - 11 - 27)).unwrap();
    let ctx = egui::Context::default();
    let mut app = ledgerly_app::AppCtx {
        api: Some(ApiWorker::spawn(sample("s22-enter-choice.ledgerly")).unwrap()),
        ..ledgerly_app::AppCtx::default()
    };
    let tr = ledgerly_app::i18n::Tr::default();
    let enter = |screen: &ImportScreen| {
        screen
            .keys(&tr)
            .into_iter()
            .find(|h| h.keys == "Enter")
            .map(|h| h.label)
    };
    let mut frame = 0_u32;
    let mut run = |events: Vec<Event>, screen: &mut ImportScreen| {
        frame += 1;
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1366.0, 768.0))),
            time: Some(f64::from(frame) * 0.1),
            events,
            ..RawInput::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            let _action = screen.ui(ui, &mut app);
        });
        out.textures_delta.clear();
        texts(&out.shapes)
    };
    let mut click_on = |text: &str, screen: &mut ImportScreen| {
        let mut shown = Vec::new();
        for _ in 0..5 {
            shown = run(Vec::new(), screen);
        }
        let at = shown
            .iter()
            .find(|(t, _)| t == text)
            .map_or_else(|| panic!("{text:?} is not on screen"), |(_, r)| r.center());
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        run(vec![Event::PointerMoved(at)], screen);
        run(vec![button(true)], screen);
        run(vec![button(false)], screen);
        run(Vec::new(), screen);
    };
    click_on("Transactions (vouchers)", &mut screen);
    assert_eq!(enter(&screen).as_deref(), Some("Import the file"));
    click_on("its exact name", &mut screen);
    assert_eq!(enter(&screen).as_deref(), Some("Import the company"));
}

#[test]
fn alt_o_is_the_import_file_action_in_both_presets_and_on_the_keyboard_map() {
    // derived: "Alt+O (Import)" (https://help.tallysolutions.com/import-data-from-xml-or-json/,
    // read 2026-11-27) in both presets; the keyboard map shows it under "Go anywhere", as the mock's
    // KEYMAP table (design/mock-v2/index.html) does.
    use ledgerly_app::keymap::{Action, Chord, classic, standard};
    use ledgerly_app::screens::keymap_map::layout;
    let alt_o = Chord::key(Key::O).with_alt();
    assert_eq!(classic().action_for(alt_o), Some(Action::ImportFile));
    assert_eq!(standard().action_for(alt_o), Some(Action::ImportFile));
    let go = layout()
        .into_iter()
        .find(|g| g.title == "Go anywhere")
        .unwrap();
    assert!(
        go.rows.iter().any(|r| r.action == Action::ImportFile
            && r.label == "Import masters or vouchers from a file"),
        "{go:?}"
    );
}

#[test]
fn alt_o_puts_the_keyboard_in_the_file_path_so_enter_imports_the_file() {
    // derived: Alt+O (Import) asks for the file ("Alt+O (Import) > Masters", then the file path:
    // https://help.tallysolutions.com/import-data-from-xml-or-json/, read 2026-11-27), so the
    // screen opened by Alt+O has the keyboard in the path and the key bar says Enter imports it.
    use ledgerly_app::Screen;
    let mut screen = ImportScreen::for_file(date!(2026 - 11 - 27)).unwrap();
    let ctx = egui::Context::default();
    let mut app = ledgerly_app::AppCtx {
        api: Some(ApiWorker::spawn(sample("s22-alt-o-focus.ledgerly")).unwrap()),
        ..ledgerly_app::AppCtx::default()
    };
    for i in 0..10 {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1366.0, 768.0))),
            time: Some(f64::from(i) * 0.1),
            ..RawInput::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            let _action = screen.ui(ui, &mut app);
        });
        out.textures_delta.clear();
    }
    let tr = ledgerly_app::i18n::Tr::default();
    let enter = screen
        .keys(&tr)
        .into_iter()
        .find(|h| h.keys == "Enter")
        .map(|h| h.label);
    assert_eq!(enter.as_deref(), Some("Import the file"));
}

const PAYMENT: &str = r#"<ENVELOPE><HEADER><TALLYREQUEST>Import Data</TALLYREQUEST></HEADER><BODY><IMPORTDATA>
<REQUESTDESC><REPORTNAME>Vouchers</REPORTNAME></REQUESTDESC><REQUESTDATA>
<TALLYMESSAGE><VOUCHER VCHTYPE="Payment" ACTION="Create"><DATE>20261125</DATE><GUID>g-app-pay-1</GUID><VOUCHERTYPENAME>Payment</VOUCHERTYPENAME>
<ALLLEDGERENTRIES.LIST><LEDGERNAME>Hamali Charges</LEDGERNAME><ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE><AMOUNT>-100.00</AMOUNT></ALLLEDGERENTRIES.LIST>
<ALLLEDGERENTRIES.LIST><LEDGERNAME>Cash</LEDGERNAME><ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>100.00</AMOUNT></ALLLEDGERENTRIES.LIST>
</VOUCHER></TALLYMESSAGE></REQUESTDATA></IMPORTDATA></BODY></ENVELOPE>"#;

#[test]
fn a_transactions_file_imported_twice_with_overwrite_on_is_overwritten_not_doubled() {
    // derived: "Overwrite voucher when a voucher with same GUID exists" set to Yes: "the existing
    // voucher gets overwritten" (https://help.tallysolutions.com/import-data-faq/, read
    // 2026-11-27). The payment is dated 25 Nov 2026, after the sample's lock (31 Oct 2026).
    let worker = ApiWorker::spawn(sample("s22-file-guid.ledgerly")).unwrap();
    let masters = fixture("s22-file-guid-masters.xml", MASTERS);
    let payment = fixture("s22-file-guid-payment.xml", PAYMENT);
    let mut screen = screen();
    screen.set_file_form(
        Some(FileKind::Masters),
        &masters.display().to_string(),
        Some(MasterBehaviour::IgnoreDuplicates),
    );
    screen.start_file_import(Some(&worker));
    finished(&mut screen);
    let mut counts = Vec::new();
    for _ in 0..2 {
        screen.set_file_form(
            Some(FileKind::Transactions),
            &payment.display().to_string(),
            None,
        );
        screen.set_file_overwrite(true);
        screen.start_file_import(Some(&worker));
        finished(&mut screen);
        let FileImportState::Imported(summary) = screen.file_state() else {
            panic!("imported, got {:?}", screen.file_state());
        };
        counts.push([summary.created, summary.altered, summary.errors]);
    }
    assert_eq!(counts, vec![[1, 0, 0], [0, 1, 0]]);
}

#[test]
fn the_drawn_error_note_is_in_hindi_under_hindi() {
    // derived: "import.file.error" is "त्रुटि: {text}" in hi.toml (unsourced, s22); the note under
    // the form must use it, not a fixed English "Error: ". The missing choice is the app's own
    // reason, so it is in Hindi too.
    use ledgerly_app::Screen;
    let hi = ledgerly_app::i18n::Catalog::builtin().unwrap().tr("hi");
    let worker = ApiWorker::spawn(sample("s22-hi-error.ledgerly")).unwrap();
    let masters = fixture("s22-hi-error.xml", MASTERS);
    let mut screen = screen();
    screen.set_file_form(
        Some(FileKind::Masters),
        &masters.display().to_string(),
        None,
    );
    // The shell sets the language at the start of every frame, so a press of Import the file
    // runs under it.
    ledgerly_app::i18n::set_current(&hi);
    screen.start_file_import(Some(&worker));
    let ctx = egui::Context::default();
    let mut app = ledgerly_app::AppCtx {
        api: Some(worker),
        ..ledgerly_app::AppCtx::default()
    };
    let mut shown = Vec::new();
    for i in 0..5 {
        ledgerly_app::i18n::set_current(&hi);
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1366.0, 768.0))),
            time: Some(f64::from(i) * 0.1),
            ..RawInput::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            let _action = screen.ui(ui, &mut app);
        });
        out.textures_delta.clear();
        shown = texts(&out.shapes);
    }
    ledgerly_app::i18n::set_current(&ledgerly_app::i18n::Tr::default());
    let want = "त्रुटि: चुनें कि कंपनी में पहले से मौजूद मास्टर का क्या करें";
    assert!(
        shown.iter().any(|(t, _)| t.contains(want)),
        "{:?}",
        shown.iter().map(|(t, _)| t).collect::<Vec<_>>()
    );
}
