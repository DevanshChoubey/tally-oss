//! Card W4 tests: the AI and privacy screen over a real company file (spec 6.8, rule A10).
//!
//! Provenance. The wording checked here is the mock's, quoted from `design/mock-v2/index.html`:
//! the option cards at lines 1905-1907, the feature rows at 1908, the log at 1915, the never list
//! at 1916 and the keys at 1920. The keys do not list Esc, which the shell draws for every screen.
//! The log and the stored tier are written to a company file by the test itself, so the expected
//! rows are the ones the test wrote (`derived`). None is an output of the code under test.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    reason = "test code: rule B2 lets tests unwrap and panic; layout checks measure in points"
)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use egui::epaint::{ClippedShape, Shape};
use egui::{Color32, Event, FontFamily, Key, Modifiers, Pos2, RawInput, Rect, vec2};
use ledgerly_api::{AiTier, Api};
use ledgerly_app::screens::ai_settings::{
    AiSettingsScreen, CLOUD_STATUS, LOG_EMPTY, LOG_META, LOG_META_EMPTY, NEVER, OFF_NOTE,
    OPTION_CLOUD, OPTION_LOCAL, OPTION_OFF, UNSET_NOTE, key_hints, read_probe,
};
use ledgerly_app::shell::Shell;
use ledgerly_app::shell::topbar::ai_chip;
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{Clock, CompanyId, EventSeq, StateCode, UserId};
use ledgerly_model::{Address, Company, EntityKind, Features, NegativeStockPolicy, ProposalRecord};
use ledgerly_store::{Actor, AuditMeta, Event as StoreEvent, SqliteStore, Store};
use time::OffsetDateTime;
use time::macros::date;

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(29).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    }
}

/// A new company file, with the given AI proposal rows written to its log.
fn file_with_log(name: &str, rows: &[ProposalRecord]) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    drop(Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap());
    if !rows.is_empty() {
        let mut store = SqliteStore::open(&path).unwrap();
        for row in rows {
            let meta = AuditMeta {
                actor: Actor::User(UserId(1)),
                at: OffsetDateTime::UNIX_EPOCH,
                reason: None,
                app_version: "test".to_owned(),
            };
            store
                .commit(&[StoreEvent::AiProposalRecorded(row.clone())], &meta)
                .unwrap();
        }
    }
    path
}

fn new_file(name: &str) -> PathBuf {
    file_with_log(name, &[])
}

fn proposal() -> ProposalRecord {
    ProposalRecord {
        model: "local-small".to_owned(),
        model_version: "1".to_owned(),
        prompt_template_version: "cloud-v1".to_owned(),
        input_hash: "0123456789abcdef".to_owned(),
        output_json: "{\"a\":1}".to_owned(),
        human_edits_json: None,
        accepted: true,
    }
}

/// The app context with the company file open on a worker.
fn open(path: &Path) -> AppCtx {
    let api = Api::open(path).unwrap();
    AppCtx {
        api: Some(ApiWorker::spawn(api).unwrap()),
        ..AppCtx::default()
    }
}

/// Every piece of text the frame drew, joined. Galleys hold the text of labels and cells.
fn drawn_text(shapes: &[ClippedShape]) -> String {
    fn walk(shape: &Shape, out: &mut String) {
        if let Shape::Text(text) = shape {
            out.push_str(text.galley.text());
            out.push('\n');
        } else if let Shape::Vec(shapes) = shape {
            for shape in shapes {
                walk(shape, out);
            }
        }
    }
    let mut out = String::new();
    for clipped in shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

/// One frame with the given keys pressed. Returns what the frame drew.
fn frame(screen: &mut AiSettingsScreen, app: &mut AppCtx, keys: &[(Key, Modifiers)]) -> String {
    let ctx = egui::Context::default();
    let events = keys
        .iter()
        .map(|&(key, modifiers)| Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        })
        .collect();
    let input = RawInput {
        events,
        ..RawInput::default()
    };
    let out = ctx.run_ui(input, |ui| {
        let _stay = screen.ui(ui, app);
    });
    let text = drawn_text(&out.shapes);
    out.drop_without_applying_deltas();
    text
}

