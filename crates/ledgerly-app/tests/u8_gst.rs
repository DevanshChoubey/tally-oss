//! Card U8: GST workspace. Spec 6.3 (exception buckets; 2B and IMS: suggest Accept/Reject/Pending,
//! "user confirms every action") and the mock's keys (J/K, A/R/P, 1/2/3, Enter, Esc).
//!
//! Provenance: the key bindings and the rule that only the user confirms an action are `derived`
//! from the spec and the mock (`design/mock/index.html`, "GST reconciliation" key help). Counts
//! are derived by counting the fixture rows written below. Amount text is derived by Indian digit
//! grouping (last three digits, then pairs). Nothing here is copied from the screen's output.
//! Supplier names, GSTINs and amounts are fictional; the GSTINs get their check character from
//! `ledgerly_core::gstin_check_character`, which only builds valid test input.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_app::screens::gst::{
    ActionError, Bucket, ConfirmedAction, ExceptionItem, Feed, GstCmd, GstScreen, GstTab,
    ImsAction, MatchState, ReconRow, amount_cell, date_cell, difference_cell,
};
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{Gstin, Paise, Unsupported, VoucherId, gstin_check_character};
use time::macros::date;

// ---- fixture ---------------------------------------------------------------------------------

fn gstin(first14: &str) -> Gstin {
    let check = gstin_check_character(first14).unwrap();
    Gstin::parse(&format!("{first14}{check}")).unwrap()
}

fn row(
    supplier: &str,
    first14: &str,
    invoice: &str,
    state: MatchState,
    bucket: Bucket,
    suggested: ImsAction,
    voucher: Option<u64>,
) -> ReconRow {
    ReconRow {
        supplier: supplier.to_owned(),
        gstin: gstin(first14),
        invoice_no: invoice.to_owned(),
        books_date: Some(date!(2026 - 11 - 03)),
        portal_date: Some(date!(2026 - 11 - 03)),
        taxable_books: Some(Paise::new(124_000_000)),
        taxable_2b: Some(Paise::new(124_000_000)),
        tax_books: Some(Paise::new(22_320_000)),
        tax_2b: Some(Paise::new(22_320_000)),
        difference: Some(Paise::ZERO),
        state,
        bucket,
        suggested,
        voucher: voucher.map(VoucherId),
        total_books: Some(Paise::new(146_320_000)),
        total_2b: Some(Paise::new(146_320_000)),
        stored_action: None,
        level: None,
    }
}

/// Six rows. Positions: 0 Matched/Ready, 1 ValueDiffers/Mismatch, 2 DateDiffers/Mismatch,
/// 3 NotInBooks/NeedsCorrection (no voucher), 4 NotIn2b/NotRelevant, 5 Matched/Ready.
fn rows() -> Vec<ReconRow> {
    let mut date_row = row(
        "Omkar Printers",
        "27ZZZZZ0117Z1Z",
        "OP-0912",
        MatchState::DateDiffers,
        Bucket::Mismatch,
        ImsAction::Pending,
        Some(103),
    );
    date_row.books_date = Some(date!(2026 - 11 - 12));
    date_row.portal_date = Some(date!(2026 - 11 - 21));
    let mut no_books = row(
        "Sahyadri Electricals",
        "27ZZZZZ0118Z1Z",
        "SE/774",
        MatchState::NotInBooks,
        Bucket::NeedsCorrection,
        ImsAction::Pending,
        None,
    );
    no_books.books_date = None;
    no_books.taxable_books = None;
    no_books.tax_books = None;
    let mut not_in_2b = row(
        "Shivneri Couriers",
        "27ZZZZZ0119Z1Z",
        "SC/11/448",
        MatchState::NotIn2b,
        Bucket::NotRelevant,
        ImsAction::Pending,
        Some(105),
    );
    not_in_2b.portal_date = None;
    not_in_2b.taxable_2b = None;
    not_in_2b.tax_2b = None;
    vec![
        row(
            "Kaveri Packaging",
            "27ZZZZZ0113Z1Z",
            "KP/2611/0087",
            MatchState::Matched,
            Bucket::Ready,
            ImsAction::Accept,
            Some(101),
        ),
        row(
            "Vardhaman Logistics",
            "27ZZZZZ0116Z1Z",
            "VL-3381",
            MatchState::ValueDiffers,
            Bucket::Mismatch,
            ImsAction::Pending,
            Some(102),
        ),
        date_row,
        no_books,
        not_in_2b,
        row(
            "Annapurna Stationers",
            "27ZZZZZ0120Z1Z",
            "AS-2207",
            MatchState::Matched,
            Bucket::Ready,
            ImsAction::Accept,
            Some(106),
        ),
    ]
}

