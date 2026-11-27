//! The per-company AI tier and the AI activity log (card W4, spec 6.8).
//!
//! The tier is kept in the company file as a `SettingChanged` event under the key
//! [`AI_TIER_KEY`]; the last such event in sequence order is the tier in force. Every change is
//! an audited event. The cloud tier is refused: its API key must live in the OS keychain (spec
//! 9), and no key store is wired into the app, so the company file never holds a key.
//!
//! The activity log is the `AiProposalRecorded` events of the company file. Nothing writes those
//! yet (no assistant feature is wired), so the log is empty until one does.

use std::io::Write;
use std::path::Path;

use ledgerly_core::{Outcome, Unsupported};
use ledgerly_store::Event;
use time::OffsetDateTime;
use time::macros::format_description;

use crate::error::ApiError;
use crate::events::read_all;
use crate::facade::Api;

/// The setting key under which the AI tier is stored.
const AI_TIER_KEY: &str = "ai.tier";

/// The AI tier of a company (spec 6.8). `Cloud` is named so the file can record it, but the
/// facade refuses to set it until a key store exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiTier {
    /// AI is off for this company.
    Off,
    /// The local model on this computer. Nothing leaves the computer.
    Local,
    /// A cloud provider with the user's own key.
    Cloud,
}

impl AiTier {
    /// The words the tier is stored as in the company file.
    #[must_use]
    pub fn stored(self) -> &'static str {
        match self {
            AiTier::Off => "off",
            AiTier::Local => "local",
            AiTier::Cloud => "cloud",
        }
    }

    /// The tier a stored word names, or `None` for a word this build does not know.
    #[must_use]
    pub fn from_stored(word: &str) -> Option<Self> {
        match word {
            "off" => Some(AiTier::Off),
            "local" => Some(AiTier::Local),
            "cloud" => Some(AiTier::Cloud),
            _ => None,
        }
    }
}

/// One recorded assistant proposal and the user's response, as the log shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiLogEntry {
    /// When the proposal was recorded.
    pub at: OffsetDateTime,
    /// The model that produced it.
    pub model: String,
    /// SHA-256 of the input, hex.
    pub input_hash: String,
    /// The proposal as produced, JSON.
    pub output_json: String,
    /// Whether the user accepted it.
    pub accepted: bool,
}

impl Api {
    /// The AI tier stored in the company file, or `None` when none was ever chosen.
    ///
    /// # Errors
    /// - [`ApiError::Store`] when the audit log cannot be read.
    /// - [`ApiError::UnknownAiTier`] when the last stored tier is a word this build does not know.
    pub fn ai_tier(&self) -> Result<Option<AiTier>, ApiError> {
        let records = read_all(self.store())?;
        let last = records
            .iter()
            .rev()
            .find_map(|record| tier_word(&record.event));
        match last {
            None => Ok(None),
            Some(word) => {
                AiTier::from_stored(word)
                    .map(Some)
                    .ok_or_else(|| ApiError::UnknownAiTier {
                        value: word.clone(),
                    })
            }
        }
    }

    /// Stores `tier` for the company as an audited setting change. Choosing the tier already
    /// stored writes nothing. `Cloud` is `Unsupported`, and nothing is written, until a key vault
    /// is attached, the provider is described and its key is saved ([`Api::ai_settings`]).
    ///
    /// # Errors
    /// - [`ApiError::Store`] when the tier cannot be read or the change cannot be committed.
    /// - [`ApiError::UnknownAiTier`] when the stored tier is a word this build does not know.
    pub fn set_ai_tier(&mut self, tier: AiTier) -> Result<Outcome<()>, ApiError> {
        if tier == AiTier::Cloud
            && let Some(refusal) = self.cloud_refusal()?
        {
            return Ok(Outcome::Unsupported(refusal));
        }
        let before = self.ai_tier()?;
        if before == Some(tier) {
            return Ok(Outcome::Value(()));
        }
        let event = Event::SettingChanged {
            key: AI_TIER_KEY.to_owned(),
            before: before.map_or("", AiTier::stored).to_owned(),
            after: tier.stored().to_owned(),
        };
        let actor = self.actor().clone();
        self.commit_as(&[event], &actor)?;
        Ok(Outcome::Value(()))
    }

