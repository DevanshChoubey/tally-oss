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

#[test]
fn date_words_come_from_the_hindi_file() {
    use ledgerly_app::ui::format;
    use ledgerly_core::Date;
    use time::Month;
    let hi = Catalog::builtin().unwrap().tr("hi");
    let en = Catalog::builtin().unwrap().tr("en");
    // provenance: 21 Nov 2026 is a Saturday. 1 Jan 2026 is a Thursday (derived); day 325 of the
    // year is 324 days later, and 324 = 46 x 7 + 2, so Thursday + 2 = Saturday. The English
    // line is the mock's Home line (index.html, "Saturday 21 Nov 2026").
    let d = Date::from_calendar_date(2026, Month::November, 21).unwrap();
    assert_eq!(format::date_in(d, &en), "21 Nov 2026");
    assert_eq!(format::long_date_in(d, &en), "Saturday 21 Nov 2026");
    // The Hindi month and weekday words are unsourced (hi.toml); the digits stay Latin.
    assert_eq!(format::date_in(d, &hi), "21 नवंबर 2026");
    assert_eq!(format::long_date_in(d, &hi), "शनिवार 21 नवंबर 2026");
}
