//! The only place the app raises a native file dialog.
//!
//! Dialogs are off until the real window calls [`enable_system_dialogs`] (from `shell::run`), so
//! no test, capture or headless run can put a dialog on somebody's desktop: every function here
//! answers "cancelled" (`None`) at once while they are off.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the real window has switched native file dialogs on.
static SYSTEM_DIALOGS: AtomicBool = AtomicBool::new(false);

/// Lets the app raise native file dialogs. Only the real window calls this.
pub fn enable_system_dialogs() {
    SYSTEM_DIALOGS.store(true, Ordering::SeqCst);
}

/// `true` once [`enable_system_dialogs`] was called.
#[must_use]
pub fn system_dialogs_enabled() -> bool {
    SYSTEM_DIALOGS.load(Ordering::SeqCst)
}

/// A save dialog: the path chosen, or `None` when cancelled (or dialogs are off). `filters` are
/// `(name, extensions)` pairs.
#[must_use]
pub fn save_file(
    title: &str,
    suggested: Option<&str>,
    filters: &[(&str, &[&str])],
) -> Option<PathBuf> {
    if !system_dialogs_enabled() {
        return None;
    }
    let mut dialog = rfd::FileDialog::new().set_title(title);
    if let Some(name) = suggested {
        dialog = dialog.set_file_name(name);
    }
    for (name, extensions) in filters {
        dialog = dialog.add_filter(*name, extensions);
    }
    dialog.save_file()
}

/// An open dialog: the path chosen, or `None` when cancelled (or dialogs are off).
#[must_use]
pub fn open_file(title: &str, filters: &[(&str, &[&str])]) -> Option<PathBuf> {
    if !system_dialogs_enabled() {
        return None;
    }
    let mut dialog = rfd::FileDialog::new().set_title(title);
    for (name, extensions) in filters {
        dialog = dialog.add_filter(*name, extensions);
    }
    dialog.pick_file()
}
