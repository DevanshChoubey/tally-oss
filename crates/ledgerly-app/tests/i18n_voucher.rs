//! Hindi on the voucher entry screens (payment, receipt, journal, contra, memorandum, reversing
//! journal) and their create-ledger form: every word comes from the translation files.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_app::i18n::Catalog;

#[test]
fn voucher_words_are_translated_and_not_missing() {
    let catalog = Catalog::builtin().unwrap();
    let hi = catalog.tr("hi");
    let en = catalog.tr("en");
    for key in [
        "voucher.title",
        "voucher.kind.contra",
        "voucher.kind.payment",
        "voucher.kind.receipt",
        "voucher.kind.journal",
        "voucher.kind.memorandum",
        "voucher.kind.reversing_journal",
        "voucher.sub.single",
        "voucher.sub.double",
        "voucher.key.next_field",
        "voucher.key.create_ledger",
        "voucher.key.delete_line",
        "voucher.save",
        "voucher.discard",
        "voucher.no_masters",
        "voucher.no_company",
        "voucher.switch_side",
        "voucher.hint_ledger",
        "voucher.hint_amount",
        "voucher.hint_cash_bank",
        "voucher.hint_optional",
        "voucher.balance",
        "voucher.balance_now",
        "voucher.new_ledger",
        "voucher.create_ledger_row",
        "voucher.balance_ok",
        "voucher.balance_needs",
        "voucher.difference_unavailable",
        "voucher.financial_year",
        "voucher.label.date",
        "voucher.label.account",
        "voucher.label.narration",
        "voucher.col.particulars",
        "voucher.account_takes_total",
        "voucher.confirm_discard",
        "voucher.not_supported",
        "voucher.no_ledger_match",
        "voucher.pick_ledger",
        "voucher.pick_cash_bank_first",
        "voucher.pick_paid_first",
        "voucher.type_amount",
        "voucher.zero_amount",
        "voucher.cash_bank_once",
        "voucher.checked",
        "voucher.saved",
        "voucher.not_saved",
        "voucher.saving",
        "voucher.new_ledger.no_group",
        "voucher.new_ledger.no_name",
        "voucher.new_ledger.no_such_group",
        "voucher.new_ledger.taken",
        "voucher.put_cursor_on_ledger",
        "voucher.creating_ledger",
        "voucher.ledger_not_created",
        "voucher.create_form.title",
        "voucher.create_form.name",
        "voucher.create_form.choose_group",
        "voucher.create_form.create",
        "voucher.create_form.cancel",
    ] {
        assert!(!hi.t(key).starts_with('\u{27E6}'), "{key} is missing");
        assert_ne!(hi.t(key), en.t(key), "{key} is not translated");
    }
    assert_eq!(en.t("voucher.save"), "Save");
}
