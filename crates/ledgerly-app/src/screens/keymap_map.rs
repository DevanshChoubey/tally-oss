//! Keyboard map (mock `keys.classic` and `keys.standard`): every action with its keys, grouped as
//! the mock groups them.
//!
//! The keys in use are [`keymap::DEFAULT_PRESET`]. The shell owns its keymap and has no hook to
//! replace it, so this screen cannot switch the keys. Its tabs choose which preset the map shows,
//! for reference only, and the screen always says which preset is in use. Keys the mock lists
//! that no [`Action`] exists for (Alt+R, Ctrl+H, Ctrl+J, Alt+O and the list and proposal keys)
//! are not shown: nothing in the app can say what they do.

use egui::{Key, Modifiers, RichText, Ui};

use super::wire;
use crate::keymap::{self, Action, Keymap, PRESET_NAMES};
use crate::screen::KeyHint;
use crate::theme::{SPACE_2, SPACE_3, TEXT_BODY, TEXT_META, Tokens};
use crate::ui::card::card;
use crate::ui::keycap::keycap;
use crate::ui::page_head::page_head;
use crate::ui::tabs::tabs;
use crate::ui::{bold_font, tokens};
use crate::{AppCtx, Screen, ScreenAction};

/// Salt of the id that stores which preset the map shows, as an index into [`PRESET_NAMES`].
const VIEW_SALT: &str = "ledgerly-keymap-view";

/// The tab labels, in the order of [`PRESET_NAMES`].
const TAB_LABELS: [&str; 2] = ["Classic accountant keys", "Standard desktop keys"];

/// The mock's groups, in its order (`design/mock-v2/index.html`, the `KEYMAP` table, lines
/// 1835-1841). Each row is an action and the words the mock gives it.
const MOCK_GROUPS: [(&str, &[(Action, &str)]); 4] = [
    (
        "Go anywhere",
        &[
            (
                Action::OpenPalette,
                "Command palette: jump, search, ask, type an entry",
            ),
            (Action::SwitchPalette, "Switch to, replacing this screen"),
            (Action::Back, "Back one level, never loses a saved voucher"),
            (Action::Help, "This map"),
            (Action::SwitchCompany, "Switch company"),
        ],
    ),
    (
        "Vouchers",
        &[
            (Action::NewSales, "Sales"),
            (Action::NewPurchase, "Purchase"),
            (Action::NewPayment, "Payment"),
            (Action::NewReceipt, "Receipt"),
            (Action::NewContra, "Contra"),
            (Action::NewJournal, "Journal"),
        ],
    ),
    (
        "Inside a voucher",
        &[
            (Action::Save, "Save"),
            (Action::CreateMaster, "Create a master from the field"),
            (Action::AlterMaster, "Alter the master in the field"),
            (Action::DeleteLine, "Delete line"),
            (Action::ChangePeriod, "Change date or period"),
        ],
    ),
    (
        "Reports",
        &[
            (Action::ToggleDetail, "Detailed or condensed"),
            (Action::Print, "Print"),
            (Action::Export, "Export"),
            (Action::Email, "Email"),
        ],
    ),
];

/// The note under the tabs for the classic preset, as the mock words it.
const CLASSIC_NOTE: &str = "Classic keeps the function keys accountants already use: F8 sales, F5 payment, Ctrl+A save, Enter to move on.";

/// The note under the tabs for the standard preset, as the mock words it.
const STANDARD_NOTE: &str =
    "Standard uses common desktop shortcuts such as Ctrl+S. Function keys still work as aliases.";

/// One row of the map: an action and the words the map shows for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyRow {
    /// The action the row shows.
    pub action: Action,
    /// The words for the action on this map.
    pub label: &'static str,
}

/// One card of the map: a title and its rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyGroup {
    /// The card's heading.
    pub title: &'static str,
    /// The rows, in order.
    pub rows: Vec<KeyRow>,
}

/// The map's layout: the mock's groups, then an "Other actions" group for every remaining
/// [`Action`], so each action appears exactly once.
#[must_use]
pub fn layout() -> Vec<KeyGroup> {
    let mut groups: Vec<KeyGroup> = MOCK_GROUPS
        .iter()
        .map(|&(title, rows)| KeyGroup {
            title,
            rows: rows
                .iter()
                .map(|&(action, label)| KeyRow { action, label })
                .collect(),
        })
        .collect();
    let placed: Vec<Action> = groups
        .iter()
        .flat_map(|group| group.rows.iter().map(|row| row.action))
        .collect();
    let rest: Vec<KeyRow> = Action::ALL
        .iter()
        .copied()
        .filter(|action| !placed.contains(action))
        .map(|action| KeyRow {
            action,
            label: action.label(),
        })
        .collect();
    if !rest.is_empty() {
        groups.push(KeyGroup {
            title: "Other actions",
            rows: rest,
        });
    }
    groups
}

/// The tab label of a preset name, or a plain fallback for a name that is not a preset.
fn tab_label(name: &str) -> &'static str {
    PRESET_NAMES
        .iter()
        .position(|preset| *preset == name)
        .and_then(|i| TAB_LABELS.get(i).copied())
        .unwrap_or("this preset")
}

