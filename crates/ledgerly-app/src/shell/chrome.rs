//! Drawing of the window chrome below the top bar: the tab strip, the breadcrumbs and the key bar.
//! Chrome only reports what the user asked for; [`super::Shell`] applies it after the frame.
//!
//! The key bar's words come from [`key_hints`], which is a pure function so tests can read them
//! without drawing.

use egui::{RichText, Ui};

use super::stack::Tabs;
use crate::screen::KeyHint;
use crate::theme::{SPACE_3, TEXT_META, Tokens};
use crate::ui::{keycap, tabs};

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

/// Breadcrumb trail for the active tab. Earlier crumbs go back to that screen when clicked.
pub(super) fn breadcrumbs(ui: &mut Ui, trail: &[String], t: &Tokens) -> Vec<ChromeCmd> {
    let mut cmds = Vec::new();
    ui.horizontal(|ui| {
        let last = trail.len().saturating_sub(1);
        for (i, title) in trail.iter().enumerate() {
            if i == last {
                ui.label(RichText::new(title).strong().color(t.text));
            } else {
                if ui.link(title).clicked() {
                    cmds.push(ChromeCmd::PopTo(i.saturating_add(1)));
                }
                ui.label(RichText::new("\u{203A}").color(t.text_3));
            }
        }
    });
    cmds
}

/// The key bar's entries, as `(keys, what it does)` pairs: the screen's own hints first, then
/// Esc (only when the stack can go back), Ctrl+K, Ctrl+T, Ctrl+W and Ctrl+\.
#[must_use]
pub fn key_hints(screen: &[KeyHint], can_go_back: bool) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = screen
        .iter()
        .map(|hint| (hint.keys.clone(), hint.label.clone()))
        .collect();
    if can_go_back {
        out.push(("Esc".to_owned(), "Back".to_owned()));
    }
    out.push(("Ctrl+K".to_owned(), "Search".to_owned()));
    out.push(("Ctrl+T".to_owned(), "New tab".to_owned()));
    out.push(("Ctrl+W".to_owned(), "Close tab".to_owned()));
    out.push(("Ctrl+\\".to_owned(), "Sidebar".to_owned()));
    out
}

/// Draws the key bar's entries from [`key_hints`]: a keycap, then its words.
pub(super) fn key_bar(ui: &mut Ui, hints: &[(String, String)], t: &Tokens) {
    ui.horizontal(|ui| {
        for (keys, label) in hints {
            keycap::keycap(ui, keys);
            ui.label(RichText::new(label.as_str()).size(TEXT_META).color(t.text_2));
            ui.add_space(SPACE_3);
        }
    });
}
