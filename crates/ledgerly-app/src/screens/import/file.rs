//! Import from an XML file into the open company (S22): `Api::import_tally_xml_file` run on the
//! API worker, and the panel that asks for it and shows what it did.
//!
//! The panel follows the import screen of
//! <https://help.tallysolutions.com/import-data-from-xml-or-json/> (read 2026-11-27): Masters or
//! Transactions, the file, and for masters the behaviour for a master that is already in the
//! company. The three behaviours are the ones of <https://help.tallysolutions.com/import-data-faq/>
//! (Combine Opening Balance, Ignore Duplicates, Modify with New Data). The page does not say which
//! one is preselected, so none is: the user picks one, and an import without it is refused in
//! words (rule: never a silent default). The result is the API's summary: the counts, then every
//! object that was not imported with its reason. The screen computes no figure (rule C4).

use std::path::PathBuf;

use egui::{RichText, Ui};
use ledgerly_api::{FileImportSummary, MasterBehaviour, XmlImport};
use ledgerly_core::{Outcome, Unsupported};

use crate::screens::wire::{Done, Latest, unsupported_text};
use crate::theme::{TEXT_BODY, TEXT_META};
use crate::ui::button::{ButtonKind, button_icon};
use crate::ui::icon::Icon;
use crate::ui::note::{NoteKind, note};
use crate::ui::{a11y, bold_font, para, tokens};
use crate::worker::ApiWorker;

/// What the file holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// Groups and ledgers.
    Masters,
    /// Vouchers.
    Transactions,
}

/// What the file import shows now.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum FileImportState {
    /// Nothing has been run.
    #[default]
    Idle,
    /// The import is running on the worker.
    Running,
    /// The file was read; the summary says what went in and what did not.
    Imported(FileImportSummary),
    /// The API refused (for example the books could not be read), with its reason.
    Refused(Unsupported),
    /// The form is missing a choice, or the file could not be read, in words.
    Failed(String),
}

/// The panel's fields and the running call.
#[derive(Debug, Default)]
pub(super) struct FileImport {
    pub(super) kind: Option<FileKind>,
    pub(super) path: String,
    pub(super) behaviour: Option<MasterBehaviour>,
    pub(super) state: FileImportState,
    /// Whether the path field had the keyboard last frame, so Enter imports the file.
    pub(super) path_focused: bool,
    /// Frames left in which to scroll the panel into view (the screen was opened with Alt+O).
    /// More than one, since the first frame lays out a scroll area that does not know its
    /// content's height yet.
    pub(super) reveal: u8,
    /// Scroll the result into view on the next frame: an import finished or was refused.
    show_result: bool,
    latest: Latest<Result<Outcome<FileImportSummary>, String>>,
}

impl FileImport {
    /// Checks the form and starts the import. A missing choice is a `Failed` state and nothing
    /// runs.
    pub(super) fn start(&mut self, worker: Option<&ApiWorker>) {
        self.show_result = true;
        let Some(kind) = self.kind else {
            self.state = FileImportState::Failed(crate::i18n::t("import.file.err_kind"));
            return;
        };
        let path = self.path.trim().to_owned();
        if path.is_empty() {
            self.state = FileImportState::Failed(crate::i18n::t("import.file.err_path"));
            return;
        }
        let what = match (kind, self.behaviour) {
            (FileKind::Transactions, _) => XmlImport::Transactions,
            (FileKind::Masters, Some(behaviour)) => XmlImport::Masters(behaviour),
            (FileKind::Masters, None) => {
                self.state = FileImportState::Failed(crate::i18n::t("import.file.err_behaviour"));
                return;
            }
        };
        let Some(handle) = worker else {
            self.state = FileImportState::Failed(crate::i18n::t("import.file.err_no_company"));
            return;
        };
        self.state = FileImportState::Running;
        let path = PathBuf::from(path);
        self.latest.start(handle, move |api| {
            api.import_tally_xml_file(what, &path)
                .map_err(|e| e.to_string())
        });
    }

    /// Takes a finished answer, if there is one.
    pub(super) fn tick(&mut self) {
        let done = self.latest.poll();
        if done.is_some() {
            self.show_result = true;
        }
        match done {
            Some(Done::Ready(Ok(Outcome::Value(summary)))) => {
                // Objects may have been committed, so the shell reads the books again.
                crate::shell::submit::note_changed();
                self.state = FileImportState::Imported(summary);
            }
            Some(Done::Ready(Ok(Outcome::Unsupported(u)))) => {
                self.state = FileImportState::Refused(u);
            }
            Some(Done::Ready(Err(message))) => self.state = FileImportState::Failed(message),
            Some(Done::Lost) => {
                self.state = FileImportState::Failed(crate::i18n::t("import.file.err_lost"));
            }
            None => {}
        }
    }

