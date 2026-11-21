//! Command palette. **Card U2** implements this module.
//!
//! Frozen interface (plan card U2):
//!
//! ```text
//! pub struct Palette { .. }   // results come from ledgerly-api search after W7
//! ```
//!
//! The palette searches commands, reports, masters (name, alias, GSTIN), vouchers (number, party,
//! amount, date), settings and AI intents (spec section 7). Until the API's search lands (W7) the
//! host feeds it a list of [`PaletteEntry`] values with [`Palette::set_entries`]; the palette only
//! ranks and shows them. It never computes a figure: an entry's `detail` is text the host got from
//! the API.
//!
//! Two ways to open it, as in the spec's screen-stack rule: [`PaletteMode::OnTop`] (the screen
//! you came from is kept and Esc returns to it) and [`PaletteMode::Replace`] (the choice replaces
//! the current screen). The palette reports which the user used; the host acts on it.
//!
//! Ranking uses integer scores only. Every word the user typed must match the title, an alias or
//! the detail: an exact match scores highest, then a prefix, a word prefix, a substring, and last
//! a subsequence ("trbl" finds "Trial Balance"). Ties go to recently chosen entries, then to the
//! host's order.

use std::cmp::Reverse;

use ledgerly_api::SearchHit;

use crate::keymap::{Action, Keymap};
use crate::screens::report_view::date_text;
use crate::screens::wire::{Done, Latest};
use crate::shell::navigate::{ledger_key, report_key, voucher_key};
use crate::worker::ApiWorker;

/// How many recent choices are remembered.
const MAX_RECENTS: usize = 8;

/// What choosing an entry should do. The string is an opaque key the host understands (a report
/// id, a ledger id, a voucher id, a setting name, an AI intent name).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PaletteTarget {
    /// Run an action from the keymap.
    Command(Action),
    /// Open a report.
    Report(String),
    /// Open a master (ledger, item, party).
    Master(String),
    /// Open a voucher.
    Voucher(String),
    /// Open a setting.
    Setting(String),
    /// Start an AI intent (a proposal; never a posting).
    AiIntent(String),
}

/// One row the palette can show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteEntry {
    /// What choosing the row does.
    pub target: PaletteTarget,
    /// The main text.
    pub title: String,
    /// Other words that find this row (an alias, a GSTIN, a voucher number).
    pub aliases: Vec<String>,
    /// Secondary text shown beside the title, searched at lower weight (a key hint, a party).
    pub detail: String,
}

impl PaletteEntry {
    /// A row with a title and nothing else.
    #[must_use]
    pub fn new(target: PaletteTarget, title: impl Into<String>) -> PaletteEntry {
        PaletteEntry {
            target,
            title: title.into(),
            aliases: Vec::new(),
            detail: String::new(),
        }
    }

    /// Adds words that find this row.
    #[must_use]
    pub fn with_aliases<I, S>(mut self, aliases: I) -> PaletteEntry
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.aliases = aliases.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the secondary text.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> PaletteEntry {
        self.detail = detail.into();
        self
    }
}

/// How the palette was opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteMode {
    /// Open the choice on top of the current screen; Esc returns.
    OnTop,
    /// The choice replaces the current screen.
    Replace,
}

/// What the user chose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteChoice {
    /// What to do.
    pub target: PaletteTarget,
    /// The row's title, for the breadcrumb.
    pub title: String,
    /// Whether to stack on top of or replace the current screen.
    pub mode: PaletteMode,
}

/// One entry per keymap action, with the first chord as the detail text.
#[must_use]
pub fn command_entries(keymap: &Keymap) -> Vec<PaletteEntry> {
    Action::ALL
        .iter()
        .map(|action| {
            PaletteEntry::new(PaletteTarget::Command(*action), action.label())
                .with_aliases([action.id()])
                .with_detail(keymap.shortcut_label(*action).unwrap_or_default())
        })
        .collect()
}

/// Ranks `entries` for `query`, best first, and returns their indices. An empty query keeps every
/// entry: recent ones first (most recent first), then the rest in the given order.
#[must_use]
pub fn rank(query: &str, entries: &[PaletteEntry], recents: &[PaletteTarget]) -> Vec<usize> {
    let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut scored: Vec<(Reverse<u32>, usize, usize)> = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let Some(score) = entry_score(&terms, entry) else {
            continue;
        };
        let recent = recents
            .iter()
            .position(|r| *r == entry.target)
            .unwrap_or(usize::MAX);
        scored.push((Reverse(score), recent, index));
    }
    scored.sort_unstable();
    scored.into_iter().map(|(_, _, index)| index).collect()
}

