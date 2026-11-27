use ledgerly_core::Date;
use ledgerly_model::Books;

use crate::i18n::Tr;

/// One key hint a screen shows in the key bar: the key label (such as `Ctrl+A`) and what it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyHint {
    /// The key or chord, as the key bar draws it in a keycap.
    pub keys: String,
    /// What the key does on this screen, in the app's own words.
    pub label: String,
}

impl KeyHint {
    /// A hint for `keys` that does `label`.
    #[must_use]
    pub fn new(keys: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            keys: keys.into(),
            label: label.into(),
        }
    }
}

/// The AI tier the open company is set to (`ledgerly_api::AiTier`). The top bar's AI chip shows
/// it; `None` on [`AppCtx::ai_tier`] means no tier has been stored yet, and the chip says so.
pub use ledgerly_api::AiTier;

/// One screen on the screen stack (spec 7).
pub trait Screen {
    /// Title shown in the breadcrumb and tab.
    fn title(&self, tr: &Tr) -> String;

    /// The page-head title. Defaults to [`Screen::title`]. A screen that shows a page head (see
    /// `ui::page_head`) draws this; the shell does not draw page heads itself.
    fn page_title(&self, tr: &Tr) -> String {
        self.title(tr)
    }

    /// The screen's name at the left of the key bar (`.keys .ctx` in the mock). Defaults to
    /// [`Screen::page_title`]; Home, whose page title is a greeting, says "Home".
    fn key_bar_name(&self, tr: &Tr) -> String {
        self.page_title(tr)
    }

    /// The one-line purpose under the page title, when the screen has one. Defaults to none.
    fn subtitle(&self, _tr: &Tr) -> Option<String> {
        None
    }

    /// The title the sidebar selects its entry by. Defaults to [`Screen::title`]. A screen a
    /// drill opened returns the title of the screen the drill started from, so that screen's
    /// entry stays selected (index.html:1568, 1594, 1600: the group, ledger and voucher states
    /// belong to the balance sheet).
    fn sidebar_title(&self, tr: &Tr) -> String {
        self.title(tr)
    }

    /// Whether the shell draws the breadcrumbs above this screen when it is not the root.
    /// Defaults to `true`; the setup wizard, drawn as one card with no crumbs in the mock
    /// (index.html `SCREENS.setup`), says `false`.
    fn shows_breadcrumbs(&self) -> bool {
        true
    }

    /// The keys this screen handles, shown in the key bar before the shell's own keys. Defaults
    /// to none, so a screen that has not opted in shows only the shell's keys.
    fn keys(&self, _tr: &Tr) -> Vec<KeyHint> {
        Vec::new()
    }

    /// The label of the row the cursor is on, for the screens that list rows (a report line, a
    /// master, a GST table, a bank account). The screenshot harness reads it to move to a row by
    /// name and to check it reached the screen it meant to; no screen draws from it. Defaults to
    /// none, which also means "no row yet" while a list is still loading.
    fn cursor_label(&self, _ctx: &AppCtx) -> Option<String> {
        None
    }

    /// A shell key that would open this same screen was pressed while it is on top (Alt+O on
    /// the import screen). The screen goes to the part that key opens; returns whether it did.
    /// Defaults to `false`: nothing happens.
    fn reopen(&mut self, _action: crate::keymap::Action) -> bool {
        false
    }

    /// Draws the screen and returns what the stack should do next.
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut AppCtx) -> ScreenAction;
}

/// What the screen stack does after a frame.
pub enum ScreenAction {
    /// Stay on this screen.
    Stay,
    /// Open a screen on top (Esc returns here).
    Push(Box<dyn Screen>),
    /// Replace this screen.
    Replace(Box<dyn Screen>),
    /// Close this screen.
    Pop,
}

/// Shared state screens read. Until Wave 2 wiring (W6, W7) screens read a `Books` fixture; after
/// wiring, every figure comes from `ledgerly-api` (rule C4: the UI never sums anything).
///
/// Every field the top bar shows is either filled from data the shell already has or is `None`
/// (the chip then says it is not known). The default is always the "not known" state, never a
/// made-up value.
#[derive(Debug, Default)]
pub struct AppCtx {
    /// The open company's books, when one is open. The company name and financial year come from
    /// here.
    pub books: Option<Books>,
    /// The worker that runs every `ledgerly-api` call off the UI thread, when a company file is
    /// open. Wave 2 wiring (W6, W7) moves screens from `books` to this.
    pub api: Option<crate::worker::ApiWorker>,
    /// The period lock date of the open company (`Api::lock_date`, the last `PeriodLocked`
    /// event), which the shell reads with each books fetch. `None` when the books were never
    /// locked or no company is open; the lock chip is hidden then.
    pub lock_date: Option<Date>,
    /// The AI tier stored for the open company. `None` until a tier store exists, so the AI chip
    /// reads "AI: not set up" and never claims a tier nothing enforces.
    pub ai_tier: Option<AiTier>,
    /// The strings in the language the user chose (`i18n`). Screens draw their text with it.
    pub tr: crate::i18n::Tr,
}
