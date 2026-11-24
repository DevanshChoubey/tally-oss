//! Home dashboard (card S-A, wave 2): the page head, the KPI tiles, the AI area and the nudges.
//! The drawing is in `home/draw.rs`; this file holds the data, the figures, the keys and the
//! choice of body.
//!
//! Frozen interface (plan card U3, unchanged in wave 2):
//!
//! ```text
//! pub struct HomeScreen { .. }
//! impl Screen for HomeScreen { .. }
//! ```
//!
//! What the screen shows, from the mock's home (`design/mock-v2/index.html`, `SCREENS.home`, lines
//! 1041 to 1078; the light capture is `design/mock-v2/captures/states/light/home.default.webp`):
//!
//! * a page head with "Day book" and "New sale" (F8) on the right. Once drawn, the title is the
//!   mock's greeting without the user's name ([`greeting`], the user's decision of 2026-11-23)
//!   and the line under it names today, the financial year and the lock ([`date_line`]). Before
//!   the first frame it is "Home"; the key bar always says "Home".
//! * the KPI row: "Cash in hand", "Bank accounts", "Receivables", "Payables" and "GST cash
//!   payable". Each figure is an [`Outcome<Paise>`] from a report output. The screen never sums anything (rule C4), and a
//!   figure the engine cannot give shows "Not supported yet" with its reason (rule A1).
//! * the AI area. With AI off it shows the mock's note. Otherwise the plain-words line and the
//!   review list come with the assistant, so each is a labelled placeholder that runs nothing.
//! * "Sales by month", which stays "Not supported yet" until the engine has a monthly sales series.
//! * the nudges. Their rules are not built, so the card shows the host's attention items and a
//!   placeholder when there are none.
//!
//! The "Enter" and "Look up" lists are gone: the sidebar does that (decision 9 of
//! `docs/status/2026-11-22-ui-parity-report.md`).
//!
//! Keys handled here, from the mock: F8 sales invoice, F6 receipt, F5 payment, F9 purchase from a
//! bill, B balance sheet, D day book, G GST returns, M masters, Ctrl+K search. The letters are
//! ignored while a text field has focus. Ctrl+Shift+J opens the describe-a-transaction row (the
//! table of keys says why it is not Ctrl+J). Modifiers match exactly: Alt+D or Shift+F5 is not D
//! or F5. The key map of card U2 is not wired in yet (Wave 2), so the keys are read directly, as the
//! shell does for Ctrl+T.

pub(crate) mod draw;

use crate::i18n::Tr;
use egui::{Event, InputState, Key, Modifiers, Rect, Ui};
use ledgerly_api::{
    Api, Cell, ColumnSpec, Detail, Report, ReportFilter, ReportKind, ReportParams, ReviewQueue,
    RowKind,
};
use ledgerly_core::format::format_inr;
use ledgerly_core::{Date, DrCr, Outcome, Paise, Period, SignedAmount, Unsupported};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{ReservedGroup, Scenario};

use crate::screen::{AiTier, KeyHint};
use crate::screens::review::{self, QueuePanel};
use crate::screens::wire::{Done, Latest};
use crate::theme::SPACE_3;
use crate::ui::format;
use crate::worker::ApiWorker;
use crate::{AppCtx, Screen, ScreenAction};

/// Title of the page head. The mock's greeting needs a user name, which the app does not have yet.
/// Smallest width of a KPI tile that still fits its figure, in points.
pub const TILE_MIN_WIDTH: f32 = 200.0;
/// Width from which the lower row shows its two panels side by side, in points (the mock's
/// two-column main grid, which needs about 720 points).
pub const WIDE_MIN_WIDTH: f32 = 720.0;
/// Text shown in place of a figure the engine cannot compute yet.
/// The English footnote of the GST tile.
const GST_FOOTNOTE: &str = "after accepted ITC";
/// The start of a balance tile's footnote, before its date.
const CLOSING_PREFIX: &str = "Closing balance as of ";
/// Reason the cash-and-bank tile is not a figure: the engine has the two groups, not their total.
/// Reason the GST tile is not a figure: the engine has no payable after set-off.
const GST_REASON: &str = "the engine gives the GSTR-3B working (tax and credit separately) but no payable after set-off, and this screen does not subtract";

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
    /// The Review queue (the card's button).
    ReviewQueue,
}

