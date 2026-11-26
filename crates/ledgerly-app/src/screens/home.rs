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
//!   mock's greeting with the user's first name when one is stored ([`greeting_for`])
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

pub mod chart;
mod chart_draw;
pub(crate) mod draw;
pub mod kpi;
mod load;
pub mod nudge;
mod nudge_draw;
mod review_draw;

pub use kpi::home_kpis;

use crate::i18n::Tr;
use egui::{Event, InputState, Key, Modifiers, Rect, Ui};
use ledgerly_api::{CUSTOMER_LATE_DAYS, DuplicatePayment, ReviewQueue};
use ledgerly_core::format::format_inr;
use ledgerly_core::{Date, Outcome, Paise};

use self::chart::Chart;
use self::load::{HomeLoad, home_load};
use self::nudge::{Nudge, NudgeAction};
use crate::screen::{AiTier, KeyHint};
use crate::screens::review::{self, QueuePanel};
use crate::screens::wire::{Done, Latest};
use crate::theme::SPACE_3;
use crate::worker::ApiWorker;
use crate::{AppCtx, Screen, ScreenAction};

/// The footnote of the GST tile, as the engine layer writes it.
const GST_FOOTNOTE: &str = "GSTR-3B working, after accepted ITC";
/// What a closing-balance footnote starts with.
const CLOSING_PREFIX: &str = "Closing balance as of ";
/// Smallest width of a KPI tile that still fits its figure, in points.
pub const TILE_MIN_WIDTH: f32 = 200.0;
/// Width from which the lower row shows its two panels side by side, in points (the mock's
/// two-column main grid, which needs about 720 points).
pub const WIDE_MIN_WIDTH: f32 = 720.0;
/// The rows the Home card lists before "All N" (the mock shows four).
pub const REVIEW_ROWS: usize = 4;
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
    /// AI and privacy (the AI-off note's button).
    AiSettings,
    /// The import screen ("Import from another app").
    ImportFromAnotherApp,
    /// Bills outstanding from customers (the late-bills nudge's "Show list").
    OutstandingReceivables,
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

/// A tile's title in `tr`. The English identity of each tile is its label ("Cash and bank",
/// "GST cash payable, Nov"); an unknown label is shown as it is.
fn tile_label_in(label: &str, tr: &Tr) -> String {
    let key = match label {
        "Cash and bank" => "home.tile.cash_and_bank",
        "Cash in hand" => "home.tile.cash_in_hand",
        "Bank accounts" => "home.tile.bank_accounts",
        "Receivables" => "home.tile.receivables",
        "Payables" => "home.tile.payables",
        "GST cash payable" => "home.tile.gst_cash_payable",
        other => {
            if let Some(month) = other
                .strip_prefix("GST cash payable, ")
                .and_then(crate::ui::format::month_from_abbrev)
            {
                return tr.tf(
                    "home.tile.gst_cash_payable_month",
                    &[("month", &crate::ui::format::month_abbrev_in(month, tr))],
                );
            }
            return other.to_owned();
        }
    };
    tr.t(key)
}

/// A count and the rest of a footnote, when `text` is `"<count> <rest>"`.
fn split_count(text: &str) -> Option<(&str, &str)> {
    let (count, rest) = text.split_once(' ')?;
    (!count.is_empty() && count.bytes().all(|b| b.is_ascii_digit())).then_some((count, rest))
}

