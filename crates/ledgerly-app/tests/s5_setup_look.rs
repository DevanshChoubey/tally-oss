//! The company setup wizard's look (mock `SCREENS.setup`, `design/mock-v2/index.html` 1855-1895,
//! and its CSS at 311-323): one card with the step rail, a rule and the step's pane; round rail
//! markers; option cards with the accent on the chosen one; a secondary Back and a primary
//! Continue, each with its keycap; launch cards and "Go to Home" on Done; the key bar's name and
//! hints.
//!
//! Provenance. Every expected size, wording and colour role is read from the mock at the line
//! named next to it. Colours are the theme's tokens, which are the mock's `:root` variables; the
//! test asks which token a shape was painted in, never what the screen computed. The wizard is
//! driven as a user would: keys and typed text, the same script as the capture harness
//! (`examples/ui_snapshots.rs`, `setup_shot`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    reason = "test code: rule B2 lets tests unwrap and panic; layout checks measure in points"
)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

/// How long a test waits for the background company file; far above any loaded run's need, so
/// only a create that never answers fails it.
const CREATE_DEADLINE: Duration = Duration::from_secs(120);

use egui::epaint::{ClippedShape, Shape};
use egui::{Color32, Event, FontFamily, Key, Modifiers, Pos2, RawInput, Rect, vec2};
use ledgerly_app::screens::setup::SetupScreen;
use ledgerly_app::shell::KeyHint;
use ledgerly_app::theme::{self, ThemeKind, Tokens};
use ledgerly_app::ui::probe::read_probe;
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{Clock, OffsetDateTime};

/// What a frame painted, sorted by kind.
#[derive(Default)]
struct Painted {
    /// Every text galley: its text, where it is drawn, and whether it is in the bold family.
    texts: Vec<(String, Rect, bool)>,
    /// Every rectangle: where, its fill and its stroke colour (transparent when unstroked).
    rects: Vec<(Rect, Color32, Color32)>,
    /// Every circle: centre, radius, fill and stroke colour.
    circles: Vec<(Pos2, f32, Color32, Color32)>,
    /// Every stroked path (icons): the rectangle it spans and its colour.
    paths: Vec<(Rect, Color32)>,
    /// Every line segment, as the rectangle its two ends span.
    lines: Vec<Rect>,
}

impl Painted {
    fn walk(&mut self, shape: &Shape) {
        match shape {
            Shape::Text(text) => {
                let bold = text
                    .galley
                    .job
                    .sections
                    .first()
                    .is_some_and(|s| s.format.font_id.family != FontFamily::Proportional);
                self.texts.push((
                    text.galley.text().to_owned(),
                    Rect::from_min_size(text.pos, text.galley.size()),
                    bold,
                ));
            }
            Shape::Rect(r) => {
                let stroke = if r.stroke.width > 0.0 {
                    r.stroke.color
                } else {
                    Color32::TRANSPARENT
                };
                self.rects.push((r.rect, r.fill, stroke));
            }
            Shape::Circle(c) => {
                let stroke = if c.stroke.width > 0.0 {
                    c.stroke.color
                } else {
                    Color32::TRANSPARENT
                };
                self.circles.push((c.center, c.radius, c.fill, stroke));
            }
            Shape::Path(p) => {
                let colour = match &p.stroke.color {
                    egui::epaint::ColorMode::Solid(c) => *c,
                    egui::epaint::ColorMode::UV(_) => Color32::TRANSPARENT,
                };
                self.paths.push((Rect::from_points(&p.points), colour));
            }
            Shape::LineSegment { points, .. } => {
                self.lines.push(Rect::from_two_pos(points[0], points[1]));
            }
            Shape::Vec(shapes) => {
                for s in shapes {
                    self.walk(s);
                }
            }
            Shape::Noop
            | Shape::Ellipse(_)
            | Shape::Band(_)
            | Shape::Mesh(_)
            | Shape::QuadraticBezier(_)
            | Shape::CubicBezier(_)
            | Shape::Callback(_) => {}
        }
    }

    /// The texts drawn inside `area`.
    fn texts_in(&self, area: Rect) -> Vec<&str> {
        self.texts
            .iter()
            .filter(|(_, r, _)| area.expand(1.0).contains_rect(*r))
            .map(|(t, _, _)| t.as_str())
            .collect()
    }

    /// Whether a rectangle centred on `area` (within a point) is filled with `fill`.
    fn filled(&self, area: Rect, fill: Color32) -> bool {
        self.rects
            .iter()
            .any(|(r, f, _)| *f == fill && (r.center() - area.center()).length() < 1.0)
    }