/// The Home card's review list: the panel the Review queue draws, or why it has none yet. Both
/// come from the same `Api::review_queue` answer.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReviewCard<'a> {
    /// The queue's panel, once it has been read.
    pub panel: Option<&'a QueuePanel>,
    /// Why the queue could not be read, when it could not.
    pub error: Option<&'a str>,
}

/// One KPI tile. Every field comes from a report output; the screen computes none of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kpi {
    /// Tile title, for example "Receivables".
    pub label: String,
    /// The figure from the report, or why the report cannot give it.
    pub value: Outcome<Paise>,
    /// Line under the figure, for example "Closing balance as of 30 Apr 2026".
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
    pub fn word_in(self, tr: &Tr) -> String {
        match self {
            AttentionKind::Pending => tr.t("home.attention.pending"),
            AttentionKind::Due => tr.t("home.attention.due"),
            AttentionKind::Done => tr.t("home.attention.done"),
        }
    }

    /// The English word.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            AttentionKind::Pending => "Pending",
            AttentionKind::Due => "Due",
            AttentionKind::Done => "Done",
        }
    }
}

/// One line of the nudges card.
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
    /// Company name for the page's company line.
    pub company_name: String,
    /// Line under the company name, for example how the figures are dated.
    pub subtitle: String,
    /// KPI tiles, in display order.
    pub kpis: Vec<Kpi>,
    /// Nudge items, in display order. Empty until the nudge rules exist.
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
    /// `true` when `value_text` is a figure, `false` when it is the unsupported words.
    pub supported: bool,
}

/// The text of a tile's figure: `₹12,34,567.00` for a value, "Not supported yet" otherwise.
#[must_use]
pub fn value_text(value: &Outcome<Paise>) -> String {
    value_text_in(value, &Tr::default())
}

/// The text of a tile's figure in `tr`: the figure, or the unsupported words in that language.
#[must_use]
pub fn value_text_in(value: &Outcome<Paise>, tr: &Tr) -> String {
    match value {
        Outcome::Value(amount) => format!("\u{20B9}{}", format_inr(*amount)),
        Outcome::Unsupported(_) => tr.t("home.not_supported"),
    }
}

/// The view of one tile. An unsupported tile shows the reason instead of its footnote, so the
/// user learns why the number is missing.
#[must_use]
pub fn kpi_view_in(kpi: &Kpi, tr: &Tr) -> KpiView {
    let (footnote_text, supported) = match &kpi.value {
        Outcome::Value(_) => (footnote_in(&kpi.footnote, tr), true),
        Outcome::Unsupported(u) => (format!("{}: {}", u.what, u.reason), false),
    };
    KpiView {
        label: tile_label_in(&kpi.label, tr),
        value_text: value_text_in(&kpi.value, tr),
        footnote_text,
        supported,
    }
}

/// The English view of a tile, for callers without a language of their own.
#[must_use]
pub fn kpi_view(kpi: &Kpi) -> KpiView {
    kpi_view_in(kpi, &Tr::default())
}

/// A tile's title in `tr`. The English identity of each tile is its label; an unknown label is
/// shown as it is.
fn tile_label_in(label: &str, tr: &Tr) -> String {
    let key = match label {
        "Cash in hand" => "home.tile.cash_in_hand",
        "Bank accounts" => "home.tile.bank_accounts",
        "Receivables" => "home.tile.receivables",
        "Payables" => "home.tile.payables",
        "GST cash payable" => "home.tile.gst_cash_payable",
        other => return other.to_owned(),
    };
    tr.t(key)
}

/// A value tile's footnote in `tr`: the closing line with its date from the formatter, or the
/// GST line. Any other footnote is shown as it is.
fn footnote_in(footnote: &str, tr: &Tr) -> String {
    if footnote == GST_FOOTNOTE {
        return tr.t("home.after_itc");
    }
    match footnote.strip_prefix(CLOSING_PREFIX) {
        Some(date) => tr.tf("home.closing_as_of", &[("date", date)]),
        None => footnote.to_owned(),
    }
}

/// How many KPI tiles sit side by side in `width` points: four when all four fit, then two,
/// then one. Four is 4 x 200 + 3 x 12 = 836 points, two is 2 x 200 + 12 = 412 points.
#[must_use]
pub fn tile_columns(width: f32) -> usize {
    const FOUR: f32 = 4.0 * TILE_MIN_WIDTH + 3.0 * SPACE_3;
    const TWO: f32 = 2.0 * TILE_MIN_WIDTH + SPACE_3;
    if width >= FOUR {
        4
    } else if width >= TWO {
        2
    } else {
        1
    }
}

