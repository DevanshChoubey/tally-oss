//! The assistant's features through one gate (round s21, plan round C; spec 6.8).
//!
//! [`Api::ai_gate`] decides whether a feature may run now: the company's tier, the feature's
//! switch, and the provider's own status. The provider is the one attached for the session
//! ([`Api::attach_assistant`]), or else the local model at the chosen file, which this build
//! cannot run. Every feature screen asks the gate first and shows its refusal as a typed state.
//!
//! A read that runs is always logged: a draft as `DraftStored` (it then waits in the review
//! queue), a failure as `ReadFailed`. A refused read runs nothing and logs nothing.

use ledgerly_assist::nl_entry::MasterLedger;
use ledgerly_assist::ocr::{RejectedField, read_bill};
use ledgerly_assist::provider::{AiProvider, ProviderInfo, ProviderStatus};
use ledgerly_assist::{AssistError, Proposal};
use ledgerly_core::{CancelToken, EventSeq, Outcome};
use ledgerly_model::{AiActivity, AiActivityKind, AiFeature};
use sha2::{Digest, Sha256};

use crate::ai::AiTier;
use crate::ai_drafts::NewAiDraft;
use crate::error::ApiError;
use crate::facade::Api;

/// The prompt template version the plain-words reader records.
const WORDS_TEMPLATE: &str = ledgerly_assist::nl_entry::PROMPT_TEMPLATE_VERSION;

/// The version the bill reader records in place of a prompt template: OCR lines are read by
/// `ledgerly_assist::ocr::read_bill`, a fixed rule set, not a prompt.
const BILL_READER: &str = "ocr/1";

/// What reading a bill photo gave.
#[derive(Debug, Clone, PartialEq)]
pub enum BillRead {
    /// A purchase draft, stored and waiting for the person's decision.
    Drafted {
        /// The stored draft's id.
        id: EventSeq,
        /// The draft as read.
        proposal: Proposal,
        /// Fields found on the bill but not used, and why.
        rejected: Vec<RejectedField>,
    },
    /// The gate refused; nothing ran and nothing was logged.
    Refused(AiGate),
    /// The provider or the reader could not read the photo; logged as a failed read.
    Failed(String),
}

/// Whether a feature may run now, and if not, why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiGate {
    /// It may run, with this provider.
    Ready(ProviderInfo),
    /// No AI tier is chosen for the company.
    NotChosen,
    /// AI is off for the company.
    Off,
    /// The feature is switched off in AI and privacy.
    FeatureOff,
    /// No model or runtime is installed; the text says what is missing.
    NotInstalled(String),
    /// The provider is not set up; the text says what is missing.
    NotConfigured(String),
}

/// What reading a sentence gave.
#[derive(Debug, Clone, PartialEq)]
pub enum WordsRead {
    /// A draft, stored and waiting for the person's decision.
    Drafted {
        /// The stored draft's id.
        id: EventSeq,
        /// The draft as read.
        proposal: Proposal,
    },
    /// The gate refused; nothing ran and nothing was logged.
    Refused(AiGate),
    /// The provider ran and could not read the sentence; logged as a failed read.
    Failed(String),
}

impl Api {
    /// Whether `feature` may run now (see the module documentation).
    ///
    /// # Errors
    /// [`ApiError::Store`] when the audit log cannot be read.
    pub fn ai_gate(&self, feature: AiFeature) -> Result<AiGate, ApiError> {
        let tier = match self.ai_tier()? {
            None => return Ok(AiGate::NotChosen),
            Some(AiTier::Off) => return Ok(AiGate::Off),
            Some(tier @ (AiTier::Local | AiTier::Cloud)) => tier,
        };
        if !self.ai_feature_on(feature)? {
            return Ok(AiGate::FeatureOff);
        }
        let status = match (self.assistant(), tier) {
            (Some(provider), AiTier::Local | AiTier::Cloud) => {
                return Ok(status_gate(provider.status(), provider.info()));
            }
            (None, AiTier::Local) => self.local_model()?.status(),
            (None, AiTier::Cloud) => ProviderStatus::NotConfigured {
                reason: "this build has no connection to a cloud provider; nothing is sent"
                    .to_owned(),
            },
            (_, AiTier::Off) => return Ok(AiGate::Off),
        };
        Ok(match status {
            ProviderStatus::Ready => AiGate::NotInstalled(
                "this build links no model runtime, so nothing can answer".to_owned(),
            ),
            ProviderStatus::NotInstalled { reason } => AiGate::NotInstalled(reason),
            ProviderStatus::NotConfigured { reason } => AiGate::NotConfigured(reason),
        })
    }

