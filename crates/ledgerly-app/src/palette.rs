//! Command palette: one box for screens, reports, ledgers, vouchers, commands and settings
//! (`design/mock-v2/DESIGN.md`, "Command palette"; `design/mock-v2/index.html:1108-1162`).
//!
//! Frozen interface (plan card U2), kept as it was:
//!
//! ```text
//! pub struct Palette { .. }   // results come from ledgerly-api search after W7
//! ```
//!
//! The palette searches commands, reports, masters (name, alias, GSTIN), vouchers (number, party,
//! amount, date), settings and AI intents (spec section 7). The host feeds it a list of
//! [`PaletteEntry`] values with [`Palette::set_entries`]; the API's hits come in through
//! [`PaletteSearch`]. The palette only ranks and shows them. It never computes a figure: an entry's
//! `detail` is text the host got from the API.
//!
//! The query may start with a prefix that narrows the rows (see [`PaletteScope`]): `#` vouchers,
//! `@` ledgers and parties, `>` commands, `?` ask your books and `+` a plain-words entry. `?` and
//! `+` are not answered in this version: their one row says so, and choosing it does nothing.
//!
//! Two ways to open it, as in the spec's screen-stack rule: [`PaletteMode::OnTop`] (the screen
//! you came from is kept and Esc returns to it) and [`PaletteMode::Replace`] (the choice replaces
//! the current screen). Enter chooses in the mode the palette was opened with, and Shift+Enter in
//! the other one. The palette reports which the user used; the host acts on it.
//!
//! Ranking uses integer scores only. Every word the user typed must match the title, an alias or
//! the detail: an exact match scores highest, then a prefix, a word prefix, a substring, and last
//! a subsequence ("trbl" finds "Trial Balance"). Ties go to recently chosen entries, then to the
//! host's order. The rows are then grouped under headings (see `Section` in the `scope` module),
//! keeping their ranked order inside each heading.
//!
//! Recent choices are kept for this session only: they are held in memory and not saved, so a
//! restart forgets them. The palette keeps the last eight.

use std::cmp::Reverse;

use crate::keymap::{Action, Keymap};
use crate::screen::KeyHint;

mod draw;
mod scope;
mod search;

pub use draw::palette_rect;
pub use scope::PaletteScope;
pub use search::{PaletteSearch, entries_from_hits, unavailable_reason};

use scope::{Section, assistant_row};

use crate::shell::navigate::new_ledger_key;

/// The rows the empty query lists under "Type a prefix": the text each fills, its title and the
/// words under it (`design/mock-v2/index.html:1116`).
const PREFIX_ROWS: [(&str, &str, &str); 5] = [
    ("#", "# voucher number", "e.g. #0412"),
    ("@", "@ party or ledger", "name, alias or GSTIN"),
    (">", "> command", "e.g. > export"),
    (
        "?",
        "? ask your books",
        "answer is a report the engine runs",
    ),
    (
        "+",
        "+ entry in plain words",
        "drafts a voucher for you to review",
    ),
];

/// Where `query` first appears in `title`, ignoring case, as byte offsets into `title`; `None`
/// when it does not appear as one run of letters, or when `query` is empty. This is the part of
/// a row's title the palette marks, as the mock's `hl` does (`design/mock-v2/index.html:1112`).
/// Letters are compared one by one, so a letter whose lower case is longer does not shift the
/// offsets.
#[must_use]
pub fn match_span(title: &str, query: &str) -> Option<(usize, usize)> {
    let wanted: Vec<char> = query.chars().collect();
    if wanted.is_empty() {
        return None;
    }
    let same = |a: char, b: char| a == b || a.to_lowercase().eq(b.to_lowercase());
    let letters: Vec<(usize, char)> = title.char_indices().collect();
    for start in 0..letters.len() {
        let run = letters.get(start..start.saturating_add(wanted.len()))?;
        if run.iter().zip(&wanted).all(|((_, a), b)| same(*a, *b)) {
            let (first, _) = run.first()?;
            let (last, c) = run.last()?;
            return Some((*first, last.saturating_add(c.len_utf8())));
        }
    }
    None
}

