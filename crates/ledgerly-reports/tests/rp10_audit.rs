//! Card RP10: auditor report tests. Records are built by hand; every expected value is derived in
//! a comment (by construction of the records, or by calendar arithmetic).
//!
//! Column layout of every event report: 0 Seq, 1 Date, 2 Time, 3 User, 4 Action, 5 Object,
//! 6 Reason, then report-specific columns (7.. : Field, Before, After for history, log and
//! deleted/altered; Voucher date, Latest earlier date, Days back for back-dated; Voucher date,
//! Lock date for edits after lock).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    CompanyId, Date, EventSeq, GroupId, LedgerId, OffsetDateTime, Outcome, Paise, Period,
    SignedAmount, StateCode, UserId, VoucherId, VoucherTypeId,
};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, Group, LedgerLine,
    MasterSnapshot, Masters, NegativeStockPolicy, NumberingConfig, NumberingMethod, RestartPeriod,
    Scenario, Voucher, VoucherMode, VoucherStatus, VoucherType,
};
use ledgerly_reports::audit::{audit_report, integrity_certificate};
use ledgerly_reports::{
    AuditReportKind, Cell, ColumnSpec, Detail, DrillKey, Report, ReportError, ReportFilter,
    ReportKind, ReportParams,
};
use ledgerly_store::verify::{ChainProblem, Checkpoint, VerifyReport};
use ledgerly_store::{Actor, AuditMeta, AuditRecord, Event};
use time::macros::{date, datetime};

fn books() -> Books {
    let mut masters = Masters::default();
    masters.voucher_types.insert(
        VoucherTypeId(1),
        VoucherType {
            id: VoucherTypeId(1),
            name: "Journal".to_owned(),
            base: BaseVoucherKind::Journal,
            abbreviation: "Jrnl".to_owned(),
            numbering: NumberingConfig {
                method: NumberingMethod::Automatic,
                prevent_renumbering: false,
                prefix: String::new(),
                suffix: String::new(),
                width: 0,
                restart: RestartPeriod::Never,
                series: vec![],
            },
            active: true,
            tally: None,
        },
    );
    Books {
        company: Company {
            id: CompanyId(1),
            name: "Test Traders".to_owned(),
            mailing_name: None,
            address: Address::default(),
            state: StateCode::new(27).unwrap(),
            pan: None,
            registrations: vec![],
            fy_start: date!(2026 - 04 - 01),
            books_begin: date!(2026 - 04 - 01),
            entity_kind: EntityKind::Proprietorship,
            features: Features::default(),
            negative_stock: NegativeStockPolicy::Block,
        },
        masters,
        vouchers: BTreeMap::new(),
        postings: vec![],
        stock_moves: vec![],
    }
}

fn voucher(id: u64, type_id: u64, on: Date, narration: &str) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id: VoucherTypeId(type_id),
        number: Some(format!("V{id}")),
        series: None,
        date: on,
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::AsVoucher,
        party: None,
        place_of_supply: None,
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: narration.to_owned(),
        ledger_lines: vec![],
        inventory_lines: vec![],
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn group(id: u64, name: &str) -> Group {
    Group {
        id: GroupId(id),
        name: name.to_owned(),
        aliases: vec![],
        parent: None,
        reserved: None,
        nature: None,
        affects_gross_profit: false,
        tally: None,
    }
}

fn rec_by(seq: u64, user: u64, at: OffsetDateTime, event: Event) -> AuditRecord {
    AuditRecord {
        seq: EventSeq(seq),
        prev_hash: [0; 32],
        hash: [0; 32],
        meta: AuditMeta {
            actor: Actor::User(UserId(user)),
            at,
            reason: None,
            app_version: "0.0.0-test".to_owned(),
        },
        event,
    }
}

/// A record by user 1 on 15 April 2026, 10:00 at +05:30.
fn rec(seq: u64, event: Event) -> AuditRecord {
    rec_by(seq, 1, datetime!(2026-04-15 10:00 +5:30), event)
}

