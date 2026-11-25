//! The GST returns screen drawn to the mock (`design/mock-v2/index.html`, `SCREENS.gst` at lines
//! 1665-1724): the page head, the tabs with their keys, the key bars of each tab, the GSTR-3B table
//! and the 2B and IMS tab before anything is imported.
//!
//! Only the look follows the mock. Every figure comes from the engine (rule C4), and a mock row
//! the engine has no figure for is left out and listed in `design/PARITY-GAPS.md`, never drawn
//! with a made-up number.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::{Gstr3bTaxAmounts, Gstr3bWorking};
use ledgerly_app::screens::gst::look::{self, Line3b};
use ledgerly_app::screens::gst::{Bucket, ExceptionItem, Feed, GstScreen, GstTab};
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{Gstin, Paise, Period, StateCode, VoucherId, gstin_check_character};
use time::macros::date;

fn gstin() -> Gstin {
    let first14 = "27ABCDE1234F1Z";
    let check = gstin_check_character(first14).unwrap();
    Gstin::parse(&format!("{first14}{check}")).unwrap()
}

fn pairs(s: &GstScreen) -> Vec<(String, String)> {
    s.keys().into_iter().map(|k| (k.keys, k.label)).collect()
}

fn owned(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(k, l)| ((*k).to_owned(), (*l).to_owned()))
        .collect()
}

// ---- the page head ------------------------------------------------------------------------------

#[test]
fn the_tabs_read_like_the_mocks() {
    // provenance: index.html:1665: "GSTR-1 [1]", "GSTR-3B [2]", "2B and IMS [3]", each with its
    // key as a keycap. Catches "2B and IMS reconciliation (3)".
    let labels: Vec<&str> = GstTab::ALL.iter().map(|t| t.label()).collect();
    assert_eq!(labels, ["GSTR-1", "GSTR-3B", "2B and IMS"]);
    let keys: Vec<&str> = GstTab::ALL.iter().map(|t| t.key_hint()).collect();
    assert_eq!(keys, ["1", "2", "3"]);
}

#[test]
fn the_subtitle_names_the_gstin_the_state_the_period_and_the_frequency() {
    // provenance: index.html:1670: `${CO.gstin} · Maharashtra · ${per} · monthly`. The state is
    // the registration's own state (ledgerly-model `GstRegistration::state`); the return is
    // monthly because the shell opens one calendar month (`gst_return_period`). Derived: a
    // part with no source is left out, never guessed. Catches "Return period November 2026.
    // GSTIN ...".
    let g = gstin();
    let s =
        GstScreen::new("November 2026", Some(g.clone())).with_state(StateCode::new(27).unwrap());
    assert_eq!(
        s.subtitle(),
        format!("{g} \u{00B7} Maharashtra \u{00B7} November 2026 \u{00B7} monthly")
    );
    let s = GstScreen::new("November 2026", Some(g.clone()));
    assert_eq!(
        s.subtitle(),
        format!("{g} \u{00B7} November 2026 \u{00B7} monthly")
    );
    let s = GstScreen::new("November 2026", None);
    assert_eq!(s.subtitle(), "November 2026 \u{00B7} monthly");
}

// ---- keys ---------------------------------------------------------------------------------------

#[test]
fn the_return_tabs_key_bar_reads_like_the_mocks() {
    // provenance: index.html:1721: on GSTR-1 and GSTR-3B the keys are "1 2 3" Tabs, "J / K" Move,
    // Enter Open, Ctrl+E Export JSON, Esc Back. Catches "Open voucher". Spec change 2026-11-23
    // (derived, same rule as the 2B tab): the GSTR-3B tab has no rows to move through or open
    // and exports nothing, so it offers only the tabs and Esc (design/PARITY-GAPS.md).
    let mut s = GstScreen::new("November 2026", None);
    s.switch_tab(GstTab::Gstr1);
    assert_eq!(
        pairs(&s),
        owned(&[
            ("1 2 3", "Tabs"),
            ("J / K", "Move"),
            ("Enter", "Open"),
            ("Ctrl+E", "Export JSON"),
            ("Esc", "Back"),
        ])
    );
    s.switch_tab(GstTab::Gstr3b);
    assert_eq!(pairs(&s), owned(&[("1 2 3", "Tabs"), ("Esc", "Back")]));
}