/// Frames until `done` holds, so the worker's answer has been taken. Returns the last frame's
/// text. Fails the test when the answer never comes.
fn settle(
    screen: &mut AiSettingsScreen,
    app: &mut AppCtx,
    keys: &[(Key, Modifiers)],
    done: impl Fn(&AiSettingsScreen) -> bool,
) -> String {
    let mut text = frame(screen, app, keys);
    for _ in 0..2000 {
        if done(screen) {
            return text;
        }
        std::thread::sleep(Duration::from_millis(2));
        text = frame(screen, app, &[]);
    }
    panic!("the worker's answer did not arrive");
}

const NONE: Modifiers = Modifiers::NONE;

/// The stored tiers and every event that changed them, from the company file.
fn ai_settings(path: &Path) -> Vec<(String, String, String)> {
    let store = SqliteStore::open(path).unwrap();
    store
        .events(EventSeq(1), 1000)
        .unwrap()
        .into_iter()
        .filter_map(|record| {
            if let StoreEvent::SettingChanged { key, before, after } = record.event {
                Some((key, before, after))
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn the_three_states_render_the_mock_wording_and_keys() {
    let path = new_file("w4_states.ledgerly");
    let mut app = open(&path);
    let mut screen = AiSettingsScreen::default();

    // Nothing chosen yet: the three options, the note that nothing is sent, and no log rows.
    let unset = settle(&mut screen, &mut app, &[], AiSettingsScreen::loaded);
    for (title, body) in [OPTION_OFF, OPTION_LOCAL, OPTION_CLOUD] {
        assert!(unset.contains(title), "missing option title {title}");
        assert!(unset.contains(body), "missing option body {body}");
    }
    assert!(unset.contains(UNSET_NOTE));
    assert!(unset.contains(LOG_EMPTY));
    assert!(unset.contains(LOG_META_EMPTY));
    assert!(!unset.contains("Ctrl+E exports it"));
    assert_eq!(screen.stored(), None);

    // On this computer (mock state ai-settings.local, index.html:1906 and 1907): the local panel.
    let local = settle(&mut screen, &mut app, &[(Key::Num2, NONE)], |s| {
        s.stored() == Some(AiTier::Local)
    });
    assert!(local.contains("Not installed. This build has no local bill reader."));
    assert!(!local.contains("Nothing: off"));
    assert!(local.contains("Narration, date, amount, on this computer"));
    assert!(local.contains("Not built yet"));
    assert_eq!(
        screen.panel(),
        ledgerly_app::screens::ai_settings::Panel::Local
    );
    assert_eq!(
        screen.keys(&ledgerly_app::i18n::Tr::default()),
        key_hints(false),
        "the keys without a log"
    );

    // Off (mock state ai-settings.off, index.html:1905 and the off note): every feature is off.
    let off = settle(&mut screen, &mut app, &[(Key::Num1, NONE)], |s| {
        s.stored() == Some(AiTier::Off)
    });
    assert!(off.contains(OFF_NOTE), "the off note is missing");
    assert!(off.contains("Nothing: off"));
    assert!(!off.contains("Not installed. This build has no local bill reader."));

    // Cloud (mock state ai-settings.cloud, index.html:1907): the engine refuses it, and the
    // panel says why. The stored tier stays Off.
    let cloud = settle(&mut screen, &mut app, &[(Key::Num3, NONE)], |s| {
        s.notice().is_some()
    });
    assert!(cloud.contains("Cloud, your own key"));
    assert!(cloud.contains(CLOUD_STATUS));
    assert!(cloud.contains("keychain"), "the engine's reason is shown");
    assert_eq!(screen.stored(), Some(AiTier::Off));
    assert!(cloud.contains("Not built yet"));
}

#[test]
fn the_never_list_is_the_mock_list_on_every_state() {
    // index.html:1916: the five lines of "What AI may never do here".
    let path = new_file("w4_never.ledgerly");
    let mut app = open(&path);
    let mut screen = AiSettingsScreen::default();
    let text = settle(&mut screen, &mut app, &[], AiSettingsScreen::loaded);
    assert!(text.contains("What AI may never do here"));
    for line in NEVER {
        assert!(text.contains(line), "missing: {line}");
    }
    assert_eq!(NEVER.len(), 5);
}

#[test]
fn choosing_a_tier_is_kept_in_the_file_and_the_chip_follows_it() {
    let path = new_file("w4_chip.ledgerly");
    let mut app = open(&path);
    let mut screen = AiSettingsScreen::default();
    settle(&mut screen, &mut app, &[], AiSettingsScreen::loaded);
    assert_eq!(
        ai_chip(app.ai_tier, &ledgerly_app::i18n::Tr::default()),
        "AI: not set up"
    );

    frame(&mut screen, &mut app, &[(Key::Num2, NONE)]);
    settle(&mut screen, &mut app, &[], |s| {
        s.stored() == Some(AiTier::Local)
    });
    assert_eq!(app.ai_tier, Some(AiTier::Local));
    assert_eq!(
        ai_chip(app.ai_tier, &ledgerly_app::i18n::Tr::default()),
        "AI on this computer"
    );

    frame(&mut screen, &mut app, &[(Key::Num1, NONE)]);
    settle(&mut screen, &mut app, &[], |s| {
        s.stored() == Some(AiTier::Off)
    });
    assert_eq!(
        ai_chip(app.ai_tier, &ledgerly_app::i18n::Tr::default()),
        "AI off"
    );

    // Read back from the file after the company is closed and opened again.
    drop(app);
    let reopened = Api::open(&path).unwrap();
    assert_eq!(reopened.ai_tier().unwrap(), Some(AiTier::Off));
    assert_eq!(
        ai_settings(&path)
            .last()
            .map(|(_, _, after)| after.as_str()),
        Some("off")
    );
}

#[test]
fn the_top_bar_chip_reads_the_stored_tier_when_the_shell_opens_the_file() {
    let path = new_file("w4_shell_chip.ledgerly");
    {
        let mut api = Api::open(&path).unwrap();
        api.set_ai_tier(AiTier::Local).unwrap();
    }
    let mut shell = Shell::new(Box::new(AiSettingsScreen::default()));
    shell.app_ctx_mut().api = Some(ApiWorker::spawn(Api::open(&path).unwrap()).unwrap());
    for _ in 0..2000 {
        shell.sync_books();
        if shell.app_ctx_mut().ai_tier.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(shell.app_ctx_mut().ai_tier, Some(AiTier::Local));
    assert_eq!(
        ai_chip(
            shell.app_ctx_mut().ai_tier,
            &ledgerly_app::i18n::Tr::default()
        ),
        "AI on this computer"
    );
}

#[test]
fn the_log_shows_the_file_rows_and_never_sample_rows() {
    let path = file_with_log("w4_log_rows.ledgerly", &[proposal()]);
    let mut app = open(&path);
    let mut screen = AiSettingsScreen::default();
    let text = settle(&mut screen, &mut app, &[], AiSettingsScreen::loaded);
    assert!(text.contains(LOG_META), "the log's meta line with Ctrl+E");
    assert!(text.contains("local-small"));
    assert!(text.contains("{\"a\":1}"));
    assert!(text.contains("Accepted"));
    assert!(
        text.contains("0123456789ab…"),
        "the hash is shortened for the table"
    );
    // The mock's sample rows (index.html:1896-1900) must not appear for a real log.
    for sample in [
        "IMG_2041",
        "Sharma Properties",
        "5 statement lines",
        "Draft payment",
    ] {
        assert!(!text.contains(sample), "sample row leaked: {sample}");
    }
    assert!(
        screen
            .keys(&ledgerly_app::i18n::Tr::default())
            .iter()
            .any(|k| k.keys == "Ctrl+E" && k.label == "Export log")
    );
}

#[test]
fn an_empty_log_says_so_and_offers_no_export() {
    let path = new_file("w4_log_empty.ledgerly");
    let mut app = open(&path);
    let mut screen = AiSettingsScreen::default();
    let text = settle(&mut screen, &mut app, &[], AiSettingsScreen::loaded);
    assert!(text.contains(LOG_EMPTY));
    assert!(!text.contains("Ctrl+E exports it"));
    assert_eq!(screen.log().len(), 0);
    assert!(
        !screen
            .keys(&ledgerly_app::i18n::Tr::default())
            .iter()
            .any(|k| k.keys == "Ctrl+E")
    );
    // Ctrl+E with no rows does nothing: no save dialog, no notice.
    frame(&mut screen, &mut app, &[(Key::E, Modifiers::COMMAND)]);
    assert_eq!(screen.notice(), None);
}

#[test]
fn the_key_hints_match_the_mock_keys() {
    // index.html:1920: 1 Off, 2 On this computer, 3 Cloud, Ctrl+E Export log. The shell adds Esc.
    let without = key_hints(false);
    let labels: Vec<(&str, &str)> = without
        .iter()
        .map(|k| (k.keys.as_str(), k.label.as_str()))
        .collect();
    assert_eq!(
        labels,
        vec![
            ("", "AI and privacy"),
            ("1", "Off"),
            ("2", "On this computer"),
            ("3", "Cloud"),
        ]
    );
    assert_eq!(key_hints(true).len(), 5);
}

#[test]
fn no_cloud_key_reaches_the_company_file() {
    let path = new_file("w4_no_key_file.ledgerly");
    let mut app = open(&path);
    let mut screen = AiSettingsScreen::default();
    settle(&mut screen, &mut app, &[], AiSettingsScreen::loaded);
    frame(&mut screen, &mut app, &[(Key::Num2, NONE)]);
    settle(&mut screen, &mut app, &[], |s| {
        s.stored() == Some(AiTier::Local)
    });
    frame(&mut screen, &mut app, &[(Key::Num3, NONE)]);
    settle(&mut screen, &mut app, &[], |s| s.notice().is_some());
    drop(app);

    // Only the tier is written. The cloud choice is refused, so no key-like text ever is.
    let changes = ai_settings(&path);
    assert_eq!(
        changes.len(),
        1,
        "only the local choice is stored: {changes:?}"
    );
    assert!(changes.iter().all(|(key, _, _)| key == "ai.tier"));
    let bytes = std::fs::read(&path).unwrap();
    assert!(!bytes.windows(3).any(|w| w == b"sk-"));
    assert!(!bytes.windows(8).any(|w| w == b"keychain"));
}

/// What one frame painted, as the layout tests read it.
struct Painted {
    /// Every text galley, one per line.
    text: String,
    /// Every filled rectangle, with its fill.
    fills: Vec<(Rect, Color32)>,
    /// Every stroked rectangle, with its stroke colour.
    strokes: Vec<(Rect, Color32)>,
    /// Every line segment, as the rectangle its two ends span.
    lines: Vec<Rect>,
    /// The text of every galley laid out justified.
    justified: Vec<String>,
    /// The text of every galley set in the bold family.
    bold: Vec<String>,
    /// Where every text galley is drawn, as the rectangle its layout covers.
    texts: Vec<Rect>,
}

fn walk_painted(shape: &Shape, out: &mut Painted) {
    match shape {
        Shape::Text(text) => {
            let line = text.galley.text().to_owned();
            out.texts
                .push(Rect::from_min_size(text.pos, text.galley.size()));
            out.text.push_str(&line);
            out.text.push('\n');
            if text.galley.job.justify {
                out.justified.push(line.clone());
            }
            let bold = text
                .galley
                .job
                .sections
                .first()
                .is_some_and(|s| s.format.font_id.family != FontFamily::Proportional);
            if bold {
                out.bold.push(line);
            }
        }
        Shape::Rect(rect) => {
            out.fills.push((rect.rect, rect.fill));
            if rect.stroke.width > 0.0 {
                out.strokes.push((rect.rect, rect.stroke.color));
            }
        }
        Shape::LineSegment { points, .. } => {
            out.lines.push(Rect::from_two_pos(points[0], points[1]));
        }
        Shape::Vec(shapes) => {
            for shape in shapes {
                walk_painted(shape, out);
            }
        }
        Shape::Noop
        | Shape::Circle(_)
        | Shape::Ellipse(_)
        | Shape::Path(_)
        | Shape::Band(_)
        | Shape::Mesh(_)
        | Shape::QuadraticBezier(_)
        | Shape::CubicBezier(_)
        | Shape::Callback(_) => {}
    }
}

/// One frame on the content area beside the 208 wide sidebar and the page margins: 1024 by 900, reading what it painted. The context is the caller's, so
/// the layout probes the screen records stay readable after the frame.
fn paint(
    ctx: &egui::Context,
    screen: &mut AiSettingsScreen,
    app: &mut AppCtx,
    keys: &[(Key, Modifiers)],
) -> Painted {
    let events = keys
        .iter()
        .map(|&(key, modifiers)| Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        })
        .collect();
    let input = RawInput {
        events,
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1024.0, 900.0))),
        ..RawInput::default()
    };
    let out = ctx.run_ui(input, |ui| {
        let _stay = screen.ui(ui, app);
    });
    let mut painted = Painted {
        text: String::new(),
        fills: Vec::new(),
        strokes: Vec::new(),
        lines: Vec::new(),
        justified: Vec::new(),
        texts: Vec::new(),
        bold: Vec::new(),
    };
    for clipped in &out.shapes {
        walk_painted(&clipped.shape, &mut painted);
    }
    out.drop_without_applying_deltas();
    painted
}

/// Frames until `done` holds, then one more frame, so the layout is settled.
fn paint_until(
    ctx: &egui::Context,
    screen: &mut AiSettingsScreen,
    app: &mut AppCtx,
    keys: &[(Key, Modifiers)],
    done: impl Fn(&AiSettingsScreen) -> bool,
) -> Painted {
    paint(ctx, screen, app, keys);
    for _ in 0..2000 {
        if done(screen) {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
        paint(ctx, screen, app, &[]);
    }
    assert!(done(screen), "the worker's answer did not arrive");
    paint(ctx, screen, app, &[])
}

/// The local state, laid out. Returns the context (for the layout probes) and the last frame.
fn local_layout(name: &str) -> (egui::Context, Painted) {
    let path = new_file(name);
    let mut app = open(&path);
    let mut screen = AiSettingsScreen::default();
    let ctx = egui::Context::default();
    // The bold family is installed by the theme, as the app does at start-up.
    ledgerly_app::theme::apply(&ctx, ledgerly_app::theme::ThemeKind::Light);
    paint_until(&ctx, &mut screen, &mut app, &[], AiSettingsScreen::loaded);
    let painted = paint_until(&ctx, &mut screen, &mut app, &[(Key::Num2, NONE)], |s| {
        s.stored() == Some(AiTier::Local)
    });
    (ctx, painted)
}

/// A layout probe the screen records each frame. Missing means the screen did not draw it.
fn probe<T: Clone + Send + Sync + 'static>(ctx: &egui::Context, name: &str) -> T {
    read_probe::<T>(ctx, name).unwrap_or_else(|| panic!("no layout probe named {name}"))
}

#[test]
fn the_feature_table_sits_beside_the_never_card_without_overlap() {
    // The mock's row: the table takes about two thirds of the width, the never card the rest
    // (side-by-side comparison with design/mock-v2/index.html:1908-1916, screenshot in
    // claude-tmp/sbs). The two-thirds figure is read from that comparison, not computed.
    let (ctx, _) = local_layout("w4_layout_table.ledgerly");
    let table: Rect = probe(&ctx, "ai-settings.features");
    let never: Rect = probe(&ctx, "ai-settings.never");
    assert!(
        table.max.x <= never.min.x,
        "the table {table:?} runs into the never card {never:?}"
    );
    let share = table.width() / (table.width() + never.width());
    assert!(
        (0.6..=0.75).contains(&share),
        "the table takes {share:.2} of the row, the mock about two thirds"
    );
}

#[test]
fn the_selected_tier_card_has_the_accent_stroke_and_fill_and_the_others_do_not() {
    let (ctx, painted) = local_layout("w4_layout_accent.ledgerly");
    let accent: Color32 = probe(&ctx, "ai-settings.accent");
    let accent_soft: Color32 = probe(&ctx, "ai-settings.accent_soft");
    let card = |n: usize| -> Rect { probe(&ctx, &format!("ai-settings.card.{n}")) };
    let stroked_with = |rect: Rect, colour: Color32| {
        painted
            .strokes
            .iter()
            .any(|(r, c)| *c == colour && (r.center() - rect.center()).length() < 1.0)
    };
    let filled_with = |rect: Rect, colour: Color32| {
        painted
            .fills
            .iter()
            .any(|(r, c)| *c == colour && (r.center() - rect.center()).length() < 1.0)
    };
    // Local is the second card (index 1) in this state.
    assert!(
        stroked_with(card(1), accent),
        "selected card has no accent stroke"
    );
    assert!(
        filled_with(card(1), accent_soft),
        "selected card has no accent fill"
    );
    assert!(
        !stroked_with(card(0), accent),
        "an unselected card has the accent stroke"
    );
    assert!(
        !stroked_with(card(2), accent),
        "an unselected card has the accent stroke"
    );
}

#[test]
fn each_option_card_draws_its_icon_before_a_bold_title() {
    let (ctx, painted) = local_layout("w4_layout_icons.ledgerly");
    for n in 0..3 {
        let icon: Rect = probe(&ctx, &format!("ai-settings.icon.{n}"));
        let title: Rect = probe(&ctx, &format!("ai-settings.title.{n}"));
        assert!(
            icon.max.x <= title.min.x,
            "card {n}: the icon {icon:?} is not before the title {title:?}"
        );
    }
    for title in ["Off", "On this computer", "Cloud, your own key"] {
        assert!(
            painted.bold.iter().any(|t| t == title),
            "the title {title} is not set in the bold family"
        );
    }
}

#[test]
fn the_option_bodies_are_left_aligned_not_justified() {
    let (_, painted) = local_layout("w4_layout_justify.ledgerly");
    assert!(
        painted.text.contains("Redaction runs"),
        "the cloud body is drawn"
    );
    assert!(
        !painted
            .justified
            .iter()
            .any(|t| t.contains("Redaction runs")),
        "the cloud card's body is justified: {:?}",
        painted.justified
    );
}

#[test]
fn the_local_panel_spans_the_width_of_the_three_option_cards() {
    let (ctx, _) = local_layout("w4_layout_full.ledgerly");
    let local: Rect = probe(&ctx, "ai-settings.local");
    let cards: Vec<Rect> = (0..3)
        .map(|n| probe(&ctx, &format!("ai-settings.card.{n}")))
        .collect();
    let left = cards.iter().map(|r| r.min.x).fold(f32::INFINITY, f32::min);
    let right = cards
        .iter()
        .map(|r| r.max.x)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        (local.min.x - left).abs() < 1.0 && (local.max.x - right).abs() < 1.0,
        "the local panel {local:?} does not span the cards {cards:?}"
    );
}

