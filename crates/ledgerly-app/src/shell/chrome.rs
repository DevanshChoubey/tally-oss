//! Drawing of the window chrome: tab strip, breadcrumbs and the key-hint bar. Chrome only
//! reports what the user asked for; [`super::Shell`] applies it after the frame.

use egui::{RichText, Ui};

use super::stack::Tabs;
use crate::theme::{ThemeKind, Tokens};

/// What the user asked for in the chrome this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChromeCmd {
    /// Activate tab `0`.
    SelectTab(usize),
    /// Close tab `0`.
    CloseTab(usize),
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
}

/// Tab strip with the theme switcher at the right.
pub(super) fn tab_strip(ui: &mut Ui, tabs: &Tabs, theme: ThemeKind, t: &Tokens) -> Vec<ChromeCmd> {
    let mut cmds = Vec::new();
    ui.horizontal(|ui| {
        ui.label(RichText::new("Ledgerly").strong().color(t.text));
        ui.separator();
        let closable = tabs.len() > 1;
        for (i, title) in tabs.titles().into_iter().enumerate() {
            let selected = i == tabs.active_index();
            if ui.selectable_label(selected, title).clicked() {
                cmds.push(ChromeCmd::SelectTab(i));
            }
            if closable
                && ui
                    .small_button("\u{00D7}")
                    .on_hover_text("Close tab")
                    .clicked()
            {
                cmds.push(ChromeCmd::CloseTab(i));
            }
        }
        if ui
            .small_button("+")
            .on_hover_text("New tab (Ctrl+T)")
            .clicked()
        {
            cmds.push(ChromeCmd::NewTab);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let text = format!("Theme: {}", theme.label());
            if ui.button(text).on_hover_text("Switch theme").clicked() {
                cmds.push(ChromeCmd::CycleTheme);
            }
        });
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

/// Key hints along the bottom.
pub(super) fn key_bar(ui: &mut Ui, can_go_back: bool, t: &Tokens) {
    ui.horizontal(|ui| {
        let hint = if can_go_back {
            "Esc  Back     Ctrl+T  New tab     Ctrl+W  Close tab"
        } else {
            "Ctrl+T  New tab     Ctrl+W  Close tab"
        };
        ui.label(RichText::new(hint).color(t.text_2));
    });
}
