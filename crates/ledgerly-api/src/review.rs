//! The Review queue's list (mock `review`, `SCREENS.review`): the proposals waiting for a
//! decision, from every source the engine can list, and the sources it cannot list yet.
//!
//! Frozen interface:
//!
//! ```text
//! impl Api { pub fn review_queue(&self) -> Result<ReviewQueue, ApiError>; }
//! ```
//!
//! The Home card and the Review screen both call [`Api::review_queue`], so they show the same
//! items. Nothing here is confirmed: a bank match is accepted through `Api::confirm_bank_match`.
//! The list uses the same suggestions as the bank screen (newest stored statement per bank
//! ledger, no match rules), so the two agree.

use std::collections::BTreeMap;

use ledgerly_bank::matcher::Candidate;
use ledgerly_bank::statement::BankStatement;
use ledgerly_bankfmt::RawLine;
use ledgerly_core::{BankStatementId, Date, LedgerId, Outcome, Paise};

use crate::error::ApiError;
use crate::facade::Api;
use crate::nudges::DuplicatePayment;

/// A source of proposals that the Review queue names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewSource {
    /// A draft purchase or sale read from a bill photo.
    BillDraft,
    /// A draft voucher written in plain words.
    PlainWordsDraft,
    /// A bank match suggested by the matcher (connected).
    BankSuggestion,
    /// A new ledger suggested for an unexplained bank line.
    NewLedgerSuggestion,
    /// A possible duplicate voucher (rule D-02).
    DuplicateCheck,
    /// A GSTR-2B mismatch explained for a purchase record.
    Gstr2bExplanation,
}

/// A source the queue does not list, with the reason the engine gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlistedSource {
    /// The source that is not listed.
    pub source: ReviewSource,
    /// Why it is not listed, in words the screen shows as they are.
    pub reason: String,
}

/// One open proposal in the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewItem {
    /// The stored statement the proposal came from.
    pub statement: BankStatementId,
    /// The bank ledger of that statement.
    pub ledger: LedgerId,
    /// The matcher's suggestion, unchanged.
    pub candidate: Candidate,
    /// The statement line text (joined with " + " for a multi-line suggestion).
    pub description: String,
    /// The amount of the statement line, when the suggestion covers exactly one line.
    pub amount: Option<Paise>,
    /// True when the one covered line is a deposit (money in), false for a withdrawal.
    pub money_in: bool,
}

/// The Review queue: the listed proposals and the sources that are not listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewQueue {
    /// The proposals, grouped by bank ledger in ledger order, then in the matcher's order.
    pub items: Vec<ReviewItem>,
    /// The possible duplicate payments rule D-02 finds, oldest second payment first. Empty both
    /// when there are none and when D-02 could not run (then `unlisted` names it).
    pub duplicates: Vec<DuplicatePayment>,
    /// The sources the engine cannot list, each with its reason. Empty only when every source is
    /// connected.
    pub unlisted: Vec<UnlistedSource>,
}

impl ReviewQueue {
    /// The number of open proposals, when every source is listed. `None` while any source is
    /// unlisted: the count would then be a partial number, and the queue never shows one.
    #[must_use]
    pub fn count(&self) -> Option<usize> {
        if self.unlisted.is_empty() {
            Some(self.items.len() + self.duplicates.len())
        } else {
            None
        }
    }
}

/// The reason each source that the engine cannot list gives. Each is a fact about the engine,
/// not a guess about the user's data.
fn not_connected(source: ReviewSource) -> &'static str {
    match source {
        ReviewSource::BillDraft => {
            "bill photos are not read yet, so no draft purchase is stored to list"
        }
        ReviewSource::PlainWordsDraft => {
            "plain-words entry needs the assistant, which is not built, so no draft is stored"
        }
        ReviewSource::BankSuggestion => {
            "the matcher's suggestions could not be read for the bank statements"
        }
        ReviewSource::NewLedgerSuggestion => {
            "the engine does not suggest new ledgers for unexplained bank lines yet"
        }
        ReviewSource::DuplicateCheck => {
            "rule D-02 could not be run over the books, so no possible duplicate payments are listed"
        }
        ReviewSource::Gstr2bExplanation => {
            "there is no GSTR-2B reader or IMS reconciliation yet (engine item E-3)"
        }
    }
}

