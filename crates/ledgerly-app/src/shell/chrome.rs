//! Drawing of the window chrome below the top bar: the tab strip, the breadcrumbs and the key bar.
//! Chrome only reports what the user asked for; [`super::Shell`] applies it after the frame.
//!
//! The key bar's words come from [`key_hints`], which is a pure function so tests can read them
//! without drawing.

use egui::{RichText, Ui};

use super::stack::Tabs;
use crate::i18n::Tr;
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
    /// A key that would open the top screen again: the top screen goes to that part of itself
    /// ([`crate::Screen::reopen`]).
    Reopen(crate::keymap::Action),
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
pub(super) fn tab_strip(ui: &mut Ui, tabs_set: &Tabs, t: &Tokens, tr: &Tr) -> Vec<ChromeCmd> {
    let mut cmds = Vec::new();
    let slot = ui.painter().add(egui::Shape::Noop);
    let row = ui.horizontal(|ui| {
        let titles = unique_tab_labels(&tabs_set.titles(tr));
        let labels: Vec<&str> = titles.iter().map(String::as_str).collect();
        let response = tabs::tab_strip(ui, &labels, Some(tabs_set.active_index()));
        if let Some(index) = response.clicked {
            cmds.push(ChromeCmd::SelectTab(index));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if button_icon(
                ui,
                Some(Icon::Plus),
                &tr.t("chrome.new_tab"),
                Some("Ctrl+T"),
                ButtonKind::Secondary,
            )
            .clicked()
            {
                cmds.push(ChromeCmd::NewTab);
            }
            if tabs_set.len() > 1
                && button_icon(
                    ui,
                    Some(Icon::X),
                    &tr.t("chrome.close_tab"),
                    Some("Ctrl+W"),
                    ButtonKind::Secondary,
                )
                .clicked()
            {
                cmds.push(ChromeCmd::CloseActiveTab);
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
    key_hints_in(screen, can_go_back, &Tr::default())
}

/// [`key_hints`] with the shell's own words (Esc Back) in the language `tr`.
#[must_use]
pub fn key_hints_in(screen: &[KeyHint], can_go_back: bool, tr: &Tr) -> Vec<(String, String)> {
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
        out.push(("Esc".to_owned(), tr.t("chrome.back")));
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

/// A modifier key held down: the key bar then lists the keys that use it (mock `renderKeys`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    /// Alt is held.
    Alt,
    /// Ctrl is held.
    Ctrl,
}

impl Held {
    /// The held modifier in `modifiers`, Alt first, or `None`.
    #[must_use]
    pub fn of(modifiers: egui::Modifiers) -> Option<Held> {
        if modifiers.alt {
            Some(Held::Alt)
        } else if modifiers.command || modifiers.ctrl {
            Some(Held::Ctrl)
        } else {
            None
        }
    }

    /// The word the bar shows after "Holding".
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Held::Alt => "Alt",
            Held::Ctrl => "Ctrl",
        }
    }
}

/// The keys the bar lists while a modifier is held, in the mock's order and words
/// (`ALT_KEYS`, `CTRL_KEYS`, index.html:916-917). Each is listed only when `keymap` binds the
/// action to a chord with that modifier, and shows that chord, so the bar never names a key that
/// does nothing.
#[must_use]
pub fn held_hints(keymap: &crate::keymap::Keymap, held: Held) -> Vec<(String, String)> {
    use crate::keymap::Action;
    let wanted: &[(Action, &str)] = match held {
        Held::Alt => &[
            (Action::CreateMaster, "Create master here"),
            (Action::DeleteVoucher, "Delete / reject draft"),
            (Action::CancelVoucher, "Cancel voucher"),
            (Action::DuplicateVoucher, "Duplicate"),
            (Action::InsertVoucher, "Insert voucher"),
            (Action::ToggleDetail, "Detailed / condensed"),
        ],
        Held::Ctrl => &[
            (Action::Save, "Accept / save"),
            (Action::OpenPalette, "Palette"),
            (Action::SwitchPalette, "Switch to (replace)"),
            (Action::MoreDetails, "More details / source"),
            (Action::AlterMaster, "Alter master"),
            (Action::Print, "Print"),
            (Action::Export, "Export"),
            (Action::Undo, "Undo field"),
        ],
    };
    wanted
        .iter()
        .filter_map(|(action, words)| {
            let chord = keymap.chords(*action).iter().find(|chord| match held {
                Held::Alt => chord.alt && !chord.command,
                Held::Ctrl => chord.command && !chord.alt,
            })?;
            Some((chord.to_string(), (*words).to_owned()))
        })
        .collect()
}

/// The words before Ctrl+K at the right of the bar while no modifier is held.
pub const KEY_BAR_HOLD_HINT: &str = "Hold Alt or Ctrl for more \u{00B7}";

/// The right end of the key bar: each keycap and the words after it (`.keys .right`).
pub const KEY_BAR_RIGHT: [(&str, &str); 2] = [("Ctrl+K", "anything \u{00B7}"), ("?", "all keys")];

/// [`KEY_BAR_RIGHT`] with its words in the language `tr`.
#[must_use]
pub fn key_bar_right(tr: &Tr) -> [(&'static str, String); 2] {
    [
        ("Ctrl+K", tr.t("chrome.anything")),
        ("?", tr.t("chrome.all_keys")),
    ]
}

/// Gap between key bar entries, in points (`.keys{gap:14px}`).
const ENTRY_GAP: f32 = 14.0;
/// Gap between a keycap and its words (`.keys span{gap:5px}`).
const KEY_GAP: f32 = 5.0;

/// Draws the key bar: the screen's name in bold (`.keys .ctx`), its entries from [`key_hints`]
/// (a keycap, then its words), and [`KEY_BAR_RIGHT`] at the right in `text-3`.
pub(super) fn key_bar(
    ui: &mut Ui,
    context: &str,
    hints: &[(String, String)],
    held: Option<Held>,
    t: &Tokens,
    tr: &Tr,
) {
    ui.spacing_mut().item_spacing.x = KEY_GAP;
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = KEY_GAP;
        if let Some(held) = held {
            ui.label(
                RichText::new(format!("Holding {}", held.name()))
                    .font(bold_font(ui, TEXT_META))
                    .color(t.accent_ink),
            );
            ui.add_space(ENTRY_GAP - KEY_GAP);
        } else if !context.is_empty() {
            ui.label(
                RichText::new(context)
                    .font(bold_font(ui, TEXT_META))
                    .color(t.text),
            );
            ui.add_space(ENTRY_GAP - KEY_GAP);
        }
        for (keys, label) in hints {
            // The right end needs room; entries that do not fit are left out. The entry is
            // measured in a hidden copy first, since its own width counts against the room.
            let entry = |ui: &mut Ui| {
                keycap::keycap(ui, keys);
                ui.label(
                    RichText::new(label.as_str())
                        .size(TEXT_META)
                        .color(t.text_2),
                );
                ui.add_space(ENTRY_GAP - KEY_GAP);
            };
            let mut probe = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(ui.available_rect_before_wrap())
                    .layout(egui::Layout::left_to_right(egui::Align::Center))
                    .invisible(),
            );
            probe.spacing_mut().item_spacing.x = KEY_GAP;
            entry(&mut probe);
            if ui.available_width() - probe.min_rect().width() < RIGHT_END_WIDTH {
                break;
            }
            entry(ui);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = KEY_GAP;
            for (keys, label) in key_bar_right(tr).iter().rev() {
                ui.label(
                    RichText::new(label.as_str())
                        .size(TEXT_META)
                        .color(t.text_3),
                );
                keycap::keycap(ui, keys);
            }
            if held.is_none() {
                ui.label(
                    RichText::new(crate::i18n::lit(KEY_BAR_HOLD_HINT))
                        .size(TEXT_META)
                        .color(t.text_3),
                );
            }
        });
    });
}

