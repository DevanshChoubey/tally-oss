//! Natural-language entry. **Card A2** implements this module.
//!
//! Frozen interface (plan card A2):
//!
//! ```text
//! pub struct NlEntry<M> { .. }   // wraps a model client; recorded test double in tests
//! impl Proposer for NlEntry<..> { .. }
//! ```
//! The assistant (spec 6.8). It proposes; the engine computes; a person confirms. It never writes
//! to the store and never supplies a derived number (rule A10).
//!
//! # How a sentence becomes a draft
//!
//! 1. [`NlEntry`] builds a [`ModelRequest`]: a versioned prompt template, the typed text, and a
//!    grammar that forces the model to answer in one fixed JSON shape. The model sees only the
//!    typed text, never the ledger masters.
//! 2. The model's JSON is parsed strictly. The amount, date, party and account are returned as the
//!    **text the user typed**. Each must appear verbatim in the input, otherwise the output is
//!    rejected: the model cannot supply a number or a name the user did not type.
//! 3. The amount text (`25k`, `1.5 lakh`, `Rs 1,25,000/-`) is converted to [`Paise`] by this
//!    module's own deterministic parser ([`parse_amount_text`]), not by the model. Dates are parsed
//!    the same way; a date that is not in a numeric day-first or ISO form stays blank.
//! 4. Names are resolved against the masters given to [`NlEntry::new`]. A name that matches no
//!    ledger is listed in `create_ledgers` (a suggestion); an id is never invented. A name that
//!    matches several ledgers is an `Unsupported` error naming them, so the user chooses.
//!
//! Only payments and receipts are drafted here. Any other kind needs GST or items and is
//! `Unsupported` until a card with provenance covers it.

use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};

use ledgerly_core::{
    CancelToken, Date, Decimal, DrCr, LedgerId, Month, Paise, RoundingRule, Unsupported,
    round_rupees_to_paise,
};
use ledgerly_model::{BaseVoucherKind, Ledger};
use serde::Deserialize;
use thiserror::Error;

use crate::proposal::{DraftLine, Proposal, SourceRef, Transcribed, VoucherDraft};
use crate::seam::{AssistError, ProposalInput, Proposer};

/// Version of the prompt template and grammar. Recorded in the audit event (spec 6.8) so an output
/// can be traced to the template that produced it. Bump it on any change to either.
pub const PROMPT_TEMPLATE_VERSION: &str = "nl-entry/1";

const SYSTEM_PROMPT: &str = "You read one sentence an accountant typed and return one JSON \
object. Copy amount_text, date_text, party_name and account_name exactly as typed, character for \
character; never compute, convert or correct them. kind is \"payment\" when money goes out and \
\"receipt\" when money comes in; for anything else use the word that names the voucher \
(\"sales\", \"purchase\", \"journal\", \"contra\"). party_name is who was paid or who paid. \
account_name is the bank or cash account used. Use null for anything not typed. narration is a \
short note of what the entry is for. confidence_permille is your confidence in the reading, an \
integer from 0 to 1000.";

/// GBNF grammar (llama.cpp) that constrains the model to the JSON shape [`NlEntry`] parses.
pub const OUTPUT_GRAMMAR: &str = r#"root ::= "{" ws kind "," ws amount "," ws date "," ws party "," ws account "," ws narration "," ws confidence ws "}"
kind ::= "\"kind\"" ws ":" ws string
amount ::= "\"amount_text\"" ws ":" ws nullable
date ::= "\"date_text\"" ws ":" ws nullable
party ::= "\"party_name\"" ws ":" ws nullable
account ::= "\"account_name\"" ws ":" ws nullable
narration ::= "\"narration\"" ws ":" ws string
confidence ::= "\"confidence_permille\"" ws ":" ws integer
nullable ::= string | "null"
string ::= "\"" ([^"\\\x7F\x00-\x1F] | "\\" (["\\/bfnrt] | "u" [0-9a-fA-F] [0-9a-fA-F] [0-9a-fA-F] [0-9a-fA-F]))* "\""
integer ::= "0" | [1-9] [0-9]? [0-9]? [0-9]?
ws ::= [ \t\n]*
"#;

