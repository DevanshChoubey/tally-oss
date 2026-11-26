//! Print jobs for the report and voucher screens (card P1, spec section 11 M5).
//!
//! The PDF and the pages it was written from come from `ledgerly-api` (which asks
//! `ledgerly-print`) on the API worker. Ctrl+P shows the pages in the print preview
//! ([`crate::screens::print_preview`]); saving from the preview opens the save dialog on the UI
//! thread, as Export's does, and writes the preview's own bytes on a file thread
//! (`shell::spawn_file_job`), so the window never waits on the disk. The UI lays nothing out.
//!
//! Printing to a device is not built: see `design/PARITY-GAPS.md`.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};

use ledgerly_api::{Api, PrintSetup, PrintedDocument};
use ledgerly_core::Outcome;

use crate::screens::print_preview::{self, PreviewAction, PrintForm};
use crate::screens::report_view::{Notice, export_file_name};
use crate::screens::wire::{Done, Latest, unsupported_text};
use crate::shell::spawn_file_job;
use crate::worker::ApiWorker;

/// Opens the save dialog for a PDF: given the suggested file name, the path chosen, or `None`.
pub type SavePicker = Box<dyn FnMut(&str) -> Option<PathBuf>>;

/// The engine's refusal of the print settings as the card says it under the field: without the
/// lead "the print settings cannot be used: ", starting with a capital and ending with a full
/// stop (the mock's `profit-and-loss.print-range-error`).
#[must_use]
pub fn setup_words(error: &str) -> String {
    let reason = error
        .strip_prefix("the print settings cannot be used: ")
        .unwrap_or(error);
    let mut chars = reason.chars();
    let mut said: String = chars
        .next()
        .map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect());
    if !said.ends_with('.') {
        said.push('.');
    }
    said
}

/// What a print run brings back: the document (PDF and pages), or the engine's refusal, or an
/// error text.
type Rendered = Result<Outcome<PrintedDocument>, String>;

/// A print the preview can run again with other settings.
type PrintFn = dyn Fn(&mut Api, &PrintSetup) -> Result<Outcome<PrintedDocument>, ledgerly_api::ApiError>
    + Send
    + Sync;

/// The system save dialog, offering PDF.
fn save_pdf_dialog(suggested: &str) -> Option<PathBuf> {
    crate::ui::dialogs::save_file("Save as PDF", Some(suggested), &[("PDF", &["pdf"])])
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
        Ok(Outcome::Value(doc)) => {
            Notice::Info(format!("{title} is ready: {} bytes of PDF.", doc.pdf.len()))
        }
        Ok(Outcome::Unsupported(u)) => {
            Notice::Error(format!("{title} was not printed. {}", unsupported_text(u)))
        }
        Err(e) => Notice::Error(format!("{title} was not printed: {e}")),
    }
}

/// Says why the PDF at `path` was not written, in the mock's words (`profit-and-loss.print-failed`):
/// the lead "The PDF was not saved", the system's reason without its error code, and that nothing
/// was written.
#[must_use]
pub fn write_failure(path: &Path, error: &std::io::Error) -> String {
    let reason = error.to_string();
    let reason = reason
        .rsplit_once(" (os error ")
        .map_or(reason.as_str(), |(text, _code)| text);
    format!(
        "The PDF was not saved: \"{}\" could not be written ({reason}). Nothing was written.",
        path.display()
    )
}

/// Writes `bytes` to `path` whole or not at all: a `.part` file, then a rename.
fn write_pdf(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let part = path.with_extension("pdf.part");
    std::fs::write(&part, bytes).map_err(|e| write_failure(path, &e))?;
    std::fs::rename(&part, path).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        write_failure(path, &e)
    })
}

