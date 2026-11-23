//! Window, theme switching, screen stack, breadcrumbs, tabs, top bar, sidebar and key bar. **Card
//! U1** implements the stack and the tabs; **U3** (shell chrome) the top bar, the sidebar and the
//! key bar.
//!
//! Frozen interface (plan card U1):
//!
//! ```text
//! pub struct Shell { .. }
//! impl eframe::App for Shell { .. }
//! pub fn run() -> eframe::Result<()>;   // called from main.rs (U1 also owns main.rs)
//! ```
//!
//! Layout (`design/mock-v2/index.html`, the `.app` grid at lines 80-82):
//!
//! * a 48 px top bar ([`TOP_BAR_HEIGHT`]): brand, company, date, lock, search, AI tier, theme and
//!   keyboard map ([`topbar`]);
//! * a 32 px key bar at the bottom ([`KEY_BAR_HEIGHT`]): the active screen's keys
//!   ([`crate::Screen::keys`]) and the shell's own keys ([`chrome::key_hints`]);
//! * a sidebar on the left, 208 px, or a 60 px rail after Ctrl+\ ([`sidebar`]);
//! * above the page: the tab strip and the breadcrumbs ([`chrome`]). The tab strip is under the
//!   top bar, not in it, so the top bar keeps the mock's chips.
//!
//! What the shell does (spec 7):
//!
//! * Every tab is a screen stack. A screen asks for [`crate::ScreenAction::Push`] ("open on top",
//!   resumable) or [`crate::ScreenAction::Replace`]; Esc, or a click on an earlier breadcrumb, goes back.
//! * Esc is handled after the screen has drawn, and only when the screen did not consume it, so a
//!   screen can keep Esc for its own pop-ups and fields.
//! * Ctrl+T opens a tab, Ctrl+W closes the active one, Ctrl+\ shows or hides the sidebar's labels.
//!   The key map of card U2 is not wired in here: the shell reads these keys directly until Wave 2
//!   wiring.
//! * The shell sums nothing and shows no figures (rule C4). Its chips show only what is known.
//! * The command palette (Wave 3): the keymap's open-palette chords (Ctrl+K, Alt+G) open it on
//!   top of the current screen, and switch-palette (Ctrl+G) opens it so the choice replaces the
//!   screen. While it is open it takes every key, so the screen under it reacts to none. A pick
//!   opens its screen through [`sidebar::screen_for`]; a pick that opens nothing leaves a
//!   notice in the key bar ([`Shell::notice`]), never silence. The palette lists only the commands
//!   that have a route ([`palette_rows`], rule B3).
//! * The sidebar's entry for the screen on top of the stack is selected ([`Shell::selected_sidebar`]).
//! * Opening a company replaces the welcome screen with the company's home screen
//!   ([`GatewayScreen`]), and Ctrl+T then opens a new tab on that home screen.
//!
//! Not built here: split view (report beside voucher), remembering the theme or the rail between
//! runs, and scalable fonts. They need storage or screens that other cards own.

pub mod chrome;
pub mod gateway;
pub mod navigate;
pub mod sidebar;
mod stack;
pub mod submit;
pub mod topbar;
mod welcome;

use std::sync::atomic::{AtomicBool, Ordering};

use egui::{Key, Modifiers};

use crate::keymap::{self, Action, Keymap};
use crate::palette::{
    Palette, PaletteChoice, PaletteEntry, PaletteMode, PaletteSearch, PaletteTarget,
};
use crate::theme::{self, ThemeKind, Tokens};
use crate::worker::Ticket;
use crate::{AppCtx, Screen, ScreenAction};
use chrome::ChromeCmd;
use sidebar::CommandRoute;
use submit::BooksReply;

pub use crate::screen::{AiTier, KeyHint};

/// What identifies one copy of the books, so a screen that shows engine figures asks again when
/// the books change: the shell's change count (a company opened, or a command saved), the number
/// of vouchers and the newest voucher id.
pub(crate) type BooksPrint = (u64, usize, Option<ledgerly_core::VoucherId>);

