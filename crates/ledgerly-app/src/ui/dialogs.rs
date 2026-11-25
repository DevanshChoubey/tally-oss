//! The only place the app raises a native file dialog.
//!
//! Dialogs are off until the real window calls [`enable_system_dialogs`] (from `shell::run`), so
//! no test, capture or headless run can put a dialog on somebody's desktop: every function here
//! answers "cancelled" (`None`) at once while they are off.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the real window has switched native file dialogs on.
static SYSTEM_DIALOGS: AtomicBool = AtomicBool::new(false);

/// Files that the next open dialogs answer with, in order, without raising a dialog. Empty in
/// the app; the capture tool and tests fill it.
static SCRIPTED_OPENS: Mutex<VecDeque<PathBuf>> = Mutex::new(VecDeque::new());

/// Makes the next [`open_file`] answer `path` as if the user had chosen it, with no dialog. Each
/// call answers one `open_file`. For the capture tool and tests, which cannot drive a native
/// dialog; nothing in the app calls it.
pub fn script_open_file(path: PathBuf) {
    if let Ok(mut queue) = SCRIPTED_OPENS.lock() {
        queue.push_back(path);
    }
}

/// Paths that the next save dialogs answer with, in order, without raising a dialog.
static SCRIPTED_SAVES: Mutex<VecDeque<PathBuf>> = Mutex::new(VecDeque::new());

/// Makes the next [`save_file`] answer `path` as if the user had chosen it, with no dialog. Each
/// call answers one `save_file`. For the capture tool and tests; nothing in the app calls it.
pub fn script_save_file(path: PathBuf) {
    if let Ok(mut queue) = SCRIPTED_SAVES.lock() {
        queue.push_back(path);
    }
}

/// The next scripted answer, if there is one.
fn next_scripted_open() -> Option<PathBuf> {
    SCRIPTED_OPENS.lock().ok()?.pop_front()
}

/// The next scripted save answer, if there is one.
fn next_scripted_save() -> Option<PathBuf> {
    SCRIPTED_SAVES.lock().ok()?.pop_front()
}

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
    if let Some(scripted) = next_scripted_save() {
        return Some(scripted);
    }
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
    if let Some(scripted) = next_scripted_open() {
        return Some(scripted);
    }
    if !system_dialogs_enabled() {
        return None;
    }
    let mut dialog = rfd::FileDialog::new().set_title(title);
    for (name, extensions) in filters {
        dialog = dialog.add_filter(*name, extensions);
    }
    dialog.pick_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scripted_open_answers_once_and_then_dialogs_stay_off() {
        // provenance: derived. Dialogs are off in tests, so an unscripted open is cancelled; a
        // scripted one answers exactly one open with the given path, in the order scripted.
        assert_eq!(open_file("t", &[]), None);
        script_open_file(PathBuf::from("a.csv"));
        script_open_file(PathBuf::from("b.csv"));
        assert_eq!(open_file("t", &[]), Some(PathBuf::from("a.csv")));
        assert_eq!(open_file("t", &[]), Some(PathBuf::from("b.csv")));
        assert_eq!(open_file("t", &[]), None);
    }

    #[test]
    fn a_scripted_save_answers_once_and_then_dialogs_stay_off() {
        // provenance: derived. Dialogs are off in tests, so an unscripted save is cancelled; a
        // scripted one answers exactly one save with the given path, in the order scripted.
        assert_eq!(save_file("t", Some("x.pdf"), &[]), None);
        script_save_file(PathBuf::from("a.pdf"));
        script_save_file(PathBuf::from("b.pdf"));
        assert_eq!(
            save_file("t", Some("x.pdf"), &[]),
            Some(PathBuf::from("a.pdf"))
        );
        assert_eq!(
            save_file("t", Some("x.pdf"), &[]),
            Some(PathBuf::from("b.pdf"))
        );
        assert_eq!(save_file("t", Some("x.pdf"), &[]), None);
    }
}
