//! Round s18: on the sample company the default capture's invoice (SI/0418 to Shree Ganesh
//! Traders, Nashik) says "JSON ready (INV-01)", as the mock's `sales-invoice.default` does, and a
//! buyer whose address has no PIN code says what is missing.
//!
//! Provenance: derived. The mock (`design/mock-v2/index.html`, `v-einv` cell) shows "JSON ready
//! (INV-01)" for the default state, whose buyer is a registered party in the company's own state
//! with a complete address; the NIC schema page
//! (<https://einv-apisandbox.nic.in/version1.03/generate-irn.html>, read 2026-11-26) makes `Pin`
//! and the address lines required, which the sample's seed now holds for the company and the
//! buyer. The reason text for a missing buyer PIN is the API's (`party_block`):
//! "the buyer Shree Ganesh Traders has no PIN code (Pin is required)", the words of the mock's
//! `sales-invoice.einvoice-not-ready`.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::time::{Duration, Instant};

use ledgerly_api::Command;
use ledgerly_app::screens::item_invoice::{Field, ItemInvoiceScreen};
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{Clock, FinancialYear, Outcome};
use ledgerly_model::{BaseVoucherKind, Books, Masters};
use time::OffsetDateTime;
use time::macros::date;

fn frame(screen: &mut ItemInvoiceScreen, app: &mut AppCtx) {
    let ctx = egui::Context::default();
    let out = ctx.run_ui(egui::RawInput::default(), |ui| {
        let _stay = screen.ui(ui, app);
    });
    out.drop_without_applying_deltas();
}

/// SI/0418 as the default capture types it: Shree Ganesh Traders, place 27, Sales, two lines.
fn fill(screen: &mut ItemInvoiceScreen, m: &Masters) {
    screen.set_party_text("shree ganesh");
    screen.submit(m);
    assert_eq!(screen.focus(), Field::Place);
    "27".clone_into(&mut screen.draft_mut().place_text);
    screen.submit(m);
    screen.set_account_text("sales");
    screen.submit(m);
    screen.submit(m);
    assert_eq!(screen.focus(), Field::Item(0));
    for (n, (item, qty, rate)) in [
        ("drip lateral", "25", "3850"),
        ("brass nozzle", "67", "505"),
    ]
    .into_iter()
    .enumerate()
    {
        screen.set_item_text(n, item);
        screen.submit(m);
        qty.clone_into(&mut screen.draft_mut().rows[n].qty_text);
        screen.submit(m);
        rate.clone_into(&mut screen.draft_mut().rows[n].rate_text);
        screen.submit(m);
        if n == 0 {
            assert_eq!(screen.focus(), Field::Item(1));
        }
    }
}

/// The sample company's SI/0418 as the default capture types it, opened on 21 Nov 2026; with
/// `strip_buyer_pin` the buyer's PIN code is removed from the books first.
fn note_for_default_invoice(strip_buyer_pin: bool) -> Option<String> {
    let dir = tempfile::tempdir().unwrap();
    let mut api = ledgerly_sample::create(
        &dir.path().join("s.ledgerly"),
        Clock::Fixed(OffsetDateTime::UNIX_EPOCH),
    )
    .unwrap();
    if strip_buyer_pin {
        let Outcome::Value(books) = api.books().unwrap() else {
            panic!("books")
        };
        let mut buyer = books
            .masters
            .ledgers
            .values()
            .find(|l| l.name == "Shree Ganesh Traders")
            .unwrap()
            .clone();
        buyer.party.as_mut().unwrap().address.pincode = None;
        assert!(matches!(
            api.execute(Command::AlterLedger(buyer)).unwrap(),
            Outcome::Value(_)
        ));
    }
    let Outcome::Value(books) = api.books().unwrap() else {
        panic!("books")
    };
    let books: Books = books;
    let worker = ApiWorker::spawn(api).unwrap();
    let sales = books
        .masters
        .voucher_types
        .values()
        .find(|t| t.base == BaseVoucherKind::Sales)
        .unwrap()
        .id;
    let fy = FinancialYear::april(2026).unwrap();
    let Outcome::Value(mut screen) =
        ItemInvoiceScreen::open(&books, sales, fy, date!(2026 - 11 - 21)).unwrap()
    else {
        panic!("the sales type is entered on this screen");
    };
    fill(&mut screen, &books.masters);
    let mut app = AppCtx {
        books: Some(books),
        api: Some(worker),
        lock_date: None,
        ai_tier: None,
        tr: ledgerly_app::i18n::Tr::default(),
    };
    let start = Instant::now();
    loop {
        frame(&mut screen, &mut app);
        let masters = &app.books.as_ref().unwrap().masters;
        if let Some(note) = screen.einvoice_note(masters) {
            return Some(note.to_owned());
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "no e-invoice answer"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn the_sample_companys_default_invoice_says_json_ready() {
    assert_eq!(
        note_for_default_invoice(false).as_deref(),
        Some("JSON ready (INV-01)")
    );
}

#[test]
fn a_buyer_with_no_pin_says_what_is_missing() {
    assert_eq!(
        note_for_default_invoice(true).as_deref(),
        Some("the buyer Shree Ganesh Traders has no PIN code (Pin is required)")
    );
}
