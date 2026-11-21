//! Window, theme switching, screen stack, breadcrumbs and tabs. **Card U1** implements this module.
//!
//! Frozen interface (plan card U1):
//!
//! ```text
//! pub struct Shell { .. }
//! impl eframe::App for Shell { .. }
//! pub fn run() -> eframe::Result<()>;   // called from main.rs (U1 also owns main.rs)
//! ```
//!
//! What the shell does (spec 7):
//!
//! * Every tab is a screen stack. A screen asks for [`crate::ScreenAction::Push`] ("open on top",
//!   resumable) or [`crate::ScreenAction::Replace`]; Esc, or a click on an earlier breadcrumb, goes back.
//! * Esc is handled after the screen has drawn, and only when the screen did not consume it, so a
//!   screen can keep Esc for its own pop-ups and fields.
//! * Ctrl+T opens a tab, Ctrl+W closes the active one. The key map of card U2 is not wired in
//!   here: the shell reads these three keys directly until Wave 2 wiring.
//! * The shell sums nothing and shows no figures (rule C4).
//! * The command palette (Wave 3): the keymap's open-palette chords (Ctrl+K, Alt+G) open it on
//!   top of the current screen, and switch-palette (Ctrl+G) opens it so the choice replaces the
//!   screen. While it is open it takes every key, so the screen under it reacts to none. A pick
//!   opens its screen through [`navigate::open_target`]; a pick that opens nothing leaves a
//!   notice in the key bar ([`Shell::notice`]), never silence.
//! * Opening a company replaces the welcome screen with the company's home screen
//!   ([`GatewayScreen`]), and Ctrl+T then opens a new tab on that home screen.
//!
//! Not built here: split view (report beside voucher), remembering the theme between runs, and
//! scalable fonts. They need storage or screens that other cards own.

mod chrome;
pub mod gateway;
pub mod navigate;
mod stack;
pub mod submit;
mod welcome;

use std::sync::atomic::{AtomicBool, Ordering};

use egui::{Key, Modifiers};

use crate::keymap::{self, Action, Keymap};
use crate::palette::{Palette, PaletteChoice, PaletteSearch, PaletteTarget};
use crate::theme::{self, ThemeKind};
use crate::worker::Ticket;
use crate::{AppCtx, Screen};
use chrome::ChromeCmd;
use navigate::NavError;
use submit::BooksReply;

pub use gateway::GatewayScreen;
pub use stack::{ScreenStack, StackChange, Tabs};
pub use welcome::{WelcomeScreen, company_from_form};

/// Set by a screen that wants the palette opened (the home screen's Ctrl+K link); the shell
/// takes it on its next frame.
static PALETTE_WANTED: AtomicBool = AtomicBool::new(false);

/// Asks the shell to open the palette on top of the current screen on its next frame.
pub(crate) fn ask_for_palette() {
    PALETTE_WANTED.store(true, Ordering::SeqCst);
}

/// The application: tabs of screen stacks, the active theme and the shared screen context.
pub struct Shell {
    tabs: Tabs,
    app_ctx: AppCtx,
    theme: ThemeKind,
    applied_theme: Option<ThemeKind>,
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
    /// Why the last palette pick opened nothing, or why the key map could not be read.
    notice: Option<String>,
}

impl Shell {
    /// A shell with one tab showing `root`, in the light theme.
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