    /// Whether a rectangle centred on `area` (within a point) is stroked in `edge`.
    fn stroked(&self, area: Rect, edge: Color32) -> bool {
        self.rects
            .iter()
            .any(|(r, _, s)| *s == edge && (r.center() - area.center()).length() < 1.0)
    }

    /// The marker circle centred in `area`, if one was painted: fill and stroke colours.
    fn marker(&self, area: Rect) -> Option<(Color32, Color32)> {
        self.circles
            .iter()
            .find(|(c, radius, _, _)| *radius > 8.0 && (*c - area.center()).length() < 1.0)
            .map(|(_, _, fill, stroke)| (*fill, *stroke))
    }
}

/// The wizard with its context, driven frame by frame on a 1024 by 900 content area.
struct Wizard {
    ctx: egui::Context,
    screen: SetupScreen,
    app: AppCtx,
}

const NONE: Modifiers = Modifiers::NONE;

fn key(k: Key) -> Event {
    Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: NONE,
    }
}

fn click_at(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: NONE,
    }
}

impl Wizard {
    fn new() -> Self {
        let ctx = egui::Context::default();
        // The bold family and the tokens are installed by the theme, as the app does at start-up.
        theme::apply(&ctx, ThemeKind::Light);
        let mut wizard = Self {
            ctx,
            screen: SetupScreen::with_clock(Clock::Fixed(OffsetDateTime::UNIX_EPOCH)),
            app: AppCtx::default(),
        };
        // Two frames, so rows of option cards settle at one height.
        wizard.frame(vec![]);
        wizard.frame(vec![]);
        wizard
    }

    /// One frame with `events`. Returns what it painted and what the screen asked for.
    fn frame(&mut self, events: Vec<Event>) -> (Painted, ScreenAction) {
        let input = RawInput {
            events,
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1024.0, 900.0))),
            ..RawInput::default()
        };
        let mut action = ScreenAction::Stay;
        let out = self.ctx.run_ui(input, |ui| {
            action = self.screen.ui(ui, &mut self.app);
        });
        let mut painted = Painted::default();
        for ClippedShape { shape, .. } in &out.shapes {
            painted.walk(shape);
        }
        out.drop_without_applying_deltas();
        (painted, action)
    }

    /// Frames, one per event, then two quiet ones, returning the last.
    fn script(&mut self, events: Vec<Event>) -> Painted {
        for e in events {
            self.frame(vec![e]);
        }
        self.frame(vec![]);
        self.frame(vec![]).0
    }

    fn probe<T: Clone + Send + Sync + 'static>(&self, name: &str) -> T {
        read_probe::<T>(&self.ctx, name).unwrap_or_else(|| panic!("no layout probe named {name}"))
    }

    /// Frames until the background create has finished and Done (step 7 of 7) is on screen.
    ///
    /// The wait is bounded by wall-clock time, not by a count of frames: creating the company file
    /// is real disk work (a sqlite file, with fsync) that takes 0.15 s alone but more than the old
    /// 2000-frame budget (about 9 s) when the whole workspace's 3000 tests and other builds
    /// share the disk and the cores. That run failed at 8.975 s with the wizard still on step 6.
    fn wait_for_done(&mut self) {
        let deadline = Instant::now() + CREATE_DEADLINE;
        while self.bar_name() != "Setup, step 7 of 7" && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
            self.frame(vec![]);
        }
        assert_eq!(
            self.bar_name(),
            "Setup, step 7 of 7",
            "the company was not created"
        );
    }

    /// The key bar's name, which names the step.
    fn bar_name(&self) -> String {
        self.screen.key_bar_name(&ledgerly_app::i18n::Tr::default())
    }
}

fn light() -> Tokens {
    theme::tokens(ThemeKind::Light)
}

/// Where the drive test writes its company file. Removed first: the wizard never overwrites.
fn fresh_path(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    path
}

