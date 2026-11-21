//! Card S-E (GST returns): the monthly return period the screen opens with (decision 7 of the UI
//! parity report, rule B4), the keys and page head the screen shows, and the checks that stop an
//! export before the engine is asked.
//!
//! Provenance: every expected date and month below is derived by hand from the rule in
//! `shell/navigate.rs` (the module header and `gst_return_period`), not from this crate's output.
//! The company is fictional; its GSTIN is generated with the check character of the GSTIN
//! algorithm, as `c4_gst.rs` does.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_app::Screen;
use ledgerly_app::screens::gst::{Feed, GstCmd, GstScreen, GstTab};
use ledgerly_app::shell::navigate::{gst_return_period, return_label};
use ledgerly_core::{
    CompanyId, Gstin, Period, StateCode, Unsupported, VoucherId, VoucherTypeId,
    gstin_check_character,
};
use ledgerly_model::{
    Address, Books, Company, EntityKind, Features, GstRegistration, Masters, NegativeStockPolicy,
    Voucher, VoucherMode, VoucherStatus,
};
use time::macros::date;

/// A GSTIN for a fictional Maharashtra registration, with its check character.
fn gstin() -> Gstin {
    let first14 = "27ABCDE1234F1Z";
    let check = gstin_check_character(first14).unwrap();
    Gstin::parse(&format!("{first14}{check}")).unwrap()
}

