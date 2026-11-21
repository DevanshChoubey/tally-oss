//! Card U1: screen stack, breadcrumbs, tabs and key handling. The expected behaviour is the
//! contract in spec 7 ("Screen stack with visible breadcrumbs and open-screen tabs; 'open on top'
//! (resumable) vs 'replace'", "Esc back") and the `ScreenAction` docs in `screen.rs`; each test
//! says which sentence it checks. No test here involves a number.

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_app::shell::chrome::key_hints;
use ledgerly_app::shell::topbar::{ai_chip, company_chip, date_chip, lock_chip};
use ledgerly_app::shell::{AiTier, KeyHint, ScreenStack, Shell, StackChange, Tabs};
use ledgerly_app::theme::{self, ThemeKind};
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{CompanyId, StateCode};
use ledgerly_model::{Address, Books, Company, EntityKind, Features, Masters, NegativeStockPolicy};
use time::macros::date;

/// Does nothing but have a title.
struct Leaf(&'static str);

impl Screen for Leaf {
    fn title(&self) -> String {
        self.0.to_owned()
    }
    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut AppCtx) -> ScreenAction {
        ui.label(self.0);
        ScreenAction::Stay
    }
}

/// Returns the given action once, on its first frame, then stays.
struct Once(&'static str, Option<ScreenAction>);

impl Screen for Once {
    fn title(&self) -> String {
        self.0.to_owned()
    }
    fn ui(&mut self, _ui: &mut egui::Ui, _ctx: &mut AppCtx) -> ScreenAction {
        self.1.take().unwrap_or(ScreenAction::Stay)
    }
}

/// Keeps Esc for itself, like a screen with an open pop-up.
struct KeepsEsc;

impl Screen for KeepsEsc {
    fn title(&self) -> String {
        "Keeps Esc".to_owned()
    }
    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut AppCtx) -> ScreenAction {
        ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        ScreenAction::Stay
    }
}