    /// Asks for the file with the system dialog and puts its path in the field.
    pub(super) fn choose_file(&mut self) {
        let title = crate::i18n::t("import.file.dialog_title");
        let filter = crate::i18n::t("import.file.filter");
        if let Some(path) = crate::ui::dialogs::open_file(&title, &[(&filter, &["xml"])]) {
            self.path = path.display().to_string();
        }
    }
}

/// The summary line of a finished import, for example "Masters: 9 created, 0 altered, 0
/// combined, 1 ignored, 0 not imported."
#[must_use]
pub fn summary_line(summary: &FileImportSummary) -> String {
    let what = match summary.what {
        XmlImport::Masters(_) => crate::i18n::t("import.file.what_masters"),
        XmlImport::Transactions => crate::i18n::t("import.file.what_transactions"),
    };
    crate::i18n::tf(
        "import.file.summary",
        &[
            ("what", &what),
            ("created", &summary.created.to_string()),
            ("altered", &summary.altered.to_string()),
            ("combined", &summary.combined.to_string()),
            ("ignored", &summary.ignored.to_string()),
            ("errors", &summary.errors.to_string()),
        ],
    )
}

/// What the state shows, as lines: the summary line, then every exception as `object: reason`,
/// then every ignored object and every field read but not imported. Empty when idle.
#[must_use]
pub fn state_lines(state: &FileImportState) -> Vec<String> {
    match state {
        FileImportState::Idle => Vec::new(),
        FileImportState::Running => vec![crate::i18n::t("import.file.running")],
        FileImportState::Refused(u) => vec![unsupported_text(u)],
        FileImportState::Failed(text) => vec![format!("Error: {text}")],
        FileImportState::Imported(summary) => {
            let mut lines = vec![summary_line(summary)];
            lines.extend(
                summary
                    .exceptions
                    .iter()
                    .map(|e| format!("{}: {}", e.object, e.reason)),
            );
            lines.extend(summary.ignored_notes.iter().cloned());
            lines.extend(summary.warnings.iter().cloned());
            lines
        }
    }
}

/// The behaviours, in the order of the import FAQ.
const BEHAVIOURS: [(MasterBehaviour, &str); 3] = [
    (
        MasterBehaviour::CombineOpeningBalance,
        "import.file.combine",
    ),
    (MasterBehaviour::IgnoreDuplicates, "import.file.ignore"),
    (MasterBehaviour::ModifyWithNewData, "import.file.modify"),
];

/// What the user pressed in the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FilePress {
    /// Nothing.
    None,
    /// Import the file.
    Import,
    /// Choose the file with the dialog.
    Choose,
}

/// Draws the panel. `can_run` is false with no company open; the reason is said.
pub(super) fn draw_file(
    ui: &mut Ui,
    file: &mut FileImport,
    can_run: bool,
    choice_row: fn(&mut Ui, &str, bool) -> egui::Response,
) -> FilePress {
    let t = tokens(ui);
    let mut press = FilePress::None;
    let head = ui.label(
        RichText::new(crate::i18n::t("import.file.heading"))
            .font(bold_font(ui, crate::theme::TEXT_H2))
            .color(t.text),
    );
    if file.reveal > 0 {
        file.reveal -= 1;
        head.scroll_to_me(Some(egui::Align::TOP));
    }
    ui.add_space(2.0);
    para(ui, &crate::i18n::t("import.file.meta"), TEXT_META, t.text_2);
    ui.add_space(8.0);
    if !can_run {
        note(
            ui,
            NoteKind::Warn,
            &crate::i18n::t("import.file.no_company"),
        );
        ui.add_space(8.0);
    }
    let running = file.state == FileImportState::Running;
    ui.add_enabled_ui(can_run && !running, |ui| {
        draw_choices(ui, file, choice_row);
        press = draw_path(ui, file);
    });
    let show = std::mem::take(&mut file.show_result);
    draw_result(ui, &file.state, show);
    press
}

/// Masters or Transactions, and for masters what to do with one that exists.
fn draw_choices(
    ui: &mut Ui,
    file: &mut FileImport,
    choice_row: fn(&mut Ui, &str, bool) -> egui::Response,
) {
    let t = tokens(ui);
    {
        ui.label(
            RichText::new(crate::i18n::t("import.file.what"))
                .size(TEXT_META)
                .color(t.text_2),
        );
        ui.columns(2, |cols| {
            if let [masters, transactions] = cols {
                if choice_row(
                    masters,
                    &crate::i18n::t("import.file.masters"),
                    file.kind == Some(FileKind::Masters),
                )
                .clicked()
                {
                    file.kind = Some(FileKind::Masters);
                }
                if choice_row(
                    transactions,
                    &crate::i18n::t("import.file.transactions"),
                    file.kind == Some(FileKind::Transactions),
                )
                .clicked()
                {
                    file.kind = Some(FileKind::Transactions);
                }
            }
        });
        if file.kind == Some(FileKind::Masters) {
            ui.add_space(8.0);
            ui.label(
                RichText::new(crate::i18n::t("import.file.existing"))
                    .size(TEXT_META)
                    .color(t.text_2),
            );
            for (behaviour, key) in BEHAVIOURS {
                if choice_row(ui, &crate::i18n::t(key), file.behaviour == Some(behaviour)).clicked()
                {
                    file.behaviour = Some(behaviour);
                }
                ui.add_space(4.0);
            }
        }
    }
}

