//! S16: a credit or debit note says what the original invoice carried: its date and the quantity
//! invoiced for the item (the engine's figure, shown under the note's header).
//!
//! The accounting product's note pages ask the original invoice number and date and say nothing
//! about showing its quantity (help.tallysolutions.com/tally-prime/sales-process/sales-return-tally/,
//! read 2026-11-26); showing it is a product choice that stops a return larger than the sale.
//! Expected values come from the sample company's seed (`godavari.toml`): SI/0412 is dated
//! 14 Nov 2026 with 67 of the brass nozzle set to Shree Ganesh Traders; PB/0125 (supplier's bill
//! RP/5398 of Rupa Polymers) is dated 15 Oct 2026 with 58 coils of the HDPE pipe.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;
use std::time::Duration;

use egui::RawInput;
use ledgerly_api::Api;
use ledgerly_app::screens::item_invoice::{InvoiceMode, ItemInvoiceScreen};
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{Clock, FinancialYear, Outcome};
use ledgerly_model::{BaseVoucherKind, Books};
use time::OffsetDateTime;
use time::macros::date;

fn sample(name: &str) -> (Api, Books) {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let api = ledgerly_sample::create(&path, Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    let Ok(Outcome::Value(books)) = api.books() else {
        panic!("the books did not load");
    };
    (api, books)
}

fn id_of_ledger(books: &Books, name: &str) -> ledgerly_core::LedgerId {
    books
        .masters
        .ledgers
        .values()
        .find(|l| l.name == name)
        .unwrap()
        .id
}

fn open_note(books: &Books, kind: BaseVoucherKind) -> ItemInvoiceScreen {
    let id = books
        .masters
        .voucher_types
        .values()
        .find(|t| t.base == kind)
        .unwrap()
        .id;
    let fy = FinancialYear::april(2026).unwrap();
    match ItemInvoiceScreen::open_in(books, id, fy, date!(2026 - 11 - 21), InvoiceMode::Item)
        .unwrap()
    {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("unexpectedly unsupported: {u:?}"),
    }
}

fn settle(s: &mut ItemInvoiceScreen, books: &Books, api: Api) {
    let mut app = AppCtx {
        tr: ledgerly_app::i18n::Tr::default(),
        books: Some(books.clone()),
        api: Some(ApiWorker::spawn(api).unwrap()),
        lock_date: None,
        ai_tier: None,
    };
    let ctx = egui::Context::default();
    for _ in 0..2000 {
        let out = ctx.run_ui(RawInput::default(), |ui| {
            let _stay = s.ui(ui, &mut app);
        });
        out.drop_without_applying_deltas();
        if s.invoiced_line(&books.masters).is_some() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_credit_note_shows_the_date_and_quantity_of_the_invoice_it_is_against() {
    // provenance: derived from the seed (see the module doc): SI/0412, 14 Nov 2026, 67 of the
    // brass nozzle set; the note returns that item, so only that item is listed.
    let (api, books) = sample("s16_hint_credit.ledgerly");
    let mut s = open_note(&books, BaseVoucherKind::CreditNote);
    s.draft_mut().party = Some(id_of_ledger(&books, "Shree Ganesh Traders"));
    "SI/0412".clone_into(&mut s.draft_mut().reference_text);
    let item = books
        .masters
        .stock_items
        .values()
        .find(|i| i.name == "Brass nozzle set (6 pc)")
        .unwrap()
        .id;
    s.draft_mut().rows.first_mut().unwrap().item = Some(item);
    settle(&mut s, &books, api);
    assert_eq!(
        s.invoiced_line(&books.masters).as_deref(),
        Some("Invoiced on SI/0412, 14 Nov 2026: 67 set of Brass nozzle set (6 pc)")
    );
}

#[test]
fn a_debit_note_shows_the_suppliers_bill() {
    // provenance: derived from the seed: bill RP/5398 of Rupa Polymers, 15 Oct 2026, 58 coils.
    let (api, books) = sample("s16_hint_debit.ledgerly");
    let mut s = open_note(&books, BaseVoucherKind::DebitNote);
    s.draft_mut().party = Some(id_of_ledger(&books, "Rupa Polymers"));
    "RP/5398".clone_into(&mut s.draft_mut().reference_text);
    let item = books
        .masters
        .stock_items
        .values()
        .find(|i| i.name == "HDPE pipe coil 32 mm")
        .unwrap()
        .id;
    s.draft_mut().rows.first_mut().unwrap().item = Some(item);
    settle(&mut s, &books, api);
    assert_eq!(
        s.invoiced_line(&books.masters).as_deref(),
        Some("Invoiced on RP/5398, 15 Oct 2026: 58 coil of HDPE pipe coil 32 mm")
    );
}

#[test]
fn an_item_that_was_not_on_the_invoice_says_so() {
    // provenance: derived from the seed: SI/0412 has the lathe fitting (25) and the nozzle set
    // (67) only; the HDPE pipe coil is not on it.
    let (api, books) = sample("s16_hint_absent.ledgerly");
    let mut s = open_note(&books, BaseVoucherKind::CreditNote);
    s.draft_mut().party = Some(id_of_ledger(&books, "Shree Ganesh Traders"));
    "SI/0412".clone_into(&mut s.draft_mut().reference_text);
    let item = books
        .masters
        .stock_items
        .values()
        .find(|i| i.name == "HDPE pipe coil 32 mm")
        .unwrap()
        .id;
    s.draft_mut().rows.first_mut().unwrap().item = Some(item);
    settle(&mut s, &books, api);
    assert_eq!(
        s.invoiced_line(&books.masters).as_deref(),
        Some("Invoiced on SI/0412, 14 Nov 2026: HDPE pipe coil 32 mm is not on it")
    );
}

#[test]
fn a_sales_invoice_has_no_such_line() {
    // provenance: derived. Only a note has an original invoice.
    let (_api, books) = sample("s16_hint_sales.ledgerly");
    let s = open_note(&books, BaseVoucherKind::Sales);
    assert_eq!(s.invoiced_line(&books.masters), None);
}

#[test]
fn a_note_s_mode_list_offers_item_and_accounting_only() {
    // provenance: product choice (the engine taxes an as-voucher entry for sales and purchase
    // invoices only; the pages read, help.tallysolutions.com/sales-of-goods-services/ on
    // 2026-11-26, name the three modes for those). Derived: a credit note is offered Item and
    // Accounting; asking for As Voucher anyway leaves the mode as it was and says why.
    let (_api, books) = sample("s16_note_modes.ledgerly");
    let mut s = open_note(&books, BaseVoucherKind::CreditNote);
    assert_eq!(
        s.mode_choices(),
        vec![InvoiceMode::Item, InvoiceMode::Accounting]
    );
    s.choose_mode(InvoiceMode::AsVoucher);
    assert_eq!(s.mode(), InvoiceMode::Item);
    assert!(
        s.error_text().unwrap().contains("sales and purchase"),
        "{:?}",
        s.error_text()
    );
}
