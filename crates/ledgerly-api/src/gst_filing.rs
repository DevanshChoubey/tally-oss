//! The company's GST return frequency (round S9).
//!
//! Kept in the company file as a `SettingChanged` event under [`GST_FILING_KEY`]; the last such
//! event in sequence order is the frequency in force. `None` means none was ever chosen.

use ledgerly_core::Outcome;
use ledgerly_store::Event;

use crate::error::ApiError;
use crate::events::read_all;
use crate::facade::Api;

/// The setting key under which the filing frequency is stored.
const GST_FILING_KEY: &str = "gst.filing";

/// How often the company files its GST returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GstFiling {
    /// A return every month.
    Monthly,
    /// A return every quarter (the quarterly scheme for small taxpayers).
    Quarterly,
}

impl GstFiling {
    /// The word the frequency is stored as in the company file.
    #[must_use]
    pub fn stored(self) -> &'static str {
        match self {
            GstFiling::Monthly => "monthly",
            GstFiling::Quarterly => "quarterly",
        }
    }

    /// The frequency a stored word names, or `None` for a word this build does not know.
    #[must_use]
    pub fn from_stored(word: &str) -> Option<Self> {
        match word {
            "monthly" => Some(GstFiling::Monthly),
            "quarterly" => Some(GstFiling::Quarterly),
            _ => None,
        }
    }

    /// The word the screen shows.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            GstFiling::Monthly => "Monthly",
            GstFiling::Quarterly => "Quarterly",
        }
    }
}

impl Api {
    /// The filing frequency stored in the company file, or `None` when none was chosen.
    ///
    /// # Errors
    /// - [`ApiError::Store`] when the log cannot be read.
    /// - [`ApiError::UnknownGstFiling`] when the stored word is not one this build knows.
    pub fn gst_filing(&self) -> Result<Option<GstFiling>, ApiError> {
        let records = read_all(self.store())?;
        let last = records.iter().rev().find_map(|record| match &record.event {
            Event::SettingChanged { key, after, .. } if key == GST_FILING_KEY => Some(after),
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
            | Event::BankStatementStored { .. } => None,
        });
        match last {
            None => Ok(None),
            Some(word) => {
                GstFiling::from_stored(word)
                    .map(Some)
                    .ok_or_else(|| ApiError::UnknownGstFiling {
                        value: word.clone(),
                    })
            }
        }
    }

    /// Stores `filing` for the company as an audited setting change. Choosing the frequency
    /// already stored writes nothing.
    ///
    /// # Errors
    /// - [`ApiError::Store`] when the log cannot be read or the change cannot be committed.
    /// - [`ApiError::UnknownGstFiling`] when the stored word is not one this build knows.
    pub fn set_gst_filing(&mut self, filing: GstFiling) -> Result<Outcome<()>, ApiError> {
        let before = self.gst_filing()?;
        if before == Some(filing) {
            return Ok(Outcome::Value(()));
        }
        let event = Event::SettingChanged {
            key: GST_FILING_KEY.to_owned(),
            before: before.map_or("", GstFiling::stored).to_owned(),
            after: filing.stored().to_owned(),
        };
        let actor = self.actor().clone();
        self.commit_as(&[event], &actor)?;
        Ok(Outcome::Value(()))
    }
}