/// The score of an entry for all terms, or `None` when some term matches nothing.
fn entry_score(terms: &[String], entry: &PaletteEntry) -> Option<u32> {
    let title = entry.title.to_lowercase();
    let aliases: Vec<String> = entry.aliases.iter().map(|a| a.to_lowercase()).collect();
    let detail = entry.detail.to_lowercase();
    let mut total: u32 = 0;
    for term in terms {
        let mut best: Option<u32> = term_score(term, &title);
        for alias in &aliases {
            let s = term_score(term, alias).map(|s| s.saturating_sub(50));
            best = best.max(s);
        }
        let d = term_score(term, &detail).map(|s| s / 2);
        best = best.max(d);
        total = total.saturating_add(best?);
    }
    Some(total)
}

/// How well a lower-case `term` matches a lower-case `text`, or `None` for no match.
fn term_score(term: &str, text: &str) -> Option<u32> {
    if term.is_empty() || text.is_empty() {
        return None;
    }
    if text == term {
        return Some(1000);
    }
    if text.starts_with(term) {
        return Some(800);
    }
    if text
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| !word.is_empty() && word.starts_with(term))
    {
        return Some(600);
    }
    if text.contains(term) {
        return Some(400);
    }
    let gaps = subsequence_gaps(term, text)?;
    Some(300u32.saturating_sub(gaps.saturating_mul(10)).max(1))
}

/// If the characters of `term` appear in `text` in order, the number of other characters between
/// the first and last of them.
fn subsequence_gaps(term: &str, text: &str) -> Option<u32> {
    let mut wanted = term.chars().peekable();
    let mut first: Option<usize> = None;
    let mut last: usize = 0;
    let mut matched: usize = 0;
    for (position, c) in text.chars().enumerate() {
        let Some(&next) = wanted.peek() else {
            break;
        };
        if next == c {
            if first.is_none() {
                first = Some(position);
            }
            last = position;
            matched += 1;
            wanted.next();
        }
    }
    if wanted.peek().is_some() {
        return None;
    }
    let first = first?;
    let span = last.saturating_sub(first).saturating_add(1);
    let gaps = span.saturating_sub(matched);
    Some(u32::try_from(gaps).unwrap_or(u32::MAX))
}

/// The palette's state: closed, or open with a query, a list and a selected row.
#[derive(Debug, Clone, Default)]
pub struct Palette {
    open: Option<PaletteMode>,
    query: String,
    /// The rows the host fed in ([`Palette::set_entries`]).
    entries: Vec<PaletteEntry>,
    /// The rows the API search found for the current query ([`Palette::set_remote_entries`]).
    remote: Vec<PaletteEntry>,
    /// `entries` then `remote` (without rows the host already has): what is ranked and shown.
    merged: Vec<PaletteEntry>,
    /// A line shown under the rows, for example why part of the books could not be searched.
    note: Option<String>,
    recents: Vec<PaletteTarget>,
    selected: usize,
    focus_pending: bool,
}

impl Palette {
    /// A closed palette with no entries.
    #[must_use]
    pub fn new() -> Palette {
        Palette::default()
    }

    /// Replaces the list the palette searches (commands plus whatever the API returned).
    pub fn set_entries(&mut self, entries: Vec<PaletteEntry>) {
        self.entries = entries;
        self.rebuild();
    }

    /// Replaces the rows the API search found (see [`PaletteSearch`]). They are kept apart from
    /// the host's rows, so [`Palette::set_entries`] never wipes them. A row whose target the
    /// host already lists is not shown twice.
    pub fn set_remote_entries(&mut self, entries: Vec<PaletteEntry>) {
        self.remote = entries;
        self.rebuild();
    }

    /// Sets the line shown under the rows; `None` clears it.
    pub fn set_note(&mut self, note: Option<String>) {
        self.note = note;
    }

    /// The line shown under the rows, if any.
    #[must_use]
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    fn rebuild(&mut self) {
        let mut merged = self.entries.clone();
        for row in &self.remote {
            if !self.entries.iter().any(|host| host.target == row.target) {
                merged.push(row.clone());
            }
        }
        self.merged = merged;
        self.clamp_selection();
    }

    /// Is the palette showing?
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// How it was opened, while open.
    #[must_use]
    pub fn mode(&self) -> Option<PaletteMode> {
        self.open
    }

    /// Opens it with an empty query and the first row selected.
    pub fn open(&mut self, mode: PaletteMode) {
        self.open = Some(mode);
        self.query.clear();
        self.selected = 0;
        self.focus_pending = true;
        self.remote.clear();
        self.note = None;
        self.rebuild();
    }

    /// Opens it if `action` is one of the two palette actions; returns whether it was.
    pub fn open_for(&mut self, action: Action) -> bool {
        if action == Action::OpenPalette {
            self.open(PaletteMode::OnTop);
            true
        } else if action == Action::SwitchPalette {
            self.open(PaletteMode::Replace);
            true
        } else {
            false
        }
    }

