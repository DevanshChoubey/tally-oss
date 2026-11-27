//! Backups and updates (mock `backup.*`, round s23): the backup settings stored with the company,
//! Back up now, the backups in the folder (newest first), a restore that writes a new file and
//! checks its audit chain, what an upgrade on open did, and the opt-in update check.
//!
//! Behaviour follows TallyPrime's help pages (read 2026-11-27): "Local Backup"
//! (<https://help.tallysolutions.com/local-backup/>: a backup data path, a backup frequency that
//! defaults to Daily, "No. of Restorable Versions" 7 by default, 1 to 366, and backups that never
//! overwrite one another) and "Restore Backup Data"
//! (<https://help.tallysolutions.com/restore-backup-data/>: versions listed by date and time of
//! backup, newest first). Ledgerly also checks the audit chain of every backup and restore, and a
//! restore never replaces a file (the product offers Replace; see `design/PARITY-GAPS.md`).
//!
//! Every company call runs on the API worker; the update check runs on its own thread, only when
//! the user turned it on and pressed Check now. No figure is drawn here.

#![allow(clippy::float_arithmetic, reason = "layout in points")]

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};

use egui::{Key, Modifiers, Ui};
use ledgerly_api::StoreError;
use ledgerly_api::update_check::{UpdateCheck, UpdateChecker};
use ledgerly_api::{
    ApiError, BackupFile, BackupFrequency, BackupKind, BackupSettings, MAX_KEEP, PredefinedStep,
    Restored,
};
use ledgerly_model::ReservedLedger;

use crate::i18n::Tr;
use crate::open_company::OpenNote;
use crate::screen::KeyHint;
use crate::theme::TEXT_META;
use crate::ui::button::{ButtonKind, button};
use crate::ui::card::card;
use crate::ui::icon::Icon;
use crate::ui::mock_table::{self, Width, panel_title};
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::page_head;
use crate::ui::tabs::{tab_strip_with_keys, tab_strip_with_keys_width};
use crate::ui::{para, tokens};
use crate::worker::{Poll, Ticket};
use crate::{AppCtx, Screen, ScreenAction};

/// Which tab is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// Backup and restore.
    Backup,
    /// The update check.
    Updates,
}

/// What the worker read for the screen.
#[derive(Debug, Clone)]
struct Loaded {
    /// The stored settings.
    settings: BackupSettings,
    /// The Backups folder beside the company file.
    default_folder: PathBuf,
    /// The company file.
    company_file: PathBuf,
    /// The backups in the backup folder, newest first.
    backups: Vec<BackupFile>,
}

/// A finished job of the screen.
#[derive(Debug)]
enum Done {
    /// Settings saved, or why not.
    Saved(Result<(), String>),
    /// A backup taken, or why not.
    Taken(Result<PathBuf, String>),
    /// A restore, or why not.
    Restored(Result<Restored, String>),
}

/// The Backups and updates screen.
#[derive(Debug)]
pub struct BackupScreen {
    tab: Tab,
    open_note: Option<OpenNote>,
    loaded: Option<Loaded>,
    load: Option<Ticket<Result<Loaded, String>>>,
    folder: String,
    frequency: BackupFrequency,
    versions: String,
    selected: usize,
    restoring: bool,
    restore_to: String,
    job: Option<Ticket<Done>>,
    notice: Option<(NoteKind, String)>,
    pref_dir: Option<PathBuf>,
    update_on: bool,
    pref_error: Option<String>,
    checker: UpdateChecker,
    update_job: Option<Receiver<UpdateCheck>>,
    update_result: Option<UpdateCheck>,
}

impl Default for BackupScreen {
    fn default() -> Self {
        Self::new(Tab::Backup)
    }
}

impl BackupScreen {
    /// The screen on `tab`, with the update choice read from the user's config folder.
    #[must_use]
    pub fn new(tab: Tab) -> Self {
        Self::with_update_check(tab, crate::recent::default_dir(), UpdateChecker::github())
    }