/// What a model client is asked to complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRequest {
    /// [`PROMPT_TEMPLATE_VERSION`] at the time of the request.
    pub template_version: &'static str,
    /// The instruction part of the prompt.
    pub system: String,
    /// The text the user typed, unchanged.
    pub user_text: String,
    /// Grammar the output must follow ([`OUTPUT_GRAMMAR`]).
    pub grammar: &'static str,
}

/// A model that completes a [`ModelRequest`] with grammar-constrained JSON text.
///
/// This is not a second assistant seam: [`Proposer`] stays the one seam the app calls, and
/// [`NlEntry`] is its implementation. `ModelClient` is the narrower seam beneath it (rule C2: a
/// trait only where more than one implementation exists). Today the only implementation is
/// [`RecordedModel`], the recorded test double. The local llama.cpp client and the cloud client
/// (card A4) are the planned real implementations; until one lands, the trait is public only
/// because [`NlEntry`] is generic over it and must be nameable by those cards, and it is not
/// part of the frozen interface (rule C7), which is `NlEntry` and its `Proposer` impl.
pub trait ModelClient {
    /// Returns the model's raw output for the request.
    ///
    /// # Errors
    /// [`AssistError`] when the model cannot be reached or fails, or [`AssistError::Cancelled`].
    fn complete(&self, request: &ModelRequest, cancel: &CancelToken)
    -> Result<String, AssistError>;
}

/// A model double that returns recorded outputs keyed by the exact typed text. It never guesses:
/// an input without a recording is an error.
#[derive(Debug, Default)]
pub struct RecordedModel {
    recordings: Vec<(String, String)>,
    calls: AtomicUsize,
}

impl RecordedModel {
    /// An empty double.
    pub fn new() -> Self {
        RecordedModel::default()
    }

    /// Records `output` as the answer for the typed text `input`.
    #[must_use]
    pub fn with_recording(mut self, input: impl Into<String>, output: impl Into<String>) -> Self {
        self.recordings.push((input.into(), output.into()));
        self
    }

    /// How many times [`ModelClient::complete`] was called.
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl ModelClient for RecordedModel {
    fn complete(
        &self,
        request: &ModelRequest,
        _cancel: &CancelToken,
    ) -> Result<String, AssistError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.recordings
            .iter()
            .find(|(input, _)| *input == request.user_text)
            .map(|(_, output)| output.clone())
            .ok_or_else(|| AssistError::Output {
                reason: "no recorded model output for this input".to_owned(),
            })
    }
}

/// A ledger master the assistant may resolve names against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterLedger {
    /// Identifier.
    pub id: LedgerId,
    /// Name.
    pub name: String,
    /// Alternative names.
    pub aliases: Vec<String>,
}

impl From<&Ledger> for MasterLedger {
    fn from(ledger: &Ledger) -> Self {
        MasterLedger {
            id: ledger.id,
            name: ledger.name.clone(),
            aliases: ledger.aliases.clone(),
        }
    }
}

/// Natural-language voucher entry over a model client and the ledger masters.
#[derive(Debug)]
pub struct NlEntry<M> {
    model: M,
    ledgers: Vec<MasterLedger>,
}

impl<M: ModelClient> NlEntry<M> {
    /// Wraps `model`; `ledgers` are the masters names are resolved against.
    pub fn new(model: M, ledgers: Vec<MasterLedger>) -> Self {
        NlEntry { model, ledgers }
    }

    /// The wrapped model client (tests read recorded calls from it).
    pub fn model(&self) -> &M {
        &self.model
    }

    fn request_for(text: &str) -> ModelRequest {
        ModelRequest {
            template_version: PROMPT_TEMPLATE_VERSION,
            system: SYSTEM_PROMPT.to_owned(),
            user_text: text.to_owned(),
            grammar: OUTPUT_GRAMMAR,
        }
    }
}

