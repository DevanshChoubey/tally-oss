//! The GSTR-2B reconciliation and the user's Invoice Management System (IMS) actions (round s11).
//!
//! A 2B file is kept in the company file as a `Gstr2bImported` event, exactly as imported. The
//! rows are recomputed from the books each time, so a change in the books shows at once. The
//! user's IMS action on a document is a `ImsActionRecorded` event; the last one for a document
//! is the action in force. Marking GSTR-3B filed for a period (`Gstr3bFiled`) makes that
//! period's file and actions final.

use ledgerly_core::{Date, Gstin, Outcome, Paise, Period, VoucherId};
use ledgerly_gst::recon2b::{
    DocAmounts, ImsAction, MatchLevel, ReconState, portal_documents, reconcile_2b,
};
use ledgerly_gst_formats::gstr2b::{Gstr2b, parse_gstr2b};
use ledgerly_store::{AuditRecord, Event, StoredImsAction};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use time::Month;

use crate::error::ApiError;
use crate::events::read_all;
use crate::facade::Api;

/// One row of the reconciliation: a document of the 2B file and how the books compare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gstr2bRow {
    /// The supplier's GSTIN as the file writes it.
    pub supplier_gstin: String,
    /// The supplier's name: the books' ledger with that GSTIN, else the file's trade name, else
    /// the GSTIN.
    pub supplier_name: String,
    /// The document number as the file writes it.
    pub doc_number: String,
    /// How the document compares with the books.
    pub state: ReconState,
    /// How well the matcher matched it.
    pub level: MatchLevel,
    /// The action the engine suggests.
    pub suggested: ImsAction,
    /// The action the user recorded, `None` when none was.
    pub action: Option<ImsAction>,
    /// The books side, `None` when no voucher is paired.
    pub books: Option<DocAmounts>,
    /// The 2B side.
    pub portal: DocAmounts,
    /// 2B total less books total, `None` unless both are known.
    pub difference: Option<Paise>,
    /// The voucher paired with the document.
    pub voucher: Option<VoucherId>,
}

/// The reconciliation of one return period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gstr2bReconciliation {
    /// The name of the imported file, `None` when none was imported for the period.
    pub file_name: Option<String>,
    /// One row per document of the file, in file order.
    pub rows: Vec<Gstr2bRow>,
    /// The day GSTR-3B for the period was marked filed, `None` when it was not.
    pub filed_on: Option<Date>,
}

fn stored(action: ImsAction) -> StoredImsAction {
    match action {
        ImsAction::Accept => StoredImsAction::Accept,
        ImsAction::Reject => StoredImsAction::Reject,
        ImsAction::Pending => StoredImsAction::Pending,
    }
}

fn shown(action: StoredImsAction) -> ImsAction {
    match action {
        StoredImsAction::Accept => ImsAction::Accept,
        StoredImsAction::Reject => ImsAction::Reject,
        StoredImsAction::Pending => ImsAction::Pending,
    }
}

fn month_name(month: Month) -> &'static str {
    match month {
        Month::January => "January",
        Month::February => "February",
        Month::March => "March",
        Month::April => "April",
        Month::May => "May",
        Month::June => "June",
        Month::July => "July",
        Month::August => "August",
        Month::September => "September",
        Month::October => "October",
        Month::November => "November",
        Month::December => "December",
    }
}

pub(crate) fn same_gstin(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// The last 2B import of the company for the period, as (file name, file text).
pub(crate) fn last_import<'a>(
    records: &'a [AuditRecord],
    gstin: &Gstin,
    period: Period,
) -> Option<(&'a str, &'a str)> {
    records.iter().rev().find_map(|r| match &r.event {
        Event::Gstr2bImported {
            gstin: g,
            period_start,
            file_name,
            json,
            ..
        } if same_gstin(g, gstin.as_str()) && *period_start == period.from() => {
            Some((file_name.as_str(), json.as_str()))
        }
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
        | Event::AiProposalRecorded(_)
        | Event::BankStatementStored { .. }
        | Event::Gstr2bImported { .. }
        | Event::ImsActionRecorded { .. }
        | Event::Gstr3bFiled { .. } => None,
    })
}

pub(crate) fn filed_on(records: &[AuditRecord], gstin: &Gstin, period: Period) -> Option<Date> {
    records.iter().rev().find_map(|r| match &r.event {
        Event::Gstr3bFiled {
            gstin: g,
            period_start,
            filed_on,
        } if same_gstin(g, gstin.as_str()) && *period_start == period.from() => Some(*filed_on),
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
        | Event::AiProposalRecorded(_)
        | Event::BankStatementStored { .. }
        | Event::Gstr2bImported { .. }
        | Event::ImsActionRecorded { .. }
        | Event::Gstr3bFiled { .. } => None,
    })
}

