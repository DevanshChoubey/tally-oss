//! The sample company: Godavari Drip & Pump Works, a fictional Pune maker of drip-irrigation kit
//! and pumps, with a full chart of accounts, parties, stock items, GST invoices, purchases, bank
//! entries and a stored bank statement.
//!
//! The data is a versioned, human-readable seed (`assets/godavari.toml`) of masters and vouchers.
//! [`create`] posts it through the same [`ledgerly_api::Api`] the app uses, so every figure on
//! screen is whatever the engine computes. No total is stored in the seed. The seed is
//! fictional: names, identifiers (GSTINs use the fake PAN pattern `ZZZZZ####Z` with a valid check
//! character), bank accounts and IFSC codes are invented.

mod error;
mod load;
mod seed;

use std::path::{Path, PathBuf};

use ledgerly_api::Api;
use ledgerly_core::{Clock, Date};
use time::format_description::well_known::Iso8601;

pub use error::SampleError;

/// The seed text.
const SEED: &str = include_str!("../assets/godavari.toml");

/// The seed version this build reads.
const SEED_VERSION: u32 = 1;

/// The name of the sample company.
pub const NAME: &str = "Godavari Drip & Pump Works";

/// What the welcome card says about the sample company.
pub const CARD_TEXT: &str =
    "Godavari Drip & Pump Works, fictional data. Nothing you do there is kept.";

/// The folder, under the base folder, that holds the sample company's file.
const FOLDER: &str = "ledgerly-sample";

/// The file name of the sample company.
const FILE: &str = "godavari-drip.ledgerly";

fn parsed() -> Result<seed::Seed, SampleError> {
    let seed: seed::Seed = toml::from_str(SEED)?;
    if seed.version == SEED_VERSION {
        Ok(seed)
    } else {
        Err(SampleError::Version {
            found: seed.version,
            expected: SEED_VERSION,
        })
    }
}

/// The day the sample company is set on: the mock's "today", 21 Nov 2026. The app's screens use
/// the real date; the capture harness uses this one.
///
/// # Errors
/// [`SampleError`] when the seed cannot be read.
pub fn today() -> Result<Date, SampleError> {
    let seed = parsed()?;
    Date::parse(&seed.company.today, &Iso8601::DATE).map_err(|e| SampleError::Value {
        field: "company today".to_owned(),
        reason: e.to_string(),
    })
}

/// The path of the sample company's file under `base` (the app passes its cache folder).
#[must_use]
pub fn path_under(base: &Path) -> PathBuf {
    base.join(FOLDER).join(FILE)
}

/// Builds the sample company in a new file at `path`. The file must not exist.
///
/// # Errors
/// [`SampleError`] when the seed is bad, or the API or the engine refuses a step.
pub fn create(path: &Path, clock: Clock) -> Result<Api, SampleError> {
    let seed = parsed()?;
    Ok(load::build(&seed, path, clock)?.api)
}

/// Builds the sample company in a throwaway file under `base`, replacing one a last run left,
/// and returns it open with its path.
///
/// # Errors
/// [`SampleError::File`] when the folder cannot be prepared; otherwise as [`create`].
pub fn create_under(base: &Path, clock: Clock) -> Result<(Api, PathBuf), SampleError> {
    let path = path_under(base);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|source| SampleError::File {
            path: dir.to_path_buf(),
            source,
        })?;
    }
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(SampleError::File { path, source });
        }
    }
    let api = create(&path, clock)?;
    Ok((api, path))
}