/// The path field, Choose file and Import the file.
/// Height of the path row: one button (`ui::button`'s 32 points).
const PATH_ROW_HEIGHT: f32 = 32.0;

fn draw_path(ui: &mut Ui, file: &mut FileImport) -> FilePress {
    let t = tokens(ui);
    let mut press = FilePress::None;
    {
        ui.add_space(4.0);
        let label = crate::i18n::t("import.file.path");
        ui.label(RichText::new(&label).size(TEXT_META).color(t.text_2));
        // Right to left: the button first, at the right edge, then the field takes what is left,
        // so the row never runs past the panel. The row is one button high: a bare right-to-left
        // layout takes the whole height left in the scroll area and centres the row in it.
        let row = egui::vec2(ui.available_width(), PATH_ROW_HEIGHT);
        ui.allocate_ui_with_layout(
            row,
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                let choose = crate::i18n::t("import.file.choose");
                if button_icon(
                    ui,
                    Some(Icon::File),
                    &choose,
                    Some("Ctrl+O"),
                    ButtonKind::Secondary,
                )
                .clicked()
                {
                    press = FilePress::Choose;
                }
                let field = a11y::text_edit(
                    ui,
                    &label,
                    egui::TextEdit::singleline(&mut file.path)
                        .hint_text(crate::i18n::t("import.file.path_hint"))
                        .desired_width(ui.available_width()),
                );
                file.path_focused = field.has_focus();
            },
        );
        ui.add_space(10.0);
        if button_icon(
            ui,
            Some(Icon::Upload),
            &crate::i18n::t("import.file.run"),
            None,
            ButtonKind::Primary,
        )
        .clicked()
        {
            press = FilePress::Import;
        }
    }
    press
}

/// The result under the form: the summary, then every exception in a two-column list, then the
/// ignored objects and the fields read but not imported, each under its own heading.
fn draw_result(ui: &mut Ui, state: &FileImportState, show: bool) {
    let t = tokens(ui);
    let reveal = |r: egui::Response| {
        if show {
            r.scroll_to_me(Some(egui::Align::TOP));
        }
    };
    let summary = match state {
        FileImportState::Idle => return,
        FileImportState::Running => {
            ui.add_space(10.0);
            reveal(note(
                ui,
                NoteKind::Info,
                &crate::i18n::t("import.file.running"),
            ));
            return;
        }
        FileImportState::Refused(u) => {
            ui.add_space(10.0);
            reveal(note(ui, NoteKind::Bad, &unsupported_text(u)));
            return;
        }
        FileImportState::Failed(text) => {
            ui.add_space(10.0);
            reveal(note(ui, NoteKind::Bad, &format!("Error: {text}")));
            return;
        }
        FileImportState::Imported(summary) => summary,
    };
    ui.add_space(10.0);
    let kind_of = if summary.errors == 0 {
        NoteKind::Ok
    } else {
        NoteKind::Warn
    };
    reveal(note(
        ui,
        kind_of,
        &format!("{}\n{}", summary_line(summary), summary.path.display()),
    ));
    let sections: [(&str, Vec<(String, String)>); 3] = [
        (
            "import.file.exceptions",
            summary
                .exceptions
                .iter()
                .map(|e| (e.object.clone(), e.reason.clone()))
                .collect(),
        ),
        ("import.file.ignored", split(&summary.ignored_notes)),
        ("import.file.warnings", split(&summary.warnings)),
    ];
    for (key, rows) in sections {
        if rows.is_empty() {
            continue;
        }
        ui.add_space(10.0);
        ui.label(
            RichText::new(crate::i18n::tf(key, &[("count", &rows.len().to_string())]))
                .font(bold_font(ui, TEXT_BODY))
                .color(t.text),
        );
        ui.add_space(4.0);
        for (object, reason) in rows {
            ui.columns(2, |cols| {
                if let [left, right] = cols {
                    para(left, &object, TEXT_BODY, t.text);
                    para(right, &reason, TEXT_BODY, t.text_2);
                }
            });
            ui.add_space(2.0);
        }
    }
}

/// `object: reason` notes as pairs; the object is everything before the first `": "`, so a name
/// holding a colon keeps it.
fn split(notes: &[String]) -> Vec<(String, String)> {
    notes
        .iter()
        .map(|n| match n.split_once("': ") {
            Some((object, reason)) => (format!("{object}'"), reason.to_owned()),
            None => (n.clone(), String::new()),
        })
        .collect()
}