#[test]
fn the_local_panel_rows_are_separated_by_rules() {
    let (ctx, painted) = local_layout("w4_layout_rules.ledgerly");
    let local: Rect = probe(&ctx, "ai-settings.local");
    let rules = painted
        .lines
        .iter()
        .filter(|l| local.expand(2.0).contains_rect(**l))
        .count();
    assert!(
        rules >= 3,
        "the local panel has {rules} rules; the mock has one under each row"
    );
}

#[test]
fn the_feature_table_has_an_on_column_and_no_status_column() {
    let (_, painted) = local_layout("w4_layout_on.ledgerly");
    assert!(
        painted.text.lines().any(|l| l == "On"),
        "the feature table has no On header"
    );
    assert!(
        !painted.text.lines().any(|l| l == "Status"),
        "the feature table still has a Status column"
    );
}

#[test]
fn the_feature_text_clears_the_on_column_with_room_to_spare() {
    // The mock sets "What the model sees" text in its column with room to spare (index.html:
    // 1908-1910). In the captured app the longest line ran to the On column and was cut ("on
    // this co..."), so every feature line must end at least 12 before the On header.
    let (ctx, painted) = local_layout("w4_layout_text.ledgerly");
    let table: Rect = probe(&ctx, "ai-settings.features");
    let on_header = painted
        .texts
        .iter()
        .zip(painted.text.lines())
        .find(|(_, line)| *line == "On")
        .map(|(rect, _)| rect.min.x)
        .expect("the feature table has no On header");
    let feature_lines: Vec<(&Rect, &str)> = painted
        .texts
        .iter()
        .zip(painted.text.lines())
        .filter(|(rect, _)| table.intersects(**rect) && rect.min.x < on_header)
        .filter(|(_, line)| line.contains("on this computer") || line.starts_with("Nothing"))
        .collect();
    assert!(!feature_lines.is_empty(), "no feature lines were drawn");
    for (rect, line) in feature_lines {
        assert!(
            on_header - rect.max.x >= 12.0,
            "\"{line}\" ends at {} and the On column starts at {on_header}",
            rect.max.x
        );
    }
}