/// The capture harness's company script (`setup_shot` in `examples/ui_snapshots.rs`): Enter to
/// Company, the name, two Tabs to City (Type is between), the city, a Tab to the GSTIN, four Tabs
/// to the file (State, GST filing and Books begin are between), two Tabs off the fields. The
/// harness leaves the file blank so the capture shows the default path; a test needs a path it
/// can write to, so it types one.
fn company_script(file: &str) -> Vec<Event> {
    let mut events = vec![
        key(Key::Enter),
        Event::Text("Godavari Drip & Pump Works".to_owned()),
    ];
    events.extend([key(Key::Tab), key(Key::Tab)]);
    events.push(Event::Text("Pune".to_owned()));
    events.push(key(Key::Tab));
    // index.html:1868, the mock's GSTIN; its first two digits fill the state.
    events.push(Event::Text("27ZZZZZ0000Z1ZQ".to_owned()));
    events.extend([key(Key::Tab), key(Key::Tab), key(Key::Tab), key(Key::Tab)]);
    events.push(Event::Text(file.to_owned()));
    events.extend([key(Key::Tab), key(Key::Tab)]);
    events
}

#[test]
fn the_key_bar_names_the_step_and_lists_enter_esc_and_tab() {
    let mut w = Wizard::new();
    // index.html:1891: [['','Setup, step N of 7'],['Enter','Continue'],['Esc','Back'],
    // ['Tab','Next field']]. Start is step 1 of the 7 STEPS (index.html:1855).
    let expected = vec![
        KeyHint::new("", "Setup, step 1 of 7"),
        KeyHint::new("Enter", "Continue"),
        KeyHint::new("Esc", "Back"),
        KeyHint::new("Tab", "Next field"),
    ];
    assert_eq!(w.screen.keys(&ledgerly_app::i18n::Tr::default()), expected);
    // The shell draws the name from `key_bar_name` and drops a first hint with no key
    // (index.html:913, renderKeys), so the name must come from there too.
    assert_eq!(
        w.screen.key_bar_name(&ledgerly_app::i18n::Tr::default()),
        "Setup, step 1 of 7"
    );
    w.script(vec![key(Key::Enter)]);
    // Company is the second of the STEPS (index.html:1855).
    assert_eq!(w.bar_name(), "Setup, step 2 of 7");
}

#[test]
fn the_wizard_is_one_card_with_a_240_point_rail_and_a_rule_beside_it() {
    let mut w = Wizard::new();
    let painted = w.script(vec![]);
    let card: Rect = w.probe("setup.card");
    let rail: Rect = w.probe("setup.rail");
    // index.html:311: `.setup{grid-template-columns:240px 1fr; max-width:980px}`.
    assert!(
        (rail.width() - 240.0).abs() < 1.0,
        "the rail is {} wide",
        rail.width()
    );
    assert!(card.width() <= 980.5, "the card is {} wide", card.width());
    // The card is a surface with the border (`.card`), not a group frame.
    assert!(
        painted.filled(card, light().surface),
        "the card is not a surface"
    );
    // index.html:312: `.steps{border-right:var(--bw) solid var(--border)}`, the card's height.
    let rule = painted.lines.iter().any(|l| {
        (l.min.x - rail.max.x).abs() < 1.5
            && l.width() < 1.0
            && l.min.y <= card.min.y + 1.5
            && l.max.y >= card.max.y - 1.5
    });
    assert!(
        rule,
        "no rule from the card's top to its bottom beside the rail"
    );
}

#[test]
fn rail_markers_are_a_check_for_passed_steps_the_accent_for_the_current_and_numbers_ahead() {
    let mut w = Wizard::new();
    let painted = w.script(vec![key(Key::Enter)]);
    let t = light();
    // On Company (index 1): Start is passed, Company current, the rest ahead (index.html:1860).
    let marker = |i: usize| -> Rect { w.probe(&format!("setup.rail.marker.{i}")) };
    // index.html:314: a 22 px circle.
    assert!(
        (marker(0).width() - 22.0).abs() < 0.5,
        "the marker is not 22 wide"
    );
    // index.html:315: `.done i{background:var(--ok)}` with `ic('check')` in `--surface`.
    let (fill, _) = painted
        .marker(marker(0))
        .expect("no marker circle on Start");
    assert_eq!(fill, t.ok, "a passed step's marker is not filled with ok");
    assert!(
        painted
            .paths
            .iter()
            .any(|(r, c)| *c == t.surface && marker(0).contains_rect(*r)),
        "a passed step's marker has no check drawn in the surface colour"
    );
    assert!(
        painted.texts_in(marker(0)).is_empty(),
        "a passed step's marker shows a number"
    );
    // index.html:317: the current step's marker is the accent, its number on-accent.
    let (fill, _) = painted
        .marker(marker(1))
        .expect("no marker circle on Company");
    assert_eq!(
        fill, t.accent,
        "the current step's marker is not the accent"
    );
    assert_eq!(painted.texts_in(marker(1)), vec!["2"]);
    // index.html:316: the current row is accent-soft.
    let row: Rect = w.probe("setup.rail.row.1");
    assert!(
        painted.filled(row, t.accent_soft),
        "the current row is not accent-soft"
    );
    // index.html:314: steps ahead are numbers in a circle with the strong border.
    for i in 2..7 {
        let (fill, stroke) = painted
            .marker(marker(i))
            .unwrap_or_else(|| panic!("no marker circle on step {i}"));
        assert_eq!(
            stroke, t.border_strong,
            "step {i}'s circle has no strong border"
        );
        assert_ne!(fill, t.ok, "step {i} ahead is drawn as passed");
        assert_eq!(
            painted.texts_in(marker(i)),
            vec![(i + 1).to_string().as_str()]
        );
    }
}