#[test]
fn the_2b_tab_offers_the_ims_keys_only_when_there_are_rows_to_act_on() {
    // provenance: index.html:1721: on the 2B and IMS tab the keys are "J / K" Move, A Accept,
    // R Reject, P Pending, Y Take suggestion, X Explain, Esc Back. Y and X need the engine's
    // match and explanation (engine item E-3), so they are not offered (design/PARITY-GAPS.md).
    // Derived: with nothing imported there is no row, so A, R and P would do nothing and are not
    // offered either, as Ctrl+I is not offered in the day book; 1 2 3 still change the tab.
    let mut s = GstScreen::new("November 2026", None);
    s.switch_tab(GstTab::Reconciliation);
    assert_eq!(pairs(&s), owned(&[("1 2 3", "Tabs"), ("Esc", "Back")]));
    s.set_recon(Feed::Loaded(Vec::new()));
    assert_eq!(
        pairs(&s),
        owned(&[
            ("J / K", "Move"),
            ("A", "Accept"),
            ("R", "Reject"),
            ("P", "Pending"),
            ("Esc", "Back"),
        ])
    );
}

// ---- the GSTR-3B table --------------------------------------------------------------------------

fn tax(i: i64, c: i64, s: i64, cess: i64) -> Gstr3bTaxAmounts {
    Gstr3bTaxAmounts {
        integrated: Paise::new(i),
        central: Paise::new(c),
        state_ut: Paise::new(s),
        cess: Paise::new(cess),
    }
}

#[test]
fn the_3b_table_places_the_engines_figures_in_the_mocks_rows() {
    // provenance: index.html:1680-1684: the 3.1(a) row with its taxable value and tax heads and
    // the provenance chip "sales vouchers", then the "4 Eligible ITC" sub-heading and the indented
    // "In your books" row with the chip "purchase vouchers". Derived: the engine's credit that is
    // not claimable (`Gstr3bWorking::itc_not_claimable`) follows as its own indented row, since the
    // engine keeps it apart from 4(A)(5). The mock's IMS, deemed, 6.1 and cash rows have no engine
    // figure (engine items E-1 and E-3) and are not drawn. Cess stays as a column because the
    // engine has a figure for it. Every amount is the working's own; nothing is added.
    let w = Gstr3bWorking {
        gstin: gstin(),
        period: Period::new(date!(2026 - 11 - 01), date!(2026 - 11 - 30)).unwrap(),
        pack_version: "2026.1".to_owned(),
        outward_taxable_value: Paise::new(15_000_000),
        outward_tax: tax(900_000, 450_000, 450_000, 0),
        outward_vouchers: Vec::new(),
        itc_all_other: tax(2_952_000, 0, 0, 1_000),
        itc_vouchers: Vec::new(),
        itc_not_claimable: tax(0, 9_000, 9_000, 0),
        reverse_charge_purchases: Vec::new(),
        comparison: None,
    };
    assert_eq!(
        look::gstr3b_lines(&w),
        vec![
            Line3b::Row {
                label: "3.1(a) Outward taxable supplies",
                source: Some("sales vouchers"),
                indent: false,
                taxable: Some(Paise::new(15_000_000)),
                tax: [
                    Paise::new(900_000),
                    Paise::new(450_000),
                    Paise::new(450_000),
                    Paise::ZERO
                ],
            },
            Line3b::Sub("4 Eligible ITC"),
            Line3b::Row {
                label: "In your books",
                source: Some("purchase vouchers"),
                indent: true,
                taxable: None,
                tax: [
                    Paise::new(2_952_000),
                    Paise::ZERO,
                    Paise::ZERO,
                    Paise::new(1_000)
                ],
            },
            Line3b::Row {
                label: "Blocked or ineligible, not claimed",
                source: None,
                indent: true,
                taxable: None,
                tax: [
                    Paise::ZERO,
                    Paise::new(9_000),
                    Paise::new(9_000),
                    Paise::ZERO
                ],
            },
        ]
    );
}

#[test]
fn the_3b_table_has_the_mocks_columns_and_cess() {
    // provenance: index.html:1679: Table, Taxable, IGST, CGST, SGST. Derived: Cess is added
    // because the engine works it out (`Gstr3bTaxAmounts::cess`) and a figure is never dropped.
    assert_eq!(
        look::GSTR3B_COLUMNS,
        ["Table", "Taxable", "IGST", "CGST", "SGST", "Cess"]
    );
}

// ---- the 2B and IMS tab before anything is imported ---------------------------------------------