/// Room kept free at the right of the key bar for [`KEY_BAR_RIGHT`] and the hold hint, in points.
const RIGHT_END_WIDTH: f32 = 330.0;

#[cfg(test)]
mod tab_strip_order {
    //! The tab strip's buttons read "Close tab" then "New tab" from left to right.
    //!
    //! provenance: design/mock-v2/index.html (the tab strip, `Close tab Ctrl+W` before
    //! `New tab Ctrl+T` inside the right-hand span).
    #![allow(clippy::expect_used, reason = "test code")]

    use super::*;
    use crate::AppCtx;
    use crate::ScreenAction;
    use crate::screen::Screen;

    struct Leaf;
    impl Screen for Leaf {
        fn title(&self, _tr: &crate::i18n::Tr) -> String {
            "Leaf".to_owned()
        }
        fn ui(&mut self, _ui: &mut Ui, _ctx: &mut AppCtx) -> ScreenAction {
            ScreenAction::Stay
        }
    }

    fn x_of(shapes: &[egui::epaint::ClippedShape], label: &str) -> Option<f32> {
        fn walk(shape: &egui::Shape, label: &str, out: &mut Option<f32>) {
            match shape {
                egui::Shape::Text(t) if t.galley.text() == label => *out = Some(t.pos.x),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, label, out)),
                egui::Shape::Text(_)
                | egui::Shape::Noop
                | egui::Shape::Circle(_)
                | egui::Shape::Ellipse(_)
                | egui::Shape::LineSegment { .. }
                | egui::Shape::Path(_)
                | egui::Shape::Band(_)
                | egui::Shape::Rect(_)
                | egui::Shape::Mesh(_)
                | egui::Shape::QuadraticBezier(_)
                | egui::Shape::CubicBezier(_)
                | egui::Shape::Callback(_) => {}
            }
        }
        let mut found = None;
        for clipped in shapes {
            walk(&clipped.shape, label, &mut found);
        }
        found
    }

    #[test]
    fn close_tab_is_left_of_new_tab() {
        let mut tabs = Tabs::new(Box::new(Leaf));
        tabs.open(Box::new(Leaf));
        let ctx = egui::Context::default();
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let t = crate::ui::tokens(ui);
            let _cmds = tab_strip(ui, &tabs, &t, &Tr::default());
        });
        let close = x_of(&out.shapes, "Close tab");
        let new = x_of(&out.shapes, "New tab");
        out.drop_without_applying_deltas();
        let (close, new) = (
            close.expect("Close tab is drawn"),
            new.expect("New tab is drawn"),
        );
        assert!(close < new, "Close tab at {close}, New tab at {new}");
    }
}