    /// The screen showing what happened when the company was opened (mock `backup.upgraded`).
    #[must_use]
    pub fn after_open(note: OpenNote) -> Self {
        let mut screen = Self::new(Tab::Backup);
        screen.open_note = Some(note);
        screen
    }

    /// The screen with the update choice kept in `pref_dir` and checks made by `checker`. Tests
    /// point both at temporary places; the app uses [`BackupScreen::new`].
    #[must_use]
    pub fn with_update_check(tab: Tab, pref_dir: Option<PathBuf>, checker: UpdateChecker) -> Self {
        let (update_on, pref_error) = match pref_dir.as_deref().map(crate::update_pref::load) {
            Some(Ok(on)) => (on, None),
            Some(Err(e)) => (false, Some(e)),
            None => (false, None),
        };
        Self {
            tab,
            open_note: None,
            loaded: None,
            load: None,
            folder: String::new(),
            frequency: BackupFrequency::Off,
            versions: String::new(),
            selected: 0,
            restoring: false,
            restore_to: String::new(),
            job: None,
            notice: None,
            pref_dir,
            update_on,
            pref_error,
            checker,
            update_job: None,
            update_result: None,
        }
    }

    /// `true` once the settings and the backup list are read.
    #[must_use]
    pub fn loaded(&self) -> bool {
        self.loaded.is_some() && self.load.is_none()
    }

