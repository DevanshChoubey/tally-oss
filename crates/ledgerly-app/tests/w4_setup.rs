//! Card W4: the company setup wizard (mock `SCREENS.setup`, `design/mock-v2/index.html` 1855-1895).
//!
//! Expected values are derived by hand, or read from the mock, and the provenance is written next
//! to each one. Nothing is copied from this crate's output. Company files are written under
//! `CARGO_TARGET_TMPDIR`.
//!
//! Tally provenance: help.tallysolutions.com/set-up-company-tally/ (read 2026-11-23). It says the
//! financial year beginning date is prefilled from the computer's date settings and country, and
//! that "Books beginning from" is prefilled from it and should be changed "based on the actual
//! start date of your business transactions". So books may begin after the financial year starts.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_api::AiTier;
use ledgerly_api::Api;
use ledgerly_app::screens::setup::form::{
    CompanyForm, GstinCheck, KeyInput, KeyOutcome, Step, ai_notice, build_company, check_gstin,
    create_company, default_features, feature_rows, financial_year_start, gstin_allows_continue,
    parse_books_begin, state_from_gstin, step_outcome, unsaved_notice,
};
use ledgerly_core::{Clock, OffsetDateTime, Outcome, StateCode};
use ledgerly_model::EntityKind;
use time::macros::date;

/// A company form for the fictional firm of the mock, as the user fills it in.
fn godavari_form() -> CompanyForm {
    CompanyForm {
        name: "Godavari Drip & Pump Works".to_owned(),
        kind: EntityKind::Partnership,
        city: "Pune".to_owned(),
        // index.html:1868 (the mock's company GSTIN, with the "Check character OK" badge).
        gstin: "27ZZZZZ0000Z1ZQ".to_owned(),
        state: StateCode::new(27).unwrap(),
        // derived: 23 Nov 2026 is in April to March 2026-27, so the year starts 1 April 2026.
        fy_start: date!(2026 - 04 - 01),
        // Tally help, read 2026-11-23: the books date may be changed to the real start date.
        books_begin: date!(2026 - 06 - 01),
    }
}

#[test]
fn the_steps_come_in_the_mocks_order_and_navigate_both_ways() {
    // index.html:1855 STEPS: welcome, company, features, keyboard, ai, backup, done.
    let mut ids = Vec::new();
    let mut step = Some(Step::Welcome);
    while let Some(s) = step {
        ids.push(s.key());
        step = s.next();
    }
    assert_eq!(
        ids,
        [
            "welcome", "company", "features", "keyboard", "ai", "backup", "done"
        ]
    );
    // derived: the step labels the rail draws are the mock's (index.html:1855).
    assert_eq!(Step::Ai.label(), "AI and privacy");
    // derived: Back from the first step has nowhere to go; Back from the last returns one step.
    assert_eq!(Step::Welcome.back(), None);
    assert_eq!(Step::Done.back(), Some(Step::Backup));
    assert_eq!(Step::Done.next(), None);
}

#[test]
fn enter_continues_and_escape_goes_back_unless_a_field_has_focus() {
    // index.html:1887 Enter continues; index.html:1888 Esc goes back while not typing.
    assert_eq!(
        step_outcome(Step::Welcome, false, KeyInput::Enter),
        KeyOutcome::Continue
    );
    assert_eq!(
        step_outcome(Step::Company, false, KeyInput::Escape),
        KeyOutcome::Back
    );
    // index.html:1887: Enter does nothing while typing in a form.
    assert_eq!(
        step_outcome(Step::Company, true, KeyInput::Enter),
        KeyOutcome::Pass
    );
    // index.html:1888: Esc on the first step is not the wizard's; the shell handles it.
    assert_eq!(
        step_outcome(Step::Welcome, false, KeyInput::Escape),
        KeyOutcome::Pass
    );
    // index.html:1888: Esc does not go back while a field has focus.
    assert_eq!(
        step_outcome(Step::Features, true, KeyInput::Escape),
        KeyOutcome::Pass
    );
}

#[test]
fn a_gstin_fills_the_state_from_its_first_two_digits() {
    // index.html:1868 says the GSTIN "fills the state"; 27 is Maharashtra (mock STATES, index.html:491).
    let state = state_from_gstin("27ZZZZZ0000Z1ZQ").unwrap();
    assert_eq!(state.get(), 27);
    assert_eq!(state.name(), "Maharashtra");
}