#[test]
fn the_chosen_option_card_has_the_accent_edge_and_fill_and_an_icon_before_a_bold_title() {
    let mut w = Wizard::new();
    let painted = w.script(vec![]);
    let t = light();
    let card = |n: usize| -> Rect { w.probe(&format!("setup.option.{n}")) };
    // index.html:1863: "Create a company" is the chosen card on Start; index.html:321 gives the
    // chosen card the accent border and the accent-soft fill.
    assert!(
        painted.stroked(card(0), t.accent),
        "the chosen card has no accent edge"
    );
    assert!(
        painted.filled(card(0), t.accent_soft),
        "the chosen card has no accent fill"
    );
    for n in 1..4 {
        assert!(
            !painted.stroked(card(n), t.accent),
            "card {n} has the accent edge"
        );
    }
    // index.html:1857: `ic(icon)` then `<b>title</b>`.
    for (n, title) in [
        "Create a company",
        "Import from another accounting app",
        "Open a Ledgerly file",
        "Try the sample company",
    ]
    .into_iter()
    .enumerate()
    {
        assert!(
            painted
                .texts
                .iter()
                .any(|(text, r, bold)| text == title && *bold && card(n).contains_rect(*r)),
            "card {n} has no bold title {title}"
        );
        assert!(
            painted
                .paths
                .iter()
                .any(|(r, _)| card(n).contains_rect(*r) && r.max.x < card(n).center().x),
            "card {n} has no icon at its left"
        );
    }
}

#[test]
fn back_is_a_secondary_button_and_continue_a_primary_one_each_with_its_keycap() {
    let mut w = Wizard::new();
    let t = light();
    let painted = w.script(vec![]);
    // index.html:1861: Start has no Back (`<span></span>`) and the primary "Create a company".
    let next: Rect = w.probe("setup.continue");
    assert!(
        read_probe::<Rect>(&w.ctx, "setup.back").is_none(),
        "Start has a Back button"
    );
    assert!(
        painted.filled(next, t.accent),
        "Continue is not the primary accent button"
    );
    let inside = painted.texts_in(next);
    assert!(inside.contains(&"Create a company"), "label: {inside:?}");
    assert!(inside.contains(&"Enter"), "no Enter keycap: {inside:?}");

    let painted = w.script(vec![key(Key::Enter)]);
    let back: Rect = w.probe("setup.back");
    // index.html:124: `.btn` is the surface with the strong border.
    assert!(
        painted.filled(back, t.surface),
        "Back is not a secondary button"
    );
    assert!(
        painted.stroked(back, t.border_strong),
        "Back has no strong border"
    );
    let inside = painted.texts_in(back);
    assert!(
        inside.contains(&"Back") && inside.contains(&"Esc"),
        "Back: {inside:?}"
    );
}

