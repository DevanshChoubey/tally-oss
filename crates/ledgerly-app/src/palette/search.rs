//! The palette's search through the API worker: what the API found becomes palette rows.

use ledgerly_api::SearchHit;

use super::{Palette, PaletteEntry, PaletteTarget};
use crate::screens::wire::{Done, Latest};
use crate::shell::navigate::{ledger_key, report_key, voucher_key};
use crate::ui::format;
use crate::worker::ApiWorker;

/// The palette rows for the hits `Api::search` returned for `query`.
///
/// The API decided that each hit matches, so the palette must not drop it again by its own
/// ranking: a ledger found by alias or GSTIN, or a voucher found by party, amount or date, shows
/// the query as an alias of its row, and every word of the query is then found in that alias. The
/// order of the hits is kept (ties go to the host's order). A report the app cannot open by key
/// still gets a row, and choosing it says it is not supported (see
/// [`crate::shell::navigate::open_target`]). An [`SearchHit::Unavailable`] hit makes no row; it is
/// returned by [`unavailable_reason`].
///
/// `query` is the text after the prefix (`0412` for `#0412`), the text the API was asked.
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
                            format::date(*date),
                            matched_text(*matched)
                        )),
                )
            }
            SearchHit::Report { title, kind } => {
                let key = report_key(kind).unwrap_or_else(|| format!("unsupported-report:{title}"));
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
        MatchedOn::Name | MatchedOn::SettingName => "name",
        MatchedOn::Alias => "alias",
        MatchedOn::Gstin => "GSTIN",
        MatchedOn::Number => "number",
        MatchedOn::Party => "party",
        MatchedOn::Amount => "amount",
        MatchedOn::Date => "date",
        MatchedOn::Title => "title",
    }
}

/// The answer a search brings back: the text it searched for, and the hits.
type SearchAnswer = (String, Vec<SearchHit>);

/// Runs `Api::search` on the API worker for what the user types in a [`Palette`], one call at a
/// time (a newer query drops the older call), and feeds the rows back with
/// [`Palette::set_remote_entries`]. The UI thread never waits for the API (rule D1).
///
/// Only the text after a prefix that is searched is sent: `#` and `@` send the rest of the query,
/// `>`, `?` and `+` send nothing (see [`Palette::search_text`]).
///
/// Call [`PaletteSearch::tick`] once per frame while the palette is drawn.
#[derive(Debug, Default)]
pub struct PaletteSearch {
    /// The text last handled (`None` for a query that searches nothing), so an unchanged query is
    /// not sent again. `None` outside this means nothing was handled yet.
    handled: Option<Option<String>>,
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
    /// searched text changed. With the palette closed it forgets everything. Without a worker (no
    /// company open) only the host's rows are searched, and the palette says so.
    pub fn tick(&mut self, palette: &mut Palette, worker: Option<&ApiWorker>) {
        if !palette.is_open() {
            self.handled = None;
            self.latest = Latest::default();
            return;
        }
        let wanted: Option<String> = palette.search_text().map(str::to_owned);
        if let Some(done) = self.latest.poll() {
            Self::apply(palette, wanted.as_deref(), done);
        }
        if self.handled.as_ref() == Some(&wanted) {
            return;
        }
        self.handled = Some(wanted.clone());
        palette.set_remote_entries(Vec::new());
        palette.set_note(None);
        self.latest = Latest::default();
        let Some(text) = wanted else {
            return;
        };
        match worker {
            Some(worker) => {
                let wanted = text;
                self.latest
                    .start(worker, move |api| (wanted.clone(), api.search(&wanted)));
            }
            None => palette.set_note(Some(
                "No company is open, so ledgers and vouchers are not searched.".to_owned(),
            )),
        }
    }

    /// Shows a finished answer, if it is for the text searched now.
    fn apply(palette: &mut Palette, searched_now: Option<&str>, done: Done<SearchAnswer>) {
        match done {
            Done::Ready((asked, hits)) => {
                if searched_now != Some(asked.as_str()) {
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
