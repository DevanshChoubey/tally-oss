//! Layout probes: where a screen drew a part, recorded by name so layout tests can read it back
//! after the frame. A probe records a rectangle or a colour the screen used; it never decides
//! anything the screen draws.

use egui::Ui;

/// Records `value` under `name` for this frame, in the context's memory.
pub fn record_probe<T: Clone + Send + Sync + 'static>(ui: &Ui, name: &str, value: T) {
    let id = egui::Id::unique(name);
    ui.ctx().data_mut(|d| d.insert_temp(id, value));
}

/// The value a layout probe recorded under `name` in the last frame, if the screen drew it.
#[must_use]
pub fn read_probe<T: Clone + Send + Sync + 'static>(ctx: &egui::Context, name: &str) -> Option<T> {
    ctx.data(|d| d.get_temp::<T>(egui::Id::unique(name)))
}