impl<M: ModelClient> Proposer for NlEntry<M> {
    fn propose(
        &self,
        input: &ProposalInput,
        cancel: &CancelToken,
    ) -> Result<Proposal, AssistError> {
        if cancel.is_cancelled() {
            return Err(AssistError::Cancelled);
        }
        let text = match input {
            ProposalInput::Text(text) => text,
            ProposalInput::Document { .. }
            | ProposalInput::BankLine { .. }
            | ProposalInput::Columns { .. }
            | ProposalInput::Question(_) => {
                return Err(unsupported(
                    "natural-language entry from this input",
                    "NlEntry reads typed voucher text only; use the matching proposer for documents, bank lines, columns and questions",
                ));
            }
        };
        if text.trim().is_empty() {
            return Err(output_err("the typed text is empty"));
        }
        let raw = self.model.complete(&Self::request_for(text), cancel)?;
        if cancel.is_cancelled() {
            return Err(AssistError::Cancelled);
        }
        let parsed: ModelOutput = serde_json::from_str(&raw)
            .map_err(|e| output_err(format!("the model output is not the expected JSON: {e}")))?;
        let draft = self.build_draft(text, &parsed)?;
        Ok(Proposal::VoucherDraft(Box::new(draft)))
    }
}

/// The JSON the model must return (see [`OUTPUT_GRAMMAR`]).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelOutput {
    kind: String,
    amount_text: Option<String>,
    date_text: Option<String>,
    party_name: Option<String>,
    account_name: Option<String>,
    narration: String,
    confidence_permille: u16,
}

impl<M: ModelClient> NlEntry<M> {
    fn build_draft(&self, text: &str, out: &ModelOutput) -> Result<VoucherDraft, AssistError> {
        if out.confidence_permille > 1000 {
            return Err(output_err("confidence_permille is above 1000"));
        }
        let kind = voucher_kind(&out.kind)?;
        let confidence = out.confidence_permille;

        let amount_text = out
            .amount_text
            .as_deref()
            .ok_or_else(|| output_err("no amount was found in the typed text"))?;
        let amount_source = typed_source(text, amount_text, "amount")?;
        let paise = parse_amount_text(amount_text).map_err(|e| {
            output_err(format!(
                "amount \"{amount_text}\" cannot be read as money: {e}"
            ))
        })?;
        let amount = Transcribed {
            value: paise,
            source: amount_source,
            confidence_permille: confidence,
        };

        let date = match out.date_text.as_deref() {
            None => None,
            Some(date_text) => {
                let source = typed_source(text, date_text, "date")?;
                parse_date_text(date_text).map(|value| Transcribed {
                    value,
                    source,
                    confidence_permille: confidence,
                })
            }
        };

        let party = self.resolve_named(text, out.party_name.as_deref(), "party", confidence)?;
        let account =
            self.resolve_named(text, out.account_name.as_deref(), "account", confidence)?;

        let (party_side, account_side) = sides_for(kind)?;

        let mut lines = Vec::new();
        let mut create_ledgers: Vec<String> = Vec::new();
        for (named, side) in [(&party, party_side), (&account, account_side)] {
            let Some(named) = named else { continue };
            lines.push(DraftLine {
                ledger_name: named.name.clone(),
                ledger: named.id,
                side,
                amount: amount.clone(),
            });
            if named.id.is_none() && !create_ledgers.contains(&named.name.value) {
                create_ledgers.push(named.name.value.clone());
            }
        }

        Ok(VoucherDraft {
            kind,
            date,
            party_name: party.as_ref().map(|p| p.name.clone()),
            party: party.as_ref().and_then(|p| p.id),
            invoice_number: None,
            supplier_gstin: None,
            lines,
            items: Vec::new(),
            document_total: Some(amount),
            document_tax: None,
            create_ledgers,
            narration: out.narration.trim().to_owned(),
        })
    }

