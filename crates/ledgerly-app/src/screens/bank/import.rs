//! Choosing a statement file for the import (card S-F).
//!
//! The file is read on this computer and handed to `Api::import_bank_statement` with one of the
//! layouts the engine knows (the `ledgerly-bankfmt` registry ids). The layout comes from the
//! file's extension when the extension settles it (an Excel file can only be read with the Excel
//! layout), otherwise from the layout the user chose.

use std::path::Path;

/// The layout used until the user chooses another.
pub(super) const DEFAULT_LAYOUT: &str = "generic-csv-split";

/// Every layout the import offers, as (registry id, label).
pub(super) const LAYOUTS: [(&str, &str); 5] = [
    (
        "generic-csv-split",
        "Generic CSV, separate withdrawal and deposit columns",
    ),
    (
        "generic-csv-signed",
        "Generic CSV, one signed amount column",
    ),
    (
        "generic-csv-indicator",
        "Generic CSV, amount with a Dr/Cr marker column",
    ),
    (
        "generic-xlsx-split",
        "Generic Excel, separate withdrawal and deposit columns",
    ),
    ("generic-mt940", "SWIFT MT940 statement"),
];

/// A statement file the user picked, read into memory.
#[derive(Debug)]
pub(super) struct PickedFile {
    /// The file's bytes.
    pub(super) bytes: Vec<u8>,
    /// The registry id of the layout to parse the file with.
    pub(super) layout: &'static str,
}

/// The label of a layout. An id that is not in [`LAYOUTS`] is shown as it is.
pub(super) fn layout_label(layout: &'static str) -> &'static str {
    LAYOUTS
        .iter()
        .find(|(id, _)| *id == layout)
        .map_or(layout, |(_, label)| *label)
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
pub(super) fn pick_file(current: &'static str) -> Result<Option<PickedFile>, String> {
    let Some(path) = rfd::FileDialog::new()
        .set_title("Import bank statement")
        .add_filter("Bank statement", &["csv", "xlsx", "mt940", "sta"])
        .pick_file()
    else {
        return Ok(None);
    };
    let bytes = std::fs::read(&path).map_err(|e| format!("The file could not be read: {e}."))?;
    Ok(Some(PickedFile {
        bytes,
        layout: layout_for(&path, current),
    }))
}
