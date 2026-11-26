//! Hindi on every screen: each screen the app opens is drawn with the Hindi translation set, and
//! any English word it still shows fails the test. This is the guard behind "every screen string
//! has a Hindi entry" (spec 7): a new literal in a screen shows up here the first time the
//! screen is drawn.
//!
//! Words that are meant to stay Latin are listed in [`KEPT_LATIN`] (abbreviations the Indian
//! books use as they are) and the company's own data (names the user typed) is allowed because
//! it comes from the books, not from the screens.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::float_arithmetic,
    reason = "test code: rule B2 lets tests unwrap and panic; the walk is one long scenario and its clock is a float"
)]

use std::collections::BTreeSet;
use std::time::Duration;

use egui::epaint::ClippedShape;
use egui::{Pos2, RawInput, Rect, Shape, Vec2};
use ledgerly_api::Api;
use ledgerly_app::AppCtx;
use ledgerly_app::i18n::{Catalog, Tr, set_current};
use ledgerly_app::shell::navigate::{self, screen_for_key};
use ledgerly_app::theme::{self, ThemeKind};
use ledgerly_app::worker::ApiWorker;
use ledgerly_core::{Clock, Outcome};
use time::OffsetDateTime;

/// Latin words kept on purpose, with the reason. Every one is an abbreviation or a name the
/// Indian books use in Latin letters as they are (see `design/PARITY-GAPS.md`, "Hindi").
const KEPT_LATIN: &[&str] = &[
    "Dr",
    "Cr",
    "GST",
    "GSTIN",
    "GSTR",
    "ITC",
    "IMS",
    "PAN",
    "HSN",
    "SAC",
    "IGST",
    "CGST",
    "SGST",
    "UTGST",
    "TDS",
    "TCS",
    "PDF",
    "CSV",
    "JSON",
    "XML",
    "AI",
    "UPI",
    "NEFT",
    "RTGS",
    "IFSC",
    "TCP",
    "Ctrl",
    "Alt",
    "Shift",
    "Esc",
    "Enter",
    "Tab",
    "Del",
    "PgUp",
    "PgDn",
    "Home",
    "End",
    "Space",
    "F1",
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "F10",
    "F11",
    "F12",
    "OK",
    "Ledgerly",
    "INR",
    "XLSX",
    "No",
    "dd",
    "yyyy",
    "export",
    "Left",
    "Right",
    "PB",
    "SI",
    "CN",
    "DN",
    "Escape",
    "localhost",
    "lakh",
    "k",
    "FY",
    "B",
    "A",
    "C",
    "D",
    "E",
    "S",
    "P",
    "R",
    "N",
    "Y",
    "I",
    "G",
    "M",
    "L",
    "H",
    "K",
    "T",
    "U",
    "V",
    "X",
    "Z",
    "J",
    "O",
    "Q",
    "W",
    "F",
    "e",
    "kg",
    "pcs",
    "nos",
    "mm",
    "pt",
    "a",
    "s",
    "of",
    "to",
    "st",
    "nd",
    "rd",
    "th",
    "No",
    "ms",
    "KB",
    "MB",
    "GB",
    "Tally",
    "TallyPrime",
];

/// The words on the printed page the preview draws. The page is the PDF as the print crate writes it,
/// English by decision: its font is Latin-only (`design/PARITY-GAPS.md`, "Hindi"). Exact matches,
/// and only on the preview's own steps.
const PRINTED_PAGE: &[&str] = &[
    "Date",
    "Day Book",
    "Narration",
    "Particulars",
    "Total",
    "Voucher no.",
    "Page 1 of 1",
];