    /// Checks `name` is in the typed text and resolves it against the masters.
    fn resolve_named(
        &self,
        text: &str,
        name: Option<&str>,
        role: &str,
        confidence_permille: u16,
    ) -> Result<Option<NamedLedger>, AssistError> {
        let Some(name) = name else { return Ok(None) };
        let source = typed_source(text, name, role)?;
        let id = match resolve_ledger(&self.ledgers, name) {
            Resolution::Found(id) => Some(id),
            Resolution::Missing => None,
            Resolution::Ambiguous(candidates) => {
                return Err(unsupported(
                    format!("{role} \"{name}\" matches several ledgers"),
                    format!(
                        "choose one of: {}; the assistant does not pick between ledgers",
                        candidates.join(", ")
                    ),
                ));
            }
        };
        Ok(Some(NamedLedger {
            name: Transcribed {
                value: name.to_owned(),
                source,
                confidence_permille,
            },
            id,
        }))
    }
}

/// A name read from the text and the ledger it resolved to, if any.
struct NamedLedger {
    name: Transcribed<String>,
    id: Option<LedgerId>,
}

fn sides_for(kind: BaseVoucherKind) -> Result<(DrCr, DrCr), AssistError> {
    match kind {
        BaseVoucherKind::Payment => Ok((DrCr::Dr, DrCr::Cr)),
        BaseVoucherKind::Receipt => Ok((DrCr::Cr, DrCr::Dr)),
        BaseVoucherKind::Contra
        | BaseVoucherKind::Journal
        | BaseVoucherKind::Sales
        | BaseVoucherKind::Purchase
        | BaseVoucherKind::CreditNote
        | BaseVoucherKind::DebitNote
        | BaseVoucherKind::Memorandum
        | BaseVoucherKind::ReversingJournal
        | BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::DeliveryNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::RejectionOut
        | BaseVoucherKind::StockJournal
        | BaseVoucherKind::PhysicalStock
        | BaseVoucherKind::SalesOrder
        | BaseVoucherKind::PurchaseOrder => Err(unsupported(
            "natural-language entry for this voucher kind",
            "only payments and receipts are drafted from text",
        )),
    }
}

fn voucher_kind(word: &str) -> Result<BaseVoucherKind, AssistError> {
    match word.trim().to_lowercase().as_str() {
        "payment" => Ok(BaseVoucherKind::Payment),
        "receipt" => Ok(BaseVoucherKind::Receipt),
        "sales" | "purchase" | "journal" | "contra" => Err(unsupported(
            format!("natural-language entry for a {word} voucher"),
            "only payments and receipts are drafted from text",
        )),
        other => Err(output_err(format!(
            "unknown voucher kind \"{other}\" in the model output"
        ))),
    }
}

fn output_err(reason: impl Into<String>) -> AssistError {
    AssistError::Output {
        reason: reason.into(),
    }
}

fn unsupported(what: impl Into<String>, reason: impl Into<String>) -> AssistError {
    AssistError::Unsupported(Unsupported {
        what: what.into(),
        reason: reason.into(),
    })
}

/// Finds `needle` verbatim in `text` and returns its character range as a source reference.
fn typed_source(text: &str, needle: &str, role: &str) -> Result<SourceRef, AssistError> {
    let missing = || {
        output_err(format!(
            "the {role} \"{needle}\" does not appear in the typed text, so it is not a transcription"
        ))
    };
    if needle.is_empty() {
        return Err(missing());
    }
    let byte = text
        .match_indices(needle)
        .map(|(i, _)| i)
        .find(|&i| on_token_boundaries(text, i, needle))
        .ok_or_else(missing)?;
    let before = text.get(..byte).ok_or_else(missing)?;
    let start = before.chars().count();
    let end = start.saturating_add(needle.chars().count());
    Ok(SourceRef::Typed { start, end })
}