/// Draws the keys of one action: a keycap per chord, or "No key yet" when it has none.
fn chord_keys(ui: &mut Ui, keymap: &Keymap, action: Action) {
    let chords = keymap.chords(action);
    if chords.is_empty() {
        let t = tokens(ui);
        ui.label(RichText::new("No key yet").size(TEXT_META).color(t.text_3));
        return;
    }
    for chord in chords {
        keycap(ui, &chord.to_string());
    }
}

/// Width of the keys column inside a card, in points: room for two keycaps such as
/// `Ctrl+Shift+S` and `F8` side by side. Wider chords wrap onto a second line.
const KEYS_WIDTH: f32 = 120.0;

/// Draws one card: its heading, then a row per action with its keys and words. The card fills its
/// column and the words wrap, so a long label never widens the card past the column.
fn group_ui(ui: &mut Ui, group: &KeyGroup, keymap: &Keymap, t: &Tokens) {
    ui.set_width(ui.available_width());
    let heading = bold_font(ui, TEXT_BODY);
    ui.label(RichText::new(group.title).font(heading).color(t.text));
    ui.add_space(SPACE_2);
    for row in &group.rows {
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(KEYS_WIDTH, 0.0),
                egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true),
                |ui| {
                    ui.set_width(KEYS_WIDTH);
                    chord_keys(ui, keymap, row.action);
                },
            );
            ui.add_space(SPACE_3);
            crate::ui::para(ui, row.label, TEXT_BODY, t.text);
        });
        ui.add_space(SPACE_2);
    }
}

/// The Keyboard map screen (mock `keys`). It shows the keys in use, and a preset for reference.
#[derive(Debug, Default)]
pub struct KeymapScreen {
    /// The card rectangles of the last frame, in drawing order.
    cards: Vec<egui::Rect>,
}

impl KeymapScreen {
    /// The card rectangles of the last frame, in the order of [`layout`]. Empty before the first
    /// frame.
    #[must_use]
    pub fn card_rects(&self) -> &[egui::Rect] {
        &self.cards
    }
}

impl Screen for KeymapScreen {
    fn title(&self) -> String {
        "Keyboard map".to_owned()
    }

    fn subtitle(&self) -> Option<String> {
        Some(
            "Every action has a key. The bar at the bottom always shows the keys for this screen."
                .to_owned(),
        )
    }

    fn keys(&self) -> Vec<KeyHint> {
        vec![KeyHint::new("1", "Classic"), KeyHint::new("2", "Standard")]
    }

    fn ui(&mut self, ui: &mut Ui, _ctx: &mut AppCtx) -> ScreenAction {
        let id = ui.scope_id().with(VIEW_SALT);
        let t = tokens(ui);
        let mut view: usize = ui.ctx().data(|d| d.get_temp::<usize>(id)).unwrap_or(0);

        let pressed: Option<usize> = ui.input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::Num1) {
                Some(0)
            } else if i.consume_key(Modifiers::NONE, Key::Num2) {
                Some(1)
            } else {
                None
            }
        });
        if let Some(index) = pressed {
            view = index;
        }

        let title = self.title();
        let subtitle = self.subtitle();
        page_head(ui, &title, subtitle.as_deref(), |_ui| {});

        if let Some(clicked) = tabs(ui, &TAB_LABELS, view).clicked {
            view = clicked;
        }
        ui.ctx().data_mut(|d| d.insert_temp(id, view));

        let shown_name = PRESET_NAMES
            .get(view)
            .copied()
            .unwrap_or(keymap::DEFAULT_PRESET);
        let shown = match keymap::preset(shown_name) {
            Ok(keymap) => keymap,
            Err(error) => {
                ui.label(wire::unsupported_message(
                    "the keyboard map",
                    &error.to_string(),
                ));
                return ScreenAction::Stay;
            }
        };
        let note = if shown_name == keymap::DEFAULT_PRESET {
            CLASSIC_NOTE
        } else {
            STANDARD_NOTE
        };
        ui.add_space(SPACE_2);
        crate::ui::para(ui, note, TEXT_BODY, t.text_2);
        ui.label(
            RichText::new(format!(
                "Keys in use now: {}. Changing the preset is not available yet; this map shows {} for reference.",
                tab_label(keymap::DEFAULT_PRESET),
                tab_label(shown_name)
            ))
            .size(TEXT_META)
            .color(t.text_2),
        );
        ui.add_space(SPACE_3);

        let groups = layout();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut rects = vec![egui::Rect::NOTHING; groups.len()];
                ui.columns(3, |cols| {
                    for (i, group) in groups.iter().enumerate() {
                        if let Some(col) = cols.get_mut(i % 3) {
                            let drawn = card(col, |ui| group_ui(ui, group, &shown, &t));
                            if let Some(slot) = rects.get_mut(i) {
                                *slot = drawn.response.rect;
                            }
                            col.add_space(SPACE_3);
                        }
                    }
                });
                self.cards = rects;
            });
        ScreenAction::Stay
    }
}