fn screen() -> GstScreen {
    let mut s = GstScreen::new("November 2026", Some(gstin("27ZZZZZ0113Z1Z")));
    s.set_recon(Feed::Loaded(rows()));
    s
}

fn exceptions() -> Vec<ExceptionItem> {
    let item = |bucket, title: &str, voucher: Option<u64>| ExceptionItem {
        bucket,
        title: title.to_owned(),
        detail: "detail".to_owned(),
        voucher: voucher.map(VoucherId),
    };
    vec![
        item(Bucket::Ready, "INV-1", Some(1)),
        item(Bucket::NeedsCorrection, "INV-2", Some(2)),
        item(Bucket::NeedsCorrection, "INV-3", None),
        item(Bucket::Mismatch, "INV-4", Some(4)),
    ]
}

fn unsupported() -> Feed<Vec<ReconRow>> {
    Feed::Unsupported(Unsupported {
        what: "GSTR-2B import".to_owned(),
        reason: "the 2B reader is not built yet".to_owned(),
    })
}

fn is_stay(a: &ScreenAction) -> bool {
    matches!(a, ScreenAction::Stay)
}

fn is_pop(a: &ScreenAction) -> bool {
    matches!(a, ScreenAction::Pop)
}

// ---- opening state ---------------------------------------------------------------------------

#[test]
fn opens_on_the_reconciliation_tab_as_the_mock_does() {
    // provenance: derived - mock `let gTab='g2'` (the 2B and IMS tab is the default).
    let s = screen();
    assert_eq!(s.tab(), GstTab::Reconciliation);
    assert_eq!(s.title(&ledgerly_app::i18n::Tr::default()), "GST returns");
    // Spec change 2026-11-23: the subtitle follows the mock-v2 head (index.html:1670); it was
    // "Return period November 2026. GSTIN ...".
    assert_eq!(
        s.subtitle(&ledgerly_app::i18n::Tr::default()),
        format!(
            "{} \u{00B7} November 2026 \u{00B7} monthly",
            gstin("27ZZZZZ0113Z1Z")
        )
    );
}

#[test]
fn a_suggestion_alone_is_never_a_confirmed_action() {
    // provenance: derived - spec 6.3 "suggest Accept/Reject/Pending, user confirms every action".
    // Rows 0 and 5 carry the suggestion Accept; nothing has been pressed.
    let s = screen();
    for i in 0..6 {
        assert_eq!(s.decision(i), None, "row {i}");
    }
    assert_eq!(s.confirmed_actions(), Vec::<ConfirmedAction>::new());
    assert_eq!(s.revision(), 0);
}

// ---- buckets ---------------------------------------------------------------------------------

#[test]
fn bucket_counts_are_counts_of_the_buckets_the_engine_gave() {
    // provenance: derived - counting the fixture: Ready rows 0 and 5; NotRelevant row 4;
    // Mismatch rows 1 and 2; NeedsCorrection row 3; six rows in all.
    let s = screen();
    let t = GstTab::Reconciliation;
    assert_eq!(s.item_count(t), Some(6));
    assert_eq!(s.bucket_count(t, Bucket::Ready), Some(2));
    assert_eq!(s.bucket_count(t, Bucket::NotRelevant), Some(1));
    assert_eq!(s.bucket_count(t, Bucket::Mismatch), Some(2));
    assert_eq!(s.bucket_count(t, Bucket::NeedsCorrection), Some(1));
}