fn params(kind: AuditReportKind) -> ReportParams {
    ReportParams {
        kind: ReportKind::Audit(kind),
        period: Period::new(date!(2026 - 04 - 01), date!(2026 - 04 - 30)).unwrap(),
        as_of: date!(2026 - 04 - 30),
        scenario: Scenario::Actual,
        detail: Detail::Detailed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "test helper: call sites read better passing the kind by value"
)]
fn run_with(records: &[AuditRecord], kind: AuditReportKind, p: &ReportParams) -> Report {
    match audit_report(records, &books(), &kind, p).unwrap() {
        Outcome::Value(r) => r,
        Outcome::Unsupported(u) => panic!("unexpectedly unsupported: {u:?}"),
    }
}

fn run(records: &[AuditRecord], kind: AuditReportKind) -> Report {
    let p = params(kind.clone());
    run_with(records, kind, &p)
}

fn text(c: &Cell) -> String {
    let Cell::Text(t) = c else {
        panic!("expected a text cell, got {c:?}");
    };
    t.clone()
}

/// Sequence numbers of the event rows (depth 0), in row order.
fn seqs(r: &Report) -> Vec<String> {
    r.rows
        .iter()
        .filter(|row| row.depth == 0)
        .map(|row| text(&row.cells[0]))
        .collect()
}

/// (field, before, after) of the diff rows (depth 1), in row order.
fn diffs(r: &Report) -> Vec<(String, String, String)> {
    r.rows
        .iter()
        .filter(|row| row.depth == 1)
        .map(|row| {
            (
                text(&row.cells[7]),
                text(&row.cells[8]),
                text(&row.cells[9]),
            )
        })
        .collect()
}

fn triple(f: &str, b: &str, a: &str) -> (String, String, String) {
    (f.to_owned(), b.to_owned(), a.to_owned())
}

fn event_row_cells(r: &Report, seq: &str) -> Vec<Cell> {
    r.rows
        .iter()
        .find(|row| row.depth == 0 && row.cells[0] == Cell::Text(seq.to_owned()))
        .unwrap()
        .cells
        .clone()
}

fn posted(id: u64, type_id: u64, on: Date) -> Event {
    Event::VoucherPosted(voucher(id, type_id, on, "n"))
}

// ---------------------------------------------------------------------------------------------
// Field diff
// ---------------------------------------------------------------------------------------------

#[test]
fn alter_shows_before_and_after_of_the_changed_field_only() {
    // derived: the two vouchers differ in `narration` and nothing else, so exactly one field
    // is listed, with the old and new text.
    let before = voucher(1, 1, date!(2026 - 04 - 10), "Rent April");
    let mut after = before.clone();
    after.narration = "Rent for April".to_owned();
    let records = [
        rec(1, Event::VoucherPosted(before.clone())),
        rec(2, Event::VoucherAltered { before, after }),
    ];
    let r = run(&records, AuditReportKind::DeletedAltered);
    assert_eq!(seqs(&r), ["2"]);
    assert_eq!(
        diffs(&r),
        [triple("narration", "Rent April", "Rent for April")]
    );
    assert_eq!(text(&event_row_cells(&r, "2")[4]), "Altered voucher");
}

#[test]
fn alter_of_two_fields_lists_both_in_field_name_order_with_dates_readable() {
    // derived: `date` moved from 10 April to 11 April 2026 and `narration` changed; no other
    // field differs. Fields are listed in alphabetical order: "date" < "narration".
    let before = voucher(1, 1, date!(2026 - 04 - 10), "Rent April");
    let mut after = before.clone();
    after.date = date!(2026 - 04 - 11);
    after.narration = "Rent on 11th".to_owned();
    let records = [rec(1, Event::VoucherAltered { before, after })];
    let r = run(&records, AuditReportKind::DeletedAltered);
    assert_eq!(
        diffs(&r),
        [
            triple("date", "2026-04-10", "2026-04-11"),
            triple("narration", "Rent April", "Rent on 11th"),
        ]
    );
}

