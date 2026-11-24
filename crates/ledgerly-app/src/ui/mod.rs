//! Ledgerly component kit: the components of the UI mock v2 (`design/mock-v2/index.html`, the
//! `.chip`, `.badge`, `.card`, `.btn`, `.tabs`, `.prop`, `.conf`, `.prov`, `.note`, `.empty`,
//! `.skel` and `table.t` rules), drawn with egui.
//!
//! How the kit gets its colours: every component takes `&mut egui::Ui` and reads the tokens with
//! [`tokens`]. That asks [`crate::theme::current`] for the theme which [`crate::theme::apply`]
//! stored in the egui context. A screen therefore passes no theme: the shell applies the theme
//! once, and every component drawn after that follows it. Before `apply` has run the kit uses
//! the light theme.
//!
//! The kit paints and lays out. It never computes or formats a figure beyond [`format`](mod@format), which
//! only writes numbers the engine has already produced.

pub mod a11y;
pub mod badge;
pub mod button;
pub mod card;
pub mod chip;
pub mod conf;
pub mod empty;
pub mod format;
pub mod icon;
pub mod keycap;
pub mod mock_table;
pub mod note;
pub mod option_card;
pub mod page_head;
pub mod probe;
pub mod prop;
pub mod prov;
pub mod skeleton;
pub mod switch;
pub mod table;
pub mod tabs;

use egui::{Align, Color32, FontFamily, FontId, Layout, Response, RichText, Ui};

use crate::theme::{self, ThemeKind, Tokens};

/// The tokens of the theme the context is drawn in.
#[must_use]
pub fn tokens(ui: &Ui) -> Tokens {
    theme::tokens(kind(ui))
}

/// The theme the context is drawn in (light before [`theme::apply`] has run).
#[must_use]
pub fn kind(ui: &Ui) -> ThemeKind {
    theme::current(ui.ctx())
}

/// The bold face at `size`. If the bold family is not installed, falls back to the regular face
/// rather than drawing nothing.
#[must_use]
pub fn bold_font(ui: &Ui, size: f32) -> FontId {
    let bold = theme::bold_family();
    let bound = ui
        .ctx()
        .fonts(|f| f.definitions().families.contains_key(&bold));
    let family = if bound {
        bold
    } else {
        FontFamily::Proportional
    };
    FontId::new(size, family)
}

/// Line height as a multiple of the text size (`body{line-height:1.45}` in the mock).
pub const LINE_HEIGHT: f32 = 1.45;

/// The mock's line height for text of `size` points, rounded to a whole point.
#[must_use]
#[allow(clippy::float_arithmetic, reason = "one product")]
pub fn line_height(size: f32) -> f32 {
    (size * LINE_HEIGHT).round()
}

/// `text` at `size` in `color`, at the mock's line height.
#[must_use]
pub fn para_text(text: &str, size: f32, color: Color32) -> RichText {
    RichText::new(text)
        .size(size)
        .color(color)
        .line_height(Some(line_height(size)))
}

/// Draws `text` as a wrapping paragraph at the mock's line height, ragged right even inside a
/// justified layout (where egui would otherwise spread the words of each full row).
pub fn para(ui: &mut Ui, text: &str, size: f32, color: Color32) -> Response {
    ui.with_layout(Layout::top_down(Align::Min), |ui| {
        ui.add(egui::Label::new(para_text(text, size, color)).wrap())
    })
    .inner
}
