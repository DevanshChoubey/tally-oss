//! IMS reader and action file writer. **Card F3** implements this module.
//!
//! Frozen interface (plan card F3):
//!
//! ```text
//! pub struct Ims { .. }
//! pub fn parse_ims(json: &str) -> Result<Ims, FormatError>;
//! pub fn ims_actions_to_json(actions: &[ImsActionRecord]) -> Result<String, FormatError>;
//! ```
//!
//! # Shapes
//!
//! The Invoice Management System download has no published schema either (research R4 section 4).
//! The reader takes the same `data` / `docdata` / `b2b` / `cdnr` layout as GSTR-2B (see
//! [`crate::gstr2b`]) and adds two optional fields on each document: `action` (`A` accept, `R`
//! reject, `P` pending, `N` or absent: no action yet) and `rmrk` (remarks). Everything else is
//! kept in `extra`.
//!
//! The action file written by [`ims_actions_to_json`] uses the same naming: suppliers grouped
//! under `b2b` (invoices, list key `inv`) and `cdnr` (notes, list key `nt`, with `typ` C or D),
//! each document naming its number, date, `action` and optional `rmrk`.
//!
//! **Both shapes are pending observed**: neither has been compared with a file from the portal.
//! Until the action file is checked against a file the portal accepts, treat it as Ledgerly's
//! working format, not as an upload-ready file (rule A5).
//!
//! The writer enforces spec section 6.3: every action in the file was confirmed by the user
//! (`confirmed` is true), so nothing can be accepted, rejected or held by default.

use std::collections::BTreeMap;

use ledgerly_core::Gstin;
use serde_json::{Map, Value};
use time::Date;

use crate::FormatError;
use crate::gstr2b::{
    DocForm, InwardDoc, InwardDocKind, Obj, ReturnPeriod, SupplierGroup, parse_root, parse_section,
};

/// What the recipient does with an inward record in IMS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImsAction {
    /// Accept the record (`A`).
    Accept,
    /// Reject the record (`R`).
    Reject,
    /// Keep the record pending (`P`).
    Pending,
}

impl ImsAction {
    /// The one-letter code used in the files.
    pub const fn code(self) -> &'static str {
        match self {
            ImsAction::Accept => "A",
            ImsAction::Reject => "R",
            ImsAction::Pending => "P",
        }
    }
}

/// One inward record as the IMS download lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImsDoc {
    /// The document: number, date, value, items, and the unknown fields in its `extra`.
    pub doc: InwardDoc,
    /// The action already recorded on the portal; `None` when none was taken (`N` or absent).
    pub action: Option<ImsAction>,
    /// Remarks entered with the action (`rmrk`).
    pub remarks: Option<String>,
}

/// A parsed IMS download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ims {
    /// Recipient GSTIN (`gstin`), as written, when the file has it.
    pub gstin: Option<String>,
    /// Return period (`rtnprd`), when the file has it.
    pub return_period: Option<ReturnPeriod>,
    /// Supplier invoices (`b2b`).
    pub b2b: Vec<SupplierGroup<ImsDoc>>,
    /// Supplier credit and debit notes (`cdnr`).
    pub cdnr: Vec<SupplierGroup<ImsDoc>>,
    /// Unknown fields of the `data` object, as written.
    pub extra: Map<String, Value>,
    /// Sections of `docdata` other than `b2b` and `cdnr`, as written.
    pub docdata_extra: Map<String, Value>,
    /// Unknown fields next to `data`, as written.
    pub root_extra: Map<String, Value>,
}

/// An action the user has chosen for one inward record, to go into the action file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImsActionRecord {
    /// The supplier's GSTIN.
    pub supplier: Gstin,
    /// Invoice, credit note or debit note.
    pub doc_kind: InwardDocKind,
    /// Document number exactly as the supplier wrote it.
    pub number: String,
    /// Document date.
    pub date: Date,
    /// The chosen action.
    pub action: ImsAction,
    /// Remarks for the supplier; allowed only with [`ImsAction::Reject`] and
    /// [`ImsAction::Pending`], and not empty when given.
    pub remarks: Option<String>,
    /// `true` once the user confirmed this action. The writer refuses unconfirmed records.
    pub confirmed: bool,
}

/// Parses a downloaded IMS JSON file.
///
/// # Errors
/// [`FormatError::Json`] with the JSON path of the first problem (see
/// [`parse_gstr2b`](crate::gstr2b::parse_gstr2b)); an unknown `action` code is an error, not a
/// guess.
pub fn parse_ims(json: &str) -> Result<Ims, FormatError> {
    let root_value = parse_root(json)?;
    let mut root = Obj::root(&root_value)?;
    let mut data = root.req_obj("data")?;
    let gstin = data.opt_str("gstin")?;
    let return_period = data.opt_period("rtnprd")?;
    let mut docdata = data.req_obj("docdata")?;
    let b2b = parse_section(&mut docdata, "b2b", DocForm::Invoice, finish_ims_doc)?;
    let cdnr = parse_section(&mut docdata, "cdnr", DocForm::Note, finish_ims_doc)?;
    let docdata_extra = docdata.finish();
    let extra = data.finish();
    let root_extra = root.finish();
    Ok(Ims {
        gstin,
        return_period,
        b2b,
        cdnr,
        extra,
        docdata_extra,
        root_extra,
    })
}

fn finish_ims_doc(mut rest: Obj<'_>, mut doc: InwardDoc) -> Result<ImsDoc, FormatError> {
    let action = match rest.opt_str("action")?.as_deref() {
        Some("A") => Some(ImsAction::Accept),
        Some("R") => Some(ImsAction::Reject),
        Some("P") => Some(ImsAction::Pending),
        Some("N") | None => None,
        Some(other) => {
            return Err(FormatError::Json {
                path: rest.key_path("action"),
                reason: format!("'{other}' is not one of A, R, P, N"),
            });
        }
    };
    let remarks = rest.opt_str("rmrk")?;
    doc.extra = rest.finish();
    Ok(ImsDoc {
        doc,
        action,
        remarks,
    })
}

