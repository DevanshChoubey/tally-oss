//! Hindi on the bank reconciliation screen: its key bar and the match and status words come from
//! the translation files, not from English literals in the screen code.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_app::i18n::Catalog;
use ledgerly_app::screens::bank::{LineStatus, Tab, key_hints, kind_label_in, status_label_in};
use ledgerly_model::MatchKind;

/// The key bar's label words. `KeyHint` is not re-exported, so the labels are read in place.
macro_rules! labels {
    ($hints:expr) => {
        $hints
            .iter()
            .map(|k| k.label.clone())
            .collect::<Vec<String>>()
    };
}

#[test]
fn bank_key_bar_comes_from_the_hindi_file() {
    let hi = Catalog::builtin().unwrap().tr("hi");
    let en = Catalog::builtin().unwrap().tr("en");
    let matching = labels!(key_hints(Tab::Matching, &hi));
    // The Hindi key bar has the words from the file, and no English word from the code.
    assert!(matching.contains(&hi.t("bank.key.match")), "{matching:?}");
    assert!(
        matching.contains(&hi.t("bank.key.match_by_hand")),
        "{matching:?}"
    );
    assert!(!matching.contains(&"Match".to_owned()), "{matching:?}");
    // The English key bar keeps its English words.
    let english = labels!(key_hints(Tab::Matching, &en));
    assert!(english.contains(&"Match".to_owned()), "{english:?}");
    let statement = labels!(key_hints(Tab::Statement, &hi));
    assert!(
        statement.contains(&hi.t("bank.key.all_lines")),
        "{statement:?}"
    );
}

#[test]
fn bank_match_kind_and_status_words_come_from_the_hindi_file() {
    let hi = Catalog::builtin().unwrap().tr("hi");
    // provenance: the English words are the mock's match and status labels (index.html bank rows).
    assert_eq!(
        kind_label_in(MatchKind::AiSuggested, &hi),
        hi.t("bank.kind.ai_proposed")
    );
    assert_ne!(kind_label_in(MatchKind::Exact, &hi), "Exact");
    assert_eq!(
        status_label_in(LineStatus::Pending, &hi),
        hi.t("bank.status.pending")
    );
    assert_ne!(status_label_in(LineStatus::Matched, &hi), "Matched");
}