#[test]
fn exception_tabs_count_their_own_items() {
    // provenance: derived - counting `exceptions()`: Ready 1, NeedsCorrection 2, Mismatch 1.
    let mut s = screen();
    s.set_gstr1(Feed::Loaded(exceptions()));
    let t = GstTab::Gstr1;
    assert_eq!(s.item_count(t), Some(4));
    assert_eq!(s.bucket_count(t, Bucket::Ready), Some(1));
    assert_eq!(s.bucket_count(t, Bucket::NeedsCorrection), Some(2));
    assert_eq!(s.bucket_count(t, Bucket::Mismatch), Some(1));
    assert_eq!(s.bucket_count(t, Bucket::NotRelevant), Some(0));
    // GSTR-3B was never supplied: no count, not zero.
    assert_eq!(s.item_count(GstTab::Gstr3b), None);
    assert_eq!(s.bucket_count(GstTab::Gstr3b, Bucket::Ready), None);
}

#[test]
fn a_bucket_filter_shows_only_that_bucket_and_j_k_stay_inside_it() {
    // provenance: derived - Mismatch rows are 1 and 2 (see `rows`).
    let mut s = screen();
    s.set_bucket_filter(Some(Bucket::Mismatch));
    assert_eq!(s.visible_indices(), vec![1, 2]);
    assert_eq!(s.focused_index(), Some(1));
    s.apply(GstCmd::Next);
    assert_eq!(s.focused_index(), Some(2));
    s.apply(GstCmd::Next);
    assert_eq!(s.focused_index(), Some(2), "stops at the last row");
    s.set_bucket_filter(None);
    assert_eq!(s.visible_indices(), vec![0, 1, 2, 3, 4, 5]);
}

#[test]
fn no_rows_means_no_selection_and_the_keys_say_so() {
    // provenance: derived - with no rows there is nothing to act on or open.
    let mut s = screen();
    s.set_recon(Feed::Loaded(Vec::new()));
    assert_eq!(s.visible_indices(), Vec::<usize>::new());
    assert_eq!(s.focused_index(), None);
    s.apply(GstCmd::Decide(ImsAction::Accept));
    assert_eq!(s.notice(), Some("No row is selected."));
    s.apply(GstCmd::Open);
    assert_eq!(s.notice(), Some("No row is selected."));
}

// ---- J K A R P -------------------------------------------------------------------------------

#[test]
fn j_and_k_move_one_row_and_stop_at_the_ends() {
    // provenance: derived - mock key help "Next / previous row: J / K".
    let mut s = screen();
    assert_eq!(s.focused_index(), Some(0));
    s.apply(GstCmd::Prev);
    assert_eq!(s.focused_index(), Some(0), "stops at the first row");
    for expected in [1, 2, 3, 4, 5, 5] {
        s.apply(GstCmd::Next);
        assert_eq!(s.focused_index(), Some(expected));
    }
    s.apply(GstCmd::Prev);
    assert_eq!(s.focused_index(), Some(4));
}

#[test]
fn a_r_p_set_the_action_on_the_selected_row_only() {
    // provenance: derived - mock key help "Accept A, Reject R, Pending P".
    let mut s = screen();
    s.apply(GstCmd::Decide(ImsAction::Accept));
    assert_eq!(s.decision(0), Some(ImsAction::Accept));
    s.apply(GstCmd::Next);
    s.apply(GstCmd::Decide(ImsAction::Reject));
    assert_eq!(s.decision(1), Some(ImsAction::Reject));
    s.apply(GstCmd::Next);
    s.apply(GstCmd::Decide(ImsAction::Pending));
    assert_eq!(s.decision(2), Some(ImsAction::Pending));
    for i in 3..6 {
        assert_eq!(s.decision(i), None, "row {i} untouched");
    }
    // The user may change their mind.
    s.apply(GstCmd::Prev);
    s.apply(GstCmd::Decide(ImsAction::Accept));
    assert_eq!(s.decision(1), Some(ImsAction::Accept));
}