    /// Every recorded assistant proposal, in the order it was recorded. Empty when none was.
    ///
    /// # Errors
    /// [`ApiError::Store`] when the audit log cannot be read.
    pub fn ai_log(&self) -> Result<Vec<AiLogEntry>, ApiError> {
        let records = read_all(self.store())?;
        Ok(records
            .into_iter()
            .filter_map(|record| match record.event {
                Event::AiProposalRecorded(p) => Some(AiLogEntry {
                    at: record.meta.at,
                    model: p.model,
                    input_hash: p.input_hash,
                    output_json: p.output_json,
                    accepted: p.accepted,
                }),
                Event::MasterCreated(_)
                | Event::MasterAltered { .. }
                | Event::MasterDeleted(_)
                | Event::VoucherPosted(_)
                | Event::VoucherAltered { .. }
                | Event::VoucherCancelled { .. }
                | Event::VoucherDeleted { .. }
                | Event::SettingChanged { .. }
                | Event::PeriodLocked { .. }
                | Event::StatementImported(_)
                | Event::MatchConfirmed(_)
                | Event::BankStatementStored { .. }
                | Event::Gstr2bImported { .. }
                | Event::ImsActionRecorded { .. }
                | Event::Gstr3bFiled { .. }
                | Event::AiActivityRecorded(_) => None,
            })
            .collect())
    }

    /// Writes the AI activity log to `path` as CSV: a header line, then one line per entry with
    /// the time, model, input hash, output and the user's decision. Nothing is written when the
    /// log is empty.
    ///
    /// # Errors
    /// - [`ApiError::Store`] when the audit log cannot be read.
    /// - [`ApiError::AiLogWrite`] when the file cannot be written.
    pub fn export_ai_log(&self, path: &Path) -> Result<Outcome<()>, ApiError> {
        let entries = self.ai_log()?;
        if entries.is_empty() {
            return Ok(Outcome::Unsupported(Unsupported {
                what: "AI activity log export".to_owned(),
                reason: "no AI event has been recorded for this company yet".to_owned(),
            }));
        }
        let text = csv_text(&entries).map_err(|source| ApiError::AiLogWrite {
            path: path.to_path_buf(),
            source,
        })?;
        std::fs::File::create(path)
            .and_then(|mut file| file.write_all(text.as_bytes()))
            .map_err(|source| ApiError::AiLogWrite {
                path: path.to_path_buf(),
                source,
            })?;
        Ok(Outcome::Value(()))
    }
}

/// The stored word of an AI tier setting change, or `None` for any other event.
fn tier_word(event: &Event) -> Option<&String> {
    match event {
        Event::SettingChanged { key, after, .. } if key == AI_TIER_KEY => Some(after),
        Event::SettingChanged { .. }
        | Event::MasterCreated(_)
        | Event::MasterAltered { .. }
        | Event::MasterDeleted(_)
        | Event::VoucherPosted(_)
        | Event::VoucherAltered { .. }
        | Event::VoucherCancelled { .. }
        | Event::VoucherDeleted { .. }
        | Event::PeriodLocked { .. }
        | Event::StatementImported(_)
        | Event::MatchConfirmed(_)
        | Event::AiProposalRecorded(_)
        | Event::BankStatementStored { .. }
        | Event::Gstr2bImported { .. }
        | Event::ImsActionRecorded { .. }
        | Event::Gstr3bFiled { .. }
        | Event::AiActivityRecorded(_) => None,
    }
}

/// The CSV text of the log: the header and one line per entry.
fn csv_text(entries: &[AiLogEntry]) -> Result<String, std::io::Error> {
    let when = format_description!("[year]-[month]-[day] [hour]:[minute]");
    let mut out = String::from("When,Model,Input hash,Output,Your decision\n");
    for entry in entries {
        let at = entry.at.format(&when).map_err(std::io::Error::other)?;
        let decision = if entry.accepted {
            "Accepted"
        } else {
            "Not accepted"
        };
        let cells = [
            at.as_str(),
            entry.model.as_str(),
            entry.input_hash.as_str(),
            entry.output_json.as_str(),
            decision,
        ];
        let line: Vec<String> = cells.iter().map(|c| csv_field(c)).collect();
        out.push_str(&line.join(","));
        out.push('\n');
    }
    Ok(out)
}

/// One CSV field: wrapped in quotes when it holds a comma, a quote or a line break, with each
/// quote inside it doubled (RFC 4180).
fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}
