//! Translation layer: the files parse, Hindi has every English key, a missing key falls back to
//! English, and a community file loads without a code change.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests may unwrap; a failure is a test failure"
)]

use ledgerly_app::i18n::{Catalog, I18nError};

#[test]
fn builtin_files_parse() {
    let catalog = Catalog::builtin().unwrap();
    let codes: Vec<String> = catalog.languages().into_iter().map(|l| l.code).collect();
    assert_eq!(codes, vec!["en".to_owned(), "hi".to_owned()]);
}

#[test]
fn hindi_has_every_english_key() {
    // Expected: the empty list. A key listed here is a string Hindi users would see in English.
    let catalog = Catalog::builtin().unwrap();
    assert_eq!(catalog.missing("hi"), Vec::<String>::new());
}

#[test]
fn hindi_names_itself() {
    let catalog = Catalog::builtin().unwrap();
    let hi = catalog
        .languages()
        .into_iter()
        .find(|l| l.code == "hi")
        .unwrap();
    assert_eq!(hi.name, "हिन्दी");
}

#[test]
fn missing_hindi_key_falls_back_to_english() {
    let mut catalog = Catalog::default();
    catalog
        .add_source(
            "en",
            "[meta]\nname = \"English\"\n[strings]\na = \"Save\"\nb = \"Cancel\"\n",
        )
        .unwrap();
    catalog
        .add_source("hi", "[meta]\nname = \"हिन्दी\"\n[strings]\na = \"सहेजें\"\n")
        .unwrap();
    let hi = catalog.tr("hi");
    assert_eq!(hi.t("a"), "सहेजें");
    assert_eq!(hi.t("b"), "Cancel");
    assert_eq!(catalog.missing("hi"), vec!["b".to_owned()]);
}

#[test]
fn unknown_key_shows_its_id_not_blank() {
    let catalog = Catalog::builtin().unwrap();
    assert_eq!(
        catalog.tr("en").t("no.such.key"),
        "\u{27E6}no.such.key\u{27E7}"
    );
}

#[test]
fn unknown_language_draws_english() {
    let catalog = Catalog::builtin().unwrap();
    assert_eq!(catalog.tr("xx").code(), "en");
}

#[test]
fn placeholders_are_filled() {
    let catalog = Catalog::builtin().unwrap();
    assert_eq!(
        catalog
            .tr("en")
            .tf("chrome.language_switched", &[("name", "हिन्दी")]),
        "Language: हिन्दी"
    );
}

#[test]
fn community_file_loads_without_code_change() {
    let mut catalog = Catalog::builtin().unwrap();
    catalog
        .add_source(
            "ta",
            "[meta]\nname = \"தமிழ்\"\n[strings]\n\"chrome.back\" = \"பின்\"\n",
        )
        .unwrap();
    assert!(
        catalog
            .languages()
            .iter()
            .any(|l| l.code == "ta" && l.name == "தமிழ்")
    );
    assert_eq!(catalog.tr("ta").t("chrome.back"), "பின்");
    assert_eq!(catalog.tr("ta").t("chrome.new_tab"), "New tab");
}

#[test]
fn broken_community_file_is_refused_with_a_reason() {
    let mut catalog = Catalog::builtin().unwrap();
    let no_meta = catalog.add_source("ta", "[strings]\na = \"x\"\n");
    assert!(matches!(no_meta, Err(I18nError::Parse { .. })));
    let bad_code = catalog.add_source("../evil", "[meta]\nname = \"x\"\n[strings]\n");
    assert!(matches!(bad_code, Err(I18nError::BadCode { .. })));
    assert!(!catalog.has("ta"));
}

#[test]
fn every_key_used_in_source_exists_in_english() {
    // Scans `src` for literal keys passed to `tr.t("...")` and `tr.tf("...")`.
    let english = Catalog::builtin().unwrap();
    let en = english.tr("en");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut unknown = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for marker in ["tr.t(\"", "tr.tf(\""] {
                for (at, _) in text.match_indices(marker) {
                    let rest = &text[at + marker.len()..];
                    let key = &rest[..rest.find('"').unwrap()];
                    if en.t(key).starts_with('\u{27E6}') {
                        unknown.push(format!("{}: {key}", path.display()));
                    }
                }
            }
        }
    }
    assert_eq!(unknown, Vec::<String>::new());
}

#[test]
fn switching_language_changes_key_bar_words() {
    use ledgerly_app::shell::chrome::key_hints_in;
    let catalog = Catalog::builtin().unwrap();
    let en = key_hints_in(&[], true, &catalog.tr("en"));
    let hi = key_hints_in(&[], true, &catalog.tr("hi"));
    assert_eq!(en, vec![("Esc".to_owned(), "Back".to_owned())]);
    assert_eq!(hi, vec![("Esc".to_owned(), "वापस".to_owned())]);
}

#[test]
fn shell_switches_language_without_restart() {
    use ledgerly_app::shell::{Shell, WelcomeScreen};
    let mut shell = Shell::new(Box::new(WelcomeScreen::default()));
    assert_eq!(shell.language(), "en");
    shell.set_language("hi");
    assert_eq!(shell.language(), "hi");
    assert_eq!(shell.tr().t("chrome.back"), "वापस");
    shell.set_language("xx");
    assert_eq!(
        shell.language(),
        "hi",
        "an unloaded code leaves the language as it was"
    );
}