#[cfg(test)]
mod key_bar_fit {
    //! The key bar drops entries that do not fit instead of drawing over its right end.
    //!
    //! provenance: derived from the mock's key bar (`design/mock-v2/index.html`, `.keys`): its
    //! entries and the right end ("Hold Alt or Ctrl for more", Ctrl+K, ?) never overlap. A cold
    //! start of the release binary at 1280 points wide showed "Enter Open" drawn over "Hold Alt or
    //! Ctrl for more" on Home (2026-11-26); the check is that no two texts on the bar overlap.
    #![allow(clippy::expect_used, reason = "test code")]

    use super::*;

    fn text_rects(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, egui::Rect)> {
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match shape {
                egui::Shape::Text(t) => {
                    out.push((
                        t.galley.text().to_owned(),
                        t.galley.rect.translate(t.pos.to_vec2()),
                    ));
                }
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                egui::Shape::Noop
                | egui::Shape::Circle(_)
                | egui::Shape::Ellipse(_)
                | egui::Shape::LineSegment { .. }
                | egui::Shape::Path(_)
                | egui::Shape::Band(_)
                | egui::Shape::Rect(_)
                | egui::Shape::Mesh(_)
                | egui::Shape::QuadraticBezier(_)
                | egui::Shape::CubicBezier(_)
                | egui::Shape::Callback(_) => {}
            }
        }
        let mut out = Vec::new();
        for clipped in shapes {
            walk(&clipped.shape, &mut out);
        }
        out
    }

    #[test]
    fn no_two_texts_of_the_bar_overlap_at_1280_points() {
        let hints: Vec<(String, String)> = [
            ("F8", "Sales"),
            ("F5", "Payment"),
            ("F6", "Receipt"),
            ("B", "Balance sheet"),
            ("D", "Day book"),
            ("G", "GST returns"),
            ("M", "Masters"),
            ("Ctrl+K", "Palette"),
            ("J / K", "Review list"),
            ("Enter", "Open"),
        ]
        .iter()
        .map(|(k, l)| ((*k).to_owned(), (*l).to_owned()))
        .collect();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx, crate::theme::ThemeKind::Light);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 32.0),
            )),
            ..egui::RawInput::default()
        };
        let mut texts = Vec::new();
        for _ in 0..2 {
            let out = ctx.run_ui(input.clone(), |ui| {
                let t = crate::ui::tokens(ui);
                key_bar(ui, "Home", &hints, None, &t, &Tr::default());
            });
            texts = text_rects(&out.shapes);
            out.drop_without_applying_deltas();
        }
        assert!(texts.len() > 4, "{texts:?}");
        for (i, (a, ra)) in texts.iter().enumerate() {
            for (b, rb) in texts.iter().skip(i + 1) {
                assert!(!ra.intersects(*rb), "{a:?} {ra:?} overlaps {b:?} {rb:?}");
            }
        }
    }
}
