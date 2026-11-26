//! Round s18: the words of the e-Invoice cell, Ctrl+P on an entry screen and Ctrl+E on a voucher
//! have Hindi entries, and each Hindi text differs from the English one and keeps its `{name}`
//! placeholders.
#![allow(
    clippy::unwrap_used,
    reason = "tests may unwrap; a failure is a test failure"
)]

use ledgerly_app::i18n::Catalog;

const KEYS: &[&str] = &[
    "entry_print.nothing_saved",
    "entry_print.save_first",
    "entry_print.no_company",
    "entry_print.saved",
    "voucher_view.export_title",
    "voucher_view.exported",
    "voucher_view.not_saved",
    "voucher_view.not_written_reason",
    "voucher_view.not_written_error",
    "voucher_view.no_company_export",
    "lit.json_ready_inv_01",
    "lit.not_ready",
    "lit.upload_v1_1",
];

fn placeholders(text: &str) -> Vec<String> {
    let mut found: Vec<String> = text
        .split('{')
        .skip(1)
        .filter_map(|rest| rest.split('}').next().map(str::to_owned))
        .collect();
    found.sort();
    found
}

#[test]
fn every_einvoice_and_entry_print_word_is_translated() {
    let catalog = Catalog::builtin().unwrap();
    let (en, hi) = (catalog.tr("en"), catalog.tr("hi"));
    for key in KEYS {
        let (english, hindi) = (en.t(key), hi.t(key));
        assert_ne!(english, hindi, "{key} is not translated");
        assert_eq!(placeholders(&english), placeholders(&hindi), "{key}");
    }
}