/// How many recent choices are remembered, for this session only (see the module docs).
const MAX_RECENTS: usize = 8;

/// The placeholder of the query box (`design/mock-v2/index.html:1136`).
pub const PLACEHOLDER: &str = "Search reports, ledgers, vouchers, or type ? to ask, + to enter";

/// The prefix legend under the query box: each prefix and the word it finds, in the mock's order
/// (`design/mock-v2/index.html:1138`).
pub const LEGEND: [(&str, &str); 5] = [
    ("#", "voucher"),
    ("@", "party"),
    (">", "command"),
    ("?", "ask"),
    ("+", "entry"),
];

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
    /// Put this text in the query box: a prefix (`#`), or a prefix and the typed text (`#0412`).
    /// The palette does this itself and stays open; it is never returned in a [`PaletteChoice`].
    Fill(String),
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
    /// Secondary text shown beside the title, searched at lower weight (a key hint, a party). For
    /// a command it is the key, which is drawn as a keycap.
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

/// What choosing a row does, besides what its target says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowKind {
    /// It is chosen and remembered as recent.
    Pick,
    /// It is chosen but not remembered: offering it again later would be wrong (a ledger create,
    /// once the ledger exists).
    Once,
    /// Choosing it does nothing (an `?` or `+` row: the assistant is not available yet).
    Inert,
}

/// A row as drawn: its heading, the entry, and what choosing it does.
#[derive(Debug, Clone)]
struct Row {
    /// The heading the row sits under.
    section: Section,
    /// The entry the row shows and chooses.
    entry: PaletteEntry,
    /// What choosing it does.
    kind: RowKind,
}

impl Row {
    /// A row under `section` for `entry`.
    fn new(section: Section, entry: PaletteEntry, kind: RowKind) -> Row {
        Row {
            section,
            entry,
            kind,
        }
    }
}

