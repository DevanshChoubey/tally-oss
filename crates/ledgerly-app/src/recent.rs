//! The companies this person opened lately, kept in a small file in the user's config folder,
//! outside every company file: `<config>/ledgerly/recent-companies.toml`.
//!
//! The list is a convenience for the Start screen. It holds a company's file path and the few
//! words the Start screen shows about it (name, kind and state, financial year, the day it was
//! opened). It never holds a figure. A list that cannot be read is reported, and the Start screen
//! then shows no list; it never guesses one.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The most companies the list keeps. The oldest one drops off.
pub const MAX_RECENT: usize = 10;

/// The file name inside `<config>/ledgerly/`.
const FILE_NAME: &str = "recent-companies.toml";

/// Why the list could not be read or written.
#[derive(Debug, Error)]
pub enum RecentError {
    /// The file or its folder could not be read or written.
    #[error("the recent companies list could not be read or written at {path}: {source}")]
    Io {
        /// The file or folder.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// The file is not a valid list.
    #[error("the recent companies list at {path} is not valid: {reason}")]
    Invalid {
        /// The file.
        path: PathBuf,
        /// What the parser said.
        reason: String,
    },
}

/// One company on the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentCompany {
    /// Where its file is.
    pub path: PathBuf,
    /// The company's name.
    pub name: String,
    /// The line under the name, for example "Partnership firm · Maharashtra".
    pub detail: String,
    /// The financial year, for example "2026-27".
    pub financial_year: String,
    /// The day it was last opened, as text, for example "21 Nov 2026".
    pub last_opened: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Stored {
    #[serde(default)]
    company: Vec<RecentCompany>,
}

/// The folder the list lives in for the real app: `$XDG_CONFIG_HOME/ledgerly`, else
/// `$HOME/.config/ledgerly` (`%APPDATA%\ledgerly` on Windows, `~/Library/Application Support/ledgerly`
/// on macOS). `None` when no absolute location is known; the app then keeps no list.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    let absolute = |v: std::ffi::OsString| Some(PathBuf::from(v)).filter(|p| p.is_absolute());
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").and_then(absolute)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .and_then(absolute)
            .map(|h| h.join("Library").join("Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .and_then(absolute)
            .or_else(|| {
                std::env::var_os("HOME")
                    .and_then(absolute)
                    .map(|h| h.join(".config"))
            })
    };
    base.map(|b| b.join("ledgerly"))
}

/// Reads the list in `dir`, newest first. A folder with no list file is an empty list.
///
/// # Errors
/// [`RecentError`] when the file exists and cannot be read or is not a valid list.
pub fn load(dir: &Path) -> Result<Vec<RecentCompany>, RecentError> {
    let path = dir.join(FILE_NAME);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(RecentError::Io { path, source }),
    };
    let stored: Stored = toml::from_str(&text).map_err(|e| RecentError::Invalid {
        path: path.clone(),
        reason: e.to_string(),
    })?;
    Ok(stored.company)
}

/// Puts `entry` at the top of the list in `dir` (replacing an older line for the same file) and
/// keeps at most [`MAX_RECENT`]. Creates the folder when it is missing.
///
/// # Errors
/// [`RecentError`] when the existing list is not valid or the file cannot be written.
pub fn record(dir: &Path, entry: RecentCompany) -> Result<(), RecentError> {
    let mut list = load(dir)?;
    list.retain(|c| c.path != entry.path);
    list.insert(0, entry);
    list.truncate(MAX_RECENT);
    let path = dir.join(FILE_NAME);
    let text = toml::to_string(&Stored { company: list }).map_err(|e| RecentError::Invalid {
        path: path.clone(),
        reason: e.to_string(),
    })?;
    std::fs::create_dir_all(dir).map_err(|source| RecentError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    std::fs::write(&path, text).map_err(|source| RecentError::Io { path, source })
}

/// What the list says about the company open in `api` at `path`, opened on `today`. `None` when
/// the books are not readable, so nothing half-known is listed.
#[must_use]
pub fn describe(
    api: &ledgerly_api::Api,
    path: &Path,
    today: ledgerly_core::Date,
) -> Option<RecentCompany> {
    use ledgerly_core::{FinancialYear, Outcome};
    use ledgerly_model::EntityKind;
    let Ok(Outcome::Value(books)) = api.books() else {
        return None;
    };
    let kind = match books.company.entity_kind {
        EntityKind::Company => "Company",
        EntityKind::Llp => "Limited liability partnership",
        EntityKind::Partnership => "Partnership firm",
        EntityKind::Proprietorship => "Sole proprietorship",
        EntityKind::Other => "Trust, society or other",
    };
    Some(RecentCompany {
        path: path.to_path_buf(),
        name: books.company.name.clone(),
        detail: format!("{kind} · {}", books.company.state.name()),
        financial_year: FinancialYear {
            start: books.company.fy_start,
        }
        .label(),
        last_opened: crate::ui::format::date(today),
    })
}

/// Opens the company file at `path` and, when the list folder is known, records it there. A list
/// that cannot be written does not stop the company opening.
///
/// # Errors
/// The text to show when the file cannot be opened.
pub fn open_and_record(path: &Path, dir: Option<&Path>) -> Result<ledgerly_api::Api, String> {
    let api = ledgerly_api::Api::open(path)
        .map_err(|e| format!("The company file could not be opened: {e}"))?;
    note_opened(&api, path, dir);
    Ok(api)
}

/// Records the open company in the list in `dir`, if there is one.
pub fn note_opened(api: &ledgerly_api::Api, path: &Path, dir: Option<&Path>) {
    let Some(dir) = dir else { return };
    let Some(entry) = describe(api, path, crate::shell::navigate::today()) else {
        return;
    };
    // The list is a convenience; a failed write must not stop the company opening.
    let _unrecorded = record(dir, entry);
}
