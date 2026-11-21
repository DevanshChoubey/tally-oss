//! Home dashboard; KPI tiles read report outputs only. **Card U3** implements this module.
//!
//! Frozen interface (plan card U3):
//!
//! ```text
//! pub struct <Name>Screen { .. }
//! impl Screen for <Name>Screen { .. }
//! ```
//!
//! Here `<Name>` is `Home`: [`HomeScreen`] implements [`Screen`].
//!
//! What the screen shows (spec 7 and the mock's Home screen): the company and date line, a row of
//! KPI tiles, an "Enter" list, a "Look up" list and a "Needs attention" list.
//!
//! * **Rule C4, the UI never sums anything.** Every figure on a tile is an [`Outcome<Paise>`]
//!   handed in by the host in [`HomeData`], taken from a report output (balance sheet, GST
//!   return) and never computed here. The only transformation is formatting: [`format_inr`]
//!   gives the Indian grouping `12,34,567.00`. A tile whose report is unsupported shows the words
//!   "Not supported yet" and the reason, never a number (rule A1).
//! * **The screen opens nothing.** It cannot build the other screens (other cards own them), so a
//!   click or a key records a [`HomeTarget`] that the host takes with
//!   [`HomeScreen::take_request`] and turns into a [`crate::ScreenAction::Push`]. This is the
//!   same pattern the voucher screen uses for creating a ledger inline.
//! * **No colour-only signals** (spec 7): every attention item starts with a word
//!   ([`AttentionKind::word`]) as well as a colour.
//!
//! Keys handled here, from the mock: F8 sales invoice, F6 receipt, F5 payment, F9 purchase from a
//! bill, B balance sheet, D day book, G GST returns, M masters, Ctrl+K search. The letters are
//! ignored while a text field has focus. Ctrl+J opens the describe-a-transaction row. Modifiers
//! match exactly: Alt+D or Shift+F5 is not D or F5. The key map of card U2 is not wired in yet
//! (Wave 2), so the keys are read directly, as the shell does for Ctrl+T.
//!
//! Not built here: the mock's "engine" badge tooltip text and the narrow-width reflow to one
//! column; the tiles wrap instead.

use egui::{Color32, Event, InputState, Key, Modifiers, RichText, Sense, Ui};
use ledgerly_core::format::format_inr;
use ledgerly_core::{Outcome, Paise};

use crate::{AppCtx, Screen, ScreenAction};

/// Width of one KPI tile, in points.
const TILE_WIDTH: f32 = 230.0;
/// Width of one list box, in points.
const BOX_WIDTH: f32 = 340.0;
/// Text shown in place of a figure the engine cannot compute yet.
const UNSUPPORTED_WORDS: &str = "Not supported yet";

/// Where a click or a key on the home screen asks to go. The host maps each to a screen.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HomeTarget {
    /// New sales invoice (F8).
    SalesInvoice,
    /// New receipt voucher (F6).
    Receipt,
    /// New payment voucher (F5).
    Payment,
    /// Purchase entered from a photo of a bill (F9).
    PurchaseFromBill,
    /// Describe a transaction to the assistant.
    DescribeTransaction,
    /// Balance sheet, opened at the line named here (empty: the top).
    BalanceSheet {
        /// Name of the line to open, as the report names it; empty for the whole report.
        node: String,
    },
    /// Day book (D).
    DayBook,
    /// GST returns (G), opened on the return named here (empty: the list).
    GstReturns {
        /// Return or tab to open, as the GST screen names it; empty for the list.
        tab: String,
    },
    /// Ledgers and items (M).
    Masters,
    /// The command palette (Ctrl+K).
    Palette,
    /// Pending invoices in the invoice management system.
    PendingIms,
    /// Bank statement lines that are not matched yet.
    UnmatchedBankLines,
}

/// One KPI tile. Every field comes from a report output; the screen computes none of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kpi {
    /// Tile title, for example "Cash and bank".
    pub label: String,
    /// The figure from the report, or why the report cannot give it.
    pub value: Outcome<Paise>,
    /// Line under the figure, for example "2 bank accounts, 1 cash".
    pub footnote: String,
    /// What a click on the tile opens.
    pub target: HomeTarget,
}

/// How urgent a "Needs attention" item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttentionKind {
    /// Waiting for the user (amber dot in the mock).
    Pending,
    /// Has a due date coming (red dot in the mock).
    Due,
    /// Finished, shown for information (green dot in the mock).
    Done,
}