/// The palette's state: closed, or open with a query, a list and a selected row.
///
/// The rows shown are recomputed whenever the query, the entries or the recents change, so
/// [`Palette::results`] is always the list the user sees, in the order it is drawn.
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
    /// The targets of the recent choices, most recent first (for ties and the Recent heading).
    recents: Vec<PaletteTarget>,
    /// The recent choices as rows, parallel to `recents`. A row is kept even when the API search
    /// that found it is gone, so a recent choice can still be shown.
    recent_rows: Vec<PaletteEntry>,
    /// The rows as drawn and chosen from, grouped by heading.
    shown: Vec<Row>,
    selected: usize,
    /// Whether the query box takes the focus on the next frame.
    focus_pending: bool,
    /// Whether the query box's cursor moves to the end of the text on the next frame (after a
    /// fill).
    cursor_to_end: bool,
    /// The text whose API search has answered (or that no search will be made for), so a query
    /// with no match is only offered the no-match rows once nothing more can be found.
    settled: Option<String>,
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
        self.refresh();
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
        self.settled = None;
        self.rebuild();
    }

    /// Says that the search for `text` is over: its answer is in (or none will come, with no
    /// company open). Until then a query with no match lists nothing, because the search may
    /// still find what was typed. [`PaletteSearch`] calls this.
    pub fn search_done(&mut self, text: &str) {
        self.settled = Some(text.to_owned());
        self.refresh();
    }

    /// Whether the query box's cursor should move to the end of the text, once.
    fn take_cursor_to_end(&mut self) -> bool {
        std::mem::take(&mut self.cursor_to_end)
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
        // Closed, it lists nothing; `open` rebuilds the rows.
        self.shown.clear();
    }

    /// The text typed so far, prefix included.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Sets the query and selects the first row.
    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.selected = 0;
        self.refresh();
    }

    /// The text the API search is asked for: the query after its prefix, trimmed, when the prefix
    /// is searched (`#`, `@`, or none) and something is typed. `None` otherwise.
    #[must_use]
    pub fn search_text(&self) -> Option<&str> {
        let (scope, body) = PaletteScope::of_query(&self.query);
        let body = body.trim();
        (scope.searches_api() && !body.is_empty()).then_some(body)
    }

    /// The rows for the current query, in the order they are drawn: recent choices first while the
    /// query is empty, then grouped by heading, each heading in ranked order.
    #[must_use]
    pub fn results(&self) -> Vec<&PaletteEntry> {
        self.shown.iter().map(|row| &row.entry).collect()
    }

    /// Index of the selected row within [`Palette::results`].
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Moves the selection by `delta` rows, stopping at the first and last row.
    pub fn move_selection(&mut self, delta: isize) {
        let count = self.shown.len();
        let last = count.saturating_sub(1);
        self.selected = self.selected.saturating_add_signed(delta).min(last);
    }

    /// Chooses the selected row: closes the palette, remembers the choice as recent, and returns
    /// it. `None` when closed, when no row matches, or when the selected row is inert.
    pub fn accept(&mut self) -> Option<PaletteChoice> {
        let index = self.selected;
        self.accept_at(index)
    }

    /// Chooses the row at `index` of [`Palette::results`], in the palette's own mode. See
    /// [`Palette::accept`].
    pub fn accept_at(&mut self, index: usize) -> Option<PaletteChoice> {
        self.choose(index, false)
    }

    /// Chooses the row at `index`. `other_mode` takes the other mode from the one the palette was
    /// opened with (Shift+Enter). An inert row is not chosen, and the palette stays open. A fill
    /// row puts its text in the query box and the palette stays open.
    fn choose(&mut self, index: usize, other_mode: bool) -> Option<PaletteChoice> {
        let open = self.open?;
        let row = self.shown.get(index)?;
        let kind = row.kind;
        if kind == RowKind::Inert {
            return None;
        }
        if let PaletteTarget::Fill(text) = &row.entry.target {
            let text = text.clone();
            self.set_query(text);
            self.focus_pending = true;
            self.cursor_to_end = true;
            return None;
        }
        let entry = row.entry.clone();
        let mode = if other_mode {
            draw::flipped(open)
        } else {
            open
        };
        if kind == RowKind::Pick {
            self.remember(entry.clone());
        }
        self.close();
        Some(PaletteChoice {
            target: entry.target,
            title: entry.title,
            mode,
        })
    }

    /// The recent choices (targets), most recent first. Session only: see the module docs.
    #[must_use]
    pub fn recents(&self) -> &[PaletteTarget] {
        &self.recents
    }

    fn remember(&mut self, entry: PaletteEntry) {
        self.recents.retain(|r| *r != entry.target);
        self.recents.insert(0, entry.target.clone());
        self.recents.truncate(MAX_RECENTS);
        self.recent_rows.retain(|e| e.target != entry.target);
        self.recent_rows.insert(0, entry);
        self.recent_rows.truncate(MAX_RECENTS);
    }

    fn clamp_selection(&mut self) {
        let last = self.shown.len().saturating_sub(1);
        self.selected = self.selected.min(last);
    }

    /// Recomputes the rows for the query: keeps the rows of its scope, ranks them, adds the
    /// assistant's inert row when the prefix asks for it, and groups them under their headings.
    fn refresh(&mut self) {
        let (scope, body) = PaletteScope::of_query(&self.query);
        let body = body.trim().to_owned();
        let mut rows: Vec<Row> = Vec::new();
        if scope == PaletteScope::All && body.is_empty() {
            // The empty query lists the recent choices and the prefixes, nothing else
            // (`design/mock-v2/index.html:1115-1116`).
            rows.extend(
                self.recent_rows
                    .iter()
                    .map(|entry| Row::new(Section::Recent, entry.clone(), RowKind::Pick)),
            );
            rows.extend(PREFIX_ROWS.iter().map(|(fill, title, detail)| {
                let entry = PaletteEntry::new(PaletteTarget::Fill((*fill).to_owned()), *title)
                    .with_detail(*detail);
                Row::new(Section::Prefixes, entry, RowKind::Pick)
            }));
            self.shown = rows;
            self.clamp_selection();
            return;
        }
        let pool: Vec<PaletteEntry> = self
            .merged
            .iter()
            .filter(|entry| scope.admits(&entry.target))
            .cloned()
            .collect();
        for index in rank(&body, &pool, &self.recents) {
            if let Some(entry) = pool.get(index) {
                rows.push(Row::new(
                    Section::of(&entry.target),
                    entry.clone(),
                    RowKind::Pick,
                ));
            }
        }
        if let Some((section, entry)) = assistant_row(scope, &body) {
            rows.push(Row::new(section, entry, RowKind::Inert));
        }
        let searched = self.settled.as_deref() == Some(body.as_str());
        if rows.is_empty() && scope == PaletteScope::All && searched {
            rows.extend(no_match_rows(&body));
        }
        rows.sort_by_key(|row| row.section);
        self.shown = rows;
        self.clamp_selection();
    }

    /// The keys this palette uses, for the shell's key bar while it is open. The first hint has no
    /// key: it names the palette, as the mock's key bar does (`design/mock-v2/index.html:1159`).
    /// Enter and Shift+Enter say which mode each one chooses in, for the mode the palette was
    /// opened with.
    #[must_use]
    pub fn key_hints(&self) -> Vec<KeyHint> {
        let replace_on_enter = self.open == Some(PaletteMode::Replace);
        let (enter, shift_enter) = if replace_on_enter {
            ("Replace", "Open on top")
        } else {
            ("Open on top", "Replace")
        };
        vec![
            KeyHint::new("", "Go to"),
            KeyHint::new("\u{2191} \u{2193}", "Move"),
            KeyHint::new("Enter", enter),
            KeyHint::new("Shift+Enter", shift_enter),
            KeyHint::new("Esc", "Close"),
        ]
    }

    /// The palette's own footer, in the mock's words (`design/mock-v2/index.html:1139`): the keys
    /// of each hint, each drawn in its own keycap, and the words after them. Enter and Shift+Enter
    /// say which mode each one chooses in, as [`Palette::key_hints`] does.
    #[must_use]
    pub fn footer_hints(&self) -> Vec<(Vec<&'static str>, &'static str)> {
        let replace_on_enter = self.open == Some(PaletteMode::Replace);
        let (enter, shift_enter) = if replace_on_enter {
            ("replace screen", "open on top")
        } else {
            ("open on top", "replace screen")
        };
        vec![
            (vec!["\u{2191}", "\u{2193}"], "move"),
            (vec!["Enter"], enter),
            (vec!["Shift+Enter"], shift_enter),
            (vec!["Esc"], "close, back to where you were"),
        ]
    }
}

/// The rows a query with no match lists, once its search is over (`design/mock-v2/index.html:
/// 1129`): create a ledger of that name, or search the vouchers for it. The mock's third row,
/// "Ask", is listed only when AI is on; the assistant is not available, so it is not listed.
/// The voucher row's words say what the API search matches (`MatchedOn`).
fn no_match_rows(body: &str) -> [Row; 2] {
    let create = PaletteEntry::new(
        PaletteTarget::Master(new_ledger_key(body)),
        format!("Create ledger \"{body}\""),
    )
    .with_detail("Opens the ledger form with this name");
    let search = PaletteEntry::new(
        PaletteTarget::Fill(format!("#{body}")),
        format!("Search vouchers for \"{body}\""),
    )
    .with_detail("Number, party, amount or date");
    [
        Row::new(Section::NoMatch, create, RowKind::Once),
        Row::new(Section::NoMatch, search, RowKind::Pick),
    ]
}