    /// `true` while a save, backup or restore runs.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.load.is_some() || self.update_job.is_some()
    }

    /// The backups listed, newest first.
    #[must_use]
    pub fn backups(&self) -> &[BackupFile] {
        self.loaded.as_ref().map_or(&[], |l| l.backups.as_slice())
    }

    /// What the last action said, if anything.
    #[must_use]
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_ref().map(|(_, text)| text.as_str())
    }

    /// The last update check's result.
    #[must_use]
    pub fn update_result(&self) -> Option<&UpdateCheck> {
        self.update_result.as_ref()
    }

    /// Puts `path` in the "Restore to" field, as typing it does.
    pub fn set_restore_to(&mut self, path: impl Into<String>) {
        self.restore_to = path.into();
    }

    /// Puts `text` in the backup folder field, as typing it does.
    pub fn set_folder(&mut self, text: impl Into<String>) {
        self.folder = text.into();
    }

    /// Chooses the scheduled backup frequency, as clicking it does.
    pub fn set_frequency(&mut self, frequency: BackupFrequency) {
        self.frequency = frequency;
    }

    /// Puts `text` in the restorable versions field, as typing it does.
    pub fn set_versions(&mut self, text: impl Into<String>) {
        self.versions = text.into();
    }

    /// Selects the backup at `index` of the list (clamped), as clicking its row does.
    pub fn select(&mut self, index: usize) {
        self.selected = index.min(self.backups().len().saturating_sub(1));
    }

    fn start_load(&mut self, ctx: &AppCtx) {
        let Some(worker) = ctx.api.as_ref() else {
            return;
        };
        self.load = Some(worker.run(|api| {
            let settings = api.backup_settings().map_err(|e| e.to_string())?;
            let backups = api.backups().map_err(|e| e.to_string())?;
            Ok(Loaded {
                settings,
                default_folder: api.default_backup_folder(),
                company_file: api.company_file().to_path_buf(),
                backups,
            })
        }));
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one arm per job and reply, read top to bottom"
    )]
    fn collect(&mut self, ctx: &AppCtx) {
        if let Some(ticket) = self.load.as_mut() {
            match ticket.poll() {
                Poll::Pending => {}
                Poll::Ready(Ok(loaded)) => {
                    let first = self.loaded.is_none();
                    if first {
                        self.folder = loaded
                            .settings
                            .folder
                            .as_ref()
                            .map_or_else(String::new, |f| f.display().to_string());
                        self.frequency = loaded.settings.frequency;
                        self.versions = loaded.settings.keep.to_string();
                    }
                    self.selected = self.selected.min(loaded.backups.len().saturating_sub(1));
                    self.loaded = Some(loaded);
                    self.load = None;
                }
                Poll::Ready(Err(reason)) => {
                    self.notice = Some((NoteKind::Bad, reason));
                    self.load = None;
                }
                Poll::Taken | Poll::Lost => self.load = None,
            }
        }
        if let Some(ticket) = self.job.as_mut() {
            let done = match ticket.poll() {
                Poll::Pending => return,
                Poll::Ready(done) => Some(done),
                Poll::Taken | Poll::Lost => None,
            };
            self.job = None;
            let tr = crate::i18n::current();
            self.notice = match done {
                Some(Done::Saved(Ok(()))) => Some((NoteKind::Ok, tr.t("backup.notice.saved"))),
                Some(Done::Taken(Ok(path))) => Some((
                    NoteKind::Ok,
                    tr.tf(
                        "backup.notice.taken",
                        &[("path", &path.display().to_string())],
                    ),
                )),
                Some(Done::Restored(Ok(restored))) => {
                    self.restoring = false;
                    Some((
                        NoteKind::Ok,
                        tr.tf(
                            "backup.notice.restored",
                            &[
                                ("path", &restored.path.display().to_string()),
                                ("count", &restored.verify.events_checked.to_string()),
                            ],
                        ),
                    ))
                }
                Some(Done::Saved(Err(e)) | Done::Taken(Err(e)) | Done::Restored(Err(e))) => {
                    Some((NoteKind::Bad, e))
                }
                None => Some((
                    NoteKind::Bad,
                    tr.tf("backup.notice.failed", &[("error", "the worker stopped")]),
                )),
            };
            self.start_load(ctx);
        }
        if let Some(rx) = self.update_job.as_ref() {
            match rx.try_recv() {
                Ok(found) => {
                    self.update_result = Some(found);
                    self.update_job = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.update_result = Some(UpdateCheck::CouldNotCheck {
                        reason: "the check stopped".to_owned(),
                    });
                    self.update_job = None;
                }
            }
        }
    }

    /// Saves the settings typed. A versions field that is not 1 to 366 is refused here, with the
    /// reason, before anything is sent.
    pub fn save(&mut self, ctx: &AppCtx) {
        if self.busy() {
            return;
        }
        let tr = crate::i18n::current();
        let keep = match self.versions.trim().parse::<u16>() {
            Ok(n) if (1..=MAX_KEEP).contains(&n) => n,
            Ok(_) | Err(_) => {
                self.notice = Some((NoteKind::Bad, tr.t("backup.notice.versions_bad")));
                return;
            }
        };
        let folder = self.folder.trim();
        let settings = BackupSettings {
            folder: (!folder.is_empty()).then(|| PathBuf::from(folder)),
            frequency: self.frequency,
            keep,
        };
        let Some(worker) = ctx.api.as_ref() else {
            return;
        };
        self.job = Some(worker.run(move |api| {
            Done::Saved(
                api.set_backup_settings(&settings)
                    .map_err(|e| e.to_string()),
            )
        }));
    }

    /// Takes a backup now, into the stored folder.
    pub fn back_up_now(&mut self, ctx: &AppCtx) {
        if self.busy() {
            return;
        }
        let Some(worker) = ctx.api.as_ref() else {
            return;
        };
        self.job =
            Some(worker.run(|api| Done::Taken(api.back_up_now().map_err(|e| e.to_string()))));
    }

    /// Opens the restore pane for the selected backup, with a new file name beside the company.
    pub fn start_restore(&mut self) {
        let Some(loaded) = self.loaded.as_ref() else {
            return;
        };
        if loaded.backups.get(self.selected).is_none() {
            return;
        }
        if self.restore_to.trim().is_empty() {
            self.restore_to = restore_target(&loaded.company_file).display().to_string();
        }
        self.restoring = true;
    }

    /// Restores the selected backup to the path in "Restore to".
    pub fn restore(&mut self, ctx: &AppCtx) {
        if self.busy() || !self.restoring {
            return;
        }
        let tr = crate::i18n::current();
        let Some(backup) = self
            .loaded
            .as_ref()
            .and_then(|l| l.backups.get(self.selected))
            .map(|b| b.path.clone())
        else {
            return;
        };
        let target = PathBuf::from(self.restore_to.trim());
        if self.restore_to.trim().is_empty() {
            self.notice = Some((NoteKind::Bad, tr.t("backup.notice.no_target")));
            return;
        }
        let Some(worker) = ctx.api.as_ref() else {
            return;
        };
        self.job = Some(worker.run(move |_api| {
            Done::Restored(
                ledgerly_api::restore_backup(&backup, &target).map_err(|e| restore_error(&e, &tr)),
            )
        }));
    }

    /// Turns the update check on or off and stores the choice.
    pub fn set_update_on(&mut self, on: bool) {
        let tr = crate::i18n::current();
        self.pref_error = match self.pref_dir.as_deref() {
            Some(dir) => crate::update_pref::save(dir, on)
                .err()
                .map(|e| tr.tf("updates.pref_failed", &[("error", &e)])),
            None => None,
        };
        // Off is always honoured, even when it could not be stored.
        self.update_on = on && self.pref_error.is_none();
        if !self.update_on {
            self.update_result = None;
        }
    }

    /// Runs the update check, when it is on.
    pub fn check_now(&mut self) {
        if !self.update_on || self.update_job.is_some() {
            return;
        }
        let checker = self.checker.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("ledgerly-update-check".to_owned())
            .spawn(move || {
                let _unwanted = tx.send(checker.check(env!("CARGO_PKG_VERSION")));
            });
        match spawned {
            Ok(_thread) => {
                self.update_result = None;
                self.update_job = Some(rx);
            }
            Err(e) => {
                self.update_result = Some(UpdateCheck::CouldNotCheck {
                    reason: e.to_string(),
                });
            }
        }
    }

    fn keys_pressed(&mut self, ui: &mut Ui, ctx: &AppCtx) {
        let typing = ui.ctx().egui_wants_keyboard_input();
        let (one, two, alt_b, enter, ctrl_a, u, up, down, esc) = ui.input_mut(|i| {
            (
                !typing && i.consume_key(Modifiers::NONE, Key::Num1),
                !typing && i.consume_key(Modifiers::NONE, Key::Num2),
                i.consume_key(Modifiers::ALT, Key::B),
                !typing && i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::COMMAND, Key::A),
                !typing && i.consume_key(Modifiers::NONE, Key::U),
                !typing && i.consume_key(Modifiers::NONE, Key::ArrowUp),
                !typing && i.consume_key(Modifiers::NONE, Key::ArrowDown),
                self.restoring && i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if one {
            self.tab = Tab::Backup;
        }
        if two {
            self.tab = Tab::Updates;
        }
        match self.tab {
            Tab::Backup => {
                if alt_b {
                    self.back_up_now(ctx);
                }
                if up {
                    self.selected = self.selected.saturating_sub(1);
                }
                if down {
                    self.select(self.selected.saturating_add(1));
                }
                if enter && !self.restoring {
                    self.start_restore();
                }
                if ctrl_a {
                    if self.restoring {
                        self.restore(ctx);
                    } else {
                        self.save(ctx);
                    }
                }
                if esc {
                    self.restoring = false;
                }
            }
            Tab::Updates => {
                if u {
                    self.check_now();
                }
            }
        }
    }

    fn draw_open_note(ui: &mut Ui, note_of: &OpenNote, tr: &Tr) {
        if let Some(up) = &note_of.upgrade {
            let mut lines = vec![
                tr.tf("backup.upgraded.title", &[("from", &up.from_version)]),
                tr.tf(
                    "backup.upgraded.backup",
                    &[("path", &up.backup.display().to_string())],
                ),
            ];
            for step in &up.ledgers {
                match step {
                    PredefinedStep::AlreadyThere { .. } => {}
                    PredefinedStep::Created { name, .. } => {
                        lines.push(tr.tf("backup.upgraded.created", &[("name", name)]));
                    }
                    PredefinedStep::Adopted { name, .. } => {
                        lines.push(tr.tf("backup.upgraded.adopted", &[("name", name)]));
                    }
                    PredefinedStep::NotDone { role, reason } => lines.push(tr.tf(
                        "backup.upgraded.not_done",
                        &[("name", role_name(*role)), ("reason", reason)],
                    )),
                }
            }
            lines.push(tr.t("backup.upgraded.audit"));
            note(ui, NoteKind::Warn, &lines.join("\n"));
            ui.add_space(12.0);
        }
        if let Some(reason) = &note_of.not_upgraded {
            note(
                ui,
                NoteKind::Bad,
                &tr.tf("backup.not_upgraded", &[("reason", reason)]),
            );
            ui.add_space(12.0);
        }
        if let Some(reason) = &note_of.scheduled_failed {
            note(
                ui,
                NoteKind::Bad,
                &tr.tf("backup.scheduled_failed", &[("reason", reason)]),
            );
            ui.add_space(12.0);
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one tab's layout, read top to bottom"
    )]
    fn draw_backup(&mut self, ui: &mut Ui, ctx: &AppCtx) {
        let tr = &ctx.tr;
        let t = tokens(ui);
        let Some(loaded) = self.loaded.clone() else {
            para(ui, &tr.t("backup.loading"), TEXT_META, t.text_2);
            return;
        };
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            ui.columns(2, |cols| {
                let [left, right] = cols else { return };
                card(left, |ui| {
                    ui.set_min_width(ui.available_width());
                    panel_title(ui, Some(Icon::Save), &tr.t("backup.settings"));
                    ui.add_space(8.0);
                    para(ui, &tr.t("backup.field.folder"), TEXT_META, t.text_2);
                    crate::ui::a11y::text_edit(
                        ui,
                        &tr.t("backup.field.folder"),
                        egui::TextEdit::singleline(&mut self.folder)
                            .desired_width(ui.available_width()),
                    );
                    para(
                        ui,
                        &tr.tf(
                            "backup.folder_hint",
                            &[("folder", &loaded.default_folder.display().to_string())],
                        ),
                        TEXT_META,
                        t.text_2,
                    );
                    ui.add_space(8.0);
                    para(ui, &tr.t("backup.field.schedule"), TEXT_META, t.text_2);
                    ui.horizontal(|ui| {
                        for frequency in BackupFrequency::ALL {
                            let kind = if frequency == self.frequency {
                                ButtonKind::Primary
                            } else {
                                ButtonKind::Secondary
                            };
                            if button(ui, &tr.t(frequency_key(frequency)), None, kind).clicked() {
                                self.frequency = frequency;
                            }
                        }
                    });
                    ui.add_space(8.0);
                    para(ui, &tr.t("backup.field.versions"), TEXT_META, t.text_2);
                    crate::ui::a11y::text_edit(
                        ui,
                        &tr.t("backup.field.versions"),
                        egui::TextEdit::singleline(&mut self.versions).desired_width(80.0),
                    );
                    para(ui, &tr.t("backup.versions_hint"), TEXT_META, t.text_2);
                    ui.add_space(8.0);
                    para(ui, &tr.t("backup.schedule_note"), TEXT_META, t.text_2);
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if button(
                            ui,
                            &tr.t("backup.save"),
                            Some("Ctrl+A"),
                            ButtonKind::Primary,
                        )
                        .clicked()
                        {
                            self.save(ctx);
                        }
                        if button(
                            ui,
                            &tr.t("backup.now"),
                            Some("Alt+B"),
                            ButtonKind::Secondary,
                        )
                        .clicked()
                        {
                            self.back_up_now(ctx);
                        }
                    });
                });
                card(right, |ui| {
                    ui.set_min_width(ui.available_width());
                    panel_title(ui, Some(Icon::Clock), &tr.t("backup.list"));
                    para(ui, &tr.t("backup.list_lead"), TEXT_META, t.text_2);
                    ui.add_space(8.0);
                    if loaded.backups.is_empty() {
                        para(ui, &tr.t("backup.none"), TEXT_META, t.text_2);
                        return;
                    }
                    let rows: Vec<(String, String, String)> = loaded
                        .backups
                        .iter()
                        .map(|b| {
                            (
                                taken_text(b),
                                tr.t(kind_key(b.kind)),
                                b.path
                                    .file_name()
                                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
                            )
                        })
                        .collect();
                    let head = [
                        tr.t("backup.col.taken"),
                        tr.t("backup.col.kind"),
                        tr.t("backup.col.file"),
                    ];
                    let head: Vec<&str> = head.iter().map(String::as_str).collect();
                    let cells = rows
                        .iter()
                        .map(|(taken, kind, file)| {
                            vec![
                                mock_table::text(taken),
                                mock_table::text(kind),
                                mock_table::muted(file),
                            ]
                        })
                        .collect();
                    let (_, rects) = mock_table::table_rows(
                        ui,
                        &[Width::Fixed(150.0), Width::Fixed(96.0), Width::Rest],
                        Some(&head),
                        cells,
                        Some(self.selected),
                    );
                    for (index, rect) in rects.iter().enumerate() {
                        if ui
                            .interact(
                                *rect,
                                ui.scope_id().with(("backup-row", index)),
                                egui::Sense::click(),
                            )
                            .clicked()
                        {
                            self.selected = index;
                        }
                    }
                    ui.add_space(10.0);
                    if button(
                        ui,
                        &tr.t("backup.restore_selected"),
                        Some("Enter"),
                        ButtonKind::Secondary,
                    )
                    .clicked()
                    {
                        self.start_restore();
                    }
                });
            });
        });
        if self.restoring {
            let taken = loaded
                .backups
                .get(self.selected)
                .map_or_else(String::new, taken_text);
            ui.add_space(14.0);
            card(ui, |ui| {
                panel_title(
                    ui,
                    Some(Icon::Upload),
                    &tr.tf("backup.restore_title", &[("taken", &taken)]),
                );
                para(ui, &tr.t("backup.restore_lead"), TEXT_META, t.text_2);
                ui.add_space(8.0);
                para(ui, &tr.t("backup.field.restore_to"), TEXT_META, t.text_2);
                crate::ui::a11y::text_edit(
                    ui,
                    &tr.t("backup.field.restore_to"),
                    egui::TextEdit::singleline(&mut self.restore_to)
                        .desired_width(ui.available_width().min(640.0)),
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if button(
                        ui,
                        &tr.t("backup.restore"),
                        Some("Ctrl+A"),
                        ButtonKind::Primary,
                    )
                    .clicked()
                    {
                        self.restore(ctx);
                    }
                    if button(
                        ui,
                        &tr.t("backup.cancel"),
                        Some("Esc"),
                        ButtonKind::Secondary,
                    )
                    .clicked()
                    {
                        self.restoring = false;
                    }
                });
            });
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one tab's layout, read top to bottom"
    )]
    fn draw_updates(&mut self, ui: &mut Ui, tr: &Tr) {
        let t = tokens(ui);
        card(ui, |ui| {
            ui.horizontal(|ui| {
                let response = crate::ui::switch::switch(
                    ui,
                    self.update_on,
                    egui::Sense::click(),
                    &tr.t("updates.switch"),
                );
                if response.clicked() {
                    self.set_update_on(!self.update_on);
                }
                ui.label(
                    egui::RichText::new(tr.t("updates.switch"))
                        .font(crate::ui::bold_font(ui, crate::theme::TEXT_BODY))
                        .color(t.text),
                );
            });
            para(ui, &tr.t("updates.privacy"), TEXT_META, t.text_2);
            if let Some(e) = &self.pref_error {
                ui.add_space(8.0);
                note(ui, NoteKind::Bad, e);
            }
            ui.add_space(12.0);
            let label = if self.update_job.is_some() {
                tr.t("updates.checking")
            } else {
                tr.t("updates.check")
            };
            let enabled = self.update_on && self.update_job.is_none();
            let clicked = ui
                .add_enabled_ui(enabled, |ui| {
                    button(ui, &label, Some("U"), ButtonKind::Primary)
                })
                .inner
                .clicked();
            if clicked {
                self.check_now();
            }
            ui.add_space(12.0);
            if !self.update_on {
                para(ui, &tr.t("updates.off"), crate::theme::TEXT_BODY, t.text_2);
                return;
            }
            let this = env!("CARGO_PKG_VERSION");
            match &self.update_result {
                None => {}
                Some(UpdateCheck::UpToDate { latest }) => {
                    note(
                        ui,
                        NoteKind::Ok,
                        &tr.tf("updates.none", &[("this", this), ("latest", latest)]),
                    );
                }
                Some(UpdateCheck::CouldNotCheck { reason }) => {
                    note(
                        ui,
                        NoteKind::Warn,
                        &tr.tf("updates.failed", &[("reason", reason)]),
                    );
                }
                Some(UpdateCheck::ChecksumsDisagree { reason }) => {
                    note(
                        ui,
                        NoteKind::Bad,
                        &tr.tf("updates.disagree", &[("reason", reason)]),
                    );
                }
                Some(UpdateCheck::Available(release)) => {
                    note(
                        ui,
                        NoteKind::Ok,
                        &tr.tf(
                            "updates.available",
                            &[("version", &release.version), ("this", this)],
                        ),
                    );
                    ui.add_space(10.0);
                    let head = [tr.t("updates.col.file"), tr.t("updates.col.sha")];
                    let head: Vec<&str> = head.iter().map(String::as_str).collect();
                    let cells = release
                        .assets
                        .iter()
                        .map(|a| vec![mock_table::text(&a.name), mock_table::muted(&a.sha256)])
                        .collect();
                    mock_table::table(ui, &[Width::Fixed(320.0), Width::Rest], Some(&head), cells);
                    ui.add_space(8.0);
                    para(
                        ui,
                        &tr.tf("updates.page", &[("url", &release.page_url)]),
                        crate::theme::TEXT_BODY,
                        t.text,
                    );
                    para(ui, &tr.t("updates.how"), TEXT_META, t.text_2);
                }
            }
        });
    }
}