/// A value tile's footnote in `tr`: the counted lines the engine's figures give, or the GST
/// line. Any other footnote (and any engine reason) is shown as it is.
fn footnote_in(footnote: &str, tr: &Tr) -> String {
    if footnote == GST_FOOTNOTE {
        return tr.t("home.kpi.gstr3b_working");
    }
    if let Some(n) = footnote
        .strip_prefix("Cash and ")
        .and_then(|rest| {
            rest.strip_suffix(" bank accounts")
                .or(rest.strip_suffix(" bank account"))
        })
        .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    {
        let key = if n == "1" {
            "home.kpi.cash_foot_one"
        } else {
            "home.kpi.cash_foot_many"
        };
        return tr.tf(key, &[("n", n)]);
    }
    if let Some((n, rest)) = split_count(footnote) {
        let days = CUSTOMER_LATE_DAYS.to_string();
        let overdue = rest
            .strip_prefix("bills overdue more than ")
            .or_else(|| rest.strip_prefix("bill overdue more than "))
            .and_then(|r| r.strip_suffix(" days"));
        if overdue == Some(days.as_str()) {
            let key = if n == "1" {
                "home.kpi.overdue_foot_one"
            } else {
                "home.kpi.overdue_foot_many"
            };
            return tr.tf(key, &[("n", n), ("days", &days)]);
        }
        if rest == "supplier bills past due" || rest == "supplier bill past due" {
            let key = if n == "1" {
                "home.kpi.past_due_foot_one"
            } else {
                "home.kpi.past_due_foot_many"
            };
            return tr.tf(key, &[("n", n)]);
        }
    }
    if let Some(reason) = footnote.strip_prefix("Overdue bills not counted: ") {
        return tr.tf("home.kpi.overdue_unknown", &[("reason", reason)]);
    }
    if let Some(reason) = footnote.strip_prefix("Past-due bills not counted: ") {
        return tr.tf("home.kpi.past_due_unknown", &[("reason", reason)]);
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

/// The greeting for `hour` with the signed-in user's first name, as the mock's head has it
/// ("Good afternoon, Ananya"). With no stored name it is the plain greeting.
#[must_use]
pub fn greeting_for(hour: u8, name: Option<&str>, tr: &Tr) -> String {
    match name.and_then(|n| n.split_whitespace().next()) {
        Some(first) => format!("{}, {first}", greeting(hour, tr)),
        None => greeting(hour, tr),
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

/// The line under the new-company title (`home.empty`): the company, then its financial year
/// with the year's first and last day (`Fictional Traders · FY 2026-27 (1 Apr 2026 to 31 Mar
/// 2027)`). The days are left out if the year's end cannot be worked out.
#[must_use]
pub fn new_company_line(company: &str, fy: ledgerly_core::FinancialYear) -> String {
    let year = format!("FY {}", fy.label());
    let days = fy.end().ok().map(|end| {
        format!(
            " ({} to {})",
            crate::ui::format::date(fy.start),
            crate::ui::format::date(end)
        )
    });
    format!(
        "{company} \u{b7} {year}{}",
        days.as_deref().unwrap_or_default()
    )
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
    latest: Latest<HomeLoad>,
    /// The sales bars from the last refresh, once it has arrived.
    chart: Option<Chart>,
    /// The nudge cards from the last refresh.
    nudges: Vec<Nudge>,
    /// The possible duplicate payments from the last refresh.
    duplicates: Vec<DuplicatePayment>,
    /// The highlighted row of the review card (the mock's `.sel` row, moved with J and K).
    selected: usize,
    /// The possible duplicate whose comparison is open (`home.nudge-open`), as an index into
    /// `duplicates`.
    compare: Option<usize>,
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
    /// The signed-in user's stored name, read with the books on each refresh.
    user_name: Option<String>,
    /// The language the figures and nudges were last built in; the next refresh builds them in
    /// this one.
    tr: Tr,
    /// The date the last refresh was for, so a change of language can ask for the same again.
    last_as_of: Option<Date>,
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
            chart: None,
            nudges: Vec::new(),
            duplicates: Vec::new(),
            compare: None,
            selected: 0,
            review_latest: Latest::default(),
            review: None,
            review_error: None,
            notice: None,
            tile_rects: Vec::new(),
            head: None,
            user_name: None,
            tr: Tr::default(),
            last_as_of: None,
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
                self.review_error = Some(self.tr.t("home.err.worker_review"));
            }
            None => {}
        }
        match self.latest.poll() {
            Some(Done::Ready(load)) => {
                self.data.kpis = load.kpis;
                self.chart = Some(load.chart);
                self.nudges = load.nudges;
                self.duplicates = load.duplicates;
                self.user_name = load.user_name;
                if self.compare.is_some_and(|i| i >= self.duplicates.len()) {
                    self.compare = None;
                }
                self.notice = None;
            }
            Some(Done::Lost) => {
                self.notice = Some(self.tr.t("home.err.worker_figures"));
            }
            None => {}
        }
        let Some(as_of) = self.want.take() else {
            return;
        };
        let Some(handle) = worker else {
            self.notice = Some(self.tr.t("home.err.no_company"));
            return;
        };
        self.last_as_of = Some(as_of);
        let today = crate::shell::navigate::today();
        let tr = self.tr.clone();
        self.latest
            .start(handle, move |api| home_load(api, as_of, today, &tr));
        self.review_latest.start(handle, move |api| {
            api.review_queue_on(today).map_err(|e| e.to_string())
        });
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
            .map(|kpi| kpi_view_in(kpi, &crate::i18n::current()))
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

    /// Reads the home keys for this frame. While the comparison is open it takes Ctrl+A (open the
    /// day book) and Esc (close) and nothing else.
    fn handle_keys(&mut self, ui: &Ui) {
        if self.compare.is_some() {
            let (investigate, close) = ui.ctx().input_mut(|i| {
                (
                    consume_exact(i, Key::A, Modifiers::CTRL),
                    consume_exact(i, Key::Escape, Modifiers::NONE),
                )
            });
            if investigate {
                self.compare = None;
                self.request_open(HomeTarget::DayBook);
            } else if close {
                self.compare = None;
            }
            return;
        }
        let typing = ui.ctx().egui_wants_keyboard_input();
        let rows = self
            .review
            .as_ref()
            .map_or(0, |p| p.rows.len().min(REVIEW_ROWS));
        let (hit, down, up, open) = ui.ctx().input_mut(|i| {
            let hit = SHORTCUTS
                .iter()
                .filter(|(_, _, plain_letter, _)| !(*plain_letter && typing))
                .find(|(key, modifiers, _, _)| consume_exact(i, *key, *modifiers))
                .map(|(_, _, _, target)| target());
            let list = !typing && rows > 0;
            (
                hit,
                list && consume_exact(i, Key::J, Modifiers::NONE),
                list && consume_exact(i, Key::K, Modifiers::NONE),
                list && consume_exact(i, Key::Enter, Modifiers::NONE),
            )
        });
        if down {
            self.selected = (self.selected + 1).min(rows.saturating_sub(1));
        }
        if up {
            self.selected = self.selected.saturating_sub(1);
        }
        if open {
            self.request_open(HomeTarget::ReviewQueue);
        }
        if let Some(target) = hit {
            self.request_open(target);
        }
    }

    /// The possible duplicate whose comparison is open, as an index into the nudges' list.
    #[must_use]
    pub fn compare_open(&self) -> Option<usize> {
        self.compare
    }

    /// Opens the comparison of the possible duplicate at `index`, if there is one.
    pub fn open_compare(&mut self, index: usize) {
        if index < self.duplicates.len() {
            self.compare = Some(index);
        }
    }

    /// The nudge cards from the last refresh.
    #[must_use]
    pub fn nudges(&self) -> &[Nudge] {
        &self.nudges
    }

    /// The tiles as the drawing takes them: the view and the click target of each.
    fn tile_items(&self) -> Vec<(KpiView, HomeTarget)> {
        self.data
            .kpis
            .iter()
            .map(|kpi| {
                (
                    kpi_view_in(kpi, &crate::i18n::current()),
                    kpi.target.clone(),
                )
            })
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
        if self.compare.is_some() {
            tr.t("home.compare.title")
        } else {
            tr.t("home.title")
        }
    }

    fn subtitle(&self, _tr: &Tr) -> Option<String> {
        let line = match &self.head {
            Some((_, line)) => line.clone(),
            None => sub_line(&self.data.company_name, &self.data.subtitle),
        };
        (!line.is_empty()).then_some(line)
    }

    fn keys(&self, tr: &Tr) -> Vec<KeyHint> {
        if self.compare.is_some() {
            return vec![
                KeyHint::new("Ctrl+A", tr.t("home.compare.open_both")),
                KeyHint::new("Esc", tr.t("common.close")),
            ];
        }
        let mut keys: Vec<KeyHint> = HOME_KEYS
            .iter()
            .map(|(keys, label)| KeyHint::new(*keys, tr.t(label)))
            .collect();
        if self.review.as_ref().is_some_and(|p| !p.rows.is_empty()) {
            keys.push(KeyHint::new("J / K", tr.t("home.keys.review_list")));
            keys.push(KeyHint::new("Enter", tr.t("common.open")));
        }
        keys
    }

    #[allow(clippy::too_many_lines, reason = "the screen's one drawing pass")]
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
        if self.tr.code() != tr.code() {
            // The nudges and the chart words were made in the old language: make them again.
            self.tr = tr.clone();
            if self.want.is_none() {
                self.want = self.last_as_of;
            }
        }
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
            greeting_for(
                crate::shell::navigate::hour_now(),
                self.user_name.as_deref(),
                &tr,
            ),
            date_line(
                crate::shell::navigate::today(),
                fy.as_deref(),
                ctx.lock_date,
                &tr,
            ),
        ));
        let title = self.page_title(&tr);
        let line = self.subtitle(&tr).unwrap_or_default();
        let new_line = ctx.books.as_ref().map_or_else(
            || self.data.company_name.clone(),
            |books| {
                let fy = ledgerly_core::FinancialYear {
                    start: books.company.fy_start,
                };
                new_company_line(&self.data.company_name, fy)
            },
        );
        let ai = ai_panel(crate::release::effective_tier(ctx.ai_tier));
        let tiles = self.tile_items();
        let mut chosen = draw::Chosen::default();
        let mut rects = Vec::new();
        egui::ScrollArea::vertical().show(ui, |ui| {
            draw::header(
                ui,
                &tr,
                shown,
                &self.data.company_name,
                &new_line,
                &title,
                &line,
                &mut chosen.target,
            );
            draw::notice(ui, self.notice.as_deref());
            match shown {
                HomeBody::NewCompany { begins } => draw::welcome(ui, begins, &mut chosen.target),
                HomeBody::Loading => draw::skeleton(ui),
                HomeBody::Figures => {
                    let review = ReviewCard {
                        panel: self.review.as_ref(),
                        error: self.review_error.as_deref(),
                    };
                    let lower = draw::Lower {
                        ai,
                        chart: self.chart.as_ref(),
                        nudges: &self.nudges,
                        attention: &self.data.attention,
                        review,
                        selected: self.selected,
                    };
                    rects = draw::figures(ui, &tr, &tiles, &lower, &mut chosen);
                }
                HomeBody::NoFigures => draw::no_figures(ui, &tr),
            }
        });
        self.tile_rects = rects;
        match chosen.nudge {
            Some(NudgeAction::Compare(index)) => self.open_compare(index),
            Some(NudgeAction::Open(target)) => self.request_open(target),
            None => {}
        }
        if let Some(target) = chosen.target {
            self.request_open(target);
        }
        if let Some(pair) = self.compare.and_then(|i| self.duplicates.get(i)) {
            match nudge_draw::dialog(ui, pair) {
                nudge_draw::DialogChoice::None => {}
                nudge_draw::DialogChoice::Investigate => {
                    self.compare = None;
                    self.request_open(HomeTarget::DayBook);
                }
                nudge_draw::DialogChoice::Close => self.compare = None,
            }
        }
        ScreenAction::Stay
    }
}