#[test]
fn alter_of_an_amount_shows_rupees_and_side_not_raw_paise() {
    // derived: 10000 paise is 100.00 rupees and 25050 paise is 250.50 rupees (100 paise to the
    // rupee); both lines are debits. Only the line amount differs, so one field is listed, as
    // an amount with its side.
    let line = |paise: i64| LedgerLine {
        ledger: LedgerId(1),
        amount: SignedAmount::dr(Paise::new(paise)),
        bills: vec![],
        cost: vec![],
        bank: None,
    };
    let mut before = voucher(1, 1, date!(2026 - 04 - 10), "n");
    before.ledger_lines = vec![line(10_000)];
    let mut after = before.clone();
    after.ledger_lines = vec![line(25_050)];
    let records = [rec(1, Event::VoucherAltered { before, after })];
    let r = run(&records, AuditReportKind::DeletedAltered);
    assert_eq!(
        diffs(&r),
        [triple(
            "ledger_lines[0].amount",
            "Rs 100.00 Dr",
            "Rs 250.50 Dr"
        )]
    );
}

#[test]
fn alter_with_no_difference_lists_no_field() {
    // derived: identical before and after leave nothing to list.
    let v = voucher(1, 1, date!(2026 - 04 - 10), "same");
    let records = [rec(
        1,
        Event::VoucherAltered {
            before: v.clone(),
            after: v,
        },
    )];
    let r = run(&records, AuditReportKind::DeletedAltered);
    assert_eq!(seqs(&r), ["1"]);
    assert_eq!(diffs(&r).len(), 0);
}

#[test]
fn master_alter_shows_only_the_renamed_field() {
    // derived: groups 4 differs only in `name`; group 5's events are outside the selection.
    let g4 = group(4, "Sundry Debtors");
    let mut g4b = g4.clone();
    g4b.name = "Trade Debtors".to_owned();
    let records = [
        rec(1, Event::MasterCreated(MasterSnapshot::Group(g4.clone()))),
        rec(
            2,
            Event::MasterCreated(MasterSnapshot::Group(group(5, "Other"))),
        ),
        rec(
            3,
            Event::MasterAltered {
                before: MasterSnapshot::Group(g4),
                after: MasterSnapshot::Group(g4b),
            },
        ),
    ];
    let mut p = params(AuditReportKind::ObjectHistory);
    p.filter.groups = vec![GroupId(4)];
    let r = run_with(&records, AuditReportKind::ObjectHistory, &p);
    assert_eq!(seqs(&r), ["1", "3"]);
    assert_eq!(
        diffs(&r),
        [triple("name", "Sundry Debtors", "Trade Debtors")]
    );
    assert_eq!(r.rows[0].key, Some(DrillKey::Group(GroupId(4))));
    assert_eq!(text(&event_row_cells(&r, "3")[4]), "Altered group");
}

#[test]
fn setting_change_lists_the_setting_as_the_field() {
    // derived: one setting changed from "off" to "on"; the key is the field name.
    let records = [rec(
        1,
        Event::SettingChanged {
            key: "gst.enabled".to_owned(),
            before: "off".to_owned(),
            after: "on".to_owned(),
        },
    )];
    let r = run(&records, AuditReportKind::ObjectHistory);
    assert_eq!(diffs(&r), [triple("gst.enabled", "off", "on")]);
}

// ---------------------------------------------------------------------------------------------
// Object history ordering and filters
// ---------------------------------------------------------------------------------------------

#[test]
fn object_history_groups_events_by_object_then_sequence() {
    // derived: objects sort by (kind, id): group 4 before group 5. Group 5 has events 1 and 3,
    // group 4 has event 2, so the order is 2, 1, 3.
    let records = [
        rec(
            1,
            Event::MasterCreated(MasterSnapshot::Group(group(5, "B"))),
        ),
        rec(
            2,
            Event::MasterCreated(MasterSnapshot::Group(group(4, "A"))),
        ),
        rec(
            3,
            Event::MasterAltered {
                before: MasterSnapshot::Group(group(5, "B")),
                after: MasterSnapshot::Group(group(5, "B2")),
            },
        ),
    ];
    let r = run(&records, AuditReportKind::ObjectHistory);
    assert_eq!(seqs(&r), ["2", "1", "3"]);
}

#[test]
fn object_history_filtered_by_voucher_type_is_unsupported() {
    // Rule A1: a voucher cannot be selected yet (no voucher field in ReportFilter).
    let mut p = params(AuditReportKind::ObjectHistory);
    p.filter.voucher_types = vec![VoucherTypeId(1)];
    let out = audit_report(&[], &books(), &AuditReportKind::ObjectHistory, &p).unwrap();
    assert!(out.is_unsupported());
}