    /// Why the last palette pick opened nothing, when it did not.
    #[must_use]
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// Opens the palette for `action` (open-palette or switch-palette) with today's rows.
    fn open_palette(&mut self, action: Action) {
        let mut entries = navigate::host_entries(&self.keymap);
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
        let choice = egui::Area::new(egui::Id::unique("ledgerly-palette"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 48.0])
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style())
                    .show(ui, |ui| {
                        ui.set_min_width(520.0);
                        self.palette.ui(ui)
                    })
                    .inner
            })
            .inner;
        // Whatever key the palette did not use this frame is not passed to the screen under it.
        ui.input_mut(|i| {
            i.events.retain(|e| !matches!(e, egui::Event::Key { .. }));
        });
        if let Some(choice) = choice {
            self.pick(&choice);
        }
    }

    /// Opens what the palette pick names, or says why it opens nothing.
    fn pick(&mut self, choice: &PaletteChoice) {
        match self.tabs.open_choice(choice, &self.app_ctx) {
            Ok(_) => self.notice = None,
            Err(NavError::NotNavigation) => self.run_pick(&choice.target),
            Err(e) => self.notice = Some(e.to_string()),
        }
    }

    /// A command or assistant pick: the shell's own commands run; any other says it cannot run
    /// from the palette yet.
    fn run_pick(&mut self, target: &PaletteTarget) {
        let cmd = match target {
            PaletteTarget::Command(Action::NewTab) => Some(ChromeCmd::NewTab),
            PaletteTarget::Command(Action::CloseTab) => Some(ChromeCmd::CloseActiveTab),
            PaletteTarget::Command(Action::Back) => Some(ChromeCmd::Back),
            PaletteTarget::Command(action @ (Action::OpenPalette | Action::SwitchPalette)) => {
                self.open_palette(*action);
                return;
            }
            PaletteTarget::Command(action) => {
                self.notice = Some(format!(
                    "\"{}\" runs on the screen it belongs to; the palette cannot run it yet.",
                    action.label()
                ));
                None
            }
            PaletteTarget::AiIntent(name) => {
                self.notice = Some(format!(
                    "Not supported yet: the assistant request \"{name}\" (the assistant is not \
                     connected to the app yet)."
                ));
                None
            }
            PaletteTarget::Report(_)
            | PaletteTarget::Master(_)
            | PaletteTarget::Voucher(_)
            | PaletteTarget::Setting(_) => {
                self.notice = Some("This choice opened nothing.".to_owned());
                None
            }
        };
        if let Some(cmd) = cmd {
            self.notice = None;
            self.run_cmd(cmd);
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

    /// Draws one frame into `ui`: chrome, the top screen of the active tab, then key handling.
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

        let mut cmds = Vec::new();
        egui::Panel::top("tabs").show(ui, |ui| {
            cmds.extend(chrome::tab_strip(ui, &self.tabs, self.theme, &t));
            cmds.extend(chrome::breadcrumbs(ui, &trail, &t));
        });
        egui::Panel::bottom("keys").show(ui, |ui| {
            if let Some(notice) = &self.notice {
                ui.colored_label(ui.visuals().error_fg_color, notice.as_str());
            }
            chrome::key_bar(ui, can_go_back, &t);
        });
        self.palette_frame(ui);

        let mut screen_action = crate::ScreenAction::Stay;
        egui::CentralPanel::default_margins().show(ui, |ui| {
            let top = self.tabs.active_stack_mut().and_then(ScreenStack::top_mut);
            if let Some(screen) = top {
                screen_action = screen.ui(ui, &mut self.app_ctx);
            }
        });

        if let Some(stack) = self.tabs.active_stack_mut() {
            stack.apply(screen_action);
        }
        cmds.extend(Self::key_cmds(ui, can_go_back));
        for cmd in cmds {
            self.run_cmd(cmd);
        }
    }

    /// Reads Esc, Ctrl+T and Ctrl+W. Esc counts only if no screen consumed it this frame.
    fn key_cmds(ui: &egui::Ui, can_go_back: bool) -> Vec<ChromeCmd> {
        let mut cmds = Vec::new();
        ui.input_mut(|i| {
            if i.consume_key(Modifiers::COMMAND, Key::T) {
                cmds.push(ChromeCmd::NewTab);
            }
            if i.consume_key(Modifiers::COMMAND, Key::W) {
                cmds.push(ChromeCmd::CloseActiveTab);
            }
            if can_go_back && i.consume_key(Modifiers::NONE, Key::Escape) {
                cmds.push(ChromeCmd::Back);
            }
        });
        cmds
    }

    fn run_cmd(&mut self, cmd: ChromeCmd) {
        match cmd {
            ChromeCmd::SelectTab(i) => {
                self.tabs.select(i);
            }
            ChromeCmd::CloseTab(i) => {
                self.tabs.close(i);
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
        }
    }
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
