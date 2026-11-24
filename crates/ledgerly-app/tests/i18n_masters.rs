//! Hindi on the masters screen: its title, subtitle, key bar, kind names and the list words come
//! from the translation files, not from English literals in the screen code.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_app::Screen;
use ledgerly_app::i18n::Catalog;
use ledgerly_app::screens::masters::{MasterKind, MastersScreen};

#[test]
fn masters_kind_names_come_from_the_hindi_file() {
    let hi = Catalog::builtin().unwrap().tr("hi");
    // provenance: the English singular and plural names are the mock's (index.html:1796-1800).
    assert_eq!(
        MasterKind::Ledger.label_in(&hi),
        hi.t("masters.kind.ledger")
    );
    assert_ne!(MasterKind::Ledger.label_in(&hi), "ledger");
    assert_eq!(
        MasterKind::StockItem.plural_in(&hi),
        hi.t("masters.kind_plural.stock_item")
    );
    assert_ne!(MasterKind::StockItem.plural_in(&hi), "Stock items");
}

#[test]
fn masters_subtitle_and_key_bar_come_from_the_hindi_file() {
    let hi = Catalog::builtin().unwrap().tr("hi");
    let en = Catalog::builtin().unwrap().tr("en");
    let screen = MastersScreen::new();
    assert_eq!(
        screen.subtitle(&hi),
        Some(hi.t("masters.subtitle")),
        "the subtitle is not the Hindi file's entry"
    );
    assert_ne!(screen.subtitle(&hi), screen.subtitle(&en));
    let keys = screen.keys(&hi);
    let labels: Vec<String> = keys.iter().map(|k| k.label.clone()).collect();
    assert!(labels.contains(&hi.t("masters.key.create")), "{labels:?}");
    assert!(!labels.contains(&"Create".to_owned()), "{labels:?}");
    // The English key bar keeps its English words.
    let english: Vec<String> = screen.keys(&en).iter().map(|k| k.label.clone()).collect();
    assert!(english.contains(&"Create".to_owned()));
}
