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
//! ignored while a text field has focus. Ctrl+Shift+J opens the describe-a-transaction row (the
//! table of keys says why it is not Ctrl+J). Modifiers
//! match exactly: Alt+D or Shift+F5 is not D or F5. The key map of card U2 is not wired in yet
//! (Wave 2), so the keys are read directly, as the shell does for Ctrl+T.
//!
//! Not built here: the mock's "engine" badge tooltip text and the narrow-width reflow to one
//! column; the tiles wrap instead.

use egui::{Color32, Event, InputState, Key, Modifiers, RichText, Sense, Ui};
use ledgerly_api::{
    Api, Cell, ColumnSpec, Detail, Report, ReportFilter, ReportKind, ReportParams, RowKind,
};
use ledgerly_core::format::format_inr;
use ledgerly_core::{Date, DrCr, Outcome, Paise, Period, SignedAmount, Unsupported};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{ReservedGroup, Scenario};

use crate::screens::wire::{Done, Latest};
use crate::worker::ApiWorker;
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

/// One balance tile: which reserved group it reads and on which side that group's balance
/// normally stands.
struct Tile {
    label: &'static str,
    group: ReservedGroup,
    normal: DrCr,
    node: &'static str,
}

/// The tiles. Tally's dashboard (help page read 2026-11-21,
/// <https://help.tallysolutions.com/tally-prime/reports-in-tallyprime/dashboard/>) has tiles for
/// Cash/Bank Accounts and Receivables/Payables among its nine default tiles, so the existing
/// cash, bank, receivables and payables set is kept. The page does not define the figures, so
/// each tile here is the closing balance of one reserved group, taken from the group summary.
const TILES: [Tile; 4] = [
    Tile {
        label: "Cash in hand",
        group: ReservedGroup::CashInHand,
        normal: DrCr::Dr,
        node: "Cash-in-hand",
    },
    Tile {
        label: "Bank accounts",
        group: ReservedGroup::BankAccounts,
        normal: DrCr::Dr,
        node: "Bank accounts",
    },
    Tile {
        label: "Receivables",
        group: ReservedGroup::SundryDebtors,
        normal: DrCr::Dr,
        node: "Sundry debtors",
    },
    Tile {
        label: "Payables",
        group: ReservedGroup::SundryCreditors,
        normal: DrCr::Cr,
        node: "Sundry creditors",
    },
];

/// The home KPI tiles as of `as_of`, from `Api::report` (group summaries). Runs on the API
/// worker. A tile whose report fails or is unsupported carries `Unsupported` with the reason;
/// no tile ever carries a guessed figure.
#[must_use]
pub fn home_kpis(api: &Api, as_of: Date) -> Vec<Kpi> {
    let mut kpis: Vec<Kpi> = TILES
        .iter()
        .map(|tile| tile_kpi(api, tile, as_of))
        .collect();
    kpis.push(Kpi {
        label: "GST payable".to_owned(),
        value: Outcome::unsupported(
            "GST payable",
            "the engine gives the GSTR-3B working (tax and credit separately) but no payable \
             after set-off, and this screen does not subtract",
        ),
        footnote: "after ITC".to_owned(),
        target: HomeTarget::GstReturns {
            tab: "gstr3b".to_owned(),
        },
    });
    kpis
}

fn tile_kpi(api: &Api, tile: &Tile, as_of: Date) -> Kpi {
    let value = tile_value(api, tile, as_of);
    Kpi {
        label: tile.label.to_owned(),
        value,
        footnote: format!("Closing balance as of {as_of}"),
        target: HomeTarget::BalanceSheet {
            node: tile.node.to_owned(),
        },
    }
}

fn tile_value(api: &Api, tile: &Tile, as_of: Date) -> Outcome<Paise> {
    let what = tile.label;
    let Some(group) = reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(tile.group))
    else {
        return Outcome::unsupported(what, "the reserved group is not in the seed list");
    };
    let period = match Period::new(as_of, as_of) {
        Ok(period) => period,
        Err(e) => return Outcome::unsupported(what, format!("the date is not usable: {e}")),
    };
    let params = ReportParams {
        kind: ReportKind::GroupSummary(group.id),
        period,
        as_of,
        scenario: Scenario::Actual,
        detail: Detail::Condensed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    };
    match api.report(&params) {
        Ok(Outcome::Value(report)) => net_balance(&report, tile.normal, what),
        Ok(Outcome::Unsupported(u)) => Outcome::Unsupported(u),
        Err(e) => Outcome::unsupported(what, format!("the report failed: {e}")),
    }
}