#[test]
fn a_gstin_for_another_state_blocks_continue_with_the_mocks_wording() {
    // The state is Gujarat (24) while the GSTIN starts with 27: the mock's error state.
    let gujarat = StateCode::new(24).unwrap();
    let check = check_gstin("27ZZZZZ0000Z1ZQ", gujarat);
    // index.html:1868: the exact message the mock shows, naming the fix.
    let expected = "This GSTIN starts with 27 (Maharashtra), but the state is set to 24 Gujarat. \
                    Change the state to Maharashtra, or check the first two digits.";
    match &check {
        GstinCheck::Mismatch { message, .. } => assert_eq!(message, expected),
        GstinCheck::Blank | GstinCheck::Valid(_) | GstinCheck::Invalid(_) => {
            panic!("expected a state mismatch, got {check:?}")
        }
    }
    // index.html:1861: Continue is disabled ("Fix the GSTIN first") in this state.
    assert!(!gstin_allows_continue(&check));
}

#[test]
fn a_matching_gstin_and_a_blank_one_allow_continue() {
    let maharashtra = StateCode::new(27).unwrap();
    assert!(gstin_allows_continue(&check_gstin(
        "27ZZZZZ0000Z1ZQ",
        maharashtra
    )));
    // derived: a GSTIN is optional in the company form (a business may not be registered yet).
    assert!(gstin_allows_continue(&check_gstin("  ", maharashtra)));
}

#[test]
fn a_gstin_with_a_bad_check_character_blocks_continue() {
    // derived: change the last character of the valid GSTIN; the check character no longer agrees.
    let maharashtra = StateCode::new(27).unwrap();
    let check = check_gstin("27ZZZZZ0000Z1ZR", maharashtra);
    assert!(matches!(check, GstinCheck::Invalid(_)));
    assert!(!gstin_allows_continue(&check));
}

#[test]
fn the_financial_year_starts_on_1_april_and_books_may_begin_later() {
    // derived: 23 Nov 2026 is after 1 April 2026, so the year is 2026-27.
    assert_eq!(
        financial_year_start(date!(2026 - 11 - 23)).unwrap(),
        date!(2026 - 04 - 01)
    );
    // derived: 31 March 2026 is still in 2025-26; 1 April 2026 starts 2026-27.
    assert_eq!(
        financial_year_start(date!(2026 - 03 - 31)).unwrap(),
        date!(2025 - 04 - 01)
    );
    assert_eq!(
        financial_year_start(date!(2026 - 04 - 01)).unwrap(),
        date!(2026 - 04 - 01)
    );
    // Tally help (read 2026-11-23): the books date is changed to the real start of transactions.
    let fy = date!(2026 - 04 - 01);
    let fy_end = date!(2027 - 03 - 31);
    assert_eq!(
        parse_books_begin("2026-06-01", fy, fy_end).unwrap(),
        date!(2026 - 06 - 01)
    );
    // derived: books cannot begin before the financial year, or after it ends.
    assert!(parse_books_begin("2026-03-31", fy, fy_end).is_err());
    assert!(parse_books_begin("2027-04-01", fy, fy_end).is_err());
    assert!(parse_books_begin("not a date", fy, fy_end).is_err());
}

#[test]
fn every_feature_row_is_either_a_working_switch_or_says_why_it_is_not_available() {
    let rows = feature_rows();
    assert_eq!(rows.len(), 7, "index.html:1872 lists seven features");
    for row in &rows {
        match (row.switch.is_some(), row.unavailable) {
            (true, None) => {}
            (false, Some(reason)) => assert_ne!(reason, ""),
            (true, Some(_)) | (false, None) => {
                panic!("'{}' must be a switch or name its gap", row.label)
            }
        }
    }
    // index.html:1872 default-on rows that the model has a switch for: inventory, GST, bank, bill-wise.
    let f = default_features();
    assert!(f.inventory);
    assert!(f.gst);
    assert!(f.bank_reconciliation);
    assert!(f.bill_wise);
    // index.html:1872: Cost centres is off by default.
    assert!(!f.cost_centres);
    // derived: e-invoice and multiple currencies have no model switch, so they stay off.
    assert!(!f.batches);
    assert!(!f.godowns);
    assert!(!f.post_dated);
    assert!(!f.optional_vouchers);
}