// ---------------------------------------------------------------------------------------------
// Deleted and altered
// ---------------------------------------------------------------------------------------------

#[test]
fn deleted_altered_lists_alter_cancel_and_delete_but_not_post() {
    // derived: of five events only seq 2 (alter), 3 (cancel) and 4 (delete) are alterations
    // of a voucher; seq 1 and 5 post vouchers and seq 6 is a master event.
    let v1 = voucher(1, 1, date!(2026 - 04 - 10), "a");
    let v2 = voucher(2, 1, date!(2026 - 04 - 11), "b");
    let records = [
        rec(1, Event::VoucherPosted(v1.clone())),
        rec(
            2,
            Event::VoucherAltered {
                before: v1.clone(),
                after: v1.clone(),
            },
        ),
        rec(3, Event::VoucherCancelled { before: v1 }),
        rec(4, Event::VoucherDeleted { before: v2 }),
        rec(5, posted(3, 1, date!(2026 - 04 - 12))),
        rec(
            6,
            Event::MasterCreated(MasterSnapshot::Group(group(9, "G"))),
        ),
    ];
    let r = run(&records, AuditReportKind::DeletedAltered);
    assert_eq!(seqs(&r), ["2", "3", "4"]);
    let actions: Vec<String> = ["2", "3", "4"]
        .iter()
        .map(|s| text(&event_row_cells(&r, s)[4]))
        .collect();
    assert_eq!(
        actions,
        ["Altered voucher", "Cancelled voucher", "Deleted voucher"]
    );
    // The object label carries the voucher type name from the books, the number and the date.
    assert_eq!(
        text(&event_row_cells(&r, "4")[5]),
        "Journal V2 dated 2026-04-11 [voucher 2]"
    );
}

// ---------------------------------------------------------------------------------------------
// Back-dated entries
// ---------------------------------------------------------------------------------------------

fn back_dated_records() -> Vec<AuditRecord> {
    vec![
        rec(1, posted(1, 1, date!(2026 - 04 - 10))),
        rec(2, posted(2, 1, date!(2026 - 04 - 12))),
        rec(3, posted(3, 1, date!(2026 - 04 - 05))),
        rec(4, posted(4, 1, date!(2026 - 04 - 12))),
        rec(5, posted(5, 2, date!(2026 - 04 - 11))),
        // An alteration that moves a date earlier is not examined.
        rec(
            6,
            Event::VoucherAltered {
                before: voucher(1, 1, date!(2026 - 04 - 10), "n"),
                after: voucher(1, 1, date!(2026 - 04 - 01), "n"),
            },
        ),
    ]
}

#[test]
fn back_dated_lists_entries_dated_before_the_latest_earlier_date() {
    // derived: running latest date after each post: 10 Apr, 12 Apr, 12 Apr, 12 Apr, 12 Apr.
    // Seq 1 has no earlier entry. Seq 2 (12 Apr) is not before 10 Apr. Seq 3 (5 Apr) is before
    // 12 Apr by 12 - 5 = 7 days. Seq 4 (12 Apr) equals the latest, so it is not back-dated.
    // Seq 5 (11 Apr) is before 12 Apr by 1 day.
    let r = run(&back_dated_records(), AuditReportKind::BackDated);
    assert_eq!(seqs(&r), ["3", "5"]);
    let c3 = event_row_cells(&r, "3");
    assert_eq!(c3[7], Cell::Date(date!(2026 - 04 - 05)));
    assert_eq!(c3[8], Cell::Date(date!(2026 - 04 - 12)));
    assert_eq!(c3[9], Cell::Text("7".to_owned()));
    let c5 = event_row_cells(&r, "5");
    assert_eq!(c5[7], Cell::Date(date!(2026 - 04 - 11)));
    assert_eq!(c5[8], Cell::Date(date!(2026 - 04 - 12)));
    assert_eq!(c5[9], Cell::Text("1".to_owned()));
}