/// `true` when the lower row shows its two panels side by side at `width` points.
#[must_use]
pub fn panel_row_is_wide(width: f32) -> bool {
    width >= WIDE_MIN_WIDTH
}

/// Gap between the lower row's two columns, in points (`.grid{gap:12px}`).
const MAIN_GAP: f32 = SPACE_3;
/// Narrowest the lower row's right column gets (`minmax(320px,1fr)`).
const MAIN_RIGHT_MIN: f32 = 320.0;
/// The left column's share against the right's 1 (`minmax(0,1.55fr)`).
const MAIN_LEFT_FR: f32 = 1.55;

/// The widths of the lower row's left and right columns in `width` points, as the mock's
/// `.g-main` grid gives them, or `None` when the panels stack ([`panel_row_is_wide`]).
#[must_use]
#[allow(
    clippy::float_arithmetic,
    reason = "sharing a width between two columns"
)]
pub fn main_columns(width: f32) -> Option<(f32, f32)> {
    if !panel_row_is_wide(width) {
        return None;
    }
    let shared = width - MAIN_GAP;
    let right = (shared / (MAIN_LEFT_FR + 1.0)).max(MAIN_RIGHT_MIN);
    Some((shared - right, right))
}

/// A tile figure split for drawing: the rupee sign, drawn small (`.kpi .fig small`), and the
/// rest. Words, which carry no sign, come back whole.
#[must_use]
pub fn figure_parts(value_text: &str) -> (Option<&str>, &str) {
    match value_text.strip_prefix('\u{20B9}') {
        Some(rest) => (Some("\u{20B9}"), rest),
        None => (None, value_text),
    }
}

/// The greeting for `hour` (0 to 23): "Good morning" from 05:00, "Good afternoon" from 12:00,
/// "Good evening" from 17:00 until 05:00.
#[must_use]
pub fn greeting(hour: u8, tr: &Tr) -> String {
    match hour {
        5..=11 => tr.t("home.greet.morning"),
        12..=16 => tr.t("home.greet.afternoon"),
        _ => tr.t("home.greet.evening"),
    }
}

/// The line under the greeting: today with its weekday, then the financial year and the lock
/// when known, joined by middle dots (`Saturday 21 Nov 2026 · FY 2026-27 · books locked to 31 Oct
/// 2026`).
#[must_use]
pub fn date_line(today: Date, fy: Option<&str>, lock: Option<Date>, tr: &Tr) -> String {
    let mut parts = vec![crate::ui::format::long_date_in(today, tr)];
    if let Some(fy) = fy {
        parts.push(fy.to_owned());
    }
    if let Some(lock) = lock {
        parts.push(tr.tf(
            "home.locked_to",
            &[("date", &crate::ui::format::date_in(lock, tr))],
        ));
    }
    parts.join(" \u{b7} ")
}

/// Which body of the home screen is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeBody {
    /// The open company has no vouchers yet: first steps and no figures (the mock's empty state,
    /// `home.empty`). `begins` is the first day the books take entries for.
    NewCompany {
        /// First day on which vouchers may be entered.
        begins: Date,
    },
    /// A refresh is waiting for the engine: skeleton rows and no figure (`home.loading`).
    Loading,
    /// The figures from the last refresh (`home.default`, `home.ai-off`).
    Figures,
    /// No company is open, so there are no figures to show.
    NoFigures,
}

/// Chooses the body. A company with no vouchers always shows its first steps, so the engine's
/// zero figures are never drawn; a loading refresh shows skeletons before any figure.
#[must_use]
pub fn body(new_company: Option<Date>, loading: bool, has_figures: bool) -> HomeBody {
    match (new_company, loading, has_figures) {
        (Some(begins), _, _) => HomeBody::NewCompany { begins },
        (None, true, _) => HomeBody::Loading,
        (None, false, true) => HomeBody::Figures,
        (None, false, false) => HomeBody::NoFigures,
    }
}

/// What the AI area of the home screen shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiPanel {
    /// AI is off for the company: the mock's note says so, and the entry line and review list
    /// are not drawn.
    Off,
    /// The assistant is not built, or its tier is not set: a labelled placeholder that runs
    /// nothing.
    Placeholder,
}

