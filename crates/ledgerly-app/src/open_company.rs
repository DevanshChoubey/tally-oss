//! Opening a company file from the app (round s23): the upgrade of a file made by an older build,
//! with a backup first, and the scheduled backup when one is due.
//!
//! Every open path of the app (Start, Company setup's Open, the welcome form) goes through
//! [`open_company`]. What happened on open that the user must know about is an [`OpenNote`]; it
//! waits in [`take_pending`] until the home screen of the opened company shows it on the Backups
//! and updates screen (mock `backup.upgraded`).

use std::path::Path;
use std::sync::Mutex;

use ledgerly_api::{Api, UpgradeReport};

/// What the user must be told about an open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenNote {
    /// The upgrade written on open, with the backup taken first.
    pub upgrade: Option<UpgradeReport>,
    /// Why the file was not upgraded, when an upgrade was due and its backup or commit failed.
    /// The file then opens as it was.
    pub not_upgraded: Option<String>,
    /// Why a due scheduled backup could not be taken.
    pub scheduled_failed: Option<String>,
}

impl OpenNote {
    /// `true` when there is nothing to say.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.upgrade.is_none() && self.not_upgraded.is_none() && self.scheduled_failed.is_none()
    }
}

/// The note of the last open, until the home screen takes it.
static PENDING: Mutex<Option<OpenNote>> = Mutex::new(None);

/// Opens the company file at `path`: upgrades it when an older build made it (after a verified
/// backup), then takes the scheduled backup when one is due. A failed upgrade or scheduled backup
/// does not stop the company opening; the note says why.
///
/// # Errors
/// The text to show when the file cannot be opened at all.
pub fn open_company(path: &Path) -> Result<(Api, Option<OpenNote>), String> {
    let tr = crate::i18n::current();
    let (api, upgrade, not_upgraded) = match Api::open_and_upgrade(path) {
        Ok((api, upgrade)) => (api, upgrade, None),
        Err(upgrade_error) => {
            // The open itself or the upgrade failed. Open again without upgrading: when that
            // works, the upgrade (or its backup) was what failed, and nothing was written.
            let api = Api::open(path)
                .map_err(|e| tr.tf("open.notice.failed", &[("error", &e.to_string())]))?;
            (api, None, Some(upgrade_error.to_string()))
        }
    };
    let scheduled_failed = api.run_scheduled_backup().err().map(|e| e.to_string());
    let note = OpenNote {
        upgrade,
        not_upgraded,
        scheduled_failed,
    };
    Ok((api, (!note.is_empty()).then_some(note)))
}

/// [`open_company`], keeping the note for the home screen ([`take_pending`]).
///
/// # Errors
/// As [`open_company`].
pub fn open_company_noted(path: &Path) -> Result<Api, String> {
    let (api, note) = open_company(path)?;
    if let Ok(mut slot) = PENDING.lock() {
        *slot = note;
    }
    Ok(api)
}

/// Takes the note of the last open, if any.
pub fn take_pending() -> Option<OpenNote> {
    PENDING.lock().ok().and_then(|mut slot| slot.take())
}