#[test]
fn back_dated_filter_by_type_keeps_the_global_latest_date() {
    // derived: voucher type 2 posted only at seq 5; it is back-dated against the 12 April
    // posting of type 1, so filtering to type 2 lists seq 5 alone.
    let mut p = params(AuditReportKind::BackDated);
    p.filter.voucher_types = vec![VoucherTypeId(2)];
    let r = run_with(&back_dated_records(), AuditReportKind::BackDated, &p);
    assert_eq!(seqs(&r), ["5"]);
}

#[test]
fn back_dated_first_posting_and_in_order_postings_give_an_empty_report() {
    // derived: dates rise 1, 2, 3 April; none is before an earlier one.
    let records = [
        rec(1, posted(1, 1, date!(2026 - 04 - 01))),
        rec(2, posted(2, 1, date!(2026 - 04 - 02))),
        rec(3, posted(3, 1, date!(2026 - 04 - 03))),
    ];
    let r = run(&records, AuditReportKind::BackDated);
    assert_eq!(seqs(&r).len(), 0);
    assert_eq!(r.totals[0], Cell::Text("0 events".to_owned()));
}

// ---------------------------------------------------------------------------------------------
// Edits after lock
// ---------------------------------------------------------------------------------------------

#[test]
fn back_dated_says_so_when_the_window_does_not_start_at_event_one() {
    // derived: the records start at event 2, so event 1 (and anything before it) was not read;
    // the report lists no entry (no earlier posting is known) and says why. A window that
    // starts at event 1 carries only the standing note.
    let from_two = [rec(2, posted(1, 1, date!(2026 - 04 - 05)))];
    let r = run(&from_two, AuditReportKind::BackDated);
    assert_eq!(r.notes.len(), 2);
    assert!(r.notes[1].starts_with("The records supplied start at event 2."));
    let from_one = [rec(1, posted(1, 1, date!(2026 - 04 - 05)))];
    assert_eq!(run(&from_one, AuditReportKind::BackDated).notes.len(), 1);
}

#[test]
fn edits_after_lock_says_so_when_the_window_does_not_start_at_event_one() {
    // derived: as for back-dated entries; the lock report starts from the first record supplied.
    let from_three = [rec(3, posted(1, 1, date!(2026 - 03 - 05)))];
    let r = run(&from_three, AuditReportKind::EditsAfterLock);
    assert_eq!(r.notes.len(), 2);
    assert!(r.notes[1].starts_with("The records supplied start at event 3."));
}

#[test]
fn a_count_of_one_event_is_singular() {
    // derived: one record gives "1 event" in the total and in the user note.
    let r = run(
        &[rec(1, posted(1, 1, date!(2026 - 04 - 05)))],
        AuditReportKind::EditLog,
    );
    assert_eq!(r.totals[0], Cell::Text("1 event".to_owned()));
    assert_eq!(r.notes, ["user 1: 1 event"]);
}

#[test]
fn edits_after_lock_uses_the_lock_in_force_when_the_event_was_recorded() {
    // derived, lock dates in force by sequence number:
    //   seq 1: none        seq 2: sets 31 Mar   seq 3..6: 31 Mar   seq 7: sets 10 Mar   seq 8: 10 Mar
    // seq 1 posts 15 Mar with no lock: not listed.
    // seq 3 alters a 15 Mar voucher: 15 <= 31 Mar: listed (voucher date 15 Mar).
    // seq 4 posts 5 Apr: 5 Apr > 31 Mar: not listed.
    // seq 5 deletes a voucher dated 31 Mar: equal to the lock date: listed.
    // seq 6 posts 1 Apr: one day after the lock: not listed.
    // seq 8 alters a 15 Mar voucher after the lock moved to 10 Mar: 15 > 10: not listed.
    let v15 = voucher(1, 1, date!(2026 - 03 - 15), "n");
    let mut v15b = v15.clone();
    v15b.narration = "changed".to_owned();
    let records = [
        rec(1, Event::VoucherPosted(v15.clone())),
        rec(
            2,
            Event::PeriodLocked {
                until: date!(2026 - 03 - 31),
            },
        ),
        rec(
            3,
            Event::VoucherAltered {
                before: v15.clone(),
                after: v15b.clone(),
            },
        ),
        rec(4, posted(2, 1, date!(2026 - 04 - 05))),
        rec(
            5,
            Event::VoucherDeleted {
                before: voucher(3, 1, date!(2026 - 03 - 31), "n"),
            },
        ),
        rec(6, posted(4, 1, date!(2026 - 04 - 01))),
        rec(
            7,
            Event::PeriodLocked {
                until: date!(2026 - 03 - 10),
            },
        ),
        rec(
            8,
            Event::VoucherAltered {
                before: v15b,
                after: v15,
            },
        ),
    ];
    let r = run(&records, AuditReportKind::EditsAfterLock);
    assert_eq!(seqs(&r), ["3", "5"]);
    let c3 = event_row_cells(&r, "3");
    assert_eq!(c3[7], Cell::Date(date!(2026 - 03 - 15)));
    assert_eq!(c3[8], Cell::Date(date!(2026 - 03 - 31)));
    let c5 = event_row_cells(&r, "5");
    assert_eq!(c5[7], Cell::Date(date!(2026 - 03 - 31)));
    assert_eq!(c5[8], Cell::Date(date!(2026 - 03 - 31)));
}

