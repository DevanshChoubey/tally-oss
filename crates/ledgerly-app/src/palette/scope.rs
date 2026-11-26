//! The prefixes of the palette's query, the rows each one keeps, and the headings the rows sit
//! under (`design/mock-v2/DESIGN.md`, "Command palette"; `design/mock-v2/index.html:1113-1131`).

use super::{PaletteEntry, PaletteTarget};

/// Shown in the palette for a `?` query. The assistant is not connected in this version, so the
/// row says so and choosing it does nothing: no answer is made.
pub(crate) const ASK_NOT_AVAILABLE: &str =
    "Not available yet. The assistant is not connected, so nothing is answered.";

/// Shown in the palette for a `+` query. The assistant is not connected in this version, so the
/// row says so and choosing it does nothing: no draft is made.
pub(crate) const ENTRY_NOT_AVAILABLE: &str =
    "Not available yet. The assistant is not connected, so nothing is drafted.";

/// What the query's prefix limits the rows to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteScope {
    /// No prefix: screens, reports, ledgers, vouchers, commands and settings.
    All,
    /// `#`: vouchers, found by number, party, amount or date.
    Vouchers,
    /// `@`: ledgers and parties, found by name, alias or GSTIN.
    Parties,
    /// `>`: commands.
    Commands,
    /// `?`: ask your books. Not answered in this version.
    Ask,
    /// `+`: a plain-words entry. Not drafted in this version.
    Entry,
}

/// The prefix characters and their scopes, in the legend's order.
const PREFIXES: [(char, PaletteScope); 5] = [
    ('#', PaletteScope::Vouchers),
    ('@', PaletteScope::Parties),
    ('>', PaletteScope::Commands),
    ('?', PaletteScope::Ask),
    ('+', PaletteScope::Entry),
];

impl PaletteScope {
    /// Splits `query` into its scope and the text after the prefix. Leading spaces are ignored, so
    /// `" #0412"` is the voucher scope with `"0412"`. A query with no prefix is
    /// [`PaletteScope::All`] with the whole query.
    #[must_use]
    pub fn of_query(query: &str) -> (PaletteScope, &str) {
        let text = query.trim_start();
        for (prefix, scope) in PREFIXES {
            if let Some(rest) = text.strip_prefix(prefix) {
                return (scope, rest);
            }
        }
        (PaletteScope::All, text)
    }

    /// Whether a stored row with `target` is kept by this scope.
    #[must_use]
    pub fn admits(self, target: &PaletteTarget) -> bool {
        match self {
            PaletteScope::All => true,
            PaletteScope::Vouchers => matches!(target, PaletteTarget::Voucher(_)),
            PaletteScope::Parties => matches!(target, PaletteTarget::Master(_)),
            PaletteScope::Commands => {
                matches!(target, PaletteTarget::Command(_) | PaletteTarget::Run(_))
            }
            PaletteScope::Ask | PaletteScope::Entry => {
                matches!(target, PaletteTarget::AiIntent(_))
            }
        }
    }

    /// Whether the text typed after this scope's prefix is sent to the API search. Commands and the
    /// assistant rows are never searched there.
    #[must_use]
    pub fn searches_api(self) -> bool {
        match self {
            PaletteScope::All | PaletteScope::Vouchers | PaletteScope::Parties => true,
            PaletteScope::Commands | PaletteScope::Ask | PaletteScope::Entry => false,
        }
    }
}

/// The heading a row sits under. The rows are shown grouped by heading, in the order of this
/// enum (the mock lists reports, then ledgers, then commands; `index.html:1127-1129`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Section {
    /// The choices made this session, shown first while the query is empty.
    Recent,
    /// The prefixes, listed under the recent choices while the query is empty.
    Prefixes,
    /// Reports and screens.
    Reports,
    /// Ledgers and parties.
    Ledgers,
    /// Vouchers.
    Vouchers,
    /// Commands.
    Commands,
    /// Settings.
    Settings,
    /// Ask your books.
    Ask,
    /// A plain-words entry.
    Entry,
    /// What a query with no match can still do; drawn as `No match for "<query>"`.
    NoMatch,
}

impl Section {
    /// The heading of a row whose target is `target`.
    #[must_use]
    pub(crate) fn of(target: &PaletteTarget) -> Section {
        match target {
            PaletteTarget::Command(_) | PaletteTarget::Run(_) => Section::Commands,
            PaletteTarget::Report(_) => Section::Reports,
            PaletteTarget::Master(_) => Section::Ledgers,
            PaletteTarget::Voucher(_) => Section::Vouchers,
            PaletteTarget::Setting(_) => Section::Settings,
            PaletteTarget::AiIntent(_) => Section::Ask,
            PaletteTarget::Fill(_) => Section::Prefixes,
        }
    }

    /// The heading in `tr`.
    #[must_use]
    pub(crate) fn label_in(self, tr: &crate::i18n::Tr) -> String {
        let key = match self {
            Section::Recent => "palette.section.recent",
            Section::Prefixes => "palette.section.prefixes",
            Section::NoMatch => "palette.section.no_match",
            Section::Reports => "palette.section.reports",
            Section::Ledgers => "palette.section.ledgers",
            Section::Vouchers => "palette.section.vouchers",
            Section::Commands => "palette.section.commands",
            Section::Settings => "palette.section.settings",
            Section::Ask => "palette.section.ask",
            Section::Entry => "palette.section.entry",
        };
        tr.t(key)
    }
}

/// The inert row a `?` or `+` query shows: its title repeats what was typed and says the feature
/// is not available yet. It is built from the typed text only; no answer or draft is made.
#[must_use]
pub(crate) fn assistant_row(scope: PaletteScope, body: &str) -> Option<(Section, PaletteEntry)> {
    match scope {
        PaletteScope::All
        | PaletteScope::Vouchers
        | PaletteScope::Parties
        | PaletteScope::Commands => None,
        PaletteScope::Ask => Some((
            Section::Ask,
            assistant_entry("ask", "Ask your books", "Ask: ", body, ASK_NOT_AVAILABLE),
        )),
        PaletteScope::Entry => Some((
            Section::Entry,
            assistant_entry(
                "entry",
                "Entry in plain words",
                "Draft: ",
                body,
                ENTRY_NOT_AVAILABLE,
            ),
        )),
    }
}

/// One assistant row: `lead` and the typed text as its title, or `bare` when nothing is typed.
fn assistant_entry(name: &str, bare: &str, lead: &str, body: &str, detail: &str) -> PaletteEntry {
    let title = if body.is_empty() {
        bare.to_owned()
    } else {
        format!("{lead}{body}")
    };
    PaletteEntry::new(PaletteTarget::AiIntent(name.to_owned()), title).with_detail(detail)
}

/// The line shown in place of rows when the scope has nothing to show and nothing is typed after
/// the prefix. `None` when the scope needs no line.
#[must_use]
pub(crate) fn empty_scope_hint(scope: PaletteScope) -> Option<&'static str> {
    match scope {
        PaletteScope::All | PaletteScope::Ask | PaletteScope::Entry => None,
        PaletteScope::Vouchers => Some("Type a voucher number, party, amount or date after #."),
        PaletteScope::Parties => Some("Type a name, alias or GSTIN after @."),
        PaletteScope::Commands => Some("No commands to show."),
    }
}
