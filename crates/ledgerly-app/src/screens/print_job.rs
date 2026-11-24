//! Print jobs for the report and voucher screens (card P1, spec section 11 M5).
//!
//! The PDF comes from `ledgerly-api` (which asks `ledgerly-print`) on the API worker. The save
//! dialog opens on the UI thread, as Export's does, and the file is written on a file thread
//! (`shell::spawn_file_job`), so the window never waits on the disk. The UI draws no PDF.
//!
//! Printing to a device and the print preview are not built: see `design/PARITY-GAPS.md`.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};

use ledgerly_api::Api;
use ledgerly_core::Outcome;

use crate::screens::report_view::{Notice, export_file_name};
use crate::screens::wire::{Done, Latest, unsupported_text};
use crate::shell::spawn_file_job;
use crate::worker::ApiWorker;

/// Opens the save dialog for a PDF: given the suggested file name, the path chosen, or `None`.
pub type SavePicker = Box<dyn FnMut(&str) -> Option<PathBuf>>;

/// What a print run brings back: the PDF bytes, or the engine's refusal, or an error text.
type Rendered = Result<Outcome<Vec<u8>>, String>;

/// The system save dialog, offering PDF.
fn save_pdf_dialog(suggested: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Save as PDF")
        .set_file_name(suggested)
        .add_filter("PDF", &["pdf"])
        .save_file()
}

/// The file name a PDF of a report or voucher titled `title` is saved under: the name Export
/// suggests with `.pdf` in place of `.csv`.
#[must_use]
pub fn pdf_file_name(title: &str) -> String {
    let csv = export_file_name(title);
    let stem = csv.strip_suffix(".csv").unwrap_or(&csv);
    format!("{stem}.pdf")
}

/// What the user reads after a print that did not produce a file: the engine's answer, said in
/// words. An unsupported print and an error are both errors on screen, with the reason.
#[must_use]
pub fn print_notice(title: &str, done: &Rendered) -> Notice {
    match done {
        Ok(Outcome::Value(bytes)) => {
            Notice::Info(format!("{title} is ready: {} bytes of PDF.", bytes.len()))
        }
        Ok(Outcome::Unsupported(u)) => {
            Notice::Error(format!("{title} was not printed. {}", unsupported_text(u)))
        }
        Err(e) => Notice::Error(format!("{title} was not printed: {e}")),
    }
}

/// Writes `bytes` to `path` whole or not at all: a `.part` file, then a rename.
fn write_pdf(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let part = path.with_extension("pdf.part");
    std::fs::write(&part, bytes)
        .map_err(|e| format!("\"{}\" was not written: {e}", path.display()))?;
    std::fs::rename(&part, path).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        format!("\"{}\" was not written: {e}", path.display())
    })
}

/// One print at a time for a screen: the render on the worker, then the save.
pub struct PrintJob {
    title: String,
    render: Latest<Rendered>,
    picker: SavePicker,
    writing: Option<Receiver<Result<PathBuf, String>>>,
}

impl fmt::Debug for PrintJob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrintJob")
            .field("title", &self.title)
            .field("writing", &self.writing.is_some())
            .finish_non_exhaustive()
    }
}

impl Default for PrintJob {
    fn default() -> Self {
        Self::with_picker(Box::new(save_pdf_dialog))
    }
}

impl PrintJob {
    /// A job whose save dialog is `picker`.
    #[must_use]
    pub fn with_picker(picker: SavePicker) -> Self {
        Self {
            title: String::new(),
            render: Latest::default(),
            picker,
            writing: None,
        }
    }

    /// `true` while a PDF is being rendered or written.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.render.is_pending() || self.writing.is_some()
    }

    /// Starts `job` on the worker for the report or voucher titled `title`. A print already
    /// running is dropped, as a newer request replaces it.
    pub fn start<F>(&mut self, worker: &ApiWorker, title: &str, job: F)
    where
        F: FnOnce(&mut Api) -> Result<Outcome<Vec<u8>>, ledgerly_api::ApiError> + Send + 'static,
    {
        title.clone_into(&mut self.title);
        self.render
            .start(worker, move |api| job(api).map_err(|e| e.to_string()));
    }

    /// Moves the job on: when the PDF is ready, opens the save dialog and starts the write;
    /// when a write finishes, says where it went. Returns the notice to show, if any.
    pub fn tick(&mut self) -> Option<Notice> {
        if let Some(outcome) = self.writing.as_ref().map(Receiver::try_recv) {
            match outcome {
                Err(TryRecvError::Empty) => {}
                Ok(Ok(path)) => {
                    self.writing = None;
                    return Some(Notice::Info(format!("Saved PDF to {}.", path.display())));
                }
                Ok(Err(e)) => {
                    self.writing = None;
                    return Some(Notice::Error(e));
                }
                Err(TryRecvError::Disconnected) => {
                    self.writing = None;
                    return Some(Notice::Error(
                        "the file thread stopped before the PDF was written; nothing was saved"
                            .to_owned(),
                    ));
                }
            }
            return None;
        }
        match self.render.poll() {
            None => None,
            Some(Done::Ready(Ok(Outcome::Value(bytes)))) => {
                let suggested = pdf_file_name(&self.title);
                let path = (self.picker)(&suggested)?;
                self.save(&path, bytes)
            }
            Some(Done::Ready(other)) => Some(print_notice(&self.title, &other)),
            Some(Done::Lost) => Some(Notice::Error(format!(
                "{} was not printed: the engine stopped before the print finished",
                self.title
            ))),
        }
    }

    /// Starts writing `bytes` to `path` on a file thread.
    fn save(&mut self, path: &Path, bytes: Vec<u8>) -> Option<Notice> {
        let target = path.to_path_buf();
        match spawn_file_job(move || write_pdf(&target, &bytes).map(|()| target)) {
            Ok(rx) => {
                self.writing = Some(rx);
                None
            }
            Err(e) => Some(Notice::Error(format!("the PDF was not written: {e}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::panic,
        reason = "test code: rule B2 lets tests unwrap and panic"
    )]

    use ledgerly_core::{Outcome, Unsupported};

    use super::*;

    #[test]
    fn the_pdf_name_follows_the_title_and_is_safe_for_a_file_system() {
        // Derived from the rules of `export_file_name`: path separators become "-", edge spaces
        // and dots are dropped, an empty title is "Report", and the extension is ".pdf".
        assert_eq!(pdf_file_name("Balance sheet"), "Balance sheet.pdf");
        assert_eq!(pdf_file_name("Sales/Jun"), "Sales-Jun.pdf");
        assert_eq!(pdf_file_name(" . "), "Report.pdf");
    }

    #[test]
    fn an_unsupported_print_is_an_error_notice_that_gives_the_reason() {
        let done = Ok(Outcome::Unsupported(Unsupported {
            what: "print report".to_owned(),
            reason: "reports have no print layout yet".to_owned(),
        }));
        match print_notice("Balance sheet", &done) {
            Notice::Error(text) => {
                assert!(text.contains("reports have no print layout yet"), "{text}");
                assert!(text.contains("print report"), "{text}");
            }
            Notice::Info(text) => panic!("an unsupported print must be an error, got {text:?}"),
        }
    }

    #[test]
    fn an_engine_error_is_shown_not_hidden() {
        match print_notice("Voucher 1", &Err("the store is closed".to_owned())) {
            Notice::Error(text) => assert!(text.contains("the store is closed"), "{text}"),
            Notice::Info(text) => panic!("an engine error must be shown, got {text:?}"),
        }
    }
}