impl Api {
    /// Reads and keeps a 2B file for `period`, replacing an earlier file of the same period.
    /// Returns the number of documents in the file. Nothing is kept when the file is refused.
    ///
    /// # Errors
    /// - [`ApiError::GstrPeriodFiled`] when GSTR-3B for the period was marked filed.
    /// - [`ApiError::Format`] when the file cannot be read as a 2B file.
    /// - [`ApiError::Gstr2bWrongGstin`] when the file is for another GSTIN than `gstin`.
    /// - [`ApiError::Gstr2bWrongPeriod`] when the file is for another month than `period`'s.
    /// - [`ApiError::Store`] when the log cannot be read or the import cannot be committed.
    pub fn import_gstr2b(
        &mut self,
        gstin: &Gstin,
        period: Period,
        file_name: &str,
        json: &str,
    ) -> Result<Outcome<usize>, ApiError> {
        let records = read_all(self.store())?;
        if let Some(filed) = filed_on(&records, gstin, period) {
            return Err(ApiError::GstrPeriodFiled {
                period_start: period.from(),
                filed_on: filed,
            });
        }
        let data = parse_gstr2b(json)?;
        if !same_gstin(&data.gstin, gstin.as_str()) {
            return Err(ApiError::Gstr2bWrongGstin {
                file: data.gstin,
                company: gstin.to_string(),
            });
        }
        let from = period.from();
        let wanted = data.return_period;
        if wanted.year != from.year() || wanted.month != from.month() {
            return Err(ApiError::Gstr2bWrongPeriod {
                file: format!("{} {}", month_name(wanted.month), wanted.year),
                wanted: format!("{} {}", month_name(from.month()), from.year()),
            });
        }
        let documents = portal_documents(&data).len();
        let digest = Sha256::digest(json.as_bytes());
        let file_hash: String = digest.iter().fold(String::new(), |mut acc, b| {
            let _ = write!(acc, "{b:02x}");
            acc
        });
        let event = Event::Gstr2bImported {
            gstin: gstin.to_string(),
            period_start: from,
            file_name: file_name.to_owned(),
            file_hash,
            json: json.to_owned(),
        };
        let actor = self.actor().clone();
        self.commit_as(&[event], &actor)?;
        Ok(Outcome::Value(documents))
    }

    /// The reconciliation of `period`: one row per document of the imported 2B file, each
    /// compared with the books to the paisa. With no file imported the rows are empty.
    ///
    /// # Errors
    /// - [`ApiError::Format`] when the stored file cannot be read again.
    /// - [`ApiError::Gst`] when the matcher fails.
    /// - [`ApiError::Store`] when the log cannot be read.
    /// - [`ApiError::Stale`] after a saved change whose reload failed.
    pub fn gstr2b_reconciliation(
        &self,
        gstin: &Gstin,
        period: Period,
    ) -> Result<Outcome<Gstr2bReconciliation>, ApiError> {
        let records = read_all(self.store())?;
        let filed = filed_on(&records, gstin, period);
        let Some((file_name, json)) = last_import(&records, gstin, period) else {
            return Ok(Outcome::Value(Gstr2bReconciliation {
                file_name: None,
                rows: Vec::new(),
                filed_on: filed,
            }));
        };
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        let data = parse_gstr2b(json)?;
        let entries = reconcile_2b(books, gstin, &data, Paise::ZERO)?;
        let documents = portal_documents(&data);
        let mut rows = Vec::with_capacity(entries.len());
        for entry in entries {
            let Some((ctin, doc)) = documents.get(entry.line.portal_index) else {
                continue;
            };
            let action = records.iter().rev().find_map(|r| match &r.event {
                Event::ImsActionRecorded {
                    gstin: g,
                    period_start,
                    supplier_gstin,
                    doc_number,
                    action,
                } if same_gstin(g, gstin.as_str())
                    && *period_start == period.from()
                    && same_gstin(supplier_gstin, ctin)
                    && *doc_number == doc.number =>
                {
                    Some(shown(*action))
                }
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
                | Event::AiProposalRecorded(_)
                | Event::BankStatementStored { .. }
                | Event::Gstr2bImported { .. }
                | Event::ImsActionRecorded { .. }
                | Event::Gstr3bFiled { .. } => None,
            });
            rows.push(Gstr2bRow {
                supplier_gstin: (*ctin).to_owned(),
                supplier_name: supplier_name(books, &data, ctin),
                doc_number: doc.number.clone(),
                state: entry.state,
                level: entry.line.level,
                suggested: entry.line.suggested,
                action,
                books: entry.books,
                portal: entry.portal,
                difference: entry.difference,
                voucher: entry.line.voucher,
            });
        }
        Ok(Outcome::Value(Gstr2bReconciliation {
            file_name: Some(file_name.to_owned()),
            rows,
            filed_on: filed,
        }))
    }