impl AttentionKind {
    /// The word printed before the item, so the colour is never the only signal.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            AttentionKind::Pending => "Pending",
            AttentionKind::Due => "Due",
            AttentionKind::Done => "Done",
        }
    }
}

/// One line of "Needs attention".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttentionItem {
    /// How urgent it is.
    pub kind: AttentionKind,
    /// Main line, for example "4 invoices in IMS are pending".
    pub headline: String,
    /// Second line in muted text; empty for none.
    pub detail: String,
    /// What a click opens; `None` makes the line plain text.
    pub target: Option<HomeTarget>,
}

/// Everything the home screen shows, supplied by the host from report outputs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HomeData {
    /// Company name for the heading.
    pub company_name: String,
    /// Line under the heading, for example the date and how far the books are open.
    pub subtitle: String,
    /// KPI tiles, in display order.
    pub kpis: Vec<Kpi>,
    /// "Needs attention" items, in display order.
    pub attention: Vec<AttentionItem>,
}

/// What one KPI tile displays, as plain text. The screen draws exactly these strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KpiView {
    /// Tile title.
    pub label: String,
    /// The figure with its rupee sign, or the unsupported words.
    pub value_text: String,
    /// The footnote, or for an unsupported tile the reason.
    pub footnote_text: String,
}

/// The text of a tile's figure: `₹12,34,567.00` for a value, "Not supported yet" otherwise.
#[must_use]
pub fn value_text(value: &Outcome<Paise>) -> String {
    match value {
        Outcome::Value(amount) => format!("\u{20B9}{}", format_inr(*amount)),
        Outcome::Unsupported(_) => UNSUPPORTED_WORDS.to_owned(),
    }
}

/// The view of one tile. An unsupported tile shows the reason instead of its footnote, so the
/// user learns why the number is missing.
#[must_use]
pub fn kpi_view(kpi: &Kpi) -> KpiView {
    let footnote_text = match &kpi.value {
        Outcome::Value(_) => kpi.footnote.clone(),
        Outcome::Unsupported(u) => format!("{}: {}", u.what, u.reason),
    };
    KpiView {
        label: kpi.label.clone(),
        value_text: value_text(&kpi.value),
        footnote_text,
    }
}

/// One row of an "Enter" or "Look up" list.
#[derive(Debug, Clone, Copy)]
struct EntryRow {
    target: fn() -> HomeTarget,
    title: &'static str,
    detail: &'static str,
    key: &'static str,
}

const ENTER_ROWS: [EntryRow; 5] = [
    EntryRow {
        target: || HomeTarget::SalesInvoice,
        title: "Sales invoice",
        detail: "Item invoice with GST, e-invoice ready",
        key: "F8",
    },
    EntryRow {
        target: || HomeTarget::Receipt,
        title: "Receipt",
        detail: "Money in, against bills",
        key: "F6",
    },
    EntryRow {
        target: || HomeTarget::Payment,
        title: "Payment",
        detail: "Money out, cash or bank",
        key: "F5",
    },
    EntryRow {
        target: || HomeTarget::PurchaseFromBill,
        title: "Purchase from a photo of a bill",
        detail: "Assistant reads it, you confirm",
        key: "F9",
    },
    EntryRow {
        target: || HomeTarget::DescribeTransaction,
        title: "Describe a transaction",
        detail: "Type it in words; the assistant drafts the voucher",
        key: "Ctrl J",
    },
];

const LOOKUP_ROWS: [EntryRow; 5] = [
    EntryRow {
        target: || HomeTarget::BalanceSheet {
            node: String::new(),
        },
        title: "Balance sheet",
        detail: "Drill from any figure to the voucher",
        key: "B",
    },
    EntryRow {
        target: || HomeTarget::DayBook,
        title: "Day book",
        detail: "Every voucher, by date",
        key: "D",
    },
    EntryRow {
        target: || HomeTarget::GstReturns { tab: String::new() },
        title: "GST returns",
        detail: "GSTR-1, GSTR-3B, 2B and IMS",
        key: "G",
    },
    EntryRow {
        target: || HomeTarget::Masters,
        title: "Ledgers and items",
        detail: "Parties, stock items, HSN and rates",
        key: "M",
    },
    EntryRow {
        target: || HomeTarget::Palette,
        title: "Anything else",
        detail: "Search every ledger, voucher and report",
        key: "Ctrl K",
    },
];

/// Keys that work from the home screen, as the mock lists them. The first flag says whether the
/// key is a plain letter (ignored while a text field has focus).
/// One home-screen shortcut: key, modifiers, whether it is a plain letter, and its target.
type Shortcut = (Key, Modifiers, bool, fn() -> HomeTarget);

