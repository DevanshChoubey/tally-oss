//! s11: the read-only voucher view shows the mock's Item table and Audit card
//! (`design/mock-v2/index.html`, `balance-sheet.voucher`).
//!
//! Provenance: the sample company is the mock's own (SI/0412: Drip lateral 16 mm, 400 m roll,
//! 25 roll at 3,850.00 = 96,250.00 and Brass nozzle set (6 pc), 67 set at 505.00 = 33,835.00, the
//! mock's Item table). Derived: 25 x 3,850 = 96,250 and 67 x 505 = 33,835. The Audit wording is
//! the mock's, with the facts the log keeps.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_api::{Api, ChangeKind, EntryOrigin, VoucherAudit, VoucherChange, VoucherDetail};
use ledgerly_app::screens::report_view::{audit_lines, item_texts, voucher_view};
use ledgerly_core::{Clock, EventSeq, Outcome, Paise};
use time::OffsetDateTime;
use time::macros::{date, datetime};

fn sample(name: &str) -> Api {
    let path = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    ledgerly_sample::create(&path, Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap()
}

#[test]
fn the_item_table_lists_the_stock_lines_as_stored() {
    let api = sample("s11_items.ledgerly");
    let books = api.books().unwrap().value().unwrap();
    let id = books
        .vouchers
        .values()
        .find(|v| v.number.as_deref() == Some("SI/0412"))
        .unwrap()
        .id;
    let view = voucher_view(&books, id).unwrap();
    let rows: Vec<(String, String, String, String)> = view
        .items
        .iter()
        .map(|i| {
            let (qty, rate, amount) = item_texts(i);
            (i.name.clone(), qty, rate, amount)
        })
        .collect();
    let want = [
        (
            "Drip lateral 16 mm, 400 m roll",
            "25 roll",
            "3,850.00",
            "96,250.00",
        ),
        ("Brass nozzle set (6 pc)", "67 set", "505.00", "33,835.00"),
    ]
    .map(|(a, b, c, d)| (a.to_owned(), b.to_owned(), c.to_owned(), d.to_owned()));
    assert_eq!(rows, want);
}

fn audit(origin: EntryOrigin, changes: Vec<VoucherChange>) -> VoucherAudit {
    VoucherAudit {
        entered_by: "Ananya K.".to_owned(),
        origin,
        posted_at: datetime!(2026-11-14 12:00 +05:30),
        event: EventSeq(52),
        hash: "3fa9c1d2e4b6aa00".repeat(4),
        changes,
    }
}

fn detail(audit: Option<VoucherAudit>, lock: Option<time::Date>) -> VoucherDetail {
    VoucherDetail {
        debit_total: Paise::new(100),
        credit_total: Paise::new(100),
        audit,
        lock_date: lock,
        date: date!(2026 - 11 - 14),
    }
}

#[test]
fn a_typed_untouched_voucher_in_an_open_period_reads_as_the_mocks_card() {
    // provenance: index.html:1629: "Created 14 Nov 2026 by Ananya K., typed by hand." / "Not
    // altered since. Alter is allowed: the period is open." Plus the log record the store keeps.
    let lines = audit_lines(&detail(Some(audit(EntryOrigin::Typed, Vec::new())), None));
    assert_eq!(
        lines,
        vec![
            "Created 14 Nov 2026 by Ananya K., typed by hand.".to_owned(),
            "Not altered since. Alter is allowed: the period is open.".to_owned(),
            "Audit log entry 52, hash 3fa9c1d2e4b6.".to_owned(),
        ]
    );
}

#[test]
fn an_ai_draft_an_import_a_change_and_a_lock_are_each_said() {
    // provenance: derived from the card's rules: an AI draft names the person who accepted it
    // (the log keeps no edit count, so none is claimed); an import says so; each later change is
    // listed with its date and who; a voucher on or before the lock date says "Period locked."
    let ai = audit_lines(&detail(Some(audit(EntryOrigin::AiDraft, Vec::new())), None));
    assert_eq!(
        ai[0],
        "Created 14 Nov 2026 from an AI draft, accepted by Ananya K."
    );
    let imported = audit_lines(&detail(
        Some(audit(EntryOrigin::Imported, Vec::new())),
        None,
    ));
    assert_eq!(
        imported[0],
        "Created 14 Nov 2026 by Ananya K., through an import."
    );
    let changed = vec![VoucherChange {
        kind: ChangeKind::Altered,
        by: "Rohan D.".to_owned(),
        at: datetime!(2026-11-18 09:00 +05:30),
        event: EventSeq(60),
    }];
    let lines = audit_lines(&detail(
        Some(audit(EntryOrigin::Typed, changed)),
        Some(date!(2026 - 11 - 15)),
    ));
    assert_eq!(lines[1], "Altered 18 Nov 2026 by Rohan D.");
    assert_eq!(lines[2], "Period locked.");
}

#[test]
fn a_voucher_with_no_log_record_says_so() {
    // provenance: derived (rule A1): nothing is made up about who entered it.
    let lines = audit_lines(&detail(None, None));
    assert_eq!(
        lines[0],
        "The audit log holds no record of who entered this voucher."
    );
}

#[test]
fn the_engine_gives_the_detail_for_the_sample_voucher() {
    let api = sample("s11_detail.ledgerly");
    let books = api.books().unwrap().value().unwrap();
    let id = books
        .vouchers
        .values()
        .find(|v| v.number.as_deref() == Some("SI/0412"))
        .unwrap()
        .id;
    let Outcome::Value(d) = api.voucher_detail(id).unwrap() else {
        panic!("no detail");
    };
    let lines = audit_lines(&d);
    assert_eq!(lines[0], "Created 14 Nov 2026 by Ananya K., typed by hand.");
    // The lock is 31 Oct 2026, so a 14 Nov voucher is in an open period.
    assert_eq!(
        lines[1],
        "Not altered since. Alter is allowed: the period is open."
    );
}