/// The sources listed in the queue's fixed order when they are not connected.
const NOT_CONNECTED: [ReviewSource; 4] = [
    ReviewSource::BillDraft,
    ReviewSource::PlainWordsDraft,
    ReviewSource::NewLedgerSuggestion,
    ReviewSource::Gstr2bExplanation,
];

impl Api {
    /// The Review queue: the open bank-match proposals of the newest stored statement of each
    /// bank ledger, and the sources the engine cannot list yet.
    ///
    /// Nothing is confirmed here. A source whose suggestions the books cannot give is named in
    /// `unlisted` with the engine's reason; its rows are not made up.
    ///
    /// # Errors
    /// Whatever [`Api::bank_suggestions`] returns as an error (the stored statements cannot be
    /// read, or the session no longer holds a statement).
    pub fn review_queue(&self) -> Result<ReviewQueue, ApiError> {
        self.review_queue_on(self.clock_date())
    }

    /// [`Api::review_queue`] as it stands on `today`: rule D-02 counts a post-dated payment only
    /// from its own date, so it needs the day.
    ///
    /// # Errors
    /// The same as [`Api::review_queue`].
    pub fn review_queue_on(&self, today: Date) -> Result<ReviewQueue, ApiError> {
        let mut newest: BTreeMap<LedgerId, (BankStatementId, &BankStatement)> = BTreeMap::new();
        for (id, statement) in self.held()? {
            newest.insert(statement.ledger(), (*id, statement));
        }
        let mut items = Vec::new();
        let mut unlisted: Vec<UnlistedSource> = NOT_CONNECTED
            .iter()
            .map(|source| UnlistedSource {
                source: *source,
                reason: not_connected(*source).to_owned(),
            })
            .collect();
        let mut bank_unlisted = false;
        for (ledger, (id, statement)) in &newest {
            match self.bank_suggestions(*id, &[])? {
                Outcome::Value(candidates) => {
                    for candidate in candidates {
                        items.push(item_of(*id, *ledger, statement.lines(), candidate));
                    }
                }
                Outcome::Unsupported(u) => {
                    if !bank_unlisted {
                        bank_unlisted = true;
                        unlisted.push(UnlistedSource {
                            source: ReviewSource::BankSuggestion,
                            reason: u.reason,
                        });
                    }
                }
            }
        }
        let duplicates = match self.duplicate_payments_on(today)? {
            Outcome::Value(found) => found,
            Outcome::Unsupported(u) => {
                unlisted.push(UnlistedSource {
                    source: ReviewSource::DuplicateCheck,
                    reason: format!(
                        "{}: {}",
                        not_connected(ReviewSource::DuplicateCheck),
                        u.reason
                    ),
                });
                Vec::new()
            }
        };
        Ok(ReviewQueue {
            items,
            duplicates,
            unlisted,
        })
    }
}

/// One queue row for a matcher suggestion, with the text and amount of the lines it covers.
fn item_of(
    statement: BankStatementId,
    ledger: LedgerId,
    lines: &[RawLine],
    candidate: Candidate,
) -> ReviewItem {
    let covered: Vec<&RawLine> = candidate
        .statement_lines
        .iter()
        .filter_map(|row| lines.iter().find(|line| line.row == *row))
        .collect();
    let description = covered
        .iter()
        .map(|line| line.description.as_str())
        .collect::<Vec<_>>()
        .join(" + ");
    let (amount, money_in) = match covered.as_slice() {
        [only] => {
            let money_in = !only.credit.is_zero();
            let amount = if money_in { only.credit } else { only.debit };
            (Some(amount), money_in)
        }
        _ => (None, false),
    };
    ReviewItem {
        statement,
        ledger,
        candidate,
        description,
        amount,
        money_in,
    }
}
