//! Card S15: a first-time user's default company file lives in `~/Documents/Ledgerly/`, a folder
//! that does not exist on a new computer. The wizard's Finish must make it.
//!
//! derived: the wizard shows the default path `~/Documents/Ledgerly/<name>.ledgerly` and the
//! user never creates that folder, so creating the company must not fail with "No such file or
//! directory" (found by a cold start of the release binary with an empty home, 2026-11-26).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_api::AiTier;
use ledgerly_app::screens::setup::form::{
    CompanyForm, build_company, create_company, default_features,
};
use ledgerly_core::{Clock, OffsetDateTime, StateCode};
use ledgerly_model::EntityKind;
use time::macros::date;

#[test]
fn a_company_file_in_a_folder_that_does_not_exist_yet_is_created() {
    let form = CompanyForm {
        name: "Godavari Drip & Pump Works".to_owned(),
        kind: EntityKind::Partnership,
        city: "Pune".to_owned(),
        gstin: "27ZZZZZ0000Z1ZQ".to_owned(),
        state: StateCode::new(27).unwrap(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 06 - 01),
    };
    let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("s15_new_home");
    let _ = std::fs::remove_dir_all(&root);
    let path = root
        .join("Documents")
        .join("Ledgerly")
        .join("godavari.ledgerly");
    let company = build_company(&form, default_features()).unwrap();
    let clock = Clock::Fixed(OffsetDateTime::UNIX_EPOCH);
    let made = create_company(&path, &company, clock, AiTier::Off);
    assert!(made.is_ok(), "{:?}", made.err());
    assert!(path.exists());
}