#[test]
fn an_action_on_a_document_not_in_2b_is_refused_and_recorded_nowhere() {
    // provenance: derived - mock `setAct`: "Not in 2B yet, so there is nothing to act on in IMS".
    // Row 4 is NotIn2b.
    let mut s = screen();
    for _ in 0..4 {
        s.apply(GstCmd::Next);
    }
    assert_eq!(s.focused_index(), Some(4));
    s.apply(GstCmd::Decide(ImsAction::Accept));
    assert_eq!(s.decision(4), None);
    assert!(s.notice().unwrap().contains("not in 2B yet"));
    assert_eq!(s.revision(), 0);
    assert_eq!(
        s.decide(4, ImsAction::Reject),
        Err(ActionError::NotIn2b {
            supplier: "Shivneri Couriers".to_owned(),
            invoice_no: "SC/11/448".to_owned(),
        })
    );
    assert_eq!(
        s.decide(99, ImsAction::Reject),
        Err(ActionError::NoSuchRow { row: 99 })
    );
}

#[test]
fn deciding_without_a_reconciliation_is_an_error() {
    // provenance: derived - rule A1: an unsupported or missing feed gives an error, not a no-op.
    let mut s = GstScreen::new("November 2026", None);
    assert_eq!(
        s.decide(0, ImsAction::Accept),
        Err(ActionError::NoReconciliation)
    );
    s.set_recon(unsupported());
    assert_eq!(
        s.decide(0, ImsAction::Accept),
        Err(ActionError::NoReconciliation)
    );
    assert_eq!(s.item_count(GstTab::Reconciliation), None);
    assert_eq!(s.open_count(), 0);
    // Spec change 2026-11-23: mock-v2 head subtitle (index.html:1670).
    assert_eq!(
        s.subtitle(&ledgerly_app::i18n::Tr::default()),
        "November 2026 \u{00B7} monthly"
    );
}

#[test]
fn action_keys_do_nothing_on_the_return_tabs() {
    // provenance: derived - IMS actions belong to the reconciliation tab (mock: only g2 handles
    // a/r/p).
    let mut s = screen();
    s.set_gstr1(Feed::Loaded(exceptions()));
    s.apply(GstCmd::SwitchTab(GstTab::Gstr1));
    s.apply(GstCmd::Decide(ImsAction::Accept));
    assert_eq!(s.decision(0), None);
    assert_eq!(s.revision(), 0);
    assert!(s.notice().unwrap().contains("reconciliation tab"));
}

// ---- open actions, filter, revision ----------------------------------------------------------

#[test]
fn open_count_is_in_2b_rows_with_no_action_or_pending() {
    // provenance: derived - rows in 2B are 0,1,2,3,5 (row 4 is NotIn2b): 5 open at the start.
    // Accept row 0 -> 4; Reject row 1 -> 3; Pending row 2 stays open -> 3; Accept row 3 -> 2.
    let mut s = screen();
    assert_eq!(s.open_count(), 5);
    s.decide(0, ImsAction::Accept).unwrap();
    assert_eq!(s.open_count(), 4);
    s.decide(1, ImsAction::Reject).unwrap();
    assert_eq!(s.open_count(), 3);
    s.decide(2, ImsAction::Pending).unwrap();
    assert_eq!(s.open_count(), 3);
    s.decide(3, ImsAction::Accept).unwrap();
    assert_eq!(s.open_count(), 2);
}

#[test]
fn the_open_filter_drops_a_row_once_it_is_decided_and_the_next_row_takes_the_selection() {
    // provenance: derived - open rows are 0,1,2,3,5; accepting row 0 leaves 1,2,3,5.
    let mut s = screen();
    s.set_open_only(true);
    assert_eq!(s.visible_indices(), vec![0, 1, 2, 3, 5]);
    s.apply(GstCmd::Decide(ImsAction::Accept));
    assert_eq!(s.visible_indices(), vec![1, 2, 3, 5]);
    assert_eq!(s.focused_index(), Some(1));
    // Pending keeps the row in the open list.
    s.apply(GstCmd::Decide(ImsAction::Pending));
    assert_eq!(s.visible_indices(), vec![1, 2, 3, 5]);
    // Accept the last row: the selection moves up to the new last row.
    for _ in 0..3 {
        s.apply(GstCmd::Next);
    }
    assert_eq!(s.focused_index(), Some(5));
    s.apply(GstCmd::Decide(ImsAction::Reject));
    assert_eq!(s.visible_indices(), vec![1, 2, 3]);
    assert_eq!(s.focused_index(), Some(3));
}