/// A file name for the restored company beside `company`: `<stem>-restored.ledgerly`, or
/// `<stem>-restored-2.ledgerly` and so on when that exists.
#[must_use]
pub fn restore_target(company: &Path) -> PathBuf {
    let stem = company.file_stem().map_or_else(
        || "company".to_owned(),
        |s| s.to_string_lossy().into_owned(),
    );
    let dir = company.parent().unwrap_or_else(|| Path::new("."));
    let first = dir.join(format!("{stem}-restored.ledgerly"));
    if !first.exists() {
        return first;
    }
    (2_u32..10_000)
        .map(|n| dir.join(format!("{stem}-restored-{n}.ledgerly")))
        .find(|p| !p.exists())
        .unwrap_or(first)
}

/// The words for a failed restore: a broken chain names its record; anything else says what
/// the store said.
fn restore_error(e: &ApiError, tr: &Tr) -> String {
    if let ApiError::Store(StoreError::ChainBroken(seq)) = e {
        return tr.tf("backup.notice.refused", &[("seq", &seq.0.to_string())]);
    }
    tr.tf("backup.notice.failed", &[("error", &e.to_string())])
}

/// "21 Nov 2026, 16:00", the backup's time in India.
fn taken_text(b: &BackupFile) -> String {
    let at = match time::UtcOffset::from_hms(5, 30, 0) {
        Ok(india) => b.taken_at.to_offset(india),
        Err(_) => b.taken_at,
    };
    format!(
        "{}, {:02}:{:02}",
        crate::ui::format::date(at.date()),
        at.hour(),
        at.minute()
    )
}