/// True when the match of `needle` at byte offset `at` in `text` is a whole token: not preceded or
/// followed by an alphanumeric character, and not glued on by a digit separator (`.`, `,`, `/`,
/// `-`) that joins it to a neighbouring digit. This keeps `5k` from matching inside `25k` and `5`
/// from matching inside `1.5`.
fn on_token_boundaries(text: &str, at: usize, needle: &str) -> bool {
    let is_sep = |c: char| matches!(c, '.' | ',' | '/' | '-');
    let end = at.saturating_add(needle.len());
    let (Some(before), Some(after)) = (text.get(..at), text.get(end..)) else {
        return false;
    };
    let first_alnum = needle.chars().next().is_some_and(char::is_alphanumeric);
    let last_alnum = needle
        .chars()
        .next_back()
        .is_some_and(char::is_alphanumeric);
    let mut back = before.chars().rev();
    let prev = back.next();
    let prev2 = back.next();
    let mut fwd = after.chars();
    let next = fwd.next();
    let next2 = fwd.next();
    if prev.is_some_and(char::is_alphanumeric) && first_alnum {
        return false;
    }
    if next.is_some_and(char::is_alphanumeric) && last_alnum {
        return false;
    }
    let starts_digit = needle.chars().next().is_some_and(|c| c.is_ascii_digit());
    let ends_digit = needle
        .chars()
        .next_back()
        .is_some_and(|c| c.is_ascii_digit());
    if starts_digit && prev.is_some_and(is_sep) && prev2.is_some_and(|c| c.is_ascii_digit()) {
        return false;
    }
    if ends_digit && next.is_some_and(is_sep) && next2.is_some_and(|c| c.is_ascii_digit()) {
        return false;
    }
    true
}

/// Why a typed amount could not be read.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AmountTextError {
    /// Nothing to read.
    #[error("the amount is empty")]
    Empty,
    /// Not a number (for example `1,,0` or `12abc3`).
    #[error("\"{0}\" is not a number")]
    NotANumber(String),
    /// A unit word that is not supported.
    #[error("unknown unit \"{0}\" (supported: k, thousand, lakh, lac, cr, crore)")]
    UnknownUnit(String),
    /// More than two decimal places once the unit is applied.
    #[error("more than two decimal places of a rupee")]
    TooPrecise,
    /// Zero is not an amount to post.
    #[error("the amount is zero")]
    Zero,
    /// Too large to hold in paise.
    #[error("the amount is too large")]
    Overflow,
}

/// Reads an amount the way an accountant types it into paise, deterministically.
///
/// Accepts an optional `₹`, `Rs`, `Rs.` or `INR` prefix, digits with commas (`1,25,000`) and an
/// optional decimal part, an optional unit (`k`/`thousand` = 1,000; `lakh`/`lakhs`/`lac`/`lacs` =
/// 1,00,000; `cr`/`crore`/`crores` = 1,00,00,000) and an optional `/-` suffix. Case is ignored.
///
/// # Errors
/// [`AmountTextError`] when the text is not exactly one such amount, is zero, has more than two
/// decimal places in rupees, or overflows.
pub fn parse_amount_text(text: &str) -> Result<Paise, AmountTextError> {
    let lowered = text.trim().to_lowercase();
    if lowered.is_empty() {
        return Err(AmountTextError::Empty);
    }
    let mut rest = lowered.as_str();
    for prefix in ["₹", "inr", "rs.", "rs"] {
        if let Some(stripped) = rest.strip_prefix(prefix) {
            rest = stripped.trim_start();
            break;
        }
    }
    let rest = rest.strip_suffix("/-").unwrap_or(rest).trim_end();
    let split = rest
        .find(|c: char| !(c.is_ascii_digit() || c == ',' || c == '.'))
        .unwrap_or(rest.len());
    let number = rest.get(..split).unwrap_or_default();
    let unit = rest.get(split..).unwrap_or_default().trim();
    let not_a_number = || AmountTextError::NotANumber(number.to_owned());

    if number.is_empty()
        || number.starts_with([',', '.'])
        || number.ends_with([',', '.'])
        || number.contains(",,")
        || number.matches('.').count() > 1
    {
        return Err(not_a_number());
    }
    if let Some((whole, fraction)) = number.split_once('.')
        && (fraction.contains(',') || whole.is_empty())
    {
        return Err(not_a_number());
    }
    let digits: String = number.chars().filter(|c| *c != ',').collect();
    let value = Decimal::from_str(&digits).map_err(|_| not_a_number())?;
    let multiplier: u32 = match unit {
        "" => 1,
        "k" | "thousand" => 1_000,
        "lakh" | "lakhs" | "lac" | "lacs" => 100_000,
        "cr" | "crore" | "crores" => 10_000_000,
        other => return Err(AmountTextError::UnknownUnit(other.to_owned())),
    };
    let rupees = value
        .checked_mul(Decimal::from(multiplier))
        .ok_or(AmountTextError::Overflow)?;
    if rupees.normalize().scale() > 2 {
        return Err(AmountTextError::TooPrecise);
    }
    if rupees.is_zero() {
        return Err(AmountTextError::Zero);
    }
    // At most two decimal places, so the rounding rule changes nothing; it only converts.
    round_rupees_to_paise(rupees, RoundingRule::Truncate).map_err(|_| AmountTextError::Overflow)
}