#[test]
fn edits_after_lock_with_no_lock_event_is_empty() {
    // derived: with no PeriodLocked event nothing was ever locked.
    let records = [rec(1, posted(1, 1, date!(2020 - 01 - 01)))];
    let r = run(&records, AuditReportKind::EditsAfterLock);
    assert_eq!(seqs(&r).len(), 0);
}

#[test]
fn edits_after_lock_alteration_moving_a_date_out_of_the_locked_period_is_listed() {
    // derived: lock 31 Mar; the alteration moves a voucher from 20 Mar to 5 Apr. The old date
    // (20 Mar) is inside the locked period, so the event is listed with voucher date 20 Mar.
    let records = [
        rec(
            1,
            Event::PeriodLocked {
                until: date!(2026 - 03 - 31),
            },
        ),
        rec(
            2,
            Event::VoucherAltered {
                before: voucher(1, 1, date!(2026 - 03 - 20), "n"),
                after: voucher(1, 1, date!(2026 - 04 - 05), "n"),
            },
        ),
    ];
    let r = run(&records, AuditReportKind::EditsAfterLock);
    assert_eq!(seqs(&r), ["2"]);
    assert_eq!(
        event_row_cells(&r, "2")[7],
        Cell::Date(date!(2026 - 03 - 20))
    );
}

// ---------------------------------------------------------------------------------------------
// Edit log
// ---------------------------------------------------------------------------------------------

#[test]
fn edit_log_selects_by_the_users_own_calendar_day() {
    // derived: seq 1 is 10 Apr 23:30 at +05:30 (local day 10 Apr). Seq 2 is 11 Apr 01:00 at
    // +05:30, which is 10 Apr 19:30 UTC, but the user's day is 11 Apr. Seq 3 is 2 May.
    let records = [
        rec_by(
            1,
            1,
            datetime!(2026-04-10 23:30 +5:30),
            posted(1, 1, date!(2026 - 04 - 10)),
        ),
        rec_by(
            2,
            2,
            datetime!(2026-04-11 01:00 +5:30),
            posted(2, 1, date!(2026 - 04 - 11)),
        ),
        rec_by(
            3,
            1,
            datetime!(2026-05-02 10:00 +5:30),
            posted(3, 1, date!(2026 - 05 - 02)),
        ),
    ];
    let mut p = params(AuditReportKind::EditLog);
    p.period = Period::new(date!(2026 - 04 - 10), date!(2026 - 04 - 10)).unwrap();
    assert_eq!(
        seqs(&run_with(&records, AuditReportKind::EditLog, &p)),
        ["1"]
    );
    p.period = Period::new(date!(2026 - 04 - 11), date!(2026 - 04 - 30)).unwrap();
    let r = run_with(&records, AuditReportKind::EditLog, &p);
    assert_eq!(seqs(&r), ["2"]);
    let c = event_row_cells(&r, "2");
    assert_eq!(c[1], Cell::Date(date!(2026 - 04 - 11)));
    assert_eq!(c[2], Cell::Text("01:00:00 +05:30".to_owned()));
    assert_eq!(c[3], Cell::Text("user 2".to_owned()));
    p.period = Period::new(date!(2026 - 04 - 01), date!(2026 - 05 - 31)).unwrap();
    let all = run_with(&records, AuditReportKind::EditLog, &p);
    assert_eq!(seqs(&all), ["1", "2", "3"]);
    // derived: user 1 made seq 1 and 3, user 2 made seq 2; one event is "1 event", not
    // "1 events".
    assert_eq!(all.notes, ["user 1: 2 events", "user 2: 1 event"]);
    assert_eq!(all.totals[0], Cell::Text("3 events".to_owned()));
}