#[test]
fn the_empty_2b_tab_says_what_to_download_and_offers_the_import() {
    // provenance: index.html:1692 (the mock's recon-empty): the title is "No 2B file for
    // November yet", the sentence names Returns, GSTR-2B and the period, and says the file is
    // chosen with Ctrl+O and that nothing is sent anywhere. The "Drop it here" words are left
    // out because the screen takes no dropped files (design/PARITY-GAPS.md).
    let (title, sentence) = look::recon_empty("November 2026");
    assert_eq!(title, "No 2B file for November yet");
    assert!(sentence.contains("GSTR-2B"), "{sentence}");
    assert!(sentence.contains("November 2026"), "{sentence}");
    assert!(sentence.contains("Ctrl+O"), "{sentence}");
    assert!(sentence.contains("nothing is sent anywhere"), "{sentence}");
    assert!(!sentence.contains("E-3"), "{sentence}");
}

// ---- Enter opens the voucher --------------------------------------------------------------------

fn enter() -> RawInput {
    RawInput {
        events: vec![Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        ..RawInput::default()
    }
}

#[test]
fn enter_on_a_gstr1_row_opens_its_voucher() {
    // provenance: index.html:1720-1721: Enter on a GSTR-1 row opens it (`go(r.dataset.go)`), and
    // the key bar says "Enter Open". Bug found 2026-11-23: the screen recorded the voucher
    // (`take_drill_request`) but nothing took the request, so Enter did nothing in the app. The
    // screen now opens the voucher itself, as the day book does (`VoucherViewScreen`).
    let mut s = GstScreen::new("November 2026", None);
    s.switch_tab(GstTab::Gstr1);
    s.set_gstr1(Feed::Loaded(vec![ExceptionItem {
        bucket: Bucket::NeedsCorrection,
        title: "Voucher 3".to_owned(),
        detail: "no HSN code".to_owned(),
        voucher: Some(VoucherId(3)),
    }]));
    let ctx = egui::Context::default();
    let mut app = AppCtx::default();
    let mut action = ScreenAction::Stay;
    // egui may run the closure twice (input only in the first pass); keep the first action.
    let out = ctx.run_ui(enter(), |ui| {
        let a = s.ui(ui, &mut app);
        if matches!(action, ScreenAction::Stay) {
            action = a;
        }
    });
    out.drop_without_applying_deltas();
    let ScreenAction::Push(pushed) = action else {
        panic!("Enter did not open the voucher");
    };
    assert!(pushed.title().contains('3'), "{}", pushed.title());
}

// ---- a feed that is not loaded or unsupported is one card ---------------------------------------

/// Every string painted in one frame of `s`.
fn painted_text(s: &mut GstScreen) -> Vec<String> {
    let ctx = egui::Context::default();
    let mut app = AppCtx::default();
    let out = ctx.run_ui(RawInput::default(), |ui| {
        let _action = s.ui(ui, &mut app);
    });
    let mut texts = Vec::new();
    for clipped in &out.shapes {
        if let egui::epaint::Shape::Text(t) = &clipped.shape {
            texts.push(t.galley.text().to_owned());
        }
    }
    out.drop_without_applying_deltas();
    texts
}

#[test]
fn a_gstr1_feed_not_loaded_is_one_card_with_a_heading_and_the_message() {
    // provenance: design/mock-v2/index.html `gstNotReady` (lines 2414-2423): one card with the
    // heading "GSTR-1, <period>" and one message, no tiles, table head or side card.
    let mut s = GstScreen::new("November 2026", None);
    s.switch_tab(GstTab::Gstr1);
    let texts = painted_text(&mut s);
    assert!(
        texts.iter().any(|t| t == "GSTR-1, November 2026"),
        "{texts:?}"
    );
    assert!(
        texts.iter().any(|t| t == "Not loaded for this period yet."),
        "{texts:?}"
    );
    assert!(
        !texts.iter().any(|t| t == "Voucher"),
        "no table head: {texts:?}"
    );
}

#[test]
fn a_gstr3b_feed_not_loaded_is_one_card_with_a_heading_and_the_message() {
    // provenance: the same mock function, "GSTR-3B working, <period>".
    let mut s = GstScreen::new("November 2026", None);
    s.switch_tab(GstTab::Gstr3b);
    let texts = painted_text(&mut s);
    assert!(
        texts.iter().any(|t| t == "GSTR-3B working, November 2026"),
        "{texts:?}"
    );
    assert!(
        !texts.iter().any(|t| t == "Read this before filing"),
        "no side card: {texts:?}"
    );
}
