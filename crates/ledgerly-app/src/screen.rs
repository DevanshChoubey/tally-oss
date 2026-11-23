use ledgerly_core::Date;
use ledgerly_model::Books;

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

/// The AI tier the open company is set to. The top bar's AI chip shows it; `None` on
/// [`AppCtx::ai_tier`] means no tier has been stored yet, and the chip says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiTier {
    /// AI is off for this company.
    Off,
    /// The local model on this computer. Nothing leaves the computer.
    Local,
    /// A cloud provider with the user's own key.
    Cloud,
}

/// One screen on the screen stack (spec 7).
pub trait Screen {
    /// Title shown in the breadcrumb and tab.
    fn title(&self) -> String;

    /// The page-head title. Defaults to [`Screen::title`]. A screen that shows a page head (see
    /// `ui::page_head`) draws this; the shell does not draw page heads itself.
    fn page_title(&self) -> String {
        self.title()
    }

    /// The screen's name at the left of the key bar (`.keys .ctx` in the mock). Defaults to
    /// [`Screen::page_title`]; Home, whose page title is a greeting, says "Home".
    fn key_bar_name(&self) -> String {
        self.page_title()
    }

    /// The one-line purpose under the page title, when the screen has one. Defaults to none.
    fn subtitle(&self) -> Option<String> {
        None
    }

    /// The title the sidebar selects its entry by. Defaults to [`Screen::title`]. A screen a
    /// drill opened returns the title of the screen the drill started from, so that screen's
    /// entry stays selected (index.html:1568, 1594, 1600: the group, ledger and voucher states
    /// belong to the balance sheet).
    fn sidebar_title(&self) -> String {
        self.title()
    }

    /// The keys this screen handles, shown in the key bar before the shell's own keys. Defaults
    /// to none, so a screen that has not opted in shows only the shell's keys.
    fn keys(&self) -> Vec<KeyHint> {
        Vec::new()
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
    /// The period lock date. `None` until `ledgerly-api` exposes it: `Books` carries no lock date
    /// (`ledgerly-engine/src/locks.rs` says it is derived from the `PeriodLocked` events and passed
    /// in), and `Api` has no call that returns it. The lock chip is hidden while this is `None`.
    /// Needs an E-item.
    pub lock_date: Option<Date>,
    /// The AI tier stored for the open company. `None` until a tier store exists, so the AI chip
    /// reads "AI: not set up" and never claims a tier nothing enforces.
    pub ai_tier: Option<AiTier>,
}
