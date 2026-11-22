//! The shared chrome drawn to the mock (`design/mock-v2/index.html`): the icon path reader and
//! the `?` key that opens the keyboard map.

use egui::{Event, Key, Modifiers, Pos2, RawInput, pos2};
use ledgerly_app::shell::{ScreenStack, Shell};
use ledgerly_app::ui::icon::{self, Icon};
use ledgerly_app::{AppCtx, Screen, ScreenAction};

fn close(a: Pos2, b: Pos2) -> bool {
    (a - b).length() < 1e-3
}

#[test]
fn the_home_icon_starts_with_the_roof_points_of_its_path() {
    // provenance: derived by hand from the mock's Home path (`ICONS.home`,
    // "M3 11l9-7 9 7v9 ..."): M3 11, then relative l9-7 -> (12,4), 9 7 -> (21,11), v9 -> (21,20).
    // Catches relative commands read as absolute, or an implicit repeat of `l` dropped.
    let paths = icon::parse(Icon::Home.path());
    let first = paths.first().cloned().unwrap_or_default();
    let want = [
        pos2(3.0, 11.0),
        pos2(12.0, 4.0),
        pos2(21.0, 11.0),
        pos2(21.0, 20.0),
    ];
    assert!(first.len() >= want.len(), "too few points: {first:?}");
    for (got, want) in first.iter().zip(want) {
        assert!(close(*got, want), "got {got:?}, want {want:?}");
    }
}

#[test]
fn an_arc_ends_at_its_endpoint_and_passes_the_far_side_of_the_circle() {
    // provenance: derived by hand from SVG 1.1 F.6.5 (arc endpoint to centre conversion):
    // "M12 3a9 9 0 1 0 0 18" is the half circle of radius 9 about (12,12) from (12,3) to (12,21),
    // drawn with the large-arc flag over x = 3 (sweep 0 runs counter-clockwise in SVG's y-down
    // frame, through the left side). Catches an arc drawn as a line, or on the wrong side.
    let paths = icon::parse("M12 3a9 9 0 1 0 0 18");
    let points = paths.first().cloned().unwrap_or_default();
    let last = points.last().copied().unwrap_or(Pos2::ZERO);
    assert!(close(last, pos2(12.0, 21.0)), "ends at {last:?}");
    let min_x = points.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
    assert!((min_x - 3.0).abs() < 0.2, "leftmost x {min_x}");
    for p in &points {
        let r = (*p - pos2(12.0, 12.0)).length();
        assert!((r - 9.0).abs() < 0.05, "{p:?} is {r} from the centre");
    }
}

#[test]
fn every_icon_path_parses_to_points() {
    // provenance: derived. Each icon the chrome draws has a non-empty path; an icon that parses
    // to nothing would draw a blank space. Catches a command letter the reader does not know.
    for which in [
        Icon::Home,
        Icon::Inbox,
        Icon::Sale,
        Icon::Purchase,
        Icon::Pay,
        Icon::Recv,
        Icon::Chat,
        Icon::Book,
        Icon::Scale,
        Icon::Users,
        Icon::Gst,
        Icon::Bank,
        Icon::Shield,
        Icon::Kbd,
        Icon::Building,
        Icon::Cal,
        Icon::Lock,
        Icon::Search,
        Icon::Cpu,
        Icon::Cloud,
        Icon::Off,
        Icon::Sun,
        Icon::Contrast,
        Icon::Check,
        Icon::Chev,
        Icon::Spark,
    ] {
        let paths = icon::parse(which.path());
        assert!(!paths.is_empty(), "{which:?} has no subpaths");
        assert!(
            paths.iter().all(|p| p.len() >= 2),
            "{which:?} has a subpath with one point"
        );
    }
}

/// A root screen that draws nothing.
struct Leaf;

impl Screen for Leaf {
    fn title(&self) -> String {
        "Home".to_owned()
    }
    fn ui(&mut self, _ui: &mut egui::Ui, _ctx: &mut AppCtx) -> ScreenAction {
        ScreenAction::Stay
    }
}

fn press(ctx: &egui::Context, shell: &mut Shell, key: Key) {
    let input = RawInput {
        events: vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        ..RawInput::default()
    };
    let out = ctx.run_ui(input, |ui| shell.show(ui));
    out.drop_without_applying_deltas();
}

#[test]
fn question_mark_opens_the_keyboard_map() {
    // provenance: derived from the mock's key bar, `design/mock-v2/index.html:914`
    // ("? all keys"). Catches the right end of the key bar offering a key that does nothing.
    let ctx = egui::Context::default();
    let mut shell = Shell::new(Box::new(Leaf));
    press(&ctx, &mut shell, Key::Questionmark);
    let trail = shell
        .tabs()
        .active_stack()
        .map_or_default(ScreenStack::breadcrumbs);
    assert_eq!(trail, vec!["Home".to_owned(), "Keyboard map".to_owned()]);
}
