//! Hindi on the sales and purchase invoice body: every label, hint, button, note and key-bar word
//! comes from the translation files. The figures keep Latin digits (formatters), so only words
//! are checked here.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_app::i18n::Catalog;

#[test]
fn invoice_words_are_translated_and_not_missing() {
    let catalog = Catalog::builtin().unwrap();
    let hi = catalog.tr("hi");
    let en = catalog.tr("en");
    for key in [
        "invoice.subtitle_item",
        "invoice.subtitle_accounting",
        "invoice.col.particulars",
        "invoice.col.amount",
        "invoice.col.gst",
        "invoice.col.item",
        "invoice.col.hsn",
        "invoice.col.qty",
        "invoice.col.unit",
        "invoice.col.rate",
        "invoice.col.disc",
        "invoice.col.typed_amount",
        "invoice.hint_item",
        "invoice.hint_ledger",
        "invoice.hint_search",
        "invoice.hint_optional",
        "invoice.hint_required",
        "invoice.date",
        "invoice.place_of_supply",
        "invoice.party_gstin",
        "invoice.voucher_no",
        "invoice.choose_party",
        "invoice.check_ok",
        "invoice.unregistered",
        "invoice.total",
        "invoice.taxable_value",
        "invoice.round_off",
        "invoice.tax_not_computed",
        "invoice.narration",
        "invoice.confirm_save",
        "invoice.yes",
        "invoice.no",
        "invoice.discard",
        "invoice.confirm_discard",
        "invoice.mode.item",
        "invoice.mode.accounting",
        "invoice.save",
        "invoice.key.next_field",
        "invoice.key.previous",
        "invoice.key.list",
        "invoice.key.create_master",
        "invoice.key.change_mode",
        "invoice.key.back",
        "invoice.keep_it",
        "invoice.keep_editing",
        "invoice.no_masters",
        "invoice.no_company",
        "invoice.label.sales_ledger",
        "invoice.label.purchase_ledger",
        "invoice.label.customer",
        "invoice.label.supplier",
        "invoice.ref.order",
        "invoice.ref.supplier_invoice",
        "invoice.ref.original_invoice",
        "invoice.saving",
    ] {
        assert!(!hi.t(key).starts_with('\u{27E6}'), "{key} is missing");
        assert_ne!(hi.t(key), en.t(key), "{key} is not translated");
    }
    assert_eq!(en.t("invoice.save"), "Save");
}

#[test]
fn period_lock_note_keeps_the_dates_from_the_formatter() {
    let hi = Catalog::builtin().unwrap().tr("hi");
    let text = hi.tf(
        "invoice.period_locked",
        &[("date", "12-04-2026"), ("lock", "31-03-2026")],
    );
    assert!(text.contains("12-04-2026"));
    assert!(text.contains("31-03-2026"));
}
