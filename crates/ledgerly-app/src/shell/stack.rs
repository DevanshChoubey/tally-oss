//! Screen stack and tab set: the state behind the breadcrumbs and the open-screen tabs.

use crate::i18n::Tr;
use ledgerly_api::ReportKind;

use super::navigate::{self, NavError};
use crate::palette::{PaletteChoice, PaletteMode};
use crate::{AppCtx, Screen, ScreenAction};

/// What applying a [`ScreenAction`] did to a stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackChange {
    /// Nothing changed.
    None,
    /// A screen was opened on top of the current one.
    Pushed,
    /// The current screen was replaced.
    Replaced,
    /// The current screen was closed and the one below it is on top again.
    Popped,
    /// The screen asked to close but it is the root of its tab, so it stays.
    RootKept,
}

/// A stack of screens. "Open on top" pushes (Esc returns to the screen below, still as it was);
/// "replace" swaps the top screen. The bottom screen (the root) is never removed, so a stack is
/// never empty.
pub struct ScreenStack {
    screens: Vec<Box<dyn Screen>>,
}

impl ScreenStack {
    /// A stack holding only `root`.
    #[must_use]
    pub fn new(root: Box<dyn Screen>) -> Self {
        Self {
            screens: vec![root],
        }
    }

    /// Number of screens, at least 1.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.screens.len()
    }

    /// Opens `screen` on top of the current one.
    pub fn push(&mut self, screen: Box<dyn Screen>) {
        self.screens.push(screen);
    }

    /// Replaces the top screen with `screen`. The root can be replaced too.
    pub fn replace(&mut self, screen: Box<dyn Screen>) {
        self.screens.pop();
        self.screens.push(screen);
    }

    /// Closes the top screen. Returns `false` and changes nothing when only the root is left.
    pub fn pop(&mut self) -> bool {
        if self.screens.len() > 1 {
            self.screens.pop();
            true
        } else {
            false
        }
    }

    /// Closes screens until `depth` remain (at least 1). Returns how many were closed.
    pub fn truncate_to(&mut self, depth: usize) -> usize {
        let keep = depth.max(1);
        let before = self.screens.len();
        self.screens.truncate(keep);
        before.saturating_sub(self.screens.len())
    }

    /// Applies what a screen asked for after a frame.
    pub fn apply(&mut self, action: ScreenAction) -> StackChange {
        match action {
            ScreenAction::Stay => StackChange::None,
            ScreenAction::Push(screen) => {
                self.push(screen);
                StackChange::Pushed
            }
            ScreenAction::Replace(screen) => {
                self.replace(screen);
                StackChange::Replaced
            }
            ScreenAction::Pop => {
                if self.pop() {
                    StackChange::Popped
                } else {
                    StackChange::RootKept
                }
            }
        }
    }

    /// Opens `screen` the way the palette was opened: on top of the current one, or in its place.
    pub fn open_screen(&mut self, screen: Box<dyn Screen>, mode: PaletteMode) -> StackChange {
        match mode {
            PaletteMode::OnTop => {
                self.push(screen);
                StackChange::Pushed
            }
            PaletteMode::Replace => {
                self.replace(screen);
                StackChange::Replaced
            }
        }
    }

    /// Opens what a palette pick names (a ledger report, a voucher, a report), on top or in place
    /// as the palette was opened. Nothing changes when it fails.
    ///
    /// # Errors
    /// [`NavError`] when the pick opens no screen: a command or assistant request
    /// ([`NavError::NotNavigation`], which the host handles), a setting, or no books to open from.
    pub fn open_choice(
        &mut self,
        choice: &PaletteChoice,
        ctx: &AppCtx,
    ) -> Result<StackChange, NavError> {
        let screen = navigate::open_target(&choice.target, ctx)?;
        Ok(self.open_screen(screen, choice.mode))
    }

    /// Opens a report menu entry as a worker-mode report screen. Nothing changes when it fails.
    ///
    /// # Errors
    /// [`NavError::NoCompany`] when no books are loaded, otherwise as
    /// [`navigate::report_screen`].
    pub fn open_report(
        &mut self,
        kind: ReportKind,
        ctx: &AppCtx,
        mode: PaletteMode,
    ) -> Result<StackChange, NavError> {
        let screen = navigate::open_report(kind, ctx)?;
        Ok(self.open_screen(screen, mode))
    }

    /// Titles from the root to the top screen: the breadcrumb trail.
    #[must_use]
    pub fn breadcrumbs(&self, tr: &Tr) -> Vec<String> {
        self.screens.iter().map(|s| s.title(tr)).collect()
    }

    /// Title of the top screen.
    #[must_use]
    pub fn top_title(&self, tr: &Tr) -> String {
        self.screens.last().map_or_default(|s| s.title(tr))
    }

    /// The label of the row under the top screen's cursor ([`Screen::cursor_label`]).
    #[must_use]
    pub fn top_cursor_label(&self, ctx: &AppCtx) -> Option<String> {
        self.screens.last().and_then(|s| s.cursor_label(ctx))
    }

    /// The title the sidebar selects by for the top screen ([`Screen::sidebar_title`]).
    #[must_use]
    pub fn top_sidebar_title(&self, tr: &Tr) -> String {
        self.screens.last().map_or_default(|s| s.sidebar_title(tr))
    }

    /// The page title of the top screen ([`Screen::page_title`]), which the key bar names.
    #[must_use]
    pub fn top_page_title(&self, tr: &Tr) -> String {
        self.screens.last().map_or_default(|s| s.page_title(tr))
    }

    /// The key bar name of the top screen ([`Screen::key_bar_name`]), or empty with no screen.
    #[must_use]
    pub fn top_key_bar_name(&self, tr: &Tr) -> String {
        self.screens.last().map_or_default(|s| s.key_bar_name(tr))
    }

    /// The top screen, which is the one drawn.
    pub fn top_mut(&mut self) -> Option<&mut Box<dyn Screen>> {
        self.screens.last_mut()
    }
}