fn frequency_key(frequency: BackupFrequency) -> &'static str {
    match frequency {
        BackupFrequency::Off => "backup.freq.off",
        BackupFrequency::Daily => "backup.freq.daily",
        BackupFrequency::Weekly => "backup.freq.weekly",
        BackupFrequency::Monthly => "backup.freq.monthly",
    }
}

fn kind_key(kind: BackupKind) -> &'static str {
    match kind {
        BackupKind::Manual => "backup.kind.manual",
        BackupKind::Scheduled => "backup.kind.scheduled",
    }
}

fn role_name(role: ReservedLedger) -> &'static str {
    match role {
        ReservedLedger::Cash => "Cash",
        ReservedLedger::ProfitAndLoss => "Profit & Loss A/c",
        ReservedLedger::InvoiceRounding => "Invoice Rounding",
    }
}

impl Screen for BackupScreen {
    fn title(&self, tr: &Tr) -> String {
        tr.t("backup.title")
    }

    fn subtitle(&self, tr: &Tr) -> Option<String> {
        Some(match self.tab {
            Tab::Backup => tr.t("backup.subtitle"),
            Tab::Updates => tr.t("backup.updates_subtitle"),
        })
    }

    fn keys(&self, tr: &Tr) -> Vec<KeyHint> {
        match self.tab {
            Tab::Backup => vec![
                KeyHint::new("Alt+B", tr.t("backup.now")),
                KeyHint::new("Enter", tr.t("backup.restore_selected")),
                KeyHint::new(
                    "Ctrl+A",
                    if self.restoring {
                        tr.t("backup.restore")
                    } else {
                        tr.t("backup.save")
                    },
                ),
                KeyHint::new("2", tr.t("backup.tab.updates")),
            ],
            Tab::Updates => vec![
                KeyHint::new("U", tr.t("updates.check")),
                KeyHint::new("1", tr.t("backup.tab.backup")),
            ],
        }
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        if self.loaded.is_none() && self.load.is_none() {
            self.start_load(ctx);
        }
        self.collect(ctx);
        if self.busy() {
            ui.ctx().request_repaint();
        }
        self.keys_pressed(ui, ctx);
        let title = self.title(&ctx.tr);
        let subtitle = self.subtitle(&ctx.tr);
        let names = [
            ctx.tr.t("backup.tab.backup"),
            ctx.tr.t("backup.tab.updates"),
        ];
        let items = [(names[0].as_str(), "1"), (names[1].as_str(), "2")];
        let selected = match self.tab {
            Tab::Backup => 0,
            Tab::Updates => 1,
        };
        let mut clicked = None;
        page_head(ui, &title, subtitle.as_deref(), |ui| {
            let size = egui::vec2(tab_strip_with_keys_width(ui, &items), ui.available_height());
            clicked = ui
                .allocate_ui_with_layout(
                    size,
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| tab_strip_with_keys(ui, &items, Some(selected)).clicked,
                )
                .inner;
        });
        match clicked {
            Some(0) => self.tab = Tab::Backup,
            Some(_) => self.tab = Tab::Updates,
            None => {}
        }
        let tr = ctx.tr.clone();
        match self.tab {
            Tab::Backup => {
                if let Some(open_note) = self.open_note.clone() {
                    Self::draw_open_note(ui, &open_note, &tr);
                }
                if ctx.api.is_none() {
                    note(ui, NoteKind::Info, &tr.t("backup.no_company"));
                    return ScreenAction::Stay;
                }
                if let Some((kind, text)) = &self.notice {
                    note(ui, *kind, text);
                    ui.add_space(12.0);
                }
                self.draw_backup(ui, ctx);
            }
            Tab::Updates => self.draw_updates(ui, &tr),
        }
        ScreenAction::Stay
    }
}