/// The AI panel for the company's AI tier (`AppCtx::ai_tier`). Only `Off` hides the AI area;
/// `Local`, `Cloud` and "not set" all show the placeholder, since no assistant runs yet.
#[must_use]
pub fn ai_panel(tier: Option<AiTier>) -> AiPanel {
    match tier {
        Some(AiTier::Off) => AiPanel::Off,
        Some(AiTier::Local | AiTier::Cloud) | None => AiPanel::Placeholder,
    }
}

/// The company line under the page title: the company name, the data's subtitle, or both joined
/// with a middle dot. Empty when neither is set.
#[must_use]
pub fn sub_line(company: &str, subtitle: &str) -> String {
    match (company.is_empty(), subtitle.is_empty()) {
        (true, true) => String::new(),
        (false, true) => company.to_owned(),
        (true, false) => subtitle.to_owned(),
        (false, false) => format!("{company} \u{b7} {subtitle}"),
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

/// The tiles that read a group balance. Tally's dashboard help page
/// (<https://help.tallysolutions.com/tally-prime/reports-in-tallyprime/dashboard/>, read
/// 2026-11-21) lists Cash/Bank Accounts and Receivables/Payables among its default tiles; it does
/// not define the figures, so each tile is the closing balance of one reserved group, taken from
/// the group summary.
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

/// The home KPI tiles as of `as_of`: cash in hand, bank accounts, receivables, payables, GST.
/// Runs on the API worker. A tile whose report fails or is unsupported carries `Unsupported`
/// with the reason; no tile ever carries a guessed figure.
///
/// The mock merges cash and bank into one "Cash and bank" tile. The engine gives them as two
/// groups and this screen does not add (rule C4), so both group figures are shown until the
/// engine gives a combined total.
#[must_use]
pub fn home_kpis(api: &Api, as_of: Date) -> Vec<Kpi> {
    let mut kpis: Vec<Kpi> = TILES
        .iter()
        .map(|tile| tile_kpi(api, tile, as_of))
        .collect();
    kpis.push(gst_kpi());
    kpis
}

/// The GST tile. It stays unsupported until the engine gives the net payable after set-off.
fn gst_kpi() -> Kpi {
    Kpi {
        label: "GST cash payable".to_owned(),
        value: Outcome::unsupported("GST cash payable", GST_REASON),
        footnote: GST_FOOTNOTE.to_owned(),
        target: HomeTarget::GstReturns {
            tab: "gstr3b".to_owned(),
        },
    }
}

fn tile_kpi(api: &Api, tile: &Tile, as_of: Date) -> Kpi {
    let value = tile_value(api, tile, as_of);
    Kpi {
        label: tile.label.to_owned(),
        value,
        footnote: format!("{CLOSING_PREFIX}{}", format::date(as_of)),
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

/// The keys the key bar shows for this screen: the ones above that open a screen that exists.
/// F9 and Ctrl+Shift+J are left out because they open nothing yet.
const HOME_KEYS: [(&str, &str); 8] = [
    ("F8", "sidebar.sales"),
    ("F5", "sidebar.payment"),
    ("F6", "sidebar.receipt"),
    ("B", "sidebar.balance_sheet"),
    ("D", "sidebar.day_book"),
    ("G", "sidebar.gst_returns"),
    ("M", "sidebar.masters"),
    ("Ctrl+K", "home.palette"),
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
    /// The review queue read running on the API worker, with the KPI refresh.
    review_latest: Latest<Result<ReviewQueue, String>>,
    /// The review queue's panel from the last read, shared with the Review screen's rows.
    review: Option<QueuePanel>,
    /// Why the review queue could not be read, when it could not.
    review_error: Option<String>,
    /// Why the KPIs could not be refreshed, when they could not.
    notice: Option<String>,
    /// Where each KPI tile was drawn in the last frame, in order. Empty when no tile was drawn.
    tile_rects: Vec<Rect>,
    /// The greeting and the date line, set from the clock and the books on each frame.
    head: Option<(String, String)>,
    /// The language of the strings, set from the context on each frame.
    tr: Tr,
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
            review_latest: Latest::default(),
            review: None,
            review_error: None,
            notice: None,
            tile_rects: Vec::new(),
            head: None,
            tr: Tr::default(),
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

    /// The Home card's review list from the last read: the same panel the Review queue draws.
    #[must_use]
    pub fn review_panel(&self) -> Option<&QueuePanel> {
        self.review.as_ref()
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
        match self.review_latest.poll() {
            Some(Done::Ready(Ok(queue))) => {
                self.review = Some(review::panel(&queue));
                self.review_error = None;
            }
            Some(Done::Ready(Err(e))) => self.review_error = Some(e),
            Some(Done::Lost) => {
                self.review_error = Some(
                    "The background worker stopped, so the proposals could not be read.".to_owned(),
                );
            }
            None => {}
        }
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
        self.review_latest
            .start(handle, |api| api.review_queue().map_err(|e| e.to_string()));
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
        self.data
            .kpis
            .iter()
            .map(|kpi| kpi_view_in(kpi, &self.tr))
            .collect()
    }

    /// The screen rectangle of each KPI tile as it was drawn in the last frame, in order. Empty
    /// when the last frame drew no tiles (a loading, new-company or no-figures body).
    #[must_use]
    pub fn tile_rects(&self) -> &[Rect] {
        &self.tile_rects
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

    /// The tiles as the drawing takes them: the view and the click target of each.
    fn tile_items(&self) -> Vec<(KpiView, HomeTarget)> {
        self.data
            .kpis
            .iter()
            .map(|kpi| (kpi_view_in(kpi, &self.tr), kpi.target.clone()))
            .collect()
    }
}

impl Screen for HomeScreen {
    fn title(&self, tr: &Tr) -> String {
        tr.t("home.title")
    }

    fn page_title(&self, tr: &Tr) -> String {
        self.head
            .as_ref()
            .map_or_else(|| tr.t("home.title"), |(greet, _)| greet.clone())
    }

    fn key_bar_name(&self, tr: &Tr) -> String {
        tr.t("home.title")
    }

    fn subtitle(&self, _tr: &Tr) -> Option<String> {
        let line = match &self.head {
            Some((_, line)) => line.clone(),
            None => sub_line(&self.data.company_name, &self.data.subtitle),
        };
        (!line.is_empty()).then_some(line)
    }

    fn keys(&self, tr: &Tr) -> Vec<KeyHint> {
        HOME_KEYS
            .iter()
            .map(|(keys, label)| KeyHint::new(*keys, tr.t(label)))
            .collect()
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        self.tick(ctx.api.as_ref());
        if self.is_loading() {
            ui.ctx().request_repaint();
        }
        self.handle_keys(ui);
        let new_company = ctx
            .books
            .as_ref()
            .filter(|books| books.vouchers.is_empty())
            .map(|books| books.company.books_begin);
        let shown = body(new_company, self.is_loading(), !self.data.kpis.is_empty());
        let tr = ctx.tr.clone();
        self.tr = tr.clone();
        let fy = ctx.books.as_ref().map(|books| {
            tr.tf(
                "topbar.fy",
                &[(
                    "year",
                    &ledgerly_core::FinancialYear {
                        start: books.company.fy_start,
                    }
                    .label(),
                )],
            )
        });
        self.head = Some((
            greeting(crate::shell::navigate::hour_now(), &tr),
            date_line(
                crate::shell::navigate::today(),
                fy.as_deref(),
                ctx.lock_date,
                &tr,
            ),
        ));
        let title = self.page_title(&ctx.tr);
        let line = self.subtitle(&ctx.tr).unwrap_or_default();
        let ai = ai_panel(ctx.ai_tier);
        let tiles = self.tile_items();
        let mut chosen = None;
        let mut rects = Vec::new();
        egui::ScrollArea::vertical().show(ui, |ui| {
            draw::header(
                ui,
                &tr,
                shown,
                &self.data.company_name,
                &title,
                &line,
                &mut chosen,
            );
            draw::notice(ui, self.notice.as_deref());
            match shown {
                HomeBody::NewCompany { begins } => draw::welcome(ui, &tr, begins, &mut chosen),
                HomeBody::Loading => draw::skeleton(ui),
                HomeBody::Figures => {
                    let review = ReviewCard {
                        panel: self.review.as_ref(),
                        error: self.review_error.as_deref(),
                    };
                    rects = draw::figures(
                        ui,
                        &tr,
                        &tiles,
                        ai,
                        &self.data.attention,
                        review,
                        &mut chosen,
                    );
                }
                HomeBody::NoFigures => draw::no_figures(ui, &tr),
            }
        });
        self.tile_rects = rects;
        if let Some(target) = chosen {
            self.request_open(target);
        }
        ScreenAction::Stay
    }
}