/// The [`BooksPrint`] of `books` now.
pub(crate) fn books_print(books: &ledgerly_api::Books) -> BooksPrint {
    (
        submit::change_count(),
        books.vouchers.len(),
        books.vouchers.keys().next_back().copied(),
    )
}
pub use gateway::GatewayScreen;
pub use stack::{ScreenStack, StackChange, Tabs};
pub use welcome::{WelcomeScreen, company_from_form};

/// Height of the top bar, in points (`design/mock-v2/index.html:80`, `grid-template-rows: 48px`).
pub const TOP_BAR_HEIGHT: f32 = 48.0;
/// Height of the key bar, in points (`design/mock-v2/index.html:80`, `grid-template-rows: 32px`).
pub const KEY_BAR_HEIGHT: f32 = 32.0;

/// Set by a screen that wants the palette opened (the home screen's Ctrl+K link); the shell
/// takes it on its next frame.
static PALETTE_WANTED: AtomicBool = AtomicBool::new(false);

/// Asks the shell to open the palette on top of the current screen on its next frame.
pub(crate) fn ask_for_palette() {
    PALETTE_WANTED.store(true, Ordering::SeqCst);
}

/// The palette rows the shell offers: every keymap command that has a route (see
/// [`sidebar::command_route`]), then the report menu. A command with no route is left out, so no
/// row is a dead command (rule B3).
#[must_use]
pub fn palette_rows(keymap: &Keymap) -> Vec<PaletteEntry> {
    navigate::host_entries(keymap)
        .into_iter()
        .filter(|entry| sidebar::target_is_shown(&entry.target))
        .collect()
}

/// The application: tabs of screen stacks, the active theme and the shared screen context.
pub struct Shell {
    tabs: Tabs,
    app_ctx: AppCtx,
    theme: ThemeKind,
    applied_theme: Option<ThemeKind>,
    /// Whether the sidebar is the 60 px rail (Ctrl+\) rather than the full 208 px.
    rail: bool,
    /// The change count the books on screen were requested for.
    books_seen: u64,
    /// The books fetch in progress, if any.
    books_ticket: Option<Ticket<BooksReply>>,
    /// Why the books could not be read, when the last fetch failed.
    books_problem: Option<String>,
    /// The key map the palette chords come from.
    keymap: Keymap,
    /// The command palette.
    palette: Palette,
    /// The palette's search through the API worker.
    search: PaletteSearch,
    /// Why the last pick or chip opened nothing, or why the key map could not be read.
    notice: Option<String>,
}

impl Shell {
    /// A shell with one tab showing `root`, in the light theme, with the full sidebar.
    #[must_use]
    pub fn new(root: Box<dyn Screen>) -> Self {
        let (keymap, notice) = match keymap::try_classic() {
            Ok(keymap) => (keymap, None),
            Err(e) => (
                Keymap::unbound("none"),
                Some(format!(
                    "The key map could not be read, so the palette keys do not work: {e}"
                )),
            ),
        };
        Self {
            tabs: Tabs::new(root),
            app_ctx: AppCtx::default(),
            theme: ThemeKind::default(),
            applied_theme: None,
            rail: false,
            books_seen: submit::change_count(),
            books_ticket: None,
            books_problem: None,
            keymap,
            palette: Palette::new(),
            search: PaletteSearch::new(),
            notice,
        }
    }

    /// The command palette.
    #[must_use]
    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    /// The command palette, mutably (to type into it from outside a frame).
    pub fn palette_mut(&mut self) -> &mut Palette {
        &mut self.palette
    }

    /// Why the last pick or chip opened nothing, when it did not.
    #[must_use]
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// Whether the sidebar is the 60 px rail.
    #[must_use]
    pub fn rail(&self) -> bool {
        self.rail
    }