#[test]
fn the_revision_changes_only_when_a_confirmed_action_changes() {
    // provenance: derived - the host recomputes GSTR-3B from the revision (mock: "Change an IMS
    // action and this page recomputes"), so an unchanged action must not trigger it.
    let mut s = screen();
    s.decide(0, ImsAction::Accept).unwrap();
    assert_eq!(s.revision(), 1);
    s.decide(0, ImsAction::Accept).unwrap();
    assert_eq!(s.revision(), 1);
    s.decide(0, ImsAction::Reject).unwrap();
    assert_eq!(s.revision(), 2);
}

#[test]
fn confirmed_actions_list_only_confirmed_rows_in_row_order() {
    // provenance: derived - rows 5, 1 and 3 are decided in that order; the list follows rows.
    let mut s = screen();
    s.decide(5, ImsAction::Accept).unwrap();
    s.decide(1, ImsAction::Reject).unwrap();
    s.decide(3, ImsAction::Pending).unwrap();
    let all = rows();
    let expected = vec![
        ConfirmedAction {
            supplier_gstin: all[1].gstin.clone(),
            invoice_no: "VL-3381".to_owned(),
            portal_date: Some(date!(2026 - 11 - 03)),
            action: ImsAction::Reject,
        },
        ConfirmedAction {
            supplier_gstin: all[3].gstin.clone(),
            invoice_no: "SE/774".to_owned(),
            portal_date: Some(date!(2026 - 11 - 03)),
            action: ImsAction::Pending,
        },
        ConfirmedAction {
            supplier_gstin: all[5].gstin.clone(),
            invoice_no: "AS-2207".to_owned(),
            portal_date: Some(date!(2026 - 11 - 03)),
            action: ImsAction::Accept,
        },
    ];
    assert_eq!(s.confirmed_actions(), expected);
}

// ---- refresh ---------------------------------------------------------------------------------

#[test]
fn a_refresh_keeps_actions_by_supplier_gstin_and_invoice_number() {
    // provenance: derived - reversing the rows moves them to new positions; the action follows the
    // (GSTIN, invoice number) key, so row 0 (Kaveri) is at position 5 afterwards.
    let mut s = screen();
    s.decide(0, ImsAction::Accept).unwrap();
    s.decide(1, ImsAction::Reject).unwrap();
    let before = s.revision();
    let mut reversed = rows();
    reversed.reverse();
    s.set_recon(Feed::Loaded(reversed));
    assert_eq!(s.decision(5), Some(ImsAction::Accept));
    assert_eq!(s.decision(4), Some(ImsAction::Reject));
    for i in 0..4 {
        assert_eq!(s.decision(i), None);
    }
    assert_eq!(s.revision(), before + 1, "the order of the list changed");
    assert!(s.notice().is_none());
}

#[test]
fn a_refresh_that_loses_a_row_drops_its_action_and_says_so() {
    // provenance: derived - row 0 is removed; its confirmed Accept has nowhere to live.
    let mut s = screen();
    s.decide(0, ImsAction::Accept).unwrap();
    s.decide(1, ImsAction::Reject).unwrap();
    let mut rest = rows();
    rest.remove(0);
    s.set_recon(Feed::Loaded(rest));
    assert_eq!(
        s.decision(0),
        Some(ImsAction::Reject),
        "VL-3381 is now row 0"
    );
    assert_eq!(s.confirmed_actions().len(), 1);
    assert!(
        s.notice()
            .unwrap()
            .starts_with("1 confirmed IMS action(s) were dropped")
    );
}

#[test]
fn a_row_that_is_no_longer_in_2b_loses_its_action() {
    // provenance: derived - there is nothing to act on in IMS for a document not in 2B.
    let mut s = screen();
    s.decide(0, ImsAction::Accept).unwrap();
    let mut changed = rows();
    changed[0].state = MatchState::NotIn2b;
    s.set_recon(Feed::Loaded(changed));
    assert_eq!(s.decision(0), None);
    assert_eq!(s.confirmed_actions(), Vec::<ConfirmedAction>::new());
}