#[test]
fn the_capture_script_reaches_done_where_launch_cards_and_go_to_home_are_drawn() {
    let path = fresh_path("s5_setup_drive.ledgerly");
    let mut w = Wizard::new();
    w.script(company_script(path.to_str().unwrap()));
    // The capture's two Tabs leave the fields, so Enter continues: Features is step 3.
    w.script(vec![key(Key::Enter)]);
    assert_eq!(
        w.bar_name(),
        "Setup, step 3 of 7",
        "the capture script stops short"
    );
    // Features, Keyboard, AI, then Backup's Finish creates the company.
    w.script(vec![
        key(Key::Enter),
        key(Key::Enter),
        key(Key::Enter),
        key(Key::Enter),
    ]);
    w.wait_for_done();
    let painted = w.script(vec![]);
    let t = light();

    // index.html:1860: on Done every earlier step is passed.
    for i in 0..6 {
        let marker: Rect = w.probe(&format!("setup.rail.marker.{i}"));
        let (fill, _) = painted.marker(marker).expect("no marker");
        assert_eq!(fill, t.ok, "step {i} is not drawn as passed on Done");
    }

    // index.html:1892: three launch cards, each an icon, a bold label and a keycap.
    for (n, (label, cap)) in [
        ("Enter opening balances", "M"),
        ("Make your first invoice", "F8"),
        ("Import a bank statement", "B"),
    ]
    .into_iter()
    .enumerate()
    {
        let card: Rect = w.probe(&format!("setup.launch.{n}"));
        let inside = painted.texts_in(card);
        assert!(
            inside.contains(&cap),
            "launch {n} has no {cap} keycap: {inside:?}"
        );
        assert!(
            painted
                .texts
                .iter()
                .any(|(text, r, bold)| text.starts_with(label) && *bold && card.contains_rect(*r)),
            "launch {n} has no bold label {label}"
        );
        assert!(
            painted.paths.iter().any(|(r, _)| card.contains_rect(*r)),
            "launch {n} has no icon"
        );
    }

    // index.html:1893: a primary "Go to Home" with the Enter keycap.
    let home: Rect = w.probe("setup.home");
    assert!(painted.filled(home, t.accent), "Go to Home is not primary");
    let inside = painted.texts_in(home);
    assert!(
        inside.contains(&"Go to Home") && inside.contains(&"Enter"),
        "Go to Home: {inside:?}"
    );
    // index.html:1893: `margin-top:22px` under the launch cards; the button is 32 high
    // (`.btn{height:32px}`, index.html:124), so it does not stretch or drift down the pane.
    let launch: Rect = w.probe("setup.launch.0");
    let gap = home.min.y - launch.max.y;
    assert!(
        (gap - 22.0).abs() < 2.0,
        "Go to Home is {gap} below the launch cards"
    );
    // The card is as tall as its taller column. On Done that is the rail (derived from
    // index.html:312-314: 18 + 7 rows of 8 + 22 + 8 + 18 = 302, plus the two border points),
    // not the page.
    let card: Rect = w.probe("setup.card");
    assert!(
        (card.height() - 304.0).abs() < 4.0,
        "the card is {} tall",
        card.height()
    );
    // A click on it goes Home, as Enter does (index.html:1887).
    w.frame(vec![Event::PointerMoved(home.center())]);
    w.frame(vec![click_at(home.center(), true)]);
    let (_, action) = w.frame(vec![click_at(home.center(), false)]);
    assert!(
        matches!(action, ScreenAction::Replace(_)),
        "a click on Go to Home does not go Home"
    );
}

#[test]
fn on_done_m_opens_masters_as_its_launch_card_says() {
    let path = fresh_path("s5_setup_keys.ledgerly");
    let mut w = Wizard::new();
    w.script(company_script(path.to_str().unwrap()));
    w.script(vec![
        key(Key::Enter),
        key(Key::Enter),
        key(Key::Enter),
        key(Key::Enter),
        key(Key::Enter),
    ]);
    w.wait_for_done();
    // index.html:1889: on Done, M opens the masters (`m={M:hashFor('masters','ledgers'),..}`).
    let (_, action) = w.frame(vec![key(Key::M)]);
    let screen = match action {
        ScreenAction::Push(screen) => screen,
        ScreenAction::Stay | ScreenAction::Replace(_) | ScreenAction::Pop => {
            panic!("M on Done did not open a screen")
        }
    };
    assert_eq!(screen.title(&ledgerly_app::i18n::Tr::default()), "Masters");
}

#[test]
fn option_cards_of_a_row_are_one_height() {
    let mut w = Wizard::new();
    w.script(vec![]);
    let height = |w: &Wizard, n: usize| w.probe::<Rect>(&format!("setup.option.{n}")).height();
    // index.html:136-137: `.grid` rows stretch their cards to one height; Start has two rows
    // of two (index.html:1863, `grid g2`). The height holds from frame to frame.
    let first = [height(&w, 0), height(&w, 1), height(&w, 2), height(&w, 3)];
    assert!(
        (first[0] - first[1]).abs() < 0.5,
        "row 1 heights: {first:?}"
    );
    assert!(
        (first[2] - first[3]).abs() < 0.5,
        "row 2 heights: {first:?}"
    );
    w.script(vec![]);
    let again = [height(&w, 0), height(&w, 1), height(&w, 2), height(&w, 3)];
    assert_eq!(first, again, "the card heights move from frame to frame");
}
