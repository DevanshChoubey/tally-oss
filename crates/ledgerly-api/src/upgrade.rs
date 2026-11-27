//! Opening a company file made by an older build (round s23): a backup first, then the upgrade,
//! as one audited change.
//!
//! TallyPrime keeps the original company when it migrates data, and offers a backup during the
//! migration as good practice (TallyPrime help, "Data Migration FAQ",
//! <https://help.tallysolutions.com/data-migration-faq/>, read 2026-11-27: the backup is optional
//! and set to No by default). Ledgerly changes the company file itself, and a restore is the only
//! way back, so it always takes a verified backup before it writes an upgrade; the backup goes to
//! the company's backup folder ([`Api::backup_folder`]) and is never removed by rotation. This
//! is a deliberate difference, listed in `design/PARITY-GAPS.md`.
//!
//! What an upgrade does today:
//! - gives a company made before round s22 its predefined Cash and Profit & Loss A/c, through
//!   the engine ([`ledgerly_engine::upgrade::predefined_ledgers_upgrade`]);
//! - records the version of the build that opened the file, when the newest audit record was
//!   written by an older or unreadable version (setting [`APP_VERSION_KEY`]), so the next open
//!   by the same build does not upgrade again.
//!
//! Every change is committed in one transaction, and its audit record names the reason.
//! [`Api::open`] never upgrades; the app and the command line open through
//! [`Api::open_and_upgrade`].

use std::path::{Path, PathBuf};

use ledgerly_core::Outcome;
use ledgerly_engine::upgrade::{PredefinedStep, predefined_ledgers_upgrade};
use ledgerly_store::Event;

use crate::error::ApiError;
use crate::events::read_all;
use crate::facade::Api;

/// The version of this build.
const THIS_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The setting that records the version of the build that last upgraded the file.
pub const APP_VERSION_KEY: &str = "app.version";

/// What the upgrade on open did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeReport {
    /// The version that wrote the newest audit record before the upgrade.
    pub from_version: String,
    /// This build's version.
    pub to_version: &'static str,
    /// The verified backup taken before anything was written.
    pub backup: PathBuf,
    /// What each predefined ledger came to.
    pub ledgers: Vec<PredefinedStep>,
}

impl Api {
    /// Opens the company file at `path` and upgrades it when it was made by an older build
    /// (see the module documentation). Returns the open company and, when an upgrade was
    /// written, what it did.
    ///
    /// # Errors
    /// As [`Api::open`]; as [`Api::upgrade`]. When the backup before the upgrade fails, nothing
    /// is written and the error is returned.
    pub fn open_and_upgrade(path: &Path) -> Result<(Self, Option<UpgradeReport>), ApiError> {
        let mut api = Self::open(path)?;
        let report = api.upgrade()?;
        Ok((api, report))
    }

    /// Upgrades the open company file when it was made by an older build: takes a verified
    /// backup, then commits the upgrade's events as one audited change. Returns `None` when
    /// nothing needed upgrading; running it again after an upgrade returns `None`.
    ///
    /// A company whose books cannot be read (a stored voucher this build cannot replay) gets no
    /// ledger upgrade; only the version is recorded, after the backup.
    ///
    /// # Errors
    /// [`ApiError::Store`] when the log cannot be read, the backup cannot be written or does not
    /// verify (nothing is written then), or the commit fails; [`ApiError::Engine`] when the
    /// engine refuses a predefined ledger; [`ApiError::BackupSetting`] as for
    /// [`Api::backup_settings`].
    pub fn upgrade(&mut self) -> Result<Option<UpgradeReport>, ApiError> {
        let records = read_all(self.store())?;
        let Some(newest) = records.last() else {
            return Ok(None);
        };
        let from_version = newest.meta.app_version.clone();
        let older = is_older(&from_version, THIS_VERSION);
        let (mut events, ledgers) = match self.cached_books()? {
            Outcome::Value(books) => match predefined_ledgers_upgrade(books)? {
                Outcome::Value(plan) => (plan.events, plan.steps),
                Outcome::Unsupported(u) => {
                    tracing::warn!(what = %u.what, reason = %u.reason, "predefined ledgers not upgraded");
                    (Vec::new(), Vec::new())
                }
            },
            Outcome::Unsupported(_) => (Vec::new(), Vec::new()),
        };
        if older {
            events.push(Event::SettingChanged {
                key: APP_VERSION_KEY.to_owned(),
                before: self.setting(APP_VERSION_KEY)?.unwrap_or_default(),
                after: THIS_VERSION.to_owned(),
            });
        }
        if events.is_empty() {
            return Ok(None);
        }
        let backup = self.back_up_now()?;
        let reason = format!(
            "upgrade on open by Ledgerly {THIS_VERSION} (file last written by {from_version}); backup taken first: {}",
            backup.display()
        );
        self.commit_with_reason(&events, reason)?;
        Ok(Some(UpgradeReport {
            from_version,
            to_version: THIS_VERSION,
            backup,
            ledgers,
        }))
    }
}

/// `true` when `found` is an older version than `this`, or is not a version this build can read
/// (an upgrade with a backup is the safe side).
fn is_older(found: &str, this: &str) -> bool {
    match (parse_version(found), parse_version(this)) {
        (Some(found), Some(this)) => found < this,
        (None, _) | (_, None) => found != this,
    }
}

/// `major.minor.patch` with an optional `-pre` part, which sorts before the release.
fn parse_version(text: &str) -> Option<(u64, u64, u64, bool)> {
    let (core, pre) = match text.split_once('-') {
        Some((core, _)) => (core, true),
        None => (text, false),
    };
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    // A release sorts after its pre-releases.
    Some((major, minor, patch, !pre))
}