/// One cell of the net balance row: `None` when empty, the amount when there is one.
fn cell_amount(cell: &Cell, what: &str) -> Result<Option<SignedAmount>, Unsupported> {
    match cell {
        Cell::Empty => Ok(None),
        Cell::Amount(amount) => Ok(Some(*amount)),
        Cell::Unsupported(u) => Err(u.clone()),
        Cell::Signed(_)
        | Cell::Qty(_)
        | Cell::Text(_)
        | Cell::Date(_)
        | Cell::Provisional { .. } => Err(Unsupported {
            what: what.to_owned(),
            reason: "the net balance row holds a cell that is not a final amount".to_owned(),
        }),
    }
}

/// The group's net balance, read from the report's net balance row (never summed here). A
/// balance on the side opposite to `normal` is not shown as a plain figure, because the tile
/// has no side marker.
fn net_balance(report: &Report, normal: DrCr, what: &str) -> Outcome<Paise> {
    let refuse = |reason: &str| Outcome::unsupported(what, reason);
    let Some(row) = report
        .rows
        .iter()
        .rev()
        .find(|r| r.kind == RowKind::Subtotal)
    else {
        return refuse("the group summary has no net balance row");
    };
    let (Some(dr_cell), Some(cr_cell)) = (row.cells.first(), row.cells.get(1)) else {
        return refuse("the net balance row does not have a debit and a credit cell");
    };
    let (dr, cr) = match (cell_amount(dr_cell, what), cell_amount(cr_cell, what)) {
        (Ok(dr), Ok(cr)) => (dr, cr),
        (Err(u), Ok(_) | Err(_)) | (Ok(_), Err(u)) => return Outcome::Unsupported(u),
    };
    let amount = match (dr, cr) {
        (None, None) => return Outcome::Value(Paise::ZERO),
        (Some(a), None) | (None, Some(a)) => a,
        (Some(_), Some(_)) => return refuse("the net balance row has both a debit and a credit"),
    };
    if amount.side == normal {
        Outcome::Value(amount.amount)
    } else {
        refuse(&format!(
            "the balance is {} on the side opposite to the usual one; open the balance sheet to see it with its side",
            format_inr(amount.amount)
        ))
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
        key: "Ctrl Shift J",
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

/// The home-screen keys. Checked on 2026-11-21 against Tally's keyboard shortcut page
/// (<https://help.tallysolutions.com/tally-prime/keyboard-shortcuts/keyboard-shortcuts-tally-prime/>):
/// Ctrl+J is the report exceptions key there, so it is not used here. Ctrl+Shift+J is not listed
/// anywhere on that page. The describe row is our own key, since Tally has no assistant.
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
    (
        Key::J,
        Modifiers::COMMAND.plus(Modifiers::SHIFT),
        false,
        || HomeTarget::DescribeTransaction,
    ),
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
    /// The date the next KPI refresh is for, when one is wanted and not yet sent to the worker.
    want: Option<Date>,
    /// The KPI refresh running on the API worker.
    latest: Latest<Vec<Kpi>>,
    /// Why the KPIs could not be refreshed, when they could not.
    notice: Option<String>,
}

impl HomeScreen {
    /// A home screen showing `data`.
    #[must_use]
    pub fn new(data: HomeData) -> Self {
        Self {
            data,
            request: None,
            want: None,
            latest: Latest::default(),
            notice: None,
        }
    }

    /// Asks for the KPI tiles to be computed as of `as_of` by the API worker. The next
    /// [`HomeScreen::tick`] sends the request; the tiles change when the answer arrives.
    pub fn request_refresh(&mut self, as_of: Date) {
        self.want = Some(as_of);
    }

    /// `true` while a refresh is waiting for the worker or running on it.
    #[must_use]
    pub fn is_loading(&self) -> bool {
        self.want.is_some() || self.latest.is_pending()
    }

    /// Why the last refresh failed, when it did.
    #[must_use]
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// Advances the refresh by one step: shows a finished answer, then sends a waiting request
    /// to `worker`. With no worker (no company open) the request is dropped and the notice says
    /// so; the tiles are left as they were.
    pub fn tick(&mut self, worker: Option<&ApiWorker>) {
        match self.latest.poll() {
            Some(Done::Ready(kpis)) => {
                self.data.kpis = kpis;
                self.notice = None;
            }
            Some(Done::Lost) => {
                self.notice = Some(
                    "The background worker stopped, so the figures could not be refreshed."
                        .to_owned(),
                );
            }
            None => {}
        }
        let Some(as_of) = self.want.take() else {
            return;
        };
        let Some(handle) = worker else {
            self.notice = Some("No company is open, so there are no figures to show.".to_owned());
            return;
        };
        self.latest.start(handle, move |api| home_kpis(api, as_of));
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

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        self.tick(ctx.api.as_ref());
        if self.is_loading() {
            ui.ctx().request_repaint();
        }
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
            if let Some(notice) = &self.notice {
                ui.colored_label(ui.visuals().error_fg_color, notice.as_str());
            }
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