#[test]
fn edit_log_filtered_by_voucher_type_keeps_only_vouchers_of_that_type() {
    // derived: seq 2 is the only event on a type 2 voucher; the master event is not a voucher.
    let records = [
        rec(1, posted(1, 1, date!(2026 - 04 - 10))),
        rec(2, posted(2, 2, date!(2026 - 04 - 10))),
        rec(
            3,
            Event::MasterCreated(MasterSnapshot::Group(group(9, "G"))),
        ),
    ];
    let mut p = params(AuditReportKind::EditLog);
    p.filter.voucher_types = vec![VoucherTypeId(2)];
    assert_eq!(
        seqs(&run_with(&records, AuditReportKind::EditLog, &p)),
        ["2"]
    );
}

#[test]
fn edit_log_reason_and_actor_kinds_are_shown() {
    // derived: the reason is copied from the metadata; an import actor names its job and user.
    let mut r1 = rec(1, posted(1, 1, date!(2026 - 04 - 10)));
    r1.meta.reason = Some("year-end fix".to_owned());
    r1.meta.actor = Actor::Import {
        job: "tally-import".to_owned(),
        on_behalf_of: UserId(3),
    };
    let r = run(&[r1], AuditReportKind::EditLog);
    let c = event_row_cells(&r, "1");
    assert_eq!(
        c[3],
        Cell::Text("import 'tally-import' for user 3".to_owned())
    );
    assert_eq!(c[6], Cell::Text("year-end fix".to_owned()));
}

#[test]
fn edit_log_with_a_ledger_filter_is_unsupported() {
    // Rule A1: the log cannot be narrowed by ledger; it says so instead of ignoring the filter.
    let mut p = params(AuditReportKind::EditLog);
    p.filter.ledgers = vec![LedgerId(1)];
    let out = audit_report(&[], &books(), &AuditReportKind::EditLog, &p).unwrap();
    assert!(out.is_unsupported());
}

// ---------------------------------------------------------------------------------------------
// Input checks
// ---------------------------------------------------------------------------------------------

#[test]
fn input_order_does_not_change_the_report() {
    // derived: the report sorts by sequence number, so reversing the input changes nothing.
    let forward = back_dated_records();
    let mut backward = forward.clone();
    backward.reverse();
    let a = run(&forward, AuditReportKind::BackDated);
    let b = run(&backward, AuditReportKind::BackDated);
    assert_eq!(a, b);
}

#[test]
fn duplicate_sequence_number_is_an_error() {
    // derived: sequence numbers are unique by the log's definition (spec 6.6), so two records
    // numbered 1 cannot both be genuine; the report refuses rather than pick one.
    let records = [
        rec(1, posted(1, 1, date!(2026 - 04 - 10))),
        rec(1, posted(2, 1, date!(2026 - 04 - 11))),
    ];
    let p = params(AuditReportKind::EditLog);
    let err = audit_report(&records, &books(), &AuditReportKind::EditLog, &p).unwrap_err();
    assert!(matches!(err, ReportError::Invalid { .. }));
}

#[test]
fn mismatched_params_kind_is_an_error() {
    // derived: params built for the edit log must not produce a back-dated report.
    let p = params(AuditReportKind::EditLog);
    let err = audit_report(&[], &books(), &AuditReportKind::BackDated, &p).unwrap_err();
    assert!(matches!(err, ReportError::Invalid { .. }));
}

#[test]
fn period_columns_are_unsupported() {
    // Rule A1: a monthly layout has no meaning for an event log; it says so.
    let mut p = params(AuditReportKind::EditLog);
    p.columns = ColumnSpec::Monthly;
    let out = audit_report(&[], &books(), &AuditReportKind::EditLog, &p).unwrap();
    assert!(out.is_unsupported());
}