const SHORTCUTS: [Shortcut; 10] = [
    (Key::F8, Modifiers::NONE, false, || HomeTarget::SalesInvoice),
    (Key::F6, Modifiers::NONE, false, || HomeTarget::Receipt),
    (Key::F5, Modifiers::NONE, false, || HomeTarget::Payment),
    (Key::F9, Modifiers::NONE, false, || {
        HomeTarget::PurchaseFromBill
    }),
    (Key::B, Modifiers::NONE, true, || HomeTarget::BalanceSheet {
        node: String::new(),
    }),
    (Key::D, Modifiers::NONE, true, || HomeTarget::DayBook),
    (Key::G, Modifiers::NONE, true, || HomeTarget::GstReturns {
        tab: String::new(),
    }),
    (Key::M, Modifiers::NONE, true, || HomeTarget::Masters),
    (Key::K, Modifiers::COMMAND, false, || HomeTarget::Palette),
    (Key::J, Modifiers::COMMAND, false, || {
        HomeTarget::DescribeTransaction
    }),
];

/// The target a key opens from the home screen, or `None` for a key it does not handle.
#[must_use]
pub fn shortcut_target(key: Key, modifiers: Modifiers) -> Option<HomeTarget> {
    SHORTCUTS
        .iter()
        .find(|(k, m, _, _)| *k == key && *m == modifiers)
        .map(|(_, _, _, target)| target())
}

/// Consumes the press of exactly `key` with exactly `modifiers` held, and reports whether there was
/// one. egui's `consume_key` also accepts extra Alt or Shift, so it would let Alt+D open the day
/// book; this matches the modifiers the same way [`shortcut_target`] does.
fn consume_exact(input: &mut InputState, key: Key, modifiers: Modifiers) -> bool {
    let before = input.events.len();
    input.events.retain(|event| {
        !matches!(
            event,
            Event::Key { key: pressed_key, modifiers: held, pressed: true, .. }
                if *pressed_key == key && held.matches_exact(modifiers)
        )
    });
    input.events.len() != before
}

/// The home screen.
#[derive(Debug, Default)]
pub struct HomeScreen {
    data: HomeData,
    request: Option<HomeTarget>,
}

impl HomeScreen {
    /// A home screen showing `data`.
    #[must_use]
    pub fn new(data: HomeData) -> Self {
        Self {
            data,
            request: None,
        }
    }

    /// What the screen shows now.
    #[must_use]
    pub fn data(&self) -> &HomeData {
        &self.data
    }

    /// Replaces what the screen shows, for example after a voucher is posted and the reports are
    /// run again. A request that was not taken yet is kept.
    pub fn set_data(&mut self, data: HomeData) {
        self.data = data;
    }

    /// The text each KPI tile displays, in order.
    #[must_use]
    pub fn kpi_views(&self) -> Vec<KpiView> {
        self.data.kpis.iter().map(kpi_view).collect()
    }

    /// Records that the user asked to open `target`, as a click does. The latest request wins.
    pub fn request_open(&mut self, target: HomeTarget) {
        self.request = Some(target);
    }

    /// Takes the pending request, leaving none. The host opens the screen it names.
    pub fn take_request(&mut self) -> Option<HomeTarget> {
        self.request.take()
    }

    /// Reads the home keys for this frame.
    fn handle_keys(&mut self, ui: &Ui) {
        let typing = ui.ctx().egui_wants_keyboard_input();
        let hit = ui.ctx().input_mut(|i| {
            SHORTCUTS
                .iter()
                .filter(|(_, _, plain_letter, _)| !(*plain_letter && typing))
                .find(|(key, modifiers, _, _)| consume_exact(i, *key, *modifiers))
                .map(|(_, _, _, target)| target())
        });
        if let Some(target) = hit {
            self.request_open(target);
        }
    }

    fn draw_kpis(&mut self, ui: &mut Ui) {
        if self.data.kpis.is_empty() {
            ui.label(RichText::new("No figures yet. Open a company to see them.").weak());
            return;
        }
        let mut clicked = None;
        ui.horizontal_wrapped(|ui| {
            for (index, kpi) in self.data.kpis.iter().enumerate() {
                let view = kpi_view(kpi);
                let response = draw_tile(ui, index, &view);
                if response.clicked() {
                    clicked = Some(kpi.target.clone());
                }
            }
        });
        if let Some(target) = clicked {
            self.request_open(target);
        }
    }

