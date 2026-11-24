//! Choosing a statement file for the import (card S-F).
//!
//! The file is read on this computer and handed to `Api::import_bank_statement` with one of the
//! layouts the engine knows (the `ledgerly-bankfmt` registry ids). The layout comes from the
//! file's extension when the extension settles it (an Excel file can only be read with the Excel
//! layout), otherwise from the layout the user chose.

use std::path::Path;

use crate::i18n::Tr;

/// The layout used until the user chooses another.
pub(super) const DEFAULT_LAYOUT: &str = "generic-csv-split";

/// Every layout the import offers, as (registry id, translation key of its label).
pub(super) const LAYOUTS: [(&str, &str); 5] = [
    ("generic-csv-split", "bank.layout.csv_split"),
    ("generic-csv-signed", "bank.layout.csv_signed"),
    ("generic-csv-indicator", "bank.layout.csv_indicator"),
    ("generic-xlsx-split", "bank.layout.xlsx_split"),
    ("generic-mt940", "bank.layout.mt940"),
];

/// A statement file the user picked, read into memory.
#[derive(Debug)]
pub(super) struct PickedFile {
    /// The file's bytes.
    pub(super) bytes: Vec<u8>,
    /// The registry id of the layout to parse the file with.
    pub(super) layout: &'static str,
}

/// The label of a layout in the words of `tr`. An id that is not in [`LAYOUTS`] is shown as it is.
pub(super) fn layout_label_in(layout: &str, tr: &Tr) -> String {
    LAYOUTS
        .iter()
        .find(|(id, _)| *id == layout)
        .map_or_else(|| layout.to_owned(), |(_, key)| tr.t(key))
}

/// The layout for a file: the extension decides when it is clear, else `current`.
pub(super) fn layout_for(path: &Path, current: &'static str) -> &'static str {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("xlsx") => "generic-xlsx-split",
        Some("mt940" | "sta") => "generic-mt940",
        _ => current,
    }
}

/// Opens the system file dialog and reads the chosen file. `Ok(None)` when the user cancelled.
///
/// The dialog blocks the calling (UI) thread while it is open; the file is read after it closes.
///
/// # Errors
/// The text to show when the file cannot be read.
pub(super) fn pick_file(current: &'static str, tr: &Tr) -> Result<Option<PickedFile>, String> {
    let Some(path) = rfd::FileDialog::new()
        .set_title(tr.t("bank.import.dialog_title"))
        .add_filter(tr.t("bank.import.filter"), &["csv", "xlsx", "mt940", "sta"])
        .pick_file()
    else {
        return Ok(None);
    };
    let bytes = std::fs::read(&path)
        .map_err(|e| tr.tf("bank.import.unreadable", &[("error", &e.to_string())]))?;
    Ok(Some(PickedFile {
        bytes,
        layout: layout_for(&path, current),
    }))
}
