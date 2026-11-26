//! Hindi on the print messages: the notices a print job leaves on the report and voucher
//! screens come from the translation files (spec 7; the PDF body itself is ASCII by design, see
//! `design/PARITY-GAPS.md`).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_app::i18n::{Catalog, Tr, set_current};
use ledgerly_app::screens::print_job::print_notice;
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
    let ready: Result<Outcome<_>, String> = Ok(Outcome::Value(ledgerly_api::PrintedDocument {
        pdf: vec![0; 1234],
        pages: Vec::new(),
        copies: 1,
    }));
    let unsupported: Result<Outcome<ledgerly_api::PrintedDocument>, String> =
        Ok(Outcome::Unsupported(Unsupported {
            what: "Sales register".to_owned(),
            reason: "the engine's reason".to_owned(),
        }));
    let failed: Result<Outcome<ledgerly_api::PrintedDocument>, String> =
        Err("the store is closed".to_owned());
    for done in [&ready, &unsupported, &failed] {
        set_current(&en);
        let english = print_notice("Balance sheet", done);
        set_current(&hi);
        let hindi = print_notice("Balance sheet", done);
        set_current(&Tr::default());
        assert_ne!(text(&hindi), text(&english), "still English");
        assert!(text(&hindi).contains("Balance sheet"), "title is kept");
    }
}