/// Open-screen tabs: each tab is its own [`ScreenStack`]. There is always at least one tab and
/// one of them is active.
pub struct Tabs {
    stacks: Vec<ScreenStack>,
    active: usize,
}

impl Tabs {
    /// One tab whose root is `root`.
    #[must_use]
    pub fn new(root: Box<dyn Screen>) -> Self {
        Self {
            stacks: vec![ScreenStack::new(root)],
            active: 0,
        }
    }

    /// Number of tabs, at least 1.
    #[must_use]
    pub fn len(&self) -> usize {
        self.stacks.len()
    }

    /// Always `false`: a tab set is never empty. Present for the `len` convention.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.stacks.is_empty()
    }

    /// Index of the active tab.
    #[must_use]
    pub fn active_index(&self) -> usize {
        self.active
    }

    /// Opens a new tab with `root` and makes it active.
    pub fn open(&mut self, root: Box<dyn Screen>) {
        self.stacks.push(ScreenStack::new(root));
        self.active = self.stacks.len().saturating_sub(1);
    }

    /// Makes tab `index` active. Returns `false` when there is no such tab.
    pub fn select(&mut self, index: usize) -> bool {
        if index < self.stacks.len() {
            self.active = index;
            true
        } else {
            false
        }
    }

    /// Closes tab `index` with everything stacked in it. Returns `false` and changes nothing when
    /// there is no such tab or it is the only one. The active tab stays the same tab, or, if it
    /// was the one closed, becomes the tab that took its place (the last tab if it was last).
    pub fn close(&mut self, index: usize) -> bool {
        if self.stacks.len() <= 1 || index >= self.stacks.len() {
            return false;
        }
        self.stacks.remove(index);
        if index < self.active {
            self.active = self.active.saturating_sub(1);
        }
        self.active = self.active.min(self.stacks.len().saturating_sub(1));
        true
    }

    /// Title of each tab's top screen, in tab order.
    #[must_use]
    pub fn titles(&self, tr: &Tr) -> Vec<String> {
        self.stacks
            .iter()
            .map(|stack| stack.top_title(tr))
            .collect()
    }

    /// The active tab's stack.
    #[must_use]
    pub fn active_stack(&self) -> Option<&ScreenStack> {
        self.stacks.get(self.active)
    }

    /// The active tab's stack, mutably.
    pub fn active_stack_mut(&mut self) -> Option<&mut ScreenStack> {
        self.stacks.get_mut(self.active)
    }

    /// [`ScreenStack::open_choice`] on the active tab.
    ///
    /// # Errors
    /// As [`ScreenStack::open_choice`].
    pub fn open_choice(
        &mut self,
        choice: &PaletteChoice,
        ctx: &AppCtx,
    ) -> Result<StackChange, NavError> {
        match self.active_stack_mut() {
            Some(stack) => stack.open_choice(choice, ctx),
            None => Ok(StackChange::None),
        }
    }

    /// [`ScreenStack::open_report`] on the active tab.
    ///
    /// # Errors
    /// As [`ScreenStack::open_report`].
    pub fn open_report(
        &mut self,
        kind: ReportKind,
        ctx: &AppCtx,
        mode: PaletteMode,
    ) -> Result<StackChange, NavError> {
        match self.active_stack_mut() {
            Some(stack) => stack.open_report(kind, ctx, mode),
            None => Ok(StackChange::None),
        }
    }
}
