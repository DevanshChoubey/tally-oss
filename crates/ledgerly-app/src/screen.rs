use ledgerly_model::Books;

/// One screen on the screen stack (spec 7).
pub trait Screen {
    /// Title shown in the breadcrumb and tab.
    fn title(&self) -> String;

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
#[derive(Debug, Default)]
pub struct AppCtx {
    /// The open company's books, when one is open.
    pub books: Option<Books>,
}