/// One print at a time for a screen: the render on the worker, the preview, then the save.
pub struct PrintJob {
    title: String,
    render: Latest<Rendered>,
    picker: SavePicker,
    writing: Option<Receiver<Result<PathBuf, String>>>,
    saved: Option<PathBuf>,
    /// The document the preview shows, once the worker has produced it.
    preview: Option<PrintedDocument>,
    /// Save was asked for in the preview; the dialog opens on the next tick.
    save_wanted: bool,
    /// What the preview prints, kept so it can run again with new settings.
    job: Option<Arc<PrintFn>>,
    /// The settings as typed in the preview's card.
    form: PrintForm,
    /// Why the last update was refused: the words under the card's Pages field. The preview
    /// keeps the last good pages while this is set.
    setup_error: Option<String>,
    /// Update preview was asked for; it starts on the next frame.
    update_wanted: bool,
    /// The running render replaces an open preview rather than opening one.
    updating: bool,
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
            saved: None,
            preview: None,
            save_wanted: false,
            job: None,
            form: PrintForm::default(),
            setup_error: None,
            update_wanted: false,
            updating: false,
        }
    }

    /// The settings as typed in the preview's card.
    pub fn form_mut(&mut self) -> &mut PrintForm {
        &mut self.form
    }

    /// Draws the preview (pages and card) and acts on what the user did: Save and Close are
    /// handled here, and Update is run by [`PrintJob::run_update`] on the next frame.
    pub fn draw_preview(&mut self, ui: &mut egui::Ui) -> PreviewAction {
        let Some(doc) = self.preview.as_ref() else {
            return PreviewAction::None;
        };
        let action = print_preview::show(ui, doc, &mut self.form, self.setup_error.as_deref());
        match action {
            PreviewAction::Save => self.request_save(),
            PreviewAction::Close => self.close_preview(),
            PreviewAction::Update => self.update_wanted = true,
            PreviewAction::None => {}
        }
        action
    }

    /// Starts the update the card asked for, when it did: see [`PrintJob::update`].
    pub fn run_update(&mut self, worker: Option<&ApiWorker>) {
        if std::mem::take(&mut self.update_wanted)
            && let Some(worker) = worker
        {
            self.update(worker);
        }
    }

    /// Why the last update of the preview was refused, when it was.
    #[must_use]
    pub fn setup_error(&self) -> Option<&str> {
        self.setup_error.as_deref()
    }

    /// Prints again with the settings in the card (Update preview, or Enter). Settings that are
    /// not numbers, or that the engine refuses, are said under the card's fields and the preview
    /// keeps its last good pages; nothing is written either way.
    pub fn update(&mut self, worker: &ApiWorker) {
        let Some(job) = self.job.clone() else {
            return;
        };
        match self.form.to_setup() {
            Err(why) => self.setup_error = Some(setup_words(&why)),
            Ok(setup) => {
                self.setup_error = None;
                self.updating = true;
                self.render.start(worker, move |api| {
                    job(api, &setup).map_err(|e| e.to_string())
                });
            }
        }
    }

    /// Replaces the save dialog.
    pub fn set_picker(&mut self, picker: SavePicker) {
        self.picker = picker;
    }

    /// The document the print preview shows, when it is open.
    #[must_use]
    pub fn preview(&self) -> Option<&PrintedDocument> {
        self.preview.as_ref()
    }

    /// Closes the preview without saving (Esc). Nothing is written.
    pub fn close_preview(&mut self) {
        self.preview = None;
        self.save_wanted = false;
        self.updating = false;
        self.setup_error = None;
    }

    /// Asks to save the previewed PDF (Ctrl+P in the preview, or its Save as PDF button): the
    /// save dialog opens on the next [`PrintJob::tick`].
    pub fn request_save(&mut self) {
        self.save_wanted = self.preview.is_some();
    }

    /// `true` while the PDF is being rendered.
    #[must_use]
    pub fn is_rendering(&self) -> bool {
        self.render.is_pending()
    }

    /// The PDF this job last saved, when there is one.
    #[must_use]
    pub fn saved_path(&self) -> Option<&Path> {
        self.saved.as_deref()
    }

    /// Opens the saved PDF in the system's viewer. `None` when nothing is saved yet.
    pub fn open_saved(&self) -> Option<Notice> {
        let path = self.saved.as_deref()?;
        match open_in_viewer(path) {
            Ok(()) => None,
            Err(e) => Some(Notice::Error(e)),
        }
    }

    /// `true` while a PDF is being rendered or written.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.render.is_pending() || self.writing.is_some()
    }

    /// Starts `job` on the worker for the report or voucher titled `title`. A print already
    /// running is dropped, as a newer request replaces it, and an open preview is closed.
    pub fn start<F>(&mut self, worker: &ApiWorker, title: &str, job: F)
    where
        F: Fn(&mut Api, &PrintSetup) -> Result<Outcome<PrintedDocument>, ledgerly_api::ApiError>
            + Send
            + Sync
            + 'static,
    {
        self.close_preview();
        title.clone_into(&mut self.title);
        self.form = PrintForm::default();
        let job: Arc<PrintFn> = Arc::new(job);
        self.job = Some(Arc::clone(&job));
        let setup = PrintSetup::default();
        self.render.start(worker, move |api| {
            job(api, &setup).map_err(|e| e.to_string())
        });
    }

    /// Moves the job on: when the document is ready, opens the preview; when save was asked for,
    /// opens the save dialog and starts the write of the previewed bytes; when a write
    /// finishes, says where it went. Returns the notice to show, if any.
    pub fn tick(&mut self) -> Option<Notice> {
        if let Some(outcome) = self.writing.as_ref().map(Receiver::try_recv) {
            match outcome {
                Err(TryRecvError::Empty) => {}
                Ok(Ok(path)) => {
                    self.writing = None;
                    let shown = path.display().to_string();
                    self.saved = Some(path);
                    return Some(Notice::Info(format!("Saved PDF to {shown}.")));
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
        if std::mem::take(&mut self.save_wanted) && self.preview.is_some() {
            let suggested = pdf_file_name(&self.title);
            // A cancelled dialog leaves the preview open.
            let path = (self.picker)(&suggested)?;
            let doc = self.preview.take()?;
            return self.save(&path, doc.pdf);
        }
        match self.render.poll() {
            None => None,
            Some(Done::Ready(Ok(Outcome::Value(doc)))) => {
                self.updating = false;
                self.preview = Some(doc);
                None
            }
            Some(Done::Ready(refused)) if std::mem::take(&mut self.updating) => {
                // An update that the engine refuses leaves the last good pages in view.
                self.setup_error = Some(match refused {
                    Ok(Outcome::Unsupported(u)) => unsupported_text(&u),
                    Ok(Outcome::Value(_)) => return None,
                    Err(e) => setup_words(&e),
                });
                None
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

/// The program and arguments that open `path` in the system's PDF viewer. No shell is involved:
/// the path is one argument.
#[must_use]
pub fn viewer_command(path: &Path) -> (String, Vec<String>) {
    viewer_command_for(std::env::consts::OS, path)
}

/// [`viewer_command`] for the operating system named `os` (as [`std::env::consts::OS`] names
/// it), so every platform's command can be tested on any platform.
#[must_use]
pub fn viewer_command_for(os: &str, path: &Path) -> (String, Vec<String>) {
    let file = path.display().to_string();
    match os {
        "macos" => ("open".to_owned(), vec![file]),
        "windows" => ("explorer.exe".to_owned(), vec![file]),
        _ => ("xdg-open".to_owned(), vec![file]),
    }
}

/// Opens `path` in the system's PDF viewer, without waiting for the viewer to close.
///
/// # Errors
/// A message when the viewer cannot be started (for example, no viewer is installed).
pub fn open_in_viewer(path: &Path) -> Result<(), String> {
    let (program, args) = viewer_command(path);
    std::process::Command::new(&program)
        .args(&args)
        .spawn()
        .map(|_child| ())
        .map_err(|e| format!("the PDF could not be opened with {program}: {e}"))
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
    fn a_refused_setting_is_said_under_the_field_as_a_sentence() {
        // provenance: derived from the mock's wording (index.html, `profit-and-loss.print-range-error`):
        // "The pages to print (5-6) select no page: the document has pages 1 to 1." The engine's
        // error leads with "the print settings cannot be used: ", which the card does not repeat.
        assert_eq!(
            setup_words(
                "the print settings cannot be used: the pages to print (5-6) select no page: the document has pages 1 to 1"
            ),
            "The pages to print (5-6) select no page: the document has pages 1 to 1."
        );
        assert_eq!(
            setup_words("Copies must be a number."),
            "Copies must be a number."
        );
        assert_eq!(setup_words(""), ".");
    }

    #[test]
    fn a_failed_write_says_what_failed_why_and_that_nothing_was_written() {
        // provenance: derived from the mock's wording, "The PDF was not saved. ... Nothing was
        // written." (index.html, profit-and-loss.print-failed); the reason is the system's own,
        // with the trailing "(os error N)" code dropped as it means nothing to the reader.
        let missing = std::io::Error::from_raw_os_error(2);
        let said = write_failure(Path::new("/a/b.pdf"), &missing);
        assert!(said.starts_with("The PDF was not saved: "), "{said}");
        assert!(said.contains("\"/a/b.pdf\""), "{said}");
        assert!(!said.contains("os error"), "{said}");
        assert!(said.ends_with("Nothing was written."), "{said}");
        let other = std::io::Error::other("the disk is full");
        let said = write_failure(Path::new("c.pdf"), &other);
        assert!(said.contains("the disk is full"), "{said}");
    }

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

    #[test]
    fn the_saved_pdf_opens_with_the_systems_viewer() {
        // Derived: xdg-open on Linux, open on macOS, and explorer.exe on Windows; the file is the
        // last argument in each. Only the platform this test is built for is checked.
        let path = Path::new("/home/someone/Report.pdf");
        let (program, args) = viewer_command(path);
        assert_eq!(
            args.last().map(String::as_str),
            Some("/home/someone/Report.pdf")
        );
        if cfg!(target_os = "linux") {
            assert_eq!(program, "xdg-open");
        } else if cfg!(target_os = "macos") {
            assert_eq!(program, "open");
        } else if cfg!(target_os = "windows") {
            assert_eq!(program, "explorer.exe");
        }
    }

    #[test]
    fn on_windows_the_pdf_opens_without_cmd_so_an_ampersand_in_its_name_is_not_a_command() {
        // Derived: `cmd /C start "" <path>` re-parses its command line, so `&` in a file name
        // splits it into a second command; explorer.exe takes the path as one argument and opens
        // it with the file's default application.
        let path = Path::new(r"C:\Books\Q1 & Q2.pdf");
        let (program, args) = viewer_command_for("windows", path);
        assert_eq!(program, "explorer.exe");
        assert_eq!(args, vec![r"C:\Books\Q1 & Q2.pdf".to_owned()]);
    }
}
