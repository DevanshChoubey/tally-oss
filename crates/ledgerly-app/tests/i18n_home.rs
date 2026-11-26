//! Hindi on the home screen: the tile labels, the unsupported words, the footnotes and the
//! attention words come from the translation files, while the figures keep Latin digits in Indian
//! grouping (spec 7; engineering rules A1 and C4).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_app::i18n::Catalog;
use ledgerly_app::screens::home::{AttentionKind, HomeTarget, Kpi, kpi_view_in};
use ledgerly_core::{Outcome, Paise};

fn hindi() -> ledgerly_app::i18n::Tr {
    Catalog::builtin().unwrap().tr("hi")
}

fn receivables(value: Outcome<Paise>, footnote: &str) -> Kpi {
    Kpi {
        label: "Receivables".to_owned(),
        value,
        footnote: footnote.to_owned(),
        target: HomeTarget::BalanceSheet {
            node: "Sundry debtors".to_owned(),
        },
    }
}

#[test]
fn hindi_tile_label_comes_from_the_hindi_file() {
    let tr = hindi();
    let k = receivables(
        Outcome::Value(Paise::new(4_500_000)),
        "4 parties, oldest 38 days",
    );
    let v = kpi_view_in(&k, &tr);
    // The Hindi label is the file's entry, not the English identity.
    assert_eq!(v.label, tr.t("home.tile.receivables"));
    assert_ne!(v.label, "Receivables");
    // provenance: derived. 4_500_000 paise = 45,000 rupees -> `45,000.00`, Latin digits.
    assert_eq!(v.value_text, "\u{20B9}45,000.00");
}

#[test]
fn hindi_unsupported_words_come_from_the_hindi_file() {
    let tr = hindi();
    let k = receivables(
        Outcome::unsupported("GSTR-3B liability", "rate pack pending"),
        "after ITC",
    );
    let v = kpi_view_in(&k, &tr);
    assert_eq!(v.value_text, tr.t("home.not_supported"));
    assert_ne!(v.value_text, "Not supported yet");
    assert!(!v.value_text.chars().any(|c| c.is_ascii_digit()));
}

#[test]
fn hindi_closing_footnote_keeps_the_date_from_the_formatter() {
    let tr = hindi();
    let k = receivables(
        Outcome::Value(Paise::new(100)),
        "Closing balance as of 30 Apr 2026",
    );
    let v = kpi_view_in(&k, &tr);
    // provenance: the date text is the formatter's `30 Apr 2026`; the words come from the file.
    assert_eq!(
        v.footnote_text,
        tr.tf("home.closing_as_of", &[("date", "30 Apr 2026")])
    );
    assert!(v.footnote_text.contains("30 Apr 2026"));
}

#[test]
fn hindi_attention_word_comes_from_the_hindi_file() {
    let tr = hindi();
    assert_eq!(AttentionKind::Due.word_in(&tr), tr.t("home.attention.due"));
    assert_ne!(AttentionKind::Due.word_in(&tr), "Due");
    // The English wrapper keeps the English word for callers without a language.
    assert_eq!(AttentionKind::Due.word(), "Due");
}

#[test]
fn hindi_report_titles_and_keys_come_from_the_hindi_file() {
    // The balance sheet's page title and key bar words are read from the files, so a Hindi
    // report shows Hindi words; the English identity stays the file's English entry.
    let catalog = Catalog::builtin().unwrap();
    let hi = catalog.tr("hi");
    let en = catalog.tr("en");
    for key in [
        "report.balance_sheet",
        "report.day_book",
        "keys.move",
        "keys.drill_in",
        "keys.back_up",
        "keys.export",
        "keys.period",
    ] {
        assert_ne!(hi.t(key), en.t(key), "{key} is not translated");
        assert!(!hi.t(key).starts_with('\u{27E6}'), "{key} is missing");
    }
    assert_eq!(en.t("report.balance_sheet"), "Balance sheet");
}

#[test]
fn hindi_footer_and_theme_names_come_from_the_hindi_file() {
    let catalog = Catalog::builtin().unwrap();
    let hi = catalog.tr("hi");
    let en = catalog.tr("en");
    for key in [
        "footer.saved",
        "theme.light",
        "theme.dark",
        "theme.high_contrast",
    ] {
        assert!(!hi.t(key).starts_with('\u{27E6}'), "{key} is missing");
        assert_ne!(hi.t(key), en.t(key), "{key} is not translated");
    }
    assert_eq!(en.t("theme.high_contrast"), "High contrast");
}

#[test]
fn hindi_palette_words_come_from_the_hindi_file() {
    let catalog = Catalog::builtin().unwrap();
    let hi = catalog.tr("hi");
    let en = catalog.tr("en");
    for key in [
        "palette.placeholder",
        "palette.go_to",
        "palette.section.recent",
        "palette.section.reports",
        "palette.section.ledgers",
        "palette.section.vouchers",
        "palette.section.commands",
        "palette.section.settings",
        "palette.section.no_match",
    ] {
        assert!(!hi.t(key).starts_with('\u{27E6}'), "{key} is missing");
        assert_ne!(hi.t(key), en.t(key), "{key} is not translated");
    }
    assert!(
        hi.tf("palette.no_match_for", &[("query", "zzqx")])
            .contains("zzqx")
    );
}

#[test]
fn review_source_names_come_from_the_hindi_file() {
    use ledgerly_api::ReviewSource;
    use ledgerly_app::screens::review::source_label;
    let tr = hindi();
    // provenance: the English name is the review screen's own (index.html review queue); the
    // Hindi one is the file's `review.source.bank`.
    ledgerly_app::i18n::set_current(&tr);
    let shown = source_label(ReviewSource::BankSuggestion);
    ledgerly_app::i18n::set_current(&ledgerly_app::i18n::Tr::default());
    assert_eq!(shown, tr.t("review.source.bank"));
    assert_ne!(
        shown,
        Catalog::builtin().unwrap().tr("en").t("review.source.bank")
    );
}