    /// Reads `text` (plain words) into a draft voucher through the gate. A draft is stored and
    /// waits for the person; a failure is logged; a refusal runs and logs nothing.
    ///
    /// # Errors
    /// - [`ApiError::Store`] when the audit log cannot be read or the log entry cannot be
    ///   committed.
    /// - [`ApiError::AiBooksBlocked`] when the books are blocked and give no ledgers.
    pub fn read_words(&mut self, text: &str, cancel: &CancelToken) -> Result<WordsRead, ApiError> {
        let info = match self.ai_gate(AiFeature::PlainWords)? {
            AiGate::Ready(info) => info,
            refused @ (AiGate::NotChosen
            | AiGate::Off
            | AiGate::FeatureOff
            | AiGate::NotInstalled(_)
            | AiGate::NotConfigured(_)) => return Ok(WordsRead::Refused(refused)),
        };
        let ledgers = self.master_ledgers()?;
        let read = match self.assistant() {
            Some(provider) => provider.read_text(text, &ledgers, cancel),
            None => Err(AssistError::Output {
                reason: "no provider is attached".to_owned(),
            }),
        };
        let input_hash = hex(&Sha256::digest(text.as_bytes()));
        match read {
            Ok(proposal) => {
                let confidence_permille = confidence_of(&proposal);
                let id = self.record_ai_draft(NewAiDraft {
                    feature: AiFeature::PlainWords,
                    provider: info.name,
                    model_version: info.version,
                    prompt_template_version: WORDS_TEMPLATE.to_owned(),
                    input_label: text.to_owned(),
                    input_hash,
                    proposal: proposal.clone(),
                    confidence_permille,
                })?;
                Ok(WordsRead::Drafted { id, proposal })
            }
            Err(e) => {
                let reason = e.to_string();
                self.record_ai_activity(AiActivity {
                    feature: AiFeature::PlainWords,
                    provider: info.name,
                    model_version: info.version,
                    prompt_template_version: WORDS_TEMPLATE.to_owned(),
                    input_label: text.to_owned(),
                    input_hash,
                    kind: AiActivityKind::ReadFailed {
                        reason: reason.clone(),
                    },
                })?;
                Ok(WordsRead::Failed(reason))
            }
        }
    }

    /// Reads a bill photo through the gate for bill reading: the provider's OCR lines, then the
    /// bill reader against the company's masters. The image bytes go to the provider only; the
    /// log keeps `label` (the file name) and the SHA-256 of the bytes. A draft is stored and
    /// waits in the review queue; a failure is logged; a refusal runs nothing.
    ///
    /// # Errors
    /// [`ApiError`] when the books cannot be read or the log cannot be written.
    pub fn read_bill_photo(
        &mut self,
        bytes: &[u8],
        label: &str,
        cancel: &CancelToken,
    ) -> Result<BillRead, ApiError> {
        let info = match self.ai_gate(AiFeature::BillReading)? {
            AiGate::Ready(info) => info,
            refused @ (AiGate::NotChosen
            | AiGate::Off
            | AiGate::FeatureOff
            | AiGate::NotInstalled(_)
            | AiGate::NotConfigured(_)) => return Ok(BillRead::Refused(refused)),
        };
        let input_hash = hex(&Sha256::digest(bytes));
        let ocr = match self.assistant() {
            Some(provider) => provider.ocr(bytes, &input_hash, cancel),
            None => Err(AssistError::Output {
                reason: "no provider is attached".to_owned(),
            }),
        };
        let read = match ocr {
            Ok(ocr) => match self.books()? {
                Outcome::Value(books) => read_bill(&ocr, &books.masters),
                Outcome::Unsupported(u) => Err(AssistError::Unsupported(u)),
            },
            Err(e) => Err(e),
        };
        match read {
            Ok(reading) => {
                let proposal = Proposal::VoucherDraft(Box::new(reading.draft));
                let confidence_permille = confidence_of(&proposal);
                let id = self.record_ai_draft(NewAiDraft {
                    feature: AiFeature::BillReading,
                    provider: info.name,
                    model_version: info.version,
                    prompt_template_version: BILL_READER.to_owned(),
                    input_label: label.to_owned(),
                    input_hash,
                    proposal: proposal.clone(),
                    confidence_permille,
                })?;
                Ok(BillRead::Drafted {
                    id,
                    proposal,
                    rejected: reading.rejected,
                })
            }
            Err(e) => {
                let reason = e.to_string();
                self.record_ai_activity(AiActivity {
                    feature: AiFeature::BillReading,
                    provider: info.name,
                    model_version: info.version,
                    prompt_template_version: BILL_READER.to_owned(),
                    input_label: label.to_owned(),
                    input_hash,
                    kind: AiActivityKind::ReadFailed {
                        reason: reason.clone(),
                    },
                })?;
                Ok(BillRead::Failed(reason))
            }
        }
    }