    fn draw_list(ui: &mut Ui, heading: &str, rows: &[EntryRow]) -> Option<HomeTarget> {
        let mut clicked = None;
        frame_box(ui, |ui| {
            ui.set_width(BOX_WIDTH);
            ui.heading(heading);
            for row in rows {
                ui.horizontal(|ui| {
                    let response = ui.add(egui::Button::new(row.title).frame(false));
                    if response.clicked() {
                        clicked = Some((row.target)());
                    }
                    ui.label(RichText::new(row.detail).weak().small());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(row.key).monospace().weak());
                    });
                });
            }
        });
        clicked
    }

    fn draw_attention(&mut self, ui: &mut Ui) {
        let mut clicked = None;
        frame_box(ui, |ui| {
            ui.set_width(BOX_WIDTH);
            ui.heading("Needs attention");
            if self.data.attention.is_empty() {
                ui.label(RichText::new("Nothing needs attention.").weak());
            }
            for item in &self.data.attention {
                ui.separator();
                let color = attention_color(ui, item.kind);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(item.kind.word()).color(color).strong());
                    match &item.target {
                        Some(target) => {
                            if ui
                                .add(egui::Button::new(item.headline.as_str()).frame(false))
                                .clicked()
                            {
                                clicked = Some(target.clone());
                            }
                        }
                        None => {
                            ui.label(RichText::new(item.headline.as_str()).strong());
                        }
                    }
                });
                if !item.detail.is_empty() {
                    ui.label(RichText::new(item.detail.as_str()).weak().small());
                }
            }
        });
        if let Some(target) = clicked {
            self.request_open(target);
        }
    }
}

/// Colour of the word before an attention item. Taken from the active visuals so the three
/// themes stay consistent; the word carries the meaning.
fn attention_color(ui: &Ui, kind: AttentionKind) -> Color32 {
    match kind {
        AttentionKind::Pending => ui.visuals().warn_fg_color,
        AttentionKind::Due => ui.visuals().error_fg_color,
        AttentionKind::Done => ui.visuals().text_color(),
    }
}

/// A bordered panel in the surface colour of the active theme.
fn frame_box<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    let visuals = ui.visuals().widgets.noninteractive;
    egui::Frame::new()
        .fill(visuals.bg_fill)
        .stroke(visuals.bg_stroke)
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::same(12))
        .show(ui, add_contents)
        .inner
}

/// Draws one KPI tile and returns its click response.
fn draw_tile(ui: &mut Ui, index: usize, view: &KpiView) -> egui::Response {
    let inner = frame_box_response(ui, |ui| {
        ui.set_width(TILE_WIDTH);
        ui.label(RichText::new(view.label.as_str()).weak());
        ui.label(RichText::new(view.value_text.as_str()).size(22.0).strong());
        ui.label(RichText::new(view.footnote_text.as_str()).weak().small());
    });
    let id = ui.scope_id().with(("home-kpi", index));
    let response = ui.interact(inner.rect, id, Sense::click());
    let spoken = format!(
        "{}, {}. {}",
        view.label, view.value_text, view.footnote_text
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, spoken.as_str()));
    response
}

/// Like [`frame_box`] but returns the frame's response, to make the whole tile clickable.
fn frame_box_response(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) -> egui::Response {
    let visuals = ui.visuals().widgets.noninteractive;
    egui::Frame::new()
        .fill(visuals.bg_fill)
        .stroke(visuals.bg_stroke)
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::same(12))
        .show(ui, add_contents)
        .response
}

impl Screen for HomeScreen {
    fn title(&self) -> String {
        "Home".to_owned()
    }

    fn ui(&mut self, ui: &mut Ui, _ctx: &mut AppCtx) -> ScreenAction {
        self.handle_keys(ui);
        egui::ScrollArea::vertical().show(ui, |ui| {
            let heading = if self.data.company_name.is_empty() {
                "Ledgerly"
            } else {
                self.data.company_name.as_str()
            };
            ui.heading(heading);
            if !self.data.subtitle.is_empty() {
                ui.label(RichText::new(self.data.subtitle.as_str()).weak());
            }
            ui.add_space(8.0);
            self.draw_kpis(ui);
            ui.add_space(12.0);
            let mut chosen = None;
            ui.horizontal_wrapped(|ui| {
                chosen = chosen.take().or(Self::draw_list(ui, "Enter", &ENTER_ROWS));
                chosen = chosen
                    .take()
                    .or(Self::draw_list(ui, "Look up", &LOOKUP_ROWS));
            });
            if let Some(target) = chosen {
                self.request_open(target);
            }
            ui.horizontal_wrapped(|ui| self.draw_attention(ui));
        });
        ScreenAction::Stay
    }
}