#[test]
fn a_created_company_file_opens_with_the_chosen_name_state_and_year() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("w4_setup");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("godavari.ledgerly");
    if path.exists() {
        std::fs::remove_file(&path).unwrap();
    }
    let form = godavari_form();
    let company = build_company(&form, default_features()).unwrap();
    let clock = Clock::Fixed(OffsetDateTime::UNIX_EPOCH);
    let api = Api::create(&path, &company, clock).unwrap();
    drop(api);

    let reopened = Api::open(&path).unwrap();
    let Outcome::Value(books) = reopened.books().unwrap() else {
        panic!("the books should load");
    };
    // Expected values from the form the user filled in (see godavari_form).
    assert_eq!(books.company.name, "Godavari Drip & Pump Works");
    assert_eq!(books.company.state.get(), 27);
    assert_eq!(books.company.fy_start, date!(2026 - 04 - 01));
    assert_eq!(books.company.books_begin, date!(2026 - 06 - 01));
    // The GSTIN became the company's registration, with its state.
    assert_eq!(books.company.registrations.len(), 1);
    assert_eq!(
        books.company.registrations[0].gstin.as_str(),
        "27ZZZZZ0000Z1ZQ"
    );
}

#[test]
fn a_company_with_a_mismatched_gstin_is_not_built() {
    let mut form = godavari_form();
    form.state = StateCode::new(24).unwrap();
    assert!(build_company(&form, default_features()).is_err());
}

#[test]
fn the_steps_that_cannot_save_their_choice_say_so_on_that_step() {
    // Keyboard preset and backup folder have no store yet (design/PARITY-GAPS.md). Each such step
    // must say the choice is not saved; the other steps must not claim a gap. The AI step saves
    // its tier through `Api::set_ai_tier` (the AI and privacy screen's store); only the cloud
    // choice is refused, and `ai_notice` says so for that choice alone.
    for step in [Step::Keyboard, Step::Backup] {
        let notice = unsaved_notice(step);
        assert!(
            notice.is_some_and(|n| n.contains("not saved")),
            "{step:?} must say its choice is not saved, got {notice:?}"
        );
    }
    for step in [
        Step::Welcome,
        Step::Company,
        Step::Ai,
        Step::Features,
        Step::Done,
    ] {
        assert_eq!(unsaved_notice(step), None, "{step:?} saves what it asks");
    }
}

/// A fresh company file path under the test's temporary folder.
fn fresh_path(name: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("w4_setup");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    if path.exists() {
        std::fs::remove_file(&path).unwrap();
    }
    path
}

#[test]
fn the_ai_choice_made_in_the_wizard_is_the_companys_stored_tier() {
    // provenance: derived from the mock's AI step (index.html:1879-1884: "Pick per company") and
    // the AI and privacy screen, which reads the tier stored in the company file
    // (`Api::ai_tier`). Choosing Off or On this computer in the wizard must leave that tier stored
    // in the new file, and nothing to say about it.
    let clock = Clock::Fixed(OffsetDateTime::UNIX_EPOCH);
    for (file, tier) in [
        ("ai-off.ledgerly", AiTier::Off),
        ("ai-local.ledgerly", AiTier::Local),
    ] {
        let path = fresh_path(file);
        let company = build_company(&godavari_form(), default_features()).unwrap();
        let (api, note) = create_company(&path, &company, clock, tier).unwrap();
        assert_eq!(note, None, "{tier:?} is stored, so nothing is said");
        assert_eq!(api.ai_tier().unwrap(), Some(tier));
        drop(api);
        assert_eq!(Api::open(&path).unwrap().ai_tier().unwrap(), Some(tier));
    }
}

#[test]
fn a_cloud_choice_creates_the_company_with_no_tier_and_says_the_engines_reason() {
    // provenance: `Api::set_ai_tier(Cloud)` is Unsupported until a keychain store exists
    // (design/PARITY-GAPS.md, "AI and privacy": the cloud tier). The company is still created;
    // no tier is stored, so the top bar says AI is not set up, and the wizard passes on the
    // engine's reason, which names the keychain.
    let clock = Clock::Fixed(OffsetDateTime::UNIX_EPOCH);
    let path = fresh_path("ai-cloud.ledgerly");
    let company = build_company(&godavari_form(), default_features()).unwrap();
    let (api, note) = create_company(&path, &company, clock, AiTier::Cloud).unwrap();
    assert_eq!(api.ai_tier().unwrap(), None);
    let note = note.unwrap();
    assert!(note.contains("keychain"), "{note}");
}

#[test]
fn only_the_cloud_choice_says_on_the_ai_step_that_it_is_not_saved() {
    // provenance: as above: Off and Local are stored, Cloud is refused by the engine.
    assert!(ai_notice(AiTier::Cloud).is_some_and(|n| n.contains("not saved")));
    assert_eq!(ai_notice(AiTier::Off), None);
    assert_eq!(ai_notice(AiTier::Local), None);
}