#[test]
fn a_failed_refresh_keeps_the_confirmed_actions_for_the_next_loaded_feed() {
    // provenance: derived - a feed that is Unsupported drops no row, so the two actions the user
    // confirmed survive it and come back when the same rows are loaded again. The notice must not
    // blame 2B, because no row left 2B. The old code cleared the decisions on any non-loaded feed.
    let mut s = screen();
    s.decide(0, ImsAction::Accept).unwrap();
    s.decide(1, ImsAction::Reject).unwrap();
    let revision = s.revision();
    s.set_recon(unsupported());
    assert_eq!(
        s.revision(),
        revision,
        "no confirmed action changed when the feed was not loaded"
    );
    assert!(s.notice().is_none(), "no row left 2B, so no notice");
    s.set_recon(Feed::Loaded(rows()));
    assert_eq!(s.decision(0), Some(ImsAction::Accept));
    assert_eq!(s.decision(1), Some(ImsAction::Reject));
    assert_eq!(s.confirmed_actions().len(), 2);
    assert!(
        s.notice().is_none(),
        "the same rows came back, nothing dropped"
    );
}

#[test]
fn a_not_loaded_feed_then_a_loaded_feed_without_a_row_drops_only_that_row() {
    // provenance: derived - Accept on row 0 is held through a NotLoaded feed; the next loaded feed
    // lacks row 0, so the action is dropped with the notice saying a row left 2B.
    let mut s = screen();
    s.decide(0, ImsAction::Accept).unwrap();
    s.decide(1, ImsAction::Reject).unwrap();
    s.set_recon(Feed::NotLoaded);
    let mut rest = rows();
    rest.remove(0);
    s.set_recon(Feed::Loaded(rest));
    assert_eq!(
        s.decision(0),
        Some(ImsAction::Reject),
        "VL-3381 is now row 0"
    );
    assert_eq!(s.confirmed_actions().len(), 1);
    assert!(
        s.notice()
            .unwrap()
            .starts_with("1 confirmed IMS action(s) were dropped")
    );
}

// ---- drill, bulk, export ---------------------------------------------------------------------

#[test]
fn enter_opens_the_voucher_of_the_selected_row_once() {
    // provenance: derived - mock "Enter open"; row 0 books voucher 101.
    let mut s = screen();
    s.apply(GstCmd::Open);
    assert_eq!(s.take_drill_request(), Some(VoucherId(101)));
    assert_eq!(s.take_drill_request(), None);
}

#[test]
fn enter_on_a_row_with_no_voucher_says_so() {
    // provenance: derived - row 3 (Sahyadri) is not in the books: nothing to open.
    let mut s = screen();
    for _ in 0..3 {
        s.apply(GstCmd::Next);
    }
    s.apply(GstCmd::Open);
    assert_eq!(s.take_drill_request(), None);
    assert!(s.notice().unwrap().contains("no voucher"));
}

#[test]
fn enter_on_an_exception_opens_the_voucher_to_fix() {
    // provenance: derived - spec 6.3 "drill to fix"; INV-2 is the second item, voucher 2.
    let mut s = screen();
    s.set_gstr1(Feed::Loaded(exceptions()));
    s.apply(GstCmd::SwitchTab(GstTab::Gstr1));
    s.set_bucket_filter(Some(Bucket::NeedsCorrection));
    assert_eq!(s.visible_indices(), vec![1, 2]);
    s.apply(GstCmd::Open);
    assert_eq!(s.take_drill_request(), Some(VoucherId(2)));
    s.apply(GstCmd::Next);
    s.apply(GstCmd::Open);
    assert_eq!(s.take_drill_request(), None, "INV-3 has no voucher");
}

#[test]
fn re_apply_masters_is_recorded_for_the_tab_on_show_once() {
    // provenance: derived - spec 6.3 bulk "re-apply masters"; the screen never changes data itself.
    let mut s = screen();
    assert_eq!(s.take_reapply_request(), None);
    s.switch_tab(GstTab::Gstr3b);
    s.request_reapply();
    assert_eq!(s.take_reapply_request(), Some(GstTab::Gstr3b));
    assert_eq!(s.take_reapply_request(), None);
}