/// Reads `DD/MM/YYYY`, `DD-MM-YYYY`, `DD.MM.YYYY` (day first, as written in India) or
/// `YYYY-MM-DD`. Anything else, including two-digit years and words like "yesterday", is `None`
/// (the date stays blank for the user rather than being guessed).
fn parse_date_text(text: &str) -> Option<Date> {
    let text = text.trim();
    let separator = ['/', '-', '.'].into_iter().find(|s| text.contains(*s))?;
    let parts: Vec<&str> = text.split(separator).collect();
    let [a, b, c] = parts.as_slice() else {
        return None;
    };
    let all_digits = |s: &str| !s.is_empty() && s.chars().all(|ch| ch.is_ascii_digit());
    if !(all_digits(a) && all_digits(b) && all_digits(c)) {
        return None;
    }
    let (year, month, day) = if a.len() == 4 && b.len() <= 2 && c.len() <= 2 {
        (a, b, c)
    } else if c.len() == 4 && a.len() <= 2 && b.len() <= 2 {
        (c, b, a)
    } else {
        return None;
    };
    let year: i32 = year.parse().ok()?;
    let month = Month::try_from(month.parse::<u8>().ok()?).ok()?;
    let day: u8 = day.parse().ok()?;
    Date::from_calendar_date(year, month, day).ok()
}

enum Resolution {
    Found(LedgerId),
    Missing,
    Ambiguous(Vec<String>),
}

fn name_words(name: &str) -> Vec<String> {
    name.split_whitespace().map(str::to_lowercase).collect()
}

/// Resolves `name` against the masters: an exact match (case and spacing ignored) on a name or
/// alias wins; otherwise the words typed must be the leading words of exactly one ledger's name or
/// alias ("HDFC" for "HDFC Bank"). Several candidates are ambiguous; none is missing.
fn resolve_ledger(ledgers: &[MasterLedger], name: &str) -> Resolution {
    let wanted = name_words(name);
    if wanted.is_empty() {
        return Resolution::Missing;
    }
    for exact in [true, false] {
        let mut ids: BTreeSet<LedgerId> = BTreeSet::new();
        for ledger in ledgers {
            let matches = std::iter::once(&ledger.name)
                .chain(ledger.aliases.iter())
                .any(|candidate| {
                    let words = name_words(candidate);
                    if exact {
                        words == wanted
                    } else {
                        words.starts_with(&wanted)
                    }
                });
            if matches {
                ids.insert(ledger.id);
            }
        }
        let mut found = ids.into_iter();
        match (found.next(), found.next()) {
            (None, _) => {}
            (Some(id), None) => return Resolution::Found(id),
            (Some(first), Some(second)) => {
                let mut all = vec![first, second];
                all.extend(found);
                let names = all
                    .iter()
                    .filter_map(|id| ledgers.iter().find(|l| l.id == *id))
                    .map(|l| l.name.clone())
                    .collect();
                return Resolution::Ambiguous(names);
            }
        }
    }
    Resolution::Missing
}