/// Text that comes from the engine or the reports crate as a sentence or a table name, and stays
/// English by decision (spec 7: the engine's reasons are not translated; see
/// `design/PARITY-GAPS.md`, "Hindi"). A text is exempt when it contains one of these.
const ENGINE_TEXT: &[&str] = &[
    // GSTR-1 and GSTR-3B table and section names, as the returns engine writes them.
    "Table 4 B2B",
    "Table 12 HSN",
    "4A B2B",
    "5 B2C",
    "7 B2C",
    "9B Credit",
    "13 Documents",
    // The notes the reports engine puts under a report.
    "Profit and Loss A/c is the profit",
    "Scenario: Actual",
    "Profit figures are positive",
    "Ageing counts whole days",
    "Stock is valued per item",
    "The check covers the events",
    "Not listed because bill-wise",
    // The integrity head, as the audit log writes it.
    "event ",
    // The reasons the review queue's sources give for being unlisted.
    "there is no GSTR-2B reader",
    "the engine does not suggest new ledgers",
    "bill photos are not read yet",
    "plain-words entry needs the assistant",
    // A masked account number is data.
    "XXXX",
    // The sample bank statement's own narrations, which the review queue lists as they are.
    "NEFT",
    "ATM WDL",
    "BILLPAY",
    "CLG/CHQ",
];

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
            | Shape::Band(_)
            | Shape::Rect(_)
            | Shape::Mesh(_)
            | Shape::QuadraticBezier(_)
            | Shape::CubicBezier(_)
            | Shape::Callback(_) => {}
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, &mut out);
    }
    out
}

fn input() -> RawInput {
    RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 900.0))),
        ..RawInput::default()
    }
}

fn sample(name: &str) -> Api {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    ledgerly_sample::create_under(&dir, Clock::Fixed(OffsetDateTime::UNIX_EPOCH))
        .unwrap()
        .0
}

/// The words of `s` that are Latin letters only and at least two long.
fn latin_words(s: &str) -> Vec<String> {
    // A voucher type's name and its number can be drawn as one text ("PurchasePB/0145"): split
    // where a lower-case letter meets a capital.
    let mut spaced = String::new();
    let mut last: Option<char> = None;
    for c in s.chars() {
        if last.is_some_and(|l| l.is_ascii_lowercase()) && c.is_ascii_uppercase() {
            spaced.push(' ');
        }
        spaced.push(c);
        last = Some(c);
    }
    spaced
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 2 && w.chars().all(|c| c.is_ascii_alphabetic()))
        .map(str::to_owned)
        .collect::<Vec<_>>()
}

/// Every word the company's own data spells in Latin letters: the sample books' names.
fn data_words(books: &ledgerly_model::Books) -> BTreeSet<String> {
    let dump = format!("{books:?}");
    latin_words(&dump).into_iter().collect()
}

struct Drawn {
    key: String,
    texts: Vec<String>,
}

/// A bank statement narration: every Latin word is upper case (`UPI/QUICKPRINT XEROX/7781`).
fn is_narration(text: &str) -> bool {
    let words = latin_words(text);
    !words.is_empty() && words.iter().all(|w| w.chars().all(|c| !c.is_lowercase()))
}

