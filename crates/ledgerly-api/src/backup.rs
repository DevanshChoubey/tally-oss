//! Backup and restore of a company (round s23): the stored backup folder and schedule, backing
//! up now, the scheduled backup with rotation, the list of backups, and restore with the
//! audit-chain check.
//!
//! The model is TallyPrime's (Data > Backup & Restore), as its help describes it:
//! - "Local Backup" (<https://help.tallysolutions.com/local-backup/>, read 2026-11-27): a backup
//!   goes to the "Backup Data Path"; a schedule has a "Backup Frequency" of Once, Daily (the
//!   default), Weekly or Monthly, and "No. of Restorable Versions" (default 7, from 1 to 366)
//!   says how many of its backups are kept for restoring; a second backup of a company is saved
//!   as `_1`, `_2` and so on rather than overwritten.
//! - "Restore Backup Data" (<https://help.tallysolutions.com/restore-backup-data/>, read
//!   2026-11-27): restore picks a backup, newest first by the date and time it was taken, and
//!   restores it to the "Restore Data Path".
//!
//! Differences, kept on purpose and listed in `design/PARITY-GAPS.md`:
//! - The schedule runs when the app opens or closes a company and the backup is due (one day,
//!   seven days or one calendar month after the last scheduled backup). TallyPrime's schedules
//!   run at a set time "even when TallyPrime is closed"; Ledgerly runs no background service.
//! - There is no "Once" frequency (it needs a set time) and no incremental method: every backup is
//!   a whole, verified copy.
//! - Restore never replaces an existing company file (TallyPrime offers Skip or Replace): it
//!   writes a new file, and the audit chain of the restored file is verified first.
//!
//! The folder and the schedule are audited settings in the company file (`SettingChanged` under
//! `backup.*`), so the command line reads the same settings as the app. When no folder was
//! stored, backups go to the `Backups` folder beside the company file.

use std::path::{Path, PathBuf};

use ledgerly_core::Clock;
use ledgerly_store::backup::{
    BackupFile, BackupKind, list_backups, prune_scheduled, restore_verified, write_backup,
};
use ledgerly_store::verify::VerifyReport;

use crate::error::ApiError;
use crate::facade::Api;
use crate::tally_file_import::BACKUP_FOLDER;

/// The setting key of the backup folder (a path; empty for the default beside the file).
pub const BACKUP_FOLDER_KEY: &str = "backup.folder";
/// The setting key of the backup frequency (`off`, `daily`, `weekly` or `monthly`).
pub const BACKUP_FREQUENCY_KEY: &str = "backup.frequency";
/// The setting key of the number of scheduled backups kept.
pub const BACKUP_KEEP_KEY: &str = "backup.keep";

/// TallyPrime's default "No. of Restorable Versions".
pub const DEFAULT_KEEP: u16 = 7;
/// The most scheduled backups that can be kept (TallyPrime allows 1 to 366).
pub const MAX_KEEP: u16 = 366;

/// How often the scheduled backup runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupFrequency {
    /// No schedule: backups are taken only when asked.
    Off,
    /// One day after the last scheduled backup.
    Daily,
    /// Seven days after the last scheduled backup.
    Weekly,
    /// One calendar month after the last scheduled backup.
    Monthly,
}

impl BackupFrequency {
    /// Every frequency, in the order a screen lists them.
    pub const ALL: [Self; 4] = [Self::Off, Self::Daily, Self::Weekly, Self::Monthly];

    /// The stored word.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
        }
    }

    /// The frequency a stored word names, or `None` for a word this build does not know.
    #[must_use]
    pub fn from_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.word() == word)
    }
}

/// The backup settings of a company.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupSettings {
    /// The folder backups go to. `None`: the `Backups` folder beside the company file.
    pub folder: Option<PathBuf>,
    /// How often the scheduled backup runs.
    pub frequency: BackupFrequency,
    /// How many scheduled backups are kept (1 to [`MAX_KEEP`]).
    pub keep: u16,
}

impl Default for BackupSettings {
    /// A company with no stored settings: the default folder, no schedule, and TallyPrime's
    /// default of 7 restorable versions for when a schedule is set.
    fn default() -> Self {
        Self {
            folder: None,
            frequency: BackupFrequency::Off,
            keep: DEFAULT_KEEP,
        }
    }
}