fn leaf(name: &'static str) -> Box<dyn Screen> {
    Box::new(Leaf(name))
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

fn frame(ctx: &egui::Context, shell: &mut Shell, input: RawInput) {
    let out = ctx.run_ui(input, |ui| shell.show(ui));
    out.drop_without_applying_deltas();
}

fn titles(shell: &Shell) -> Vec<String> {
    shell
        .tabs()
        .active_stack()
        .map_or_default(ScreenStack::breadcrumbs)
}

// ---- stack ---------------------------------------------------------------------------------

#[test]
fn breadcrumbs_list_titles_from_root_to_top() {
    // spec 7: "visible breadcrumbs". Pushing Home, Balance sheet, Cash gives that trail.
    let mut s = ScreenStack::new(leaf("Home"));
    s.push(leaf("Balance sheet"));
    s.push(leaf("Cash"));
    assert_eq!(s.breadcrumbs(), ["Home", "Balance sheet", "Cash"]);
    assert_eq!(s.depth(), 3);
    assert_eq!(s.top_title(), "Cash");
}

#[test]
fn push_keeps_the_screen_below_and_pop_returns_to_it() {
    // spec 7: "open on top (resumable)"; Esc returns to the screen that was open.
    let mut s = ScreenStack::new(leaf("Voucher"));
    assert_eq!(
        s.apply(ScreenAction::Push(leaf("Report"))),
        StackChange::Pushed
    );
    assert_eq!(s.depth(), 2);
    assert_eq!(s.apply(ScreenAction::Pop), StackChange::Popped);
    assert_eq!(s.breadcrumbs(), ["Voucher"]);
}

#[test]
fn replace_swaps_the_top_and_keeps_the_depth() {
    // spec 7: "replace" does not grow the stack, and the replaced screen is gone.
    let mut s = ScreenStack::new(leaf("Home"));
    s.push(leaf("Sales"));
    assert_eq!(
        s.apply(ScreenAction::Replace(leaf("Receipt"))),
        StackChange::Replaced
    );
    assert_eq!(s.breadcrumbs(), ["Home", "Receipt"]);
}

#[test]
fn replace_on_the_root_leaves_one_screen() {
    let mut s = ScreenStack::new(leaf("Home"));
    s.replace(leaf("Other"));
    assert_eq!(s.breadcrumbs(), ["Other"]);
}

#[test]
fn the_root_is_never_popped() {
    // screen.rs: Pop means "close this screen"; a tab with no screen cannot be drawn, so the
    // root stays and the caller is told.
    let mut s = ScreenStack::new(leaf("Home"));
    assert!(!s.pop());
    assert_eq!(s.apply(ScreenAction::Pop), StackChange::RootKept);
    assert_eq!(s.depth(), 1);
}

#[test]
fn stay_changes_nothing() {
    let mut s = ScreenStack::new(leaf("Home"));
    assert_eq!(s.apply(ScreenAction::Stay), StackChange::None);
    assert_eq!(s.breadcrumbs(), ["Home"]);
}

#[test]
fn truncate_goes_back_to_a_breadcrumb() {
    // Clicking the second crumb of Home > A > B > C keeps Home and A.
    let mut s = ScreenStack::new(leaf("Home"));
    for n in ["A", "B", "C"] {
        s.push(leaf(n));
    }
    assert_eq!(s.truncate_to(2), 2);
    assert_eq!(s.breadcrumbs(), ["Home", "A"]);
    // Asking for more than exists, or for zero, never empties the stack.
    assert_eq!(s.truncate_to(9), 0);
    assert_eq!(s.truncate_to(0), 1);
    assert_eq!(s.breadcrumbs(), ["Home"]);
}

// ---- tabs ----------------------------------------------------------------------------------

#[test]
fn open_makes_the_new_tab_active() {
    let mut t = Tabs::new(leaf("One"));
    t.open(leaf("Two"));
    assert_eq!(t.len(), 2);
    assert_eq!(t.active_index(), 1);
    assert_eq!(t.titles(), ["One", "Two"]);
}

#[test]
fn a_tab_shows_the_top_screen_of_its_stack() {
    let mut t = Tabs::new(leaf("Home"));
    if let Some(s) = t.active_stack_mut() {
        s.push(leaf("Day book"));
    }
    assert_eq!(t.titles(), ["Day book"]);
}

#[test]
fn the_last_tab_cannot_be_closed() {
    let mut t = Tabs::new(leaf("Only"));
    assert!(!t.close(0));
    assert_eq!(t.len(), 1);
}

#[test]
fn closing_an_unknown_tab_does_nothing() {
    let mut t = Tabs::new(leaf("A"));
    t.open(leaf("B"));
    assert!(!t.close(5));
    assert_eq!(t.len(), 2);
}

#[test]
fn closing_a_tab_before_the_active_one_keeps_the_same_tab_active() {
    // Tabs A B C with C active; closing A leaves B C with C still active (index 1).
    let mut t = Tabs::new(leaf("A"));
    t.open(leaf("B"));
    t.open(leaf("C"));
    assert!(t.close(0));
    assert_eq!(t.titles(), ["B", "C"]);
    assert_eq!(t.active_index(), 1);
}

#[test]
fn closing_the_active_last_tab_activates_the_new_last() {
    let mut t = Tabs::new(leaf("A"));
    t.open(leaf("B"));
    assert!(t.close(1));
    assert_eq!(t.titles(), ["A"]);
    assert_eq!(t.active_index(), 0);
}

#[test]
fn closing_the_active_middle_tab_activates_its_successor() {
    // Tabs A B C with B active; closing B leaves A C with C taking its place (index 1).
    let mut t = Tabs::new(leaf("A"));
    t.open(leaf("B"));
    t.open(leaf("C"));
    assert!(t.select(1));
    assert!(t.close(1));
    assert_eq!(t.titles(), ["A", "C"]);
    assert_eq!(t.active_index(), 1);
}

#[test]
fn selecting_a_missing_tab_is_refused() {
    let mut t = Tabs::new(leaf("A"));
    assert!(!t.select(3));
    assert_eq!(t.active_index(), 0);
}

#[test]
fn each_tab_keeps_its_own_stack() {
    let mut t = Tabs::new(leaf("A"));
    if let Some(s) = t.active_stack_mut() {
        s.push(leaf("A2"));
    }
    t.open(leaf("B"));
    assert!(t.select(0));
    assert_eq!(t.active_stack().map(ScreenStack::depth), Some(2));
}

// ---- shell frames --------------------------------------------------------------------------

#[test]
fn a_screen_can_open_another_on_top_and_esc_goes_back() {
    // spec 7: "Esc back" and "open on top (resumable)".
    let ctx = egui::Context::default();
    let mut shell = Shell::new(Box::new(Once(
        "Home",
        Some(ScreenAction::Push(leaf("Report"))),
    )));
    frame(&ctx, &mut shell, RawInput::default());
    assert_eq!(titles(&shell), ["Home", "Report"]);
    frame(&ctx, &mut shell, key(Key::Escape, Modifiers::NONE));
    assert_eq!(titles(&shell), ["Home"]);
}

#[test]
fn esc_on_the_root_does_nothing() {
    let ctx = egui::Context::default();
    let mut shell = Shell::new(leaf("Home"));
    frame(&ctx, &mut shell, key(Key::Escape, Modifiers::NONE));
    assert_eq!(titles(&shell), ["Home"]);
    assert_eq!(shell.tabs().len(), 1);
}

#[test]
fn a_screen_that_consumes_esc_is_not_popped() {
    let ctx = egui::Context::default();
    let mut shell = Shell::new(leaf("Home"));
    if let Some(s) = shell.tabs_mut().active_stack_mut() {
        s.push(Box::new(KeepsEsc));
    }
    frame(&ctx, &mut shell, key(Key::Escape, Modifiers::NONE));
    assert_eq!(titles(&shell), ["Home", "Keeps Esc"]);
}

#[test]
fn replace_from_a_screen_swaps_it() {
    let ctx = egui::Context::default();
    let mut shell = Shell::new(Box::new(Once(
        "Home",
        Some(ScreenAction::Replace(leaf("Swapped"))),
    )));
    frame(&ctx, &mut shell, RawInput::default());
    assert_eq!(titles(&shell), ["Swapped"]);
}

#[test]
fn pop_from_the_root_screen_keeps_it() {
    let ctx = egui::Context::default();
    let mut shell = Shell::new(Box::new(Once("Home", Some(ScreenAction::Pop))));
    frame(&ctx, &mut shell, RawInput::default());
    assert_eq!(titles(&shell), ["Home"]);
}

#[test]
fn ctrl_t_opens_a_tab_and_ctrl_w_closes_it() {
    let ctx = egui::Context::default();
    let mut shell = Shell::new(leaf("Home"));
    frame(&ctx, &mut shell, key(Key::T, Modifiers::COMMAND));
    assert_eq!(shell.tabs().len(), 2);
    assert_eq!(shell.tabs().active_index(), 1);
    frame(&ctx, &mut shell, key(Key::W, Modifiers::COMMAND));
    assert_eq!(shell.tabs().len(), 1);
    // The last tab stays.
    frame(&ctx, &mut shell, key(Key::W, Modifiers::COMMAND));
    assert_eq!(shell.tabs().len(), 1);
}

#[test]
fn the_theme_is_applied_on_the_first_frame_and_follows_set_theme() {
    // derived: the mock paints the page with --bg (see u1_theme.rs for the hex values).
    let ctx = egui::Context::default();
    let mut shell = Shell::new(leaf("Home"));
    frame(&ctx, &mut shell, RawInput::default());
    assert_eq!(
        ctx.global_style().visuals.panel_fill,
        theme::tokens(ThemeKind::Light).bg
    );
    shell.set_theme(ThemeKind::Dark);
    frame(&ctx, &mut shell, RawInput::default());
    assert_eq!(shell.theme(), ThemeKind::Dark);
    assert_eq!(
        ctx.global_style().visuals.panel_fill,
        theme::tokens(ThemeKind::Dark).bg
    );
}

#[test]
fn the_welcome_screen_shows_no_company_without_books() {
    let ctx = egui::Context::default();
    let mut shell = Shell::with_welcome();
    frame(&ctx, &mut shell, RawInput::default());
    assert_eq!(titles(&shell), ["Home"]);
    assert!(shell.app_ctx_mut().books.is_none());
}

// ---- top bar and key bar (card U3) ----------------------------------------------------------

/// Books of a fictional company, with no vouchers, under the name `name`.
fn books_named(name: &str) -> Books {
    Books {
        company: Company {
            ineligible_itc_default: None,
            id: CompanyId(1),
            name: name.to_owned(),
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
        },
        masters: Masters::default(),
        vouchers: std::collections::BTreeMap::new(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

#[test]
fn the_top_bar_shows_the_company_name_and_year_from_the_books() {
    // provenance: derived. The top bar's company chip is `company_chip`, which the bar draws. The
    // name is the fixture's, and FY 2026-27 follows from the fixture's start date 1 April 2026.
    // Catches a chip that shows no name, or one that shows a name other than the books'.
    let ctx = AppCtx {
        books: Some(books_named("Fictional Traders")),
        ..AppCtx::default()
    };
    assert_eq!(company_chip(&ctx), "Fictional Traders FY 2026-27");
}

#[test]
fn the_chips_say_not_known_when_the_data_is_missing() {
    // provenance: derived. With no books and no stored AI tier, the chips say so in words; they
    // never show a name, a tier or a lock date that is not known. Catches a chip that shows an
    // invented value (the default would then not say "not").
    assert_eq!(company_chip(&AppCtx::default()), "No company open");
    assert_eq!(ai_chip(None), "AI: not set up");
    assert_eq!(lock_chip(None), None, "no lock date means no lock chip");
    assert_eq!(ai_chip(Some(AiTier::Local)), "AI on this computer");
    assert_eq!(date_chip(date!(2026 - 11 - 21)), "21 Nov 2026");
    assert_eq!(
        lock_chip(Some(date!(2026 - 03 - 31)))
            .unwrap_or_default()
            .as_str(),
        "Locked to 31 Mar 2026"
    );
}

#[test]
fn the_key_bar_shows_the_screen_keys_before_the_shells_own() {
    // provenance: derived. The key bar lists the screen's keys first, then Esc (only when the stack
    // can go back) and the shell's four keys. Catches a key bar that ignores the screen's keys,
    // or lists Esc on a root screen.
    let screen = [KeyHint::new("Ctrl+A", "Save")];
    assert_eq!(
        key_hints(&screen, true),
        vec![
            ("Ctrl+A".to_owned(), "Save".to_owned()),
            ("Esc".to_owned(), "Back".to_owned()),
            ("Ctrl+K".to_owned(), "Search".to_owned()),
            ("Ctrl+T".to_owned(), "New tab".to_owned()),
            ("Ctrl+W".to_owned(), "Close tab".to_owned()),
            ("Ctrl+\\".to_owned(), "Sidebar".to_owned()),
        ]
    );
    assert_eq!(
        key_hints(&[], false).first(),
        Some(&("Ctrl+K".to_owned(), "Search".to_owned())),
        "a root screen with no keys starts with the shell's keys"
    );
}

#[test]
fn a_screen_has_no_keys_and_its_title_as_page_title_unless_it_says_so() {
    // provenance: derived. The defaults of `Screen::page_title`, `subtitle` and `keys` are the
    // title, none and none, so a screen that has not opted in is drawn as before. Catches a
    // default that invents a subtitle or a key.
    let screen = Leaf("Home");
    assert_eq!(screen.page_title(), "Home");
    assert_eq!(screen.subtitle(), None);
    assert!(screen.keys().is_empty());
}