/// Books with a financial year starting 1 April 2026 and the given voucher dates.
fn books_with_vouchers(dates: &[time::Date]) -> Books {
    let company = Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(27).unwrap(),
        pan: None,
        registrations: vec![GstRegistration {
            gstin: gstin(),
            state: StateCode::new(27).unwrap(),
            effective_from: date!(2026 - 04 - 01),
            composition: false,
        }],
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    };
    let mut vouchers = BTreeMap::new();
    for (n, on) in dates.iter().enumerate() {
        let id = VoucherId(u64::try_from(n).unwrap() + 1);
        vouchers.insert(id, voucher(id, *on));
    }
    Books {
        company,
        masters: Masters::default(),
        vouchers,
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

/// A voucher dated `on`. The period rule reads only its date, so the rest is empty.
fn voucher(id: VoucherId, on: time::Date) -> Voucher {
    Voucher {
        id,
        type_id: VoucherTypeId(1),
        number: None,
        series: None,
        date: on,
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::AccountingInvoice,
        party: None,
        place_of_supply: None,
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: String::new(),
        ledger_lines: Vec::new(),
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

// ---- the monthly period (B4) -----------------------------------------------------------------

#[test]
fn today_in_the_year_gives_the_calendar_month_of_today() {
    // provenance: derived - 22 Nov 2026 is in FY 2026-27 (1 Apr 2026 to 31 Mar 2027), so the
    // month is November 2026: 1 Nov to 30 Nov (November has 30 days). The voucher of 5 Oct 2026
    // is not chosen, because today is in the year.
    let books = books_with_vouchers(&[date!(2026 - 10 - 05)]);
    let period = gst_return_period(&books, date!(2026 - 11 - 22)).unwrap();
    assert_eq!(period.from(), date!(2026 - 11 - 01));
    assert_eq!(period.to(), date!(2026 - 11 - 30));
}

#[test]
fn today_before_the_year_uses_the_month_of_the_last_voucher() {
    // provenance: derived - 15 Mar 2026 is before the year starts on 1 Apr 2026, so the last
    // voucher in the year decides: 5 Oct 2026 is later than 12 Sep 2026, so October 2026.
    let books = books_with_vouchers(&[date!(2026 - 09 - 12), date!(2026 - 10 - 05)]);
    let period = gst_return_period(&books, date!(2026 - 03 - 15)).unwrap();
    assert_eq!(period.from(), date!(2026 - 10 - 01));
    assert_eq!(period.to(), date!(2026 - 10 - 31));
}

#[test]
fn today_after_the_year_with_no_voucher_uses_the_first_month_of_the_year() {
    // provenance: derived - 10 Jun 2027 is after FY 2026-27 ends, and no voucher is dated in the
    // year, so the rule falls back to the first month of the year: April 2026 (30 days).
    let books = books_with_vouchers(&[date!(2027 - 05 - 02)]);
    let period = gst_return_period(&books, date!(2027 - 06 - 10)).unwrap();
    assert_eq!(period.from(), date!(2026 - 04 - 01));
    assert_eq!(period.to(), date!(2026 - 04 - 30));
}

#[test]
fn a_voucher_outside_the_year_is_not_the_anchor() {
    // provenance: derived - 2 May 2027 is in FY 2027-28, not in FY 2026-27, so it is ignored. The
    // last voucher in the year is 19 Aug 2026: August 2026.
    let books = books_with_vouchers(&[date!(2026 - 08 - 19), date!(2027 - 05 - 02)]);
    let period = gst_return_period(&books, date!(2027 - 06 - 10)).unwrap();
    assert_eq!(period.from(), date!(2026 - 08 - 01));
    assert_eq!(period.to(), date!(2026 - 08 - 31));
}

#[test]
fn a_february_in_a_leap_year_ends_on_the_29th() {
    // provenance: derived - 2028 is a leap year, so February 2028 has 29 days. The year starts on
    // 1 Apr 2027, so 10 Feb 2028 is in it and the month is February 2028.
    let mut books = books_with_vouchers(&[]);
    books.company.fy_start = date!(2027 - 04 - 01);
    let period = gst_return_period(&books, date!(2028 - 02 - 10)).unwrap();
    assert_eq!(period.from(), date!(2028 - 02 - 01));
    assert_eq!(period.to(), date!(2028 - 02 - 29));
}

#[test]
fn the_return_label_names_the_month() {
    // provenance: derived - the label is the month name and the year of the period's first day.
    let period = Period::new(date!(2026 - 11 - 01), date!(2026 - 11 - 30)).unwrap();
    assert_eq!(return_label(period), "November 2026");
}

// ---- the screen: title, subtitle, keys -------------------------------------------------------

#[test]
fn the_page_title_and_the_subtitle_name_the_return_period() {
    // provenance: derived - the mock's page title is "GST returns" (design/mock-v2/index.html,
    // SCREENS.gst title).
    let s = GstScreen::new("November 2026", None);
    assert_eq!(s.page_title(), "GST returns");
    assert_eq!(
        Screen::subtitle(&s),
        Some("Return period November 2026.".to_owned())
    );
}

#[test]
fn the_keys_show_export_json_and_the_ims_keys_only_on_the_2b_tab() {
    // provenance: derived - the mock's key list for the GST screen (index.html, SCREENS.gst keys)
    // has Ctrl+E Export JSON and, on the 2B tab, A R P for the IMS actions.
    let mut s = GstScreen::new("November 2026", None);
    s.switch_tab(GstTab::Gstr1);
    let keys: Vec<(String, String)> = s
        .keys()
        .into_iter()
        .map(|k| (k.keys, k.label))
        .collect();
    assert!(keys.contains(&("Ctrl+E".to_owned(), "Export JSON".to_owned())));
    assert!(!keys.iter().any(|(k, _)| k == "A R P"));
    s.switch_tab(GstTab::Reconciliation);
    let keys: Vec<String> = s.keys().into_iter().map(|k| k.keys).collect();
    assert!(keys.contains(&"A R P".to_owned()));
}

// ---- the export checks -----------------------------------------------------------------------

fn october() -> Period {
    Period::new(date!(2026 - 10 - 01), date!(2026 - 10 - 31)).unwrap()
}

#[test]
fn export_is_refused_off_the_gstr1_tab_and_says_which_tab_to_open() {
    // provenance: derived - the JSON is GSTR-1's; the reason names the key that opens it (1).
    let mut s = GstScreen::new("October 2026", Some(gstin()));
    s.request_load(gstin(), october());
    s.switch_tab(GstTab::Gstr3b);
    let reason = s.export_blocked_reason().unwrap();
    assert!(reason.contains("Press 1"), "{reason}");
    s.apply(GstCmd::ExportJson);
    assert_eq!(s.notice(), Some(reason.as_str()));
}

#[test]
fn export_says_so_when_no_return_is_loaded() {
    // provenance: derived - a screen that was never given a period has no JSON to build.
    let mut s = GstScreen::new("October 2026", Some(gstin()));
    s.switch_tab(GstTab::Gstr1);
    s.apply(GstCmd::ExportJson);
    assert_eq!(
        s.notice(),
        Some("No return is loaded, so there is no JSON to export.")
    );
}

#[test]
fn export_waits_while_gstr1_is_still_loading() {
    // provenance: derived - a return that is requested but whose GSTR-1 has not arrived yet.
    let mut s = GstScreen::new("October 2026", Some(gstin()));
    s.request_load(gstin(), october());
    s.switch_tab(GstTab::Gstr1);
    assert!(s.export_blocked_reason().unwrap().contains("still loading"));
}

#[test]
fn an_unsupported_gstr1_blocks_export_with_the_engine_reason() {
    // provenance: derived - the reason is the one the screen was given, copied through unchanged.
    let mut s = GstScreen::new("October 2026", Some(gstin()));
    s.request_load(gstin(), october());
    s.switch_tab(GstTab::Gstr1);
    s.set_gstr1(Feed::Unsupported(Unsupported {
        what: "GSTR-1 JSON".to_owned(),
        reason: "voucher 7: no HSN code".to_owned(),
    }));
    let reason = s.export_blocked_reason().unwrap();
    assert!(reason.contains("voucher 7: no HSN code"), "{reason}");
}

#[test]
fn a_ready_export_with_no_worker_says_no_company_is_open() {
    // provenance: derived - with the GSTR-1 loaded and the export asked for, the request reaches
    // the worker step; with no worker (no company open) the screen says so and builds nothing.
    let mut s = GstScreen::new("October 2026", Some(gstin()));
    s.request_load(gstin(), october());
    s.switch_tab(GstTab::Gstr1);
    s.set_gstr1(Feed::Loaded(Vec::new()));
    assert_eq!(s.export_blocked_reason(), None);
    s.apply(GstCmd::ExportJson);
    s.tick(None);
    assert!(
        s.notice()
            .unwrap()
            .contains("No company is open, so no JSON can be built."),
        "{:?}",
        s.notice()
    );
    assert_eq!(s.take_json_export(), None);
}
