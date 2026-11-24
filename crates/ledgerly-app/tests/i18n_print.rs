//! Hindi on the print messages: the notices a print job leaves on the report and voucher
//! screens come from the translation files (spec 7; the PDF body itself is ASCII by design, see
//! `design/PARITY-GAPS.md`).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::Path;

use ledgerly_app::i18n::Catalog;
use ledgerly_app::screens::print_job::{
    lost_notice_in, print_notice, print_notice_in, saved_notice_in,
};
use ledgerly_app::screens::report_view::Notice;
use ledgerly_core::{Outcome, Unsupported};

fn text(notice: &Notice) -> &str {
    match notice {
        Notice::Info(s) | Notice::Error(s) => s,
    }
}

#[test]
fn print_result_notices_come_from_the_hindi_file() {
    let hi = Catalog::builtin().unwrap().tr("hi");
    let en = Catalog::builtin().unwrap().tr("en");
    let ready: Result<Outcome<Vec<u8>>, String> = Ok(Outcome::Value(vec![0; 1234]));
    let unsupported: Result<Outcome<Vec<u8>>, String> = Ok(Outcome::Unsupported(Unsupported {
        what: "Sales register".to_owned(),
        reason: "the engine's reason".to_owned(),
    }));
    let failed: Result<Outcome<Vec<u8>>, String> = Err("the store is closed".to_owned());
    for done in [&ready, &unsupported, &failed] {
        // The English words are the screen's own: the same text as the English wrapper.
        assert_eq!(
            print_notice_in("Balance sheet", done, &en),
            print_notice("Balance sheet", done)
        );
        let hindi = print_notice_in("Balance sheet", done, &hi);
        assert_ne!(hindi, print_notice("Balance sheet", done), "still English");
        assert!(text(&hindi).contains("Balance sheet"), "title is kept");
    }
    // provenance: 1234 bytes is the length of the vector built above; the words are unsourced
    // Hindi in hi.toml (`print.ready`).
    assert_eq!(
        text(&print_notice_in("Balance sheet", &ready, &hi)),
        hi.tf(
            "print.ready",
            &[("title", "Balance sheet"), ("bytes", "1234")]
        )
    );
}

#[test]
fn saved_and_engine_stopped_notices_come_from_the_hindi_file() {
    let hi = Catalog::builtin().unwrap().tr("hi");
    let en = Catalog::builtin().unwrap().tr("en");
    let path = Path::new("/home/user/Documents/Balance sheet.pdf");
    assert_eq!(
        text(&saved_notice_in(path, &en)),
        "Saved PDF to /home/user/Documents/Balance sheet.pdf."
    );
    let saved = saved_notice_in(path, &hi);
    assert!(
        !text(&saved).contains("Saved PDF"),
        "still English: {saved:?}"
    );
    assert!(text(&saved).contains("/home/user/Documents/Balance sheet.pdf"));

    let lost = lost_notice_in("Voucher 1", &hi);
    assert_ne!(
        text(&lost),
        "Voucher 1 was not printed: the engine stopped before the print finished"
    );
    assert!(text(&lost).contains("Voucher 1"));
}