#[test]
fn export_needs_a_confirmed_action_and_hands_over_exactly_those() {
    // provenance: derived - "export the action file" (spec 6.3) lists what the user confirmed;
    // row 2 (Omkar, in 2B dated 21-11-2026) is the only one accepted.
    let mut s = screen();
    s.request_export();
    assert_eq!(s.take_export_request(), None);
    assert!(s.notice().unwrap().starts_with("Nothing to export"));
    s.decide(2, ImsAction::Accept).unwrap();
    s.request_export();
    let sent = s.take_export_request().unwrap();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].invoice_no, "OP-0912");
    assert_eq!(sent[0].portal_date, Some(date!(2026 - 11 - 21)));
    assert_eq!(sent[0].action, ImsAction::Accept);
    assert_eq!(s.take_export_request(), None);
}

// ---- cells -----------------------------------------------------------------------------------

#[test]
fn a_missing_amount_is_none_never_zero_and_amounts_group_the_indian_way() {
    // provenance: derived - Indian grouping: 1,24,000.00 is 12_400_000 paise (124000 rupees:
    // last three digits "000", then pairs "24", then "1"). Rule C4/A1: missing is not zero.
    assert_eq!(amount_cell(None), "none");
    assert_eq!(amount_cell(Some(Paise::ZERO)), "0.00");
    assert_eq!(amount_cell(Some(Paise::new(12_400_000))), "1,24,000.00");
    assert_eq!(amount_cell(Some(Paise::new(-1_200_000))), "-12,000.00");
    assert_eq!(amount_cell(Some(Paise::new(5))), "0.05");
}

#[test]
fn a_missing_difference_reads_none_like_the_other_amount_columns_not_n_a() {
    // provenance: derived - the module contract (gst.rs module doc, rule C4) says a missing amount
    // reads "none". The difference column must use the same convention as the three amount
    // columns beside it, so a row with no difference shows "none" there too.
    assert_eq!(difference_cell(None), amount_cell(None));
    assert_eq!(difference_cell(None), "none");
    assert_eq!(difference_cell(Some(Paise::new(-500))), "-5.00");
}

#[test]
fn dates_show_day_month_year_and_the_2b_date_only_when_it_differs() {
    // provenance: derived - mock shows `12-11-2026 in 2B: 21-11-2026` for a date mismatch.
    assert_eq!(date_cell(Some(date!(2026 - 11 - 03))), "03-11-2026");
    assert_eq!(date_cell(None), "none");
    let all = rows();
    assert_eq!(all[0].dates_text(), "03-11-2026");
    assert_eq!(all[2].dates_text(), "12-11-2026 (in 2B: 21-11-2026)");
    assert_eq!(all[3].dates_text(), "in 2B: 03-11-2026");
    assert_eq!(all[4].dates_text(), "03-11-2026");
}

// ---- egui ------------------------------------------------------------------------------------

fn frame(
    ctx: &egui::Context,
    screen: &mut GstScreen,
    app: &mut AppCtx,
    input: RawInput,
) -> ScreenAction {
    let mut action = ScreenAction::Stay;
    // egui may run the closure twice in one frame (a discarded pass, with input only in the
    // first); the shell applies each pass's action, so the first non-Stay action is the result.
    let out = ctx.run_ui(input, |ui| {
        let a = screen.ui(ui, app);
        if matches!(action, ScreenAction::Stay) {
            action = a;
        }
    });
    out.drop_without_applying_deltas();
    action
}

fn key(key: Key, modifiers: Modifiers) -> RawInput {
    RawInput {
        events: vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..RawInput::default()
    }
}