    /// Closes it without choosing.
    pub fn close(&mut self) {
        self.open = None;
        self.query.clear();
        self.selected = 0;
    }

    /// The text typed so far.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Sets the query and selects the first row.
    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.selected = 0;
    }

    /// The rows for the current query, best first.
    #[must_use]
    pub fn results(&self) -> Vec<&PaletteEntry> {
        rank(&self.query, &self.merged, &self.recents)
            .into_iter()
            .filter_map(|index| self.merged.get(index))
            .collect()
    }

    /// Index of the selected row within [`Palette::results`].
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Moves the selection by `delta` rows, stopping at the first and last row.
    pub fn move_selection(&mut self, delta: isize) {
        let count = self.results().len();
        let last = count.saturating_sub(1);
        self.selected = self.selected.saturating_add_signed(delta).min(last);
    }

    /// Chooses the selected row: closes the palette, remembers the choice as recent, and returns
    /// it. `None` when closed or when no row matches.
    pub fn accept(&mut self) -> Option<PaletteChoice> {
        self.accept_at(self.selected)
    }

    /// Chooses the row at `index` of [`Palette::results`].
    pub fn accept_at(&mut self, index: usize) -> Option<PaletteChoice> {
        let mode = self.open?;
        let (target, title) = {
            let row = *self.results().get(index)?;
            (row.target.clone(), row.title.clone())
        };
        self.remember(target.clone());
        self.close();
        Some(PaletteChoice {
            target,
            title,
            mode,
        })
    }

    /// The recent choices, most recent first.
    #[must_use]
    pub fn recents(&self) -> &[PaletteTarget] {
        &self.recents
    }

    fn remember(&mut self, target: PaletteTarget) {
        self.recents.retain(|r| *r != target);
        self.recents.insert(0, target);
        self.recents.truncate(MAX_RECENTS);
    }

    fn clamp_selection(&mut self) {
        let last = self.results().len().saturating_sub(1);
        self.selected = self.selected.min(last);
    }

    /// Draws the palette in `ui` while it is open and handles its keys: Up and Down move, Enter
    /// chooses, Esc closes. Returns the choice on the frame it is made. The host decides where
    /// `ui` is (a floating area near the top of the window, for example).
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<PaletteChoice> {
        self.open?;
        let (down, up, enter, escape) = ui.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
            )
        });
        if escape {
            self.close();
            return None;
        }
        if down {
            self.move_selection(1);
        }
        if up {
            self.move_selection(-1);
        }

        let edit = ui.add(
            egui::TextEdit::singleline(&mut self.query)
                .hint_text("Search commands, reports, masters, vouchers")
                .desired_width(f32::INFINITY),
        );
        if self.focus_pending {
            edit.request_focus();
            self.focus_pending = false;
        }
        if edit.changed() {
            self.selected = 0;
        }

        let rows: Vec<String> = self
            .results()
            .iter()
            .map(|e| {
                if e.detail.is_empty() {
                    e.title.clone()
                } else {
                    format!("{}    {}", e.title, e.detail)
                }
            })
            .collect();
        let mut clicked: Option<usize> = None;
        egui::ScrollArea::vertical()
            .max_height(320.0)
            .show(ui, |ui| {
                for (n, row) in rows.into_iter().enumerate() {
                    if ui.selectable_label(n == self.selected, row).clicked() {
                        clicked = Some(n);
                    }
                }
            });

        if let Some(note) = self.note.as_deref() {
            ui.label(note);
        }
        if let Some(n) = clicked {
            return self.accept_at(n);
        }
        if enter {
            return self.accept();
        }
        None
    }
}

// ---- API search through the worker -----------------------------------------------------------