/// A backup that was taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupTaken {
    /// The new backup file.
    pub path: PathBuf,
    /// Older scheduled backups removed to keep the number set, oldest first.
    pub removed: Vec<PathBuf>,
}

/// What a restore did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restored {
    /// The new company file.
    pub path: PathBuf,
    /// The audit-chain check of the restored file (it passed, or the restore was refused).
    pub verify: VerifyReport,
}

impl Api {
    /// The stored backup settings, or the defaults for what was never stored.
    ///
    /// # Errors
    /// [`ApiError::Store`] when the audit log cannot be read; [`ApiError::BackupSetting`] when a
    /// stored value is not one this build reads (it is never guessed).
    pub fn backup_settings(&self) -> Result<BackupSettings, ApiError> {
        let mut settings = BackupSettings::default();
        if let Some(folder) = self.setting(BACKUP_FOLDER_KEY)? {
            settings.folder = (!folder.is_empty()).then(|| PathBuf::from(folder));
        }
        if let Some(word) = self.setting(BACKUP_FREQUENCY_KEY)? {
            settings.frequency =
                BackupFrequency::from_word(&word).ok_or_else(|| ApiError::BackupSetting {
                    key: BACKUP_FREQUENCY_KEY,
                    value: word.clone(),
                    reason: "this build does not know that frequency".to_owned(),
                })?;
        }
        if let Some(text) = self.setting(BACKUP_KEEP_KEY)? {
            settings.keep = parse_keep(&text).ok_or_else(|| ApiError::BackupSetting {
                key: BACKUP_KEEP_KEY,
                value: text.clone(),
                reason: format!("the number kept must be from 1 to {MAX_KEEP}"),
            })?;
        }
        Ok(settings)
    }

    /// Stores the backup settings, each changed value as one audited setting change.
    ///
    /// # Errors
    /// [`ApiError::BackupSetting`] when `keep` is not from 1 to [`MAX_KEEP`] or the folder is not
    /// a path the file can hold (nothing is stored); [`ApiError::Store`] when the change cannot
    /// be written.
    pub fn set_backup_settings(&mut self, settings: &BackupSettings) -> Result<(), ApiError> {
        if settings.keep == 0 || settings.keep > MAX_KEEP {
            return Err(ApiError::BackupSetting {
                key: BACKUP_KEEP_KEY,
                value: settings.keep.to_string(),
                reason: format!("the number kept must be from 1 to {MAX_KEEP}"),
            });
        }
        let folder = match &settings.folder {
            None => String::new(),
            Some(path) => path
                .to_str()
                .ok_or_else(|| ApiError::BackupSetting {
                    key: BACKUP_FOLDER_KEY,
                    value: path.display().to_string(),
                    reason: "the folder's name is not valid text".to_owned(),
                })?
                .to_owned(),
        };
        self.change_setting(BACKUP_FOLDER_KEY, &folder)?;
        self.change_setting(BACKUP_FREQUENCY_KEY, settings.frequency.word())?;
        self.change_setting(BACKUP_KEEP_KEY, &settings.keep.to_string())?;
        Ok(())
    }

    /// The folder backups go to: the stored one, or `Backups` beside the company file.
    ///
    /// # Errors
    /// As [`Api::backup_settings`].
    pub fn backup_folder(&self) -> Result<PathBuf, ApiError> {
        Ok(match self.backup_settings()?.folder {
            Some(folder) => folder,
            None => self.default_backup_folder(),
        })
    }

    /// The `Backups` folder beside the company file.
    #[must_use]
    pub fn default_backup_folder(&self) -> PathBuf {
        self.store()
            .path()
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(BACKUP_FOLDER)
    }

    /// Takes a verified backup now, into the backup folder. A manual backup is never removed by
    /// rotation.
    ///
    /// # Errors
    /// As [`Api::backup_settings`]; [`ApiError::Store`] when the backup cannot be written or
    /// does not verify (nothing but a removed `.partial` file is left).
    pub fn back_up_now(&self) -> Result<PathBuf, ApiError> {
        let folder = self.backup_folder()?;
        Ok(write_backup(
            self.store(),
            &folder,
            self.clock(),
            BackupKind::Manual,
        )?)
    }

