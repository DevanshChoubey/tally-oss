//! Hindi in the shared UI kit: the confidence words of the confidence band come from the
//! translation files (spec 7; engineering rules A1 and C4: no figure is translated here).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_app::i18n::Catalog;
use ledgerly_app::ui::conf::{self, Confidence};

#[test]
fn confidence_words_come_from_the_hindi_file() {
    let hi = Catalog::builtin().unwrap().tr("hi");
    let en = Catalog::builtin().unwrap().tr("en");
    // provenance: the English words are the band's own (ui/conf.rs `word`, index.html `.conf`).
    for level in [
        Confidence::High,
        Confidence::Medium,
        Confidence::Low,
        Confidence::Rule,
    ] {
        assert_eq!(conf::word_in(level, &en), conf::word(level));
        let text = conf::word_in(level, &hi);
        assert_ne!(text, conf::word(level), "{level:?} is still English");
        assert!(!text.contains('\u{27E6}'), "missing key in {text}");
    }
    assert_eq!(conf::word_in(Confidence::High, &hi), hi.t("conf.high"));
}