/// Writes the action file for the records the user confirmed.
///
/// Suppliers and documents are sorted (supplier GSTIN, then document number), so the same actions
/// give the same text whatever order they were made in. See the module documentation for the
/// shape, which is pending observed.
///
/// # Errors
/// [`FormatError::Field`] naming `actions[i].<field>` when: the list is empty; a record is not
/// confirmed; remarks are given with [`ImsAction::Accept`] or are empty; a document number is
/// empty; the year is outside 1 to 9999; or two records name the same document of the same
/// supplier.
pub fn ims_actions_to_json(actions: &[ImsActionRecord]) -> Result<String, FormatError> {
    if actions.is_empty() {
        return Err(field("actions", "there are no actions to write"));
    }
    let mut suppliers: BTreeMap<String, SupplierActions> = BTreeMap::new();
    for (i, record) in actions.iter().enumerate() {
        check_record(i, record)?;
        let entry = suppliers
            .entry(record.supplier.as_str().to_owned())
            .or_default();
        add_record(i, record, entry)?;
    }
    let b2b: Vec<Value> = suppliers
        .iter()
        .filter(|(_, s)| !s.invoices.is_empty())
        .map(|(ctin, s)| {
            supplier_object(
                ctin,
                "inv",
                s.invoices.values().cloned().collect::<Vec<_>>(),
            )
        })
        .collect();
    let cdnr: Vec<Value> = suppliers
        .iter()
        .filter(|(_, s)| !s.notes.is_empty())
        .map(|(ctin, s)| supplier_object(ctin, "nt", s.notes.values().cloned().collect::<Vec<_>>()))
        .collect();
    let mut sections = Vec::new();
    if !b2b.is_empty() {
        sections.push(("b2b", Value::Array(b2b)));
    }
    if !cdnr.is_empty() {
        sections.push(("cdnr", Value::Array(cdnr)));
    }
    serde_json::to_string(&object(sections)).map_err(|e| FormatError::Json {
        path: "$".to_owned(),
        reason: e.to_string(),
    })
}

/// The documents of one supplier, keyed so that duplicates are found and output is ordered.
#[derive(Default)]
struct SupplierActions {
    invoices: BTreeMap<String, Value>,
    notes: BTreeMap<(String, &'static str), Value>,
}

fn field(name: impl Into<String>, reason: impl Into<String>) -> FormatError {
    FormatError::Field {
        field: name.into(),
        reason: reason.into(),
    }
}

fn check_record(i: usize, record: &ImsActionRecord) -> Result<(), FormatError> {
    if !record.confirmed {
        return Err(field(
            format!("actions[{i}].confirmed"),
            "the user has not confirmed this action; nothing is written by default",
        ));
    }
    if record.number.trim().is_empty() {
        return Err(field(
            format!("actions[{i}].number"),
            "the document number is empty",
        ));
    }
    match (&record.remarks, record.action) {
        (Some(text), _) if text.trim().is_empty() => Err(field(
            format!("actions[{i}].remarks"),
            "remarks are empty; leave them out instead",
        )),
        (Some(_), ImsAction::Accept) => Err(field(
            format!("actions[{i}].remarks"),
            "remarks are allowed only with Reject or Pending",
        )),
        (Some(_), ImsAction::Reject | ImsAction::Pending) | (None, _) => Ok(()),
    }
}

fn add_record(
    i: usize,
    record: &ImsActionRecord,
    entry: &mut SupplierActions,
) -> Result<(), FormatError> {
    let date = portal_date(i, record.date)?;
    let (number_key, note_type) = match record.doc_kind {
        InwardDocKind::Invoice => ("inum", None),
        InwardDocKind::CreditNote => ("ntnum", Some("C")),
        InwardDocKind::DebitNote => ("ntnum", Some("D")),
    };
    let mut fields = vec![
        (number_key, Value::String(record.number.clone())),
        ("dt", Value::String(date)),
    ];
    if let Some(code) = note_type {
        fields.push(("typ", Value::String(code.to_owned())));
    }
    fields.push(("action", Value::String(record.action.code().to_owned())));
    if let Some(text) = &record.remarks {
        fields.push(("rmrk", Value::String(text.clone())));
    }
    let value = object(fields);
    let duplicate = match note_type {
        None => entry
            .invoices
            .insert(record.number.clone(), value)
            .is_some(),
        Some(code) => entry
            .notes
            .insert((record.number.clone(), code), value)
            .is_some(),
    };
    if duplicate {
        return Err(field(
            format!("actions[{i}].number"),
            format!(
                "{} of supplier {} appears more than once",
                record.number,
                record.supplier.as_str()
            ),
        ));
    }
    Ok(())
}

fn portal_date(i: usize, date: Date) -> Result<String, FormatError> {
    if !(1..=9999).contains(&date.year()) {
        return Err(field(
            format!("actions[{i}].date"),
            "the year must be between 1 and 9999",
        ));
    }
    Ok(format!(
        "{:02}-{:02}-{:04}",
        date.day(),
        u8::from(date.month()),
        date.year()
    ))
}

fn object(entries: Vec<(&str, Value)>) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
    )
}

fn supplier_object(ctin: &str, list_key: &str, docs: Vec<Value>) -> Value {
    object(vec![
        ("ctin", Value::String(ctin.to_owned())),
        (list_key, Value::Array(docs)),
    ])
}