    /// Records the user's IMS action on one document of the period's 2B file. The last action
    /// recorded for a document is the one in force.
    ///
    /// # Errors
    /// - [`ApiError::GstrPeriodFiled`] when GSTR-3B for the period was marked filed.
    /// - [`ApiError::Gstr2bNotImported`] when no file was imported for the period.
    /// - [`ApiError::ImsDocumentNotInFile`] when the file has no such document.
    /// - [`ApiError::Format`] when the stored file cannot be read again.
    /// - [`ApiError::Store`] when the log cannot be read or the action cannot be committed.
    pub fn set_ims_action(
        &mut self,
        gstin: &Gstin,
        period: Period,
        supplier_gstin: &str,
        doc_number: &str,
        action: ImsAction,
    ) -> Result<Outcome<()>, ApiError> {
        let records = read_all(self.store())?;
        if let Some(filed) = filed_on(&records, gstin, period) {
            return Err(ApiError::GstrPeriodFiled {
                period_start: period.from(),
                filed_on: filed,
            });
        }
        let Some((_, json)) = last_import(&records, gstin, period) else {
            return Err(ApiError::Gstr2bNotImported {
                period_start: period.from(),
            });
        };
        let data = parse_gstr2b(json)?;
        let present = portal_documents(&data)
            .iter()
            .any(|(ctin, doc)| same_gstin(ctin, supplier_gstin) && doc.number == doc_number);
        if !present {
            return Err(ApiError::ImsDocumentNotInFile {
                supplier_gstin: supplier_gstin.to_owned(),
                doc_number: doc_number.to_owned(),
            });
        }
        let event = Event::ImsActionRecorded {
            gstin: gstin.to_string(),
            period_start: period.from(),
            supplier_gstin: supplier_gstin.to_owned(),
            doc_number: doc_number.to_owned(),
            action: stored(action),
        };
        let actor = self.actor().clone();
        self.commit_as(&[event], &actor)?;
        Ok(Outcome::Value(()))
    }

    /// Marks GSTR-3B filed for `period` on `filed_on`. The period's 2B file and IMS actions are
    /// final from then on.
    ///
    /// # Errors
    /// - [`ApiError::GstrPeriodFiled`] when the period was already marked filed.
    /// - [`ApiError::Store`] when the log cannot be read or the mark cannot be committed.
    pub fn mark_gstr3b_filed(
        &mut self,
        gstin: &Gstin,
        period: Period,
        filed_on_day: Date,
    ) -> Result<Outcome<()>, ApiError> {
        let records = read_all(self.store())?;
        if let Some(filed) = filed_on(&records, gstin, period) {
            return Err(ApiError::GstrPeriodFiled {
                period_start: period.from(),
                filed_on: filed,
            });
        }
        let event = Event::Gstr3bFiled {
            gstin: gstin.to_string(),
            period_start: period.from(),
            filed_on: filed_on_day,
        };
        let actor = self.actor().clone();
        self.commit_as(&[event], &actor)?;
        Ok(Outcome::Value(()))
    }
}

/// The name shown for a supplier: the books' ledger with that GSTIN, else the file's trade name,
/// else the GSTIN itself.
fn supplier_name(books: &ledgerly_model::Books, data: &Gstr2b, ctin: &str) -> String {
    let ledger = books.masters.ledgers.values().find(|l| {
        l.party
            .as_ref()
            .and_then(|p| p.gstin.as_ref())
            .is_some_and(|g| same_gstin(g.as_str(), ctin))
    });
    if let Some(l) = ledger {
        return l.name.clone();
    }
    let trade = data
        .b2b
        .iter()
        .chain(data.cdnr.iter())
        .find(|g| same_gstin(&g.ctin, ctin))
        .and_then(|g| g.trade_name.clone());
    trade.unwrap_or_else(|| ctin.to_owned())
}