    /// The sidebar entry selected for the screen on top of the active stack, if one names it.
    #[must_use]
    pub fn selected_sidebar(&self) -> Option<&'static str> {
        let title = self.tabs.active_stack()?.top_title();
        sidebar::selected_label(&title)
    }

    /// Opens the palette for `action` (open-palette or switch-palette) with today's rows.
    fn open_palette(&mut self, action: Action) {
        let mut entries = palette_rows(&self.keymap);
        entries.extend(navigate::screen_entries(self.app_ctx.books.as_ref()));
        self.palette.set_entries(entries);
        if self.palette.open_for(action) {
            self.notice = None;
        }
    }

    /// Reads the palette chords; with the palette open, draws it, takes its keys and opens the
    /// pick. Runs before the screen so the screen never sees a key meant for the palette.
    fn palette_frame(&mut self, ui: &mut egui::Ui) {
        if !self.palette.is_open() {
            let wanted = ui.input_mut(|i| {
                if self.keymap.consume_action(i, Action::SwitchPalette) {
                    Some(Action::SwitchPalette)
                } else if self.keymap.consume_action(i, Action::OpenPalette) {
                    Some(Action::OpenPalette)
                } else {
                    None
                }
            });
            let asked = PALETTE_WANTED.swap(false, Ordering::SeqCst);
            match (wanted, asked) {
                (Some(action), _) => self.open_palette(action),
                (None, true) => self.open_palette(Action::OpenPalette),
                (None, false) => return,
            }
        }
        self.search
            .tick(&mut self.palette, self.app_ctx.api.as_ref());
        if self.search.is_pending() {
            ui.ctx().request_repaint();
        }
        let choice = self.palette_panel(ui);
        // Whatever key the palette did not use this frame is not passed to the screen under it.
        ui.input_mut(|i| {
            i.events.retain(|e| !matches!(e, egui::Event::Key { .. }));
        });
        if let Some(choice) = choice {
            self.pick(&choice);
        }
    }

    /// Draws the open palette as the mock does (`design/mock-v2/index.html:273-274`): a scrim over
    /// the window, which closes the palette when clicked, and the panel at
    /// [`crate::palette::palette_rect`] with the strong border, the large radius and the overlay
    /// shadow. Returns the palette's choice this frame.
    fn palette_panel(&mut self, ui: &egui::Ui) -> Option<crate::palette::PaletteChoice> {
        let ctx = ui.ctx().clone();
        let screen = ctx.content_rect();
        let kind = crate::ui::kind(ui);
        let t = crate::ui::tokens(ui);
        let scrim_clicked = egui::Area::new(egui::Id::unique("ledgerly-palette-scrim"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .show(&ctx, |ui| {
                let (rect, response) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
                ui.painter().rect_filled(rect, 0.0, theme::scrim(kind));
                response.clicked()
            })
            .inner;
        let rect = crate::palette::palette_rect(screen);
        let choice = egui::Area::new(egui::Id::unique("ledgerly-palette"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .show(&ctx, |ui| {
                ui.set_max_height(rect.height());
                egui::Frame::new()
                    .fill(t.surface)
                    .stroke(egui::Stroke::new(
                        theme::border_width(kind),
                        t.border_strong,
                    ))
                    .corner_radius(theme::RADIUS_L)
                    .shadow(theme::shadow_2(kind))
                    .show(ui, |ui| {
                        ui.set_width(rect.width());
                        self.palette.ui(ui)
                    })
                    .inner
            })
            .inner;
        if scrim_clicked && choice.is_none() {
            self.palette.close();
        }
        choice
    }

    /// Runs a palette pick: a screen opens in the active tab, a shell command runs, and an
    /// assistant request says it is not connected yet.
    fn pick(&mut self, choice: &PaletteChoice) {
        match &choice.target {
            PaletteTarget::Command(action) => self.pick_command(*action, choice),
            PaletteTarget::AiIntent(name) => {
                self.notice = Some(format!(
                    "Not supported yet: the assistant request \"{name}\" (the assistant is not \
                     connected to the app yet)."
                ));
            }
            PaletteTarget::Report(_)
            | PaletteTarget::Master(_)
            | PaletteTarget::Voucher(_)
            | PaletteTarget::Setting(_)
            // A fill never leaves the palette; were one to, opening it says why it cannot.
            | PaletteTarget::Fill(_) => self.open_in_tab(&choice.target, choice.mode),
        }
    }

    /// A command pick: the shell's own commands run here, a command with a screen opens it, and
    /// any other command says it is not available (the palette does not list those).
    fn pick_command(&mut self, action: Action, choice: &PaletteChoice) {
        match action {
            Action::NewTab => self.run_shell_cmd(ChromeCmd::NewTab),
            Action::CloseTab => self.run_shell_cmd(ChromeCmd::CloseActiveTab),
            Action::Back => self.run_shell_cmd(ChromeCmd::Back),
            Action::OpenPalette | Action::SwitchPalette => self.open_palette(action),
            other @ (Action::NewContra
            | Action::NewPayment
            | Action::NewReceipt
            | Action::NewJournal
            | Action::NewSales
            | Action::Help
            | Action::NewPurchase
            | Action::ChangePeriod
            | Action::SwitchCompany
            | Action::CompanyFeatures
            | Action::Configure
            | Action::Save
            | Action::CreateMaster
            | Action::AlterMaster
            | Action::MoreDetails
            | Action::DeleteLine
            | Action::SwitchSide
            | Action::DeleteVoucher
            | Action::CancelVoucher
            | Action::DuplicateVoucher
            | Action::InsertVoucher
            | Action::Undo
            | Action::ToggleDetail
            | Action::Print
            | Action::Export
            | Action::Email) => match sidebar::command_route(other) {
                CommandRoute::Screen => self.open_in_tab(&choice.target, choice.mode),
                CommandRoute::Shell | CommandRoute::Hidden => {
                    self.notice = Some(format!(
                        "\"{}\" is not available from the palette.",
                        other.label()
                    ));
                }
            },
        }
    }

    /// Runs a shell command that the palette picked, clearing the notice first.
    fn run_shell_cmd(&mut self, cmd: ChromeCmd) {
        self.notice = None;
        self.run_cmd(cmd);
    }

    /// Opens `target` in the active tab: on top of the stack for [`PaletteMode::OnTop`], in place
    /// of the top screen for [`PaletteMode::Replace`]. Nothing changes when it cannot open; the
    /// reason goes to the notice.
    fn open_in_tab(&mut self, target: &PaletteTarget, mode: PaletteMode) {
        match sidebar::screen_for(target, &self.app_ctx) {
            Ok(screen) => {
                if let Some(stack) = self.tabs.active_stack_mut() {
                    stack.open_screen(screen, mode);
                }
                self.notice = None;
            }
            Err(e) => self.notice = Some(e.to_string()),
        }
    }

    /// Why the books could not be read after the last fetch, or `None` when they were read (or
    /// no company is open). Screens show no figure while this is set.
    #[must_use]
    pub fn books_problem(&self) -> Option<&str> {
        self.books_problem.as_deref()
    }

    /// Keeps `AppCtx::books` current without blocking the UI thread: when a company was opened
    /// or a command saved a change, a read of the books is queued on the worker, and the copy is
    /// taken from its ticket once it arrives. A change that arrives while a read is running
    /// replaces that read, so the books never show an older state than the last change.
    pub fn sync_books(&mut self) {
        let count = submit::change_count();
        if count != self.books_seen {
            self.books_seen = count;
            self.books_problem = None;
            self.books_ticket = self.app_ctx.api.as_ref().map(submit::fetch_books);
            if self.books_ticket.is_none() {
                self.app_ctx.books = None;
            }
        }
        // A worker put into the context without a change note still gets its books.
        if self.books_ticket.is_none()
            && self.app_ctx.books.is_none()
            && self.books_problem.is_none()
        {
            self.books_ticket = self.app_ctx.api.as_ref().map(submit::fetch_books);
        }
        let Some(ticket) = self.books_ticket.as_mut() else {
            return;
        };
        let Some(reply) = submit::settle_books(ticket) else {
            return;
        };
        self.books_ticket = None;
        match reply {
            BooksReply::Loaded(books) => {
                self.app_ctx.books = Some(*books);
                self.books_problem = None;
            }
            BooksReply::Unsupported(u) => {
                self.app_ctx.books = None;
                self.books_problem = Some(format!("{}: {}", u.what, u.reason));
            }
            BooksReply::Failed(reason) => {
                self.app_ctx.books = None;
                self.books_problem = Some(reason);
            }
        }
    }

    /// A shell with the welcome screen as its only screen.
    #[must_use]
    pub fn with_welcome() -> Self {
        Self::new(Box::new(WelcomeScreen::default()))
    }

    /// Chooses the theme; it is applied on the next frame.
    pub fn set_theme(&mut self, theme: ThemeKind) {
        self.theme = theme;
    }

    /// The theme in use.
    #[must_use]
    pub fn theme(&self) -> ThemeKind {
        self.theme
    }

    /// The tabs and their stacks.
    #[must_use]
    pub fn tabs(&self) -> &Tabs {
        &self.tabs
    }

    /// The tabs and their stacks, mutably (to open a screen from outside a frame).
    pub fn tabs_mut(&mut self) -> &mut Tabs {
        &mut self.tabs
    }

    /// The context handed to screens.
    pub fn app_ctx_mut(&mut self) -> &mut AppCtx {
        &mut self.app_ctx
    }

    /// Draws the stage (`#stage{padding:18px 24px 40px}` on the `--bg` paper): the tab strip
    /// once a second tab is open, breadcrumbs below the root, then the top screen, whose action
    /// is applied to its stack.
    fn stage(&mut self, ui: &mut egui::Ui, trail: &[String], t: &Tokens) -> Vec<ChromeCmd> {
        let mut cmds = Vec::new();
        let mut screen_action = ScreenAction::Stay;
        let stage = egui::Frame::NONE.fill(t.bg).inner_margin(egui::Margin {
            left: 24,
            right: 24,
            top: 18,
            bottom: 0,
        });
        egui::CentralPanel::default().frame(stage).show(ui, |ui| {
            // The mock has one page and no tab strip.
            if self.tabs.len() > 1 {
                cmds.extend(chrome::tab_strip(ui, &self.tabs));
            }
            // The root screen has no breadcrumbs (`phead(null, ..)` on Home in the mock).
            if trail.len() > 1 {
                cmds.extend(chrome::breadcrumbs(ui, trail, t));
            }
            let top = self.tabs.active_stack_mut().and_then(ScreenStack::top_mut);
            if let Some(screen) = top {
                screen_action = screen.ui(ui, &mut self.app_ctx);
            }
        });
        if let Some(stack) = self.tabs.active_stack_mut() {
            stack.apply(screen_action);
        }
        cmds
    }

    /// Draws one frame into `ui`: the top bar, the key bar, the sidebar, the tab strip and
    /// breadcrumbs, the palette, the top screen of the active tab, then key handling.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        if self.applied_theme != Some(self.theme) {
            theme::apply(ui.ctx(), self.theme);
            self.applied_theme = Some(self.theme);
        }
        self.sync_books();
        if self.books_ticket.is_some() {
            ui.ctx().request_repaint();
        }
        let t = theme::tokens(self.theme);
        let trail = self
            .tabs
            .active_stack()
            .map_or_default(ScreenStack::breadcrumbs);
        let can_go_back = trail.len() > 1;
        let keys = self
            .tabs
            .active_stack_mut()
            .and_then(ScreenStack::top_mut)
            .map_or_default(|screen| screen.keys());
        let (context, hints) = if self.palette.is_open() {
            chrome::palette_key_bar(&self.palette.key_hints())
        } else {
            let name = self
                .tabs
                .active_stack()
                .map_or_default(ScreenStack::top_key_bar_name);
            (name, chrome::key_hints(&keys, can_go_back))
        };
        let rail = self.rail;
        let selected = self.selected_sidebar();

        let mut cmds = Vec::new();
        egui::Panel::top("topbar")
            .exact_size(TOP_BAR_HEIGHT)
            .frame(panel_frame(&t, egui::Margin::symmetric(12, 0)))
            .show(ui, |ui| {
                cmds.extend(topbar::top_bar(
                    ui,
                    &self.app_ctx,
                    navigate::today(),
                    self.theme,
                    &t,
                ));
            });
        egui::Panel::bottom("keys")
            .exact_size(KEY_BAR_HEIGHT)
            .frame(panel_frame(&t, egui::Margin::symmetric(12, 0)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some(notice) = &self.notice {
                        ui.colored_label(ui.visuals().error_fg_color, notice.as_str());
                    }
                    chrome::key_bar(ui, &context, &hints, &t);
                });
            });
        let width = if rail {
            sidebar::RAIL_WIDTH
        } else {
            sidebar::WIDTH
        };
        egui::Panel::left("sidebar")
            .exact_size(width)
            .frame(panel_frame(&t, egui::Margin::same(8)))
            .show(ui, |ui| {
                cmds.extend(sidebar::sidebar(ui, rail, selected, &t));
            });
        self.palette_frame(ui);

        cmds.extend(self.stage(ui, &trail, &t));
        cmds.extend(Self::key_cmds(ui, can_go_back));
        for cmd in cmds {
            self.run_cmd(cmd);
        }
    }

    /// Reads Esc, Ctrl+T, Ctrl+W and Ctrl+\. Esc counts only if no screen consumed it this frame.
    fn key_cmds(ui: &egui::Ui, can_go_back: bool) -> Vec<ChromeCmd> {
        let text_focused = ui.ctx().text_edit_focused();
        let mut cmds = Vec::new();
        ui.input_mut(|i| {
            if i.consume_key(Modifiers::COMMAND, Key::T) {
                cmds.push(ChromeCmd::NewTab);
            }
            if i.consume_key(Modifiers::COMMAND, Key::W) {
                cmds.push(ChromeCmd::CloseActiveTab);
            }
            if i.consume_key(Modifiers::COMMAND, Key::Backslash) {
                cmds.push(ChromeCmd::ToggleRail);
            }
            if can_go_back && i.consume_key(Modifiers::NONE, Key::Escape) {
                cmds.push(ChromeCmd::Back);
            }
        });
        // `?` opens the keyboard map (the mock's key bar: "? all keys"), but not while a text
        // field has focus, where it is a typed character.
        if !text_focused && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Questionmark)) {
            cmds.push(ChromeCmd::OpenOnTop(crate::palette::PaletteTarget::Report(
                navigate::KEYS_KEY.to_owned(),
            )));
        }
        cmds
    }

    fn run_cmd(&mut self, cmd: ChromeCmd) {
        match cmd {
            ChromeCmd::SelectTab(i) => {
                self.tabs.select(i);
            }
            ChromeCmd::CloseActiveTab => {
                self.tabs.close(self.tabs.active_index());
            }
            ChromeCmd::NewTab => {
                if self.app_ctx.api.is_some() {
                    self.tabs.open(Box::new(GatewayScreen::new()));
                } else {
                    self.tabs.open(Box::new(WelcomeScreen::default()));
                }
            }
            ChromeCmd::PopTo(depth) => {
                if let Some(stack) = self.tabs.active_stack_mut() {
                    stack.truncate_to(depth);
                }
            }
            ChromeCmd::Back => {
                if let Some(stack) = self.tabs.active_stack_mut() {
                    stack.pop();
                }
            }
            ChromeCmd::CycleTheme => self.theme = self.theme.next(),
            ChromeCmd::OpenPalette => self.open_palette(Action::OpenPalette),
            ChromeCmd::ToggleRail => self.rail = !self.rail,
            ChromeCmd::OpenOnTop(target) => self.open_in_tab(&target, PaletteMode::OnTop),
            ChromeCmd::Replace(target) => self.open_in_tab(&target, PaletteMode::Replace),
            ChromeCmd::Say(text) => self.notice = Some(text),
        }
    }
}

/// A panel frame filled with the surface colour, with `margin` inside it.
fn panel_frame(t: &Tokens, margin: egui::Margin) -> egui::Frame {
    egui::Frame::NONE.fill(t.surface).inner_margin(margin)
}

impl eframe::App for Shell {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}

/// Opens the main window and runs the app until it is closed.
///
/// # Errors
/// Returns the windowing or graphics error eframe reports when the window cannot be created.
pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Ledgerly")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 520.0]),
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "Ledgerly",
        options,
        Box::new(|_cc| Ok(Box::new(Shell::with_welcome()))),
    )
}