#[test]
fn every_tab_draws_in_every_feed_state() {
    // provenance: derived - rule A1: NotLoaded, Unsupported and Loaded all draw without panic.
    let ctx = egui::Context::default();
    let mut app = AppCtx::default();
    let mut s = GstScreen::new("November 2026", None);
    for tab in GstTab::ALL {
        s.switch_tab(tab);
        assert!(is_stay(&frame(&ctx, &mut s, &mut app, RawInput::default())));
    }
    s.set_recon(unsupported());
    s.set_gstr1(Feed::Unsupported(Unsupported {
        what: "GSTR-1".to_owned(),
        reason: "not built".to_owned(),
    }));
    for tab in GstTab::ALL {
        s.switch_tab(tab);
        assert!(is_stay(&frame(&ctx, &mut s, &mut app, RawInput::default())));
    }
    s.set_recon(Feed::Loaded(rows()));
    s.set_gstr1(Feed::Loaded(exceptions()));
    s.set_gstr3b(Feed::Loaded(exceptions()));
    for tab in GstTab::ALL {
        s.switch_tab(tab);
        assert!(is_stay(&frame(&ctx, &mut s, &mut app, RawInput::default())));
        s.set_bucket_filter(Some(Bucket::Mismatch));
        assert!(is_stay(&frame(&ctx, &mut s, &mut app, RawInput::default())));
        s.set_bucket_filter(None);
    }
}

#[test]
fn keys_through_egui_move_and_decide() {
    // provenance: derived - mock keys J, K, A, R, P, 1, 2, 3 and Esc.
    let ctx = egui::Context::default();
    let mut app = AppCtx::default();
    let mut s = screen();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, key(Key::J, Modifiers::NONE));
    assert_eq!(s.focused_index(), Some(1));
    frame(&ctx, &mut s, &mut app, key(Key::ArrowDown, Modifiers::NONE));
    assert_eq!(s.focused_index(), Some(2));
    frame(&ctx, &mut s, &mut app, key(Key::K, Modifiers::NONE));
    assert_eq!(s.focused_index(), Some(1));
    frame(&ctx, &mut s, &mut app, key(Key::R, Modifiers::NONE));
    assert_eq!(s.decision(1), Some(ImsAction::Reject));
    frame(&ctx, &mut s, &mut app, key(Key::A, Modifiers::NONE));
    assert_eq!(s.decision(1), Some(ImsAction::Accept));
    frame(&ctx, &mut s, &mut app, key(Key::P, Modifiers::NONE));
    assert_eq!(s.decision(1), Some(ImsAction::Pending));
    frame(&ctx, &mut s, &mut app, key(Key::Num1, Modifiers::NONE));
    assert_eq!(s.tab(), GstTab::Gstr1);
    frame(&ctx, &mut s, &mut app, key(Key::Num2, Modifiers::NONE));
    assert_eq!(s.tab(), GstTab::Gstr3b);
    frame(&ctx, &mut s, &mut app, key(Key::Num3, Modifiers::NONE));
    assert_eq!(s.tab(), GstTab::Reconciliation);
}

#[test]
fn a_command_chord_does_not_set_an_action() {
    // provenance: derived - spec 7: Ctrl+A saves; it must never be read as "Accept".
    let ctx = egui::Context::default();
    let mut app = AppCtx::default();
    let mut s = screen();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, key(Key::A, Modifiers::COMMAND));
    assert_eq!(s.decision(0), None);
    assert_eq!(s.revision(), 0);
}

#[test]
fn escape_leaves_the_screen() {
    // provenance: derived - spec 7: Esc returns up the screen stack.
    let ctx = egui::Context::default();
    let mut app = AppCtx::default();
    let mut s = screen();
    assert!(is_pop(&s.apply(GstCmd::Close)));
    assert!(is_pop(&frame(
        &ctx,
        &mut s,
        &mut app,
        key(Key::Escape, Modifiers::NONE)
    )));
    assert!(is_stay(&s.apply(GstCmd::Next)));
}

#[test]
fn enter_through_egui_opens_the_voucher() {
    // provenance: derived - mock "Enter open". Spec change 2026-11-23: the screen now opens the
    // voucher itself (it pushes the voucher view, as the day book does) instead of leaving a
    // request nothing took; see w2_gst_look.rs `enter_on_a_gstr1_row_opens_its_voucher`.
    let ctx = egui::Context::default();
    let mut app = AppCtx::default();
    let mut s = screen();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    let action = frame(&ctx, &mut s, &mut app, key(Key::Enter, Modifiers::NONE));
    let ScreenAction::Push(pushed) = action else {
        panic!("Enter did not open the voucher");
    };
    assert_eq!(
        pushed.title(&ledgerly_app::i18n::Tr::default()),
        "Voucher #101"
    );
    assert_eq!(s.take_drill_request(), None);
}
