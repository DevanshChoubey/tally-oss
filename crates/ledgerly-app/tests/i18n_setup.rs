//! Hindi on the company setup wizard: the step rail names, the company type names, the key bar
//! and the unsaved-choice notices come from the translation files, not from English literals.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_app::i18n::Catalog;
use ledgerly_app::screens::setup::form::{Step, step_label_in, unsaved_notice_in};
use ledgerly_app::screens::setup::{key_hints_in, kind_label_in};
use ledgerly_model::EntityKind;

#[test]
fn setup_step_names_come_from_the_hindi_file() {
    let hi = Catalog::builtin().unwrap().tr("hi");
    let en = Catalog::builtin().unwrap().tr("en");
    // provenance: the English words are the step rail labels (index.html:1855).
    assert_eq!(
        step_label_in(Step::Company, &hi),
        hi.t("setup.step.company")
    );
    assert_ne!(step_label_in(Step::Company, &hi), "Company");
    assert_eq!(step_label_in(Step::Ai, &hi), hi.t("setup.step.ai"));
    assert_eq!(step_label_in(Step::Company, &en), "Company");
}

#[test]
fn setup_company_type_names_come_from_the_hindi_file() {
    let hi = Catalog::builtin().unwrap().tr("hi");
    // provenance: the English words are the mock's "Type" field options (index.html:1875).
    assert_eq!(
        kind_label_in(EntityKind::Partnership, &hi),
        hi.t("setup.kind.partnership")
    );
    assert_ne!(
        kind_label_in(EntityKind::Partnership, &hi),
        "Partnership firm"
    );
}

#[test]
fn setup_key_bar_comes_from_the_hindi_file() {
    let hi = Catalog::builtin().unwrap().tr("hi");
    let labels: Vec<String> = key_hints_in(&hi).iter().map(|k| k.label.clone()).collect();
    assert!(labels.contains(&hi.t("setup.key.continue")), "{labels:?}");
    assert!(labels.contains(&hi.t("setup.key.back")), "{labels:?}");
    assert!(!labels.contains(&"Continue".to_owned()), "{labels:?}");
}

#[test]
fn setup_unsaved_notice_comes_from_the_hindi_file() {
    let hi = Catalog::builtin().unwrap().tr("hi");
    assert_eq!(
        unsaved_notice_in(Step::Keyboard, &hi).as_deref(),
        Some(hi.t("setup.notice.keyboard_unsaved").as_str())
    );
    assert_eq!(unsaved_notice_in(Step::Company, &hi), None);
}
