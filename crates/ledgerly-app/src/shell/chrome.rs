//! Drawing of the window chrome below the top bar: the tab strip, the breadcrumbs and the key bar.
//! Chrome only reports what the user asked for; [`super::Shell`] applies it after the frame.
//!
//! The key bar's words come from [`key_hints`], which is a pure function so tests can read them
//! without drawing.

use egui::{RichText, Ui};

use super::stack::Tabs;
use crate::screen::KeyHint;
use crate::theme::{TEXT_META, Tokens};
use crate::ui::icon::{self, Icon};
use crate::ui::{bold_font, keycap, tabs};

/// What the user asked for in the chrome this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ChromeCmd {
    /// Activate tab `0`.
    SelectTab(usize),
    /// Open a new tab.
    NewTab,
    /// Go back to a breadcrumb: keep this many screens.
    PopTo(usize),
    /// Close the active tab (Ctrl+W).
    CloseActiveTab,
    /// Esc: close the top screen of the active tab.
    Back,
    /// Switch to the next theme.
    CycleTheme,
    /// Open the command palette on top of the current screen (Ctrl+K, or the search chip).
    OpenPalette,
    /// Show or hide the sidebar's labels (Ctrl+\).
    ToggleRail,
    /// Open this target on top of the active stack (the top bar's AI and keyboard map chips).
    OpenOnTop(crate::palette::PaletteTarget),
    /// Replace the top screen of the active stack with this target (a sidebar click).
    Replace(crate::palette::PaletteTarget),
    /// Show this line in the key bar: why a chip or a row cannot do its job yet.
    Say(String),
}

/// Tab strip: the open tabs with the active one underlined, then the new-tab and close-tab
/// buttons. Close-tab shows only when more than one tab is open, because the last tab stays.
pub(super) fn tab_strip(ui: &mut Ui, tabs_set: &Tabs) -> Vec<ChromeCmd> {
    let mut cmds = Vec::new();
    ui.horizontal(|ui| {
        let titles = tabs_set.titles();
        let labels: Vec<&str> = titles.iter().map(String::as_str).collect();
        let response = tabs::tabs(ui, &labels, tabs_set.active_index());
        if let Some(index) = response.clicked {
            cmds.push(ChromeCmd::SelectTab(index));
        }
        if ui
            .small_button("+")
            .on_hover_text("New tab (Ctrl+T)")
            .clicked()
        {
            cmds.push(ChromeCmd::NewTab);
        }
        if tabs_set.len() > 1
            && ui
                .small_button("Close tab")
                .on_hover_text("Close tab (Ctrl+W)")
                .clicked()
        {
            cmds.push(ChromeCmd::CloseActiveTab);
        }
    });
    cmds
}

/// Breadcrumb trail for the active tab (`.crumbs`: meta size, `text-3`, earlier crumbs in
/// `text-2` with a chevron after each, 6 points below). Earlier crumbs go back to that screen
/// when clicked.
pub(super) fn breadcrumbs(ui: &mut Ui, trail: &[String], t: &Tokens) -> Vec<ChromeCmd> {
    let mut cmds = Vec::new();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let last = trail.len().saturating_sub(1);
        for (i, title) in trail.iter().enumerate() {
            if i == last {
                ui.label(RichText::new(title).size(TEXT_META).color(t.text_3));
            } else {
                let crumb = egui::Label::new(RichText::new(title).size(TEXT_META).color(t.text_2))
                    .sense(egui::Sense::click());
                if ui.add(crumb).clicked() {
                    cmds.push(ChromeCmd::PopTo(i.saturating_add(1)));
                }
                icon::icon(ui, Icon::Chev, t.text_3);
            }
        }
    });
    ui.add_space(6.0);
    cmds
}

/// The key bar's entries, as `(keys, what it does)` pairs: the screen's own hints, then Esc when
/// the stack can go back. Ctrl+K and `?` sit at the right of the bar ([`KEY_BAR_RIGHT`]), as the
/// mock's `renderKeys` draws them; the tab and sidebar keys are in the keyboard map.
#[must_use]
pub fn key_hints(screen: &[KeyHint], can_go_back: bool) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = screen
        .iter()
        .map(|hint| (hint.keys.clone(), hint.label.clone()))
        .collect();
    if can_go_back {
        out.push(("Esc".to_owned(), "Back".to_owned()));
    }
    out
}

/// The key bar while the palette is open (`design/mock-v2/index.html:1159`): the palette's hint
/// with no key names the bar (its bold context, "Go to"), and its other hints follow. The screen
/// under the palette adds nothing, not even Esc Back: Esc closes the palette.
#[must_use]
pub fn palette_key_bar(palette: &[KeyHint]) -> (String, Vec<(String, String)>) {
    let context = palette
        .iter()
        .find(|hint| hint.keys.is_empty())
        .map(|hint| hint.label.clone())
        .unwrap_or_default();
    let hints = palette
        .iter()
        .filter(|hint| !hint.keys.is_empty())
        .map(|hint| (hint.keys.clone(), hint.label.clone()))
        .collect();
    (context, hints)
}

/// The right end of the key bar: each keycap and the words after it (`.keys .right`).
pub const KEY_BAR_RIGHT: [(&str, &str); 2] = [("Ctrl+K", "anything \u{00B7}"), ("?", "all keys")];

/// Gap between key bar entries, in points (`.keys{gap:14px}`).
const ENTRY_GAP: f32 = 14.0;
/// Gap between a keycap and its words (`.keys span{gap:5px}`).
const KEY_GAP: f32 = 5.0;

/// Draws the key bar: the screen's name in bold (`.keys .ctx`), its entries from [`key_hints`]
/// (a keycap, then its words), and [`KEY_BAR_RIGHT`] at the right in `text-3`.
pub(super) fn key_bar(ui: &mut Ui, context: &str, hints: &[(String, String)], t: &Tokens) {
    ui.spacing_mut().item_spacing.x = KEY_GAP;
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = KEY_GAP;
        if !context.is_empty() {
            ui.label(
                RichText::new(context)
                    .font(bold_font(ui, TEXT_META))
                    .color(t.text),
            );
            ui.add_space(ENTRY_GAP - KEY_GAP);
        }
        for (keys, label) in hints {
            keycap::keycap(ui, keys);
            ui.label(
                RichText::new(label.as_str())
                    .size(TEXT_META)
                    .color(t.text_2),
            );
            ui.add_space(ENTRY_GAP - KEY_GAP);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = KEY_GAP;
            for (keys, label) in KEY_BAR_RIGHT.iter().rev() {
                ui.label(RichText::new(*label).size(TEXT_META).color(t.text_3));
                keycap::keycap(ui, keys);
            }
        });
    });
}
