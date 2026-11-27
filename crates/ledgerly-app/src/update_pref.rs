//! Whether this person lets Ledgerly check for updates (round s23). Kept in the user's config
//! folder, outside every company file: `<config>/ledgerly/update-check.toml`. Off unless the file
//! says `enabled = true`; a missing file is off, and a file that cannot be read is reported, never
//! taken as on.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// The file name inside `<config>/ledgerly/`.
const FILE_NAME: &str = "update-check.toml";

/// What the file holds.
#[derive(Debug, Serialize, Deserialize)]
struct Stored {
    /// The user turned the check on.
    enabled: bool,
}

/// Whether the check is on, from the file in `dir`. A missing file is `Ok(false)`.
///
/// # Errors
/// The file exists and cannot be read or is not valid; the check then stays off.
pub fn load(dir: &Path) -> Result<bool, String> {
    let path = dir.join(FILE_NAME);
    match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str::<Stored>(&text)
            .map(|s| s.enabled)
            .map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Stores the choice in `dir`, creating the folder when needed.
///
/// # Errors
/// The folder or file cannot be written.
pub fn save(dir: &Path, enabled: bool) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join(FILE_NAME);
    let text = toml::to_string(&Stored { enabled }).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
}
