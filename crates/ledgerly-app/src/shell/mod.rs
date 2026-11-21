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
//!
//! Not built here: split view (report beside voucher), remembering the theme between runs, and
//! scalable fonts. They need storage or screens that other cards own.

mod chrome;
pub mod navigate;
mod stack;
pub mod submit;
mod welcome;

use egui::{Key, Modifiers};

use crate::theme::{self, ThemeKind};
use crate::{AppCtx, Screen};
use chrome::ChromeCmd;

pub use stack::{ScreenStack, StackChange, Tabs};
pub use welcome::{WelcomeScreen, company_from_form};

/// The application: tabs of screen stacks, the active theme and the shared screen context.
pub struct Shell {
    tabs: Tabs,
    app_ctx: AppCtx,
    theme: ThemeKind,
    applied_theme: Option<ThemeKind>,
}

impl Shell {
    /// A shell with one tab showing `root`, in the light theme.
    #[must_use]
    pub fn new(root: Box<dyn Screen>) -> Self {
        Self {
            tabs: Tabs::new(root),
            app_ctx: AppCtx::default(),
            theme: ThemeKind::default(),
            applied_theme: None,
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
        egui::Panel::bottom("keys").show(ui, |ui| chrome::key_bar(ui, can_go_back, &t));

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
            ChromeCmd::NewTab => self.tabs.open(Box::new(WelcomeScreen::default())),
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