fn draw_all(tr: &Tr) -> (Vec<Drawn>, BTreeSet<String>) {
    let api = sample("i18n_hindi_screens");
    let books = match api.books() {
        Ok(Outcome::Value(b)) => b,
        other => panic!("no books: {other:?}"),
    };
    let mut data = data_words(&books);
    // The states' names are the GST code list's own English words (see PARITY-GAPS, "Hindi").
    for code in 1..=99u8 {
        if let Ok(state) = ledgerly_core::StateCode::new(code) {
            data.extend(latin_words(state.name()));
        }
    }
    let bank_keys: Vec<String> = books
        .masters
        .ledgers
        .values()
        .filter(|l| l.bank.is_some())
        .map(|l| navigate::bank_key(l.id))
        .collect();
    let worker = ApiWorker::spawn(api).unwrap();
    let mut ctx = AppCtx {
        books: Some(books),
        api: Some(worker),
        tr: tr.clone(),
        ..AppCtx::default()
    };
    set_current(tr);
    let egui_ctx = egui::Context::default();
    theme::apply(&egui_ctx, ThemeKind::Light);

    let mut keys: Vec<String> = vec![
        navigate::GST_KEY,
        navigate::IMPORT_KEY,
        navigate::MASTERS_KEY,
        navigate::BANK_RECON_KEY,
        navigate::SALES_KEY,
        navigate::REVIEW_KEY,
        navigate::ASK_KEY,
        navigate::QUICK_ENTRY_KEY,
        navigate::PURCHASE_BILL_KEY,
        navigate::KEYS_KEY,
        navigate::AI_SETTINGS_KEY,
        navigate::SETUP_KEY,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    keys.extend(bank_keys);
    keys.extend(
        navigate::report_menu()
            .iter()
            .filter_map(|(_, kind)| navigate::report_key(kind)),
    );

    let mut out = Vec::new();
    for key in keys {
        let built = match screen_for_key(&key, &ctx) {
            Some(r) => r.ok(),
            None => navigate::report_from_key(&key)
                .and_then(|kind| navigate::open_report(kind, &ctx).ok()),
        };
        let Some(mut screen) = built else {
            continue;
        };
        let mut drawn: Vec<String> = vec![
            screen.title(tr),
            screen.page_title(tr),
            screen.key_bar_name(tr),
            screen.sidebar_title(tr),
        ];
        drawn.extend(screen.subtitle(tr));
        for k in screen.keys(tr) {
            drawn.push(k.label.clone());
        }
        for _ in 0..400 {
            let o = egui_ctx.run_ui(input(), |ui| {
                let _stay = screen.ui(ui, &mut ctx);
            });
            drawn.extend(texts(&o.shapes));
            o.drop_without_applying_deltas();
            std::thread::sleep(Duration::from_millis(1));
        }
        drawn.sort();
        drawn.dedup();
        out.push(Drawn { key, texts: drawn });
    }
    set_current(&Tr::default());
    (out, data)
}

#[test]
fn every_screen_speaks_hindi() {
    // Provenance: owner decision of 2026-11-26. Release 1.0.0 ships without the AI assistant, so
    // `release::AI_IN_THIS_RELEASE` is false. This test exercises the AI UI itself, so it turns the
    // switch on for this test's thread only; the 1.0.0 screens are pinned in `s19b_ai_not_in_release.rs`.
    ledgerly_app::release::set_ai_enabled(true);
    let hi = Catalog::builtin().unwrap().tr("hi");
    let (screens, data) = draw_all(&hi);
    assert!(screens.len() >= 12, "only {} screens drew", screens.len());
    let kept: BTreeSet<&str> = KEPT_LATIN.iter().copied().collect();
    let mut left: Vec<String> = Vec::new();
    for screen in &screens {
        for text in &screen.texts {
            if ENGINE_TEXT.iter().any(|e| text.contains(e)) {
                continue;
            }
            // A bank statement line's narration is the bank's own text, in capitals.
            if screen.key.starts_with("bank:") && is_narration(text) {
                continue;
            }
            let english: Vec<String> = latin_words(text)
                .into_iter()
                .filter(|w| !kept.contains(w.as_str()) && !data.contains(w))
                .collect();
            if !english.is_empty() {
                left.push(format!(
                    "[{}] {:?} -> {}",
                    screen.key,
                    text,
                    english.join(" ")
                ));
            }
        }
    }
    assert!(
        left.is_empty(),
        "{} strings still show English under Hindi:\n{}",
        left.len(),
        left.join("\n")
    );
}

fn key(k: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

/// One walk through the real shell: the keys a person presses from Home to reach the voucher
/// screens, the reports, the masters and the palette, drawn in Hindi with the sidebar, tabs and
/// key bar around them. Every text the shell paints on the way is collected.
fn walk_shell(code: &str) -> (Vec<Drawn>, BTreeSet<String>) {
    use egui::{Key, Modifiers};
    let api = sample("i18n_hindi_walk");
    let books = match api.books() {
        Ok(Outcome::Value(b)) => b,
        other => panic!("no books: {other:?}"),
    };
    let mut data = data_words(&books);
    for c in 1..=99u8 {
        if let Ok(state) = ledgerly_core::StateCode::new(c) {
            data.extend(latin_words(state.name()));
        }
    }
    let worker = ApiWorker::spawn(api).unwrap();
    let mut shell =
        ledgerly_app::shell::Shell::new(Box::new(ledgerly_app::shell::GatewayScreen::new()));
    shell.app_ctx_mut().books = Some(books);
    shell.app_ctx_mut().api = Some(worker);
    shell.set_language(code);
    let egui_ctx = egui::Context::default();
    theme::apply(&egui_ctx, ThemeKind::Light);
    let steps: Vec<(&str, Vec<egui::Event>)> = vec![
        ("home", vec![]),
        ("palette", vec![key(Key::K, Modifiers::COMMAND)]),
        ("palette closed", vec![key(Key::Escape, Modifiers::NONE)]),
        ("sales invoice", vec![key(Key::F8, Modifiers::NONE)]),
        ("back from sales", vec![key(Key::Escape, Modifiers::NONE)]),
        ("payment", vec![key(Key::F5, Modifiers::NONE)]),
        ("back from payment", vec![key(Key::Escape, Modifiers::NONE)]),
        ("receipt", vec![key(Key::F6, Modifiers::NONE)]),
        ("back from receipt", vec![key(Key::Escape, Modifiers::NONE)]),
        ("purchase", vec![key(Key::F9, Modifiers::NONE)]),
        (
            "back from purchase",
            vec![key(Key::Escape, Modifiers::NONE)],
        ),
        ("balance sheet", vec![key(Key::B, Modifiers::NONE)]),
        (
            "back from balance sheet",
            vec![key(Key::Escape, Modifiers::NONE)],
        ),
        ("day book", vec![key(Key::D, Modifiers::NONE)]),
        ("print preview", vec![key(Key::P, Modifiers::COMMAND)]),
        (
            "back from print preview",
            vec![key(Key::Escape, Modifiers::NONE)],
        ),
        (
            "back from day book",
            vec![key(Key::Escape, Modifiers::NONE)],
        ),
        ("gst", vec![key(Key::G, Modifiers::NONE)]),
        ("back from gst", vec![key(Key::Escape, Modifiers::NONE)]),
        ("masters", vec![key(Key::M, Modifiers::NONE)]),
        ("create ledger", vec![key(Key::C, Modifiers::ALT)]),
    ];
    let mut out = Vec::new();
    let mut time = 0.0_f64;
    for (name, events) in steps {
        let mut seen: Vec<String> = Vec::new();
        let frames = if name == "print preview" { 400 } else { 120 };
        for frame in 0..frames {
            let input = RawInput {
                events: if frame == 0 {
                    events.clone()
                } else {
                    Vec::new()
                },
                time: Some(time),
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                ..RawInput::default()
            };
            time += 0.016;
            let o = egui_ctx.run_ui(input, |ui| shell.show(ui));
            seen.extend(texts(&o.shapes));
            o.drop_without_applying_deltas();
            std::thread::sleep(Duration::from_millis(1));
        }
        seen.sort();
        seen.dedup();
        out.push(Drawn {
            key: format!("shell: {name}"),
            texts: seen,
        });
    }
    (out, data)
}

#[test]
fn the_shell_and_the_voucher_screens_speak_hindi() {
    let (screens, data) = walk_shell("hi");
    if std::env::var_os("LEDGERLY_DUMP_TEXTS").is_some() {
        for screen in &screens {
            eprintln!("{}: {:?}", screen.key, screen.texts);
        }
    }
    let kept: BTreeSet<&str> = KEPT_LATIN.iter().copied().collect();
    let mut left: Vec<String> = Vec::new();
    for screen in &screens {
        for text in &screen.texts {
            if ENGINE_TEXT.iter().any(|e| text.contains(e))
                || (screen.key.contains("print preview") && PRINTED_PAGE.contains(&text.as_str()))
            {
                continue;
            }
            let english: Vec<String> = latin_words(text)
                .into_iter()
                .filter(|w| !kept.contains(w.as_str()) && !data.contains(w))
                .collect();
            if !english.is_empty() {
                left.push(format!(
                    "[{}] {:?} -> {}",
                    screen.key,
                    text,
                    english.join(" ")
                ));
            }
        }
    }
    assert!(
        left.is_empty(),
        "{} strings still show English under Hindi:\n{}",
        left.len(),
        left.join("\n")
    );
}
