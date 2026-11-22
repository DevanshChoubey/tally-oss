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

pub mod badge;
pub mod button;
pub mod card;
pub mod chip;
pub mod conf;
pub mod empty;
pub mod format;
pub mod icon;
pub mod keycap;
pub mod note;
pub mod page_head;
pub mod prop;
pub mod prov;
pub mod skeleton;
pub mod table;
pub mod tabs;

use egui::{FontFamily, FontId, Ui};

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
