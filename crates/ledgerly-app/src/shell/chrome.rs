//! Drawing of the window chrome below the top bar: the tab strip, the breadcrumbs and the key bar.
//! Chrome only reports what the user asked for; [`super::Shell`] applies it after the frame.
//!
//! The key bar's words come from [`key_hints`], which is a pure function so tests can read them
//! without drawing.

use egui::{RichText, Ui};

use super::stack::Tabs;
use crate::screen::KeyHint;
use crate::theme::{TEXT_META, Tokens, border_width};
use crate::ui::button::{ButtonKind, button_icon};
use crate::ui::icon::{self, Icon};
use crate::ui::{a11y, bold_font, keycap, tabs};

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
    /// Show this text as a toast offering "Create a company".
    NeedCompany(String),
    /// Go to the setup wizard at the root of the active tab.
    ToSetup,
}

/// Labels for the tab strip: a title that two tabs share gets its place number after it for the
/// later ones ("Company setup", "Company setup 2"), so every tab reads differently.
#[must_use]
pub fn unique_tab_labels(titles: &[String]) -> Vec<String> {
    titles
        .iter()
        .enumerate()
        .map(|(i, title)| {
            let before = titles.iter().take(i).filter(|t| *t == title).count();
            if before == 0 {
                title.clone()
            } else {
                format!("{title} {}", before.saturating_add(1))
            }
        })
        .collect()
}

/// Tab strip: the open tabs in the kit's tab style (the active one underlined, a rule beneath),
/// then the new-tab and close-tab buttons at the right in the kit's buttons. Close-tab shows
/// only when more than one tab is open, because the last tab stays.
pub(super) fn tab_strip(ui: &mut Ui, tabs_set: &Tabs, t: &Tokens) -> Vec<ChromeCmd> {
    let mut cmds = Vec::new();
    let slot = ui.painter().add(egui::Shape::Noop);
    let row = ui.horizontal(|ui| {
        let titles = unique_tab_labels(&tabs_set.titles());
        let labels: Vec<&str> = titles.iter().map(String::as_str).collect();
        let response = tabs::tab_strip(ui, &labels, Some(tabs_set.active_index()));
        if let Some(index) = response.clicked {
            cmds.push(ChromeCmd::SelectTab(index));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if tabs_set.len() > 1
                && button_icon(
                    ui,
                    Some(Icon::X),
                    "Close tab",
                    Some("Ctrl+W"),
                    ButtonKind::Secondary,
                )
                .clicked()
            {
                cmds.push(ChromeCmd::CloseActiveTab);
            }
            if button_icon(
                ui,
                Some(Icon::Plus),
                "New tab",
                Some("Ctrl+T"),
                ButtonKind::Secondary,
            )
            .clicked()
            {
                cmds.push(ChromeCmd::NewTab);
            }
        });
    });
    let rect = row.response.rect;
    let clip = ui.max_rect();
    ui.painter().set(
        slot,
        egui::Shape::line_segment(
            [
                egui::pos2(clip.min.x, rect.bottom()),
                egui::pos2(clip.max.x, rect.bottom()),
            ],
            egui::Stroke::new(border_width(crate::ui::kind(ui)), t.border),
        ),
    );
    ui.add_space(10.0);
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
                let crumb = ui.add(crumb);
                a11y::name(ui, &crumb, egui::Role::Link, title);
                if crumb.clicked() {
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
/// the stack can go back and the screen has not named Esc itself. Ctrl+K and `?` sit at the right of the bar ([`KEY_BAR_RIGHT`]), as the
/// mock's `renderKeys` draws them; the tab and sidebar keys are in the keyboard map.
#[must_use]
pub fn key_hints(screen: &[KeyHint], can_go_back: bool) -> Vec<(String, String)> {
    // A first hint with no key is the screen's name (index.html:913 draws it as `.ctx`); the bar
    // already draws the name as its context, so it is not a keycap.
    let named = usize::from(screen.first().is_some_and(|hint| hint.keys.is_empty()));
    let mut out: Vec<(String, String)> = screen
        .iter()
        .skip(named)
        .map(|hint| (hint.keys.clone(), hint.label.clone()))
        .collect();
    // A screen that names Esc itself (such as "Esc Back up") gets no second Esc.
    if can_go_back && !screen.iter().any(|hint| hint.keys == "Esc") {
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
        .map_or_default(|hint| hint.label.clone());
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