#[test]
fn integrity_certificate_through_audit_report_is_unsupported() {
    // Rule A1: the certificate needs a VerifyReport that audit_report is not given.
    let p = params(AuditReportKind::IntegrityCertificate);
    let out = audit_report(&[], &books(), &AuditReportKind::IntegrityCertificate, &p).unwrap();
    assert!(out.is_unsupported());
}

#[test]
fn empty_log_gives_an_empty_report_with_zero_total() {
    // derived: no records, so no rows, no notes and a count of zero events; totals has one cell
    // per column.
    let r = run(&[], AuditReportKind::EditLog);
    assert_eq!(r.rows.len(), 0);
    assert_eq!(r.notes.len(), 0);
    assert_eq!(r.totals[0], Cell::Text("0 events".to_owned()));
    assert_eq!(r.totals.len(), r.columns.len());
}

// ---------------------------------------------------------------------------------------------
// Integrity certificate
// ---------------------------------------------------------------------------------------------

fn value_of(r: &Report, label: &str) -> String {
    let row = r.rows.iter().find(|row| row.label == label).unwrap();
    text(&row.cells[1])
}

#[test]
fn certificate_of_a_sound_chain_says_pass_with_head_and_checkpoints() {
    // derived: the hex of 32 bytes of 0xab is "ab" repeated 32 times; of 0x01, "01" repeated.
    let verify = VerifyReport {
        events_checked: 3,
        head: Some((EventSeq(3), [0xab; 32])),
        problems: vec![],
        checkpoints: vec![Checkpoint {
            day: "2026-04-10".to_owned(),
            seq: EventSeq(2),
            hash: [0x01; 32],
        }],
    };
    let r = integrity_certificate(&verify).unwrap();
    assert_eq!(value_of(&r, "Result"), "PASS");
    assert_eq!(value_of(&r, "Events checked"), "3");
    assert_eq!(
        value_of(&r, "Head"),
        format!("event 3 hash {}", "ab".repeat(32))
    );
    assert_eq!(value_of(&r, "First broken event"), "none");
    assert_eq!(value_of(&r, "Checkpoints"), "1");
    assert_eq!(
        value_of(&r, "Checkpoint 2026-04-10"),
        format!("event 2 hash {}", "01".repeat(32))
    );
    assert_eq!(
        r.rows.iter().filter(|row| row.label == "Problem").count(),
        0
    );
    assert_eq!(
        r.params.kind,
        ReportKind::Audit(AuditReportKind::IntegrityCertificate)
    );
}

#[test]
fn certificate_of_a_broken_chain_says_fail_and_names_the_first_broken_event() {
    // derived: the problems are anchored at event 5 (hash) and event 3 (gap 3 to 4); the first
    // broken event is the lowest anchor, 3. Both problems are listed.
    let verify = VerifyReport {
        events_checked: 4,
        head: Some((EventSeq(6), [0x02; 32])),
        problems: vec![
            ChainProblem::HashMismatch { seq: EventSeq(5) },
            ChainProblem::Gap {
                missing_from: EventSeq(3),
                missing_to: EventSeq(4),
            },
        ],
        checkpoints: vec![],
    };
    let r = integrity_certificate(&verify).unwrap();
    assert_eq!(value_of(&r, "Result"), "FAIL");
    assert_eq!(value_of(&r, "First broken event"), "3");
    let problems: Vec<String> = r
        .rows
        .iter()
        .filter(|row| row.label == "Problem")
        .map(|row| text(&row.cells[1]))
        .collect();
    assert_eq!(problems.len(), 2);
    assert!(problems[0].contains("event 5"));
    assert!(problems[1].contains("3 to 4"));
}

#[test]
fn certificate_of_an_empty_log_passes_with_no_head() {
    // derived: an empty log has nothing to break; there is no head.
    let verify = VerifyReport {
        events_checked: 0,
        head: None,
        problems: vec![],
        checkpoints: vec![],
    };
    let r = integrity_certificate(&verify).unwrap();
    assert_eq!(value_of(&r, "Result"), "PASS");
    assert_eq!(value_of(&r, "Head"), "none (the log is empty)");
}