    /// The backups of this company in the backup folder, newest first.
    ///
    /// # Errors
    /// As [`Api::backup_settings`]; [`ApiError::Store`] when the folder cannot be read.
    pub fn backups(&self) -> Result<Vec<BackupFile>, ApiError> {
        let folder = self.backup_folder()?;
        Ok(list_backups(&folder, self.store().path())?)
    }

    /// Whether the scheduled backup is due now: a schedule is set and no scheduled backup was
    /// taken within its period (see the module documentation).
    ///
    /// # Errors
    /// As [`Api::backups`].
    pub fn scheduled_backup_due(&self) -> Result<bool, ApiError> {
        let settings = self.backup_settings()?;
        let folder = settings
            .folder
            .clone()
            .unwrap_or_else(|| self.default_backup_folder());
        let last = list_backups(&folder, self.store().path())?
            .into_iter()
            .find(|b| b.kind == BackupKind::Scheduled);
        Ok(is_due(settings.frequency, last.as_ref(), self.clock()))
    }

    /// Takes the scheduled backup when it is due, then removes the oldest scheduled backups so
    /// that the number set remain. Returns `None` when no backup was due.
    ///
    /// # Errors
    /// As [`Api::back_up_now`]. Nothing is removed unless the new backup was written and
    /// verified.
    pub fn run_scheduled_backup(&self) -> Result<Option<BackupTaken>, ApiError> {
        if !self.scheduled_backup_due()? {
            return Ok(None);
        }
        let settings = self.backup_settings()?;
        let folder = settings
            .folder
            .unwrap_or_else(|| self.default_backup_folder());
        let path = write_backup(self.store(), &folder, self.clock(), BackupKind::Scheduled)?;
        let removed = prune_scheduled(&folder, self.store().path(), settings.keep)?;
        Ok(Some(BackupTaken { path, removed }))
    }
}

/// Restores `backup` to a new company file at `target` and verifies its audit chain. The backup
/// is never changed, and `target` must not exist.
///
/// # Errors
/// [`ApiError::Store`] with `StoreError::Io` (kind `AlreadyExists`) when `target` exists,
/// `StoreError::NotACompanyFile` when `backup` is not a company file, and
/// `StoreError::ChainBroken` when the restored file's audit chain does not verify; nothing is
/// left at `target` on any error.
pub fn restore_backup(backup: &Path, target: &Path) -> Result<Restored, ApiError> {
    let (store, verify) = restore_verified(backup, target)?;
    drop(store);
    Ok(Restored {
        path: target.to_owned(),
        verify,
    })
}

/// Reads a stored number kept.
fn parse_keep(text: &str) -> Option<u16> {
    text.parse::<u16>()
        .ok()
        .filter(|n| (1..=MAX_KEEP).contains(n))
}

/// Whether a scheduled backup is due at `clock`'s time, given the last one.
fn is_due(frequency: BackupFrequency, last: Option<&BackupFile>, clock: Clock) -> bool {
    let now = clock.now();
    let Some(last) = last else {
        return !matches!(frequency, BackupFrequency::Off);
    };
    let next = match frequency {
        BackupFrequency::Off => return false,
        BackupFrequency::Daily => last.taken_at.checked_add(time::Duration::days(1)),
        BackupFrequency::Weekly => last.taken_at.checked_add(time::Duration::days(7)),
        BackupFrequency::Monthly => add_month(last.taken_at),
    };
    // A time that cannot be computed is far in the future: not due.
    next.is_some_and(|next| now >= next)
}

/// The same day and time one calendar month later; a day the next month does not have becomes
/// its last day (31 January to 28 or 29 February).
fn add_month(at: time::OffsetDateTime) -> Option<time::OffsetDateTime> {
    let month = at.month().next();
    let year = if month == time::Month::January {
        at.year().checked_add(1)?
    } else {
        at.year()
    };

    let day = at.day().min(month.length(year));
    let date = time::Date::from_calendar_date(year, month, day).ok()?;
    Some(at.replace_date(date))
}