    /// The company's ledgers as the readers resolve names against them.
    fn master_ledgers(&self) -> Result<Vec<MasterLedger>, ApiError> {
        let books = match self.books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => return Err(ApiError::AiBooksBlocked { reason: u.reason }),
        };
        Ok(books
            .masters
            .ledgers
            .values()
            .map(|l| MasterLedger {
                id: l.id,
                name: l.name.clone(),
                aliases: l.aliases.clone(),
            })
            .collect())
    }
}

/// The gate for an attached provider's status.
fn status_gate(status: ProviderStatus, info: ProviderInfo) -> AiGate {
    match status {
        ProviderStatus::Ready => AiGate::Ready(info),
        ProviderStatus::NotInstalled { reason } => AiGate::NotInstalled(reason),
        ProviderStatus::NotConfigured { reason } => AiGate::NotConfigured(reason),
    }
}

/// A draft's overall confidence: the lowest of the confidences the reader gave its fields.
fn confidence_of(proposal: &Proposal) -> u16 {
    match proposal {
        Proposal::VoucherDraft(d) => d
            .lines
            .iter()
            .flat_map(|l| {
                [
                    l.amount.confidence_permille,
                    l.ledger_name.confidence_permille,
                ]
            })
            .chain(d.document_total.iter().map(|t| t.confidence_permille))
            .chain(d.date.iter().map(|t| t.confidence_permille))
            .min()
            .unwrap_or(0),
        Proposal::MatchSuggestion(_)
        | Proposal::ReportQuery(_)
        | Proposal::MappingSuggestion(_) => 0,
    }
}

/// Lower-case hex of `bytes`.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::panic,
        reason = "test code: rule B2 lets tests unwrap and panic"
    )]

    use super::*;
    use crate::facade::Cache;
    use crate::{AiTier, FixedAnswers};
    use ledgerly_core::{CancelToken, Clock, CompanyId, StateCode, Unsupported};
    use ledgerly_model::{Address, Company, EntityKind, Features, NegativeStockPolicy};
    use time::macros::{date, datetime};

    #[test]
    fn blocked_books_refuse_a_read_with_their_own_error_not_the_key_account_one() {
        // provenance: derived. `Api::books` returns the store's Unsupported when a voucher cannot
        // be replayed (facade.rs `reload`); the readers then have no ledgers, which is not about
        // the cloud key's keychain account.
        let dir = tempfile::tempdir().unwrap();
        let company = Company {
            ineligible_itc_default: None,
            id: CompanyId(1),
            name: "Fictional Traders".to_owned(),
            mailing_name: None,
            address: Address::default(),
            state: StateCode::new(29).unwrap(),
            pan: None,
            registrations: Vec::new(),
            fy_start: date!(2026 - 04 - 01),
            books_begin: date!(2026 - 04 - 01),
            entity_kind: EntityKind::Company,
            features: Features::default(),
            negative_stock: NegativeStockPolicy::Block,
        };
        let mut api = Api::create(
            &dir.path().join("blocked.ledgerly"),
            &company,
            Clock::Fixed(datetime!(2026-11-21 10:00 UTC)),
        )
        .unwrap();
        api.set_ai_tier(AiTier::Local).unwrap();
        api.attach_assistant(Box::new(FixedAnswers::new()));
        api.cache = Cache::Blocked(Unsupported {
            what: "voucher 7".to_owned(),
            reason: "a stock journal records no side".to_owned(),
        });
        match api.read_words("paid rent 25k", &CancelToken::new()) {
            Err(ApiError::AiBooksBlocked { reason }) => {
                assert_eq!(reason, "a stock journal records no side");
            }
            other => panic!("expected AiBooksBlocked, got {other:?}"),
        }
    }
}