/// The palette rows for the hits `Api::search` returned for `query`.
///
/// The API decided that each hit matches, so the palette must not drop it again by its own
/// ranking: a ledger found by alias or GSTIN, or a voucher found by party, amount or date, shows
/// the query as an alias of its row, and every word of the query is then found in that alias. The
/// order of the hits is kept (ties go to the host's order). A report the app cannot open by key
/// still gets a row, and choosing it says it is not supported (see
/// [`crate::shell::navigate::open_target`]). An [`SearchHit::Unavailable`] hit makes no row; it is
/// returned by [`unavailable_reason`].
#[must_use]
pub fn entries_from_hits(query: &str, hits: &[SearchHit]) -> Vec<PaletteEntry> {
    let query = query.trim();
    hits.iter()
        .filter_map(|hit| match hit {
            SearchHit::Ledger { id, name, matched } => Some(
                PaletteEntry::new(PaletteTarget::Master(ledger_key(*id)), name.as_str())
                    .with_aliases([query])
                    .with_detail(format!("Ledger, found by {}", matched_text(*matched))),
            ),
            SearchHit::Voucher {
                id,
                number,
                date,
                matched,
            } => {
                let title = number
                    .clone()
                    .unwrap_or_else(|| format!("Voucher #{}", id.0));
                Some(
                    PaletteEntry::new(PaletteTarget::Voucher(voucher_key(*id)), title)
                        .with_aliases([query])
                        .with_detail(format!(
                            "Voucher, {}, found by {}",
                            date_text(*date),
                            matched_text(*matched)
                        )),
                )
            }
            SearchHit::Report { title, kind } => {
                let key = report_key(kind)
                    .unwrap_or_else(|| format!("unsupported-report:{title}"));
                Some(PaletteEntry::new(PaletteTarget::Report(key), *title).with_detail("Report"))
            }
            SearchHit::Setting { name } => Some(
                PaletteEntry::new(PaletteTarget::Setting((*name).to_owned()), *name)
                    .with_detail("Setting"),
            ),
            SearchHit::Unavailable { .. } => None,
        })
        .collect()
}

/// What the API said it could not search, from the hits of one query.
#[must_use]
pub fn unavailable_reason(hits: &[SearchHit]) -> Option<&str> {
    hits.iter().find_map(|hit| match hit {
        SearchHit::Unavailable { reason } => Some(reason.as_str()),
        SearchHit::Ledger { .. }
        | SearchHit::Voucher { .. }
        | SearchHit::Report { .. }
        | SearchHit::Setting { .. } => None,
    })
}

/// The field a hit matched on, in words.
fn matched_text(matched: ledgerly_api::MatchedOn) -> &'static str {
    use ledgerly_api::MatchedOn;
    match matched {
        MatchedOn::Name => "name",
        MatchedOn::Alias => "alias",
        MatchedOn::Gstin => "GSTIN",
        MatchedOn::Number => "number",
        MatchedOn::Party => "party",
        MatchedOn::Amount => "amount",
        MatchedOn::Date => "date",
        MatchedOn::Title => "title",
        MatchedOn::SettingName => "name",
    }
}

/// The answer a search brings back: the query it ran for, and the hits.
type SearchAnswer = (String, Vec<SearchHit>);

/// Runs `Api::search` on the API worker for what the user types in a [`Palette`], one call at a
/// time (a newer query drops the older call), and feeds the rows back with
/// [`Palette::set_remote_entries`]. The UI thread never waits for the API (rule D1).
///
/// Call [`PaletteSearch::tick`] once per frame while the palette is drawn.
#[derive(Debug, Default)]
pub struct PaletteSearch {
    /// The trimmed query last handled, so an unchanged query is not sent again.
    handled: Option<String>,
    /// The search in flight.
    latest: Latest<SearchAnswer>,
}

impl PaletteSearch {
    /// A search with nothing sent.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a search is running whose answer is still wanted.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.latest.is_pending()
    }

    /// Advances the search by one step: takes a finished answer, then starts a search when the
    /// query changed. With the palette closed it forgets everything. Without a worker (no company
    /// open) only the host's rows are searched, and the palette says so.
    pub fn tick(&mut self, palette: &mut Palette, worker: Option<&ApiWorker>) {
        if !palette.is_open() {
            self.handled = None;
            self.latest = Latest::default();
            return;
        }
        let query = palette.query().trim().to_owned();
        if let Some(done) = self.latest.poll() {
            Self::apply(palette, &query, done);
        }
        if self.handled.as_deref() == Some(query.as_str()) {
            return;
        }
        self.handled = Some(query.clone());
        palette.set_remote_entries(Vec::new());
        palette.set_note(None);
        self.latest = Latest::default();
        if query.is_empty() {
            return;
        }
        match worker {
            Some(worker) => {
                let wanted = query;
                self.latest
                    .start(worker, move |api| (wanted.clone(), api.search(&wanted)));
            }
            None => palette.set_note(Some(
                "No company is open, so ledgers and vouchers are not searched.".to_owned(),
            )),
        }
    }

    /// Shows a finished answer, if it is for the query now typed.
    fn apply(palette: &mut Palette, query: &str, done: Done<SearchAnswer>) {
        match done {
            Done::Ready((asked, hits)) => {
                if asked != query {
                    return;
                }
                palette.set_remote_entries(entries_from_hits(&asked, &hits));
                palette.set_note(
                    unavailable_reason(&hits)
                        .map(|reason| format!("Some of the books were not searched: {reason}")),
                );
            }
            Done::Lost => palette.set_note(Some(
                "The engine stopped before it answered the search; open the company again."
                    .to_owned(),
            )),
        }
    }
}
