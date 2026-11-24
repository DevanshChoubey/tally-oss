//! s9 GST returns: the GSTR-1 tiles count sections, not vouchers.
//!
//! Provenance: derived from the design mock (`design/mock-v2/index.html`, `SCREENS.gst`, the
//! `cnt` function: a tile counts the sections whose status is its name). With the mock's six
//! sections (4A Ready, 5 Not relevant, 7 Ready, 9B Not relevant, 12 Needs correction, 13 Ready)
//! the tiles read Ready 3, Not relevant 2, Mismatch 0, Needs correction 1.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_api::{Bucket as ApiBucket, Gstr1Section};
use ledgerly_app::screens::gst::{Bucket, GstScreen, GstTab};
use ledgerly_core::Paise;

fn section(code: &'static str, status: ApiBucket) -> Gstr1Section {
    Gstr1Section {
        code,
        name: "x",
        records: 1,
        taxable: Some(Paise::ZERO),
        tax: None,
        status,
        note: None,
    }
}

#[test]
fn the_gstr1_tiles_count_sections_by_status() {
    let mut s = GstScreen::new("November 2026", None);
    assert_eq!(s.section_count(Bucket::Ready), None, "nothing loaded yet");
    s.set_sections(vec![
        section("4A", ApiBucket::Ready),
        section("5", ApiBucket::NotRelevant),
        section("7", ApiBucket::Ready),
        section("9B", ApiBucket::NotRelevant),
        section("12", ApiBucket::NeedsCorrection),
        section("13", ApiBucket::Ready),
    ]);
    assert_eq!(s.section_count(Bucket::Ready), Some(3));
    assert_eq!(s.section_count(Bucket::NotRelevant), Some(2));
    assert_eq!(s.section_count(Bucket::Mismatch), Some(0));
    assert_eq!(s.section_count(Bucket::NeedsCorrection), Some(1));
    assert_eq!(s.tab(), GstTab::Reconciliation);
}

// ---- section selection and the side card --------------------------------------------------------

use ledgerly_app::screens::gst::GstCmd;
use ledgerly_app::{AppCtx, Screen, ScreenAction};

fn with_note(code: &'static str, status: ApiBucket, note: Option<&str>) -> Gstr1Section {
    Gstr1Section {
        name: "HSN summary",
        note: note.map(str::to_owned),
        ..section(code, status)
    }
}

fn selectable() -> GstScreen {
    let mut s = GstScreen::new("November 2026", None);
    s.switch_tab(GstTab::Gstr1);
    s.set_sections(vec![
        section("4A", ApiBucket::Ready),
        with_note(
            "12",
            ApiBucket::NeedsCorrection,
            Some("item `Steel pickaxe` has no HSN or SAC code; Table 12 needs one"),
        ),
        section("13", ApiBucket::Ready),
    ]);
    s
}

fn code(s: &GstScreen) -> &'static str {
    s.selected_section().unwrap().code
}

#[test]
fn j_and_k_move_the_selected_section_and_stop_at_the_ends() {
    // provenance: derived from the mock (index.html:1720-1724): on the GSTR-1 tab J / K move the
    // `sel` row of `#g1`, which starts on the first section (`i===0`), and `listKeys` stops at
    // the first and last rows.
    let mut s = selectable();
    assert_eq!(code(&s), "4A");
    s.apply(GstCmd::Prev);
    assert_eq!(code(&s), "4A");
    s.apply(GstCmd::Next);
    assert_eq!(code(&s), "12");
    s.apply(GstCmd::Next);
    s.apply(GstCmd::Next);
    assert_eq!(code(&s), "13");
    s.apply(GstCmd::Prev);
    assert_eq!(code(&s), "12");
}

#[test]
fn the_side_card_names_what_is_wrong_with_the_selected_section() {
    // provenance: index.html:1688-1691: with the HSN row selected the side card reads "HSN summary
    // needs correction" and carries the "Fix the item master" button; any other row shows "What
    // happens next". The reason is the engine's sentence.
    let mut s = selectable();
    assert_eq!(s.side_card_title(), "What happens next");
    assert!(!s.side_card_offers_the_item_master());
    s.apply(GstCmd::Next);
    assert_eq!(s.side_card_title(), "HSN summary needs correction");
    assert!(s.side_card_offers_the_item_master());
    assert!(s.side_card_reason().unwrap().contains("Steel pickaxe"));
}

#[test]
fn enter_on_the_correction_section_opens_the_item_master() {
    // provenance: index.html:1690: the card's button is "Fix the item master" with the Enter
    // keycap and goes to Masters, items (`hashFor('masters','items')`).
    let mut s = selectable();
    let ctx = egui::Context::default();
    let mut app = AppCtx::default();
    let mut frame = |s: &mut GstScreen, key: egui::Key| {
        let input = egui::RawInput {
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..egui::RawInput::default()
        };
        let mut action = ScreenAction::Stay;
        let out = ctx.run_ui(input, |ui| {
            let a = s.ui(ui, &mut app);
            if matches!(action, ScreenAction::Stay) {
                action = a;
            }
        });
        out.drop_without_applying_deltas();
        action
    };
    // Enter on 4A has nothing to fix.
    let first = frame(&mut s, egui::Key::Enter);
    assert!(matches!(first, ScreenAction::Stay));
    frame(&mut s, egui::Key::J);
    assert_eq!(code(&s), "12");
    match frame(&mut s, egui::Key::Enter) {
        ScreenAction::Push(screen) => assert_eq!(screen.title(), "Masters"),
        ScreenAction::Stay | ScreenAction::Replace(_) | ScreenAction::Pop => {
            panic!("Enter on Table 12 did not open the item master")
        }
    }
}

// ---- Alt+F2 picks the return period -------------------------------------------------------------

fn november() -> ledgerly_core::Period {
    use time::macros::date;
    ledgerly_core::Period::new(date!(2026 - 11 - 01), date!(2026 - 11 - 30)).unwrap()
}

fn year() -> ledgerly_core::Period {
    use time::macros::date;
    ledgerly_core::Period::new(date!(2026 - 04 - 01), date!(2027 - 03 - 31)).unwrap()
}

fn gstin() -> ledgerly_core::Gstin {
    let first14 = "27ABCDE1234F1Z";
    let check = ledgerly_core::gstin_check_character(first14).unwrap();
    ledgerly_core::Gstin::parse(&format!("{first14}{check}")).unwrap()
}

#[test]
fn alt_f2_opens_a_month_picker_and_choosing_a_month_reloads_the_screen() {
    // provenance: index.html:1670 and 1719: the head carries a button "November 2026 [Alt+F2]"
    // (`data-cmd="period"`, "Alt+F2 picks the return period"). Derived: a return is one calendar
    // month of the financial year (decision 7), so the picker lists the year's twelve months,
    // April 2026 to March 2027 for a year starting 1 April 2026, with the month on show chosen.
    let mut s = GstScreen::new("November 2026", Some(gstin())).with_year(year());
    s.switch_tab(GstTab::Gstr1);
    s.request_load(gstin(), november());
    s.set_sections(vec![section("4A", ApiBucket::Ready)]);
    assert!(!s.period_picker_open());
    s.apply(GstCmd::PickPeriod);
    assert!(s.period_picker_open());
    let months = s.picker_labels();
    assert_eq!(months.len(), 12);
    assert_eq!(months.first().map(String::as_str), Some("April 2026"));
    assert_eq!(months.last().map(String::as_str), Some("March 2027"));
    assert_eq!(s.picker_selected_label().as_deref(), Some("November 2026"));
    // K goes back one month, Enter chooses it.
    s.apply(GstCmd::Prev);
    s.apply(GstCmd::Open);
    assert!(!s.period_picker_open());
    assert_eq!(
        s.subtitle(),
        format!("{} \u{00B7} October 2026 \u{00B7} monthly", gstin())
    );
    let october = {
        use time::macros::date;
        ledgerly_core::Period::new(date!(2026 - 10 - 01), date!(2026 - 10 - 31)).unwrap()
    };
    assert_eq!(s.requested_period(), Some(october));
    assert!(s.is_loading(), "the screen reloads for the chosen month");
    assert_eq!(
        s.section_count(Bucket::Ready),
        None,
        "November's sections are not shown under October"
    );
}

#[test]
fn escape_closes_the_period_picker_before_it_leaves_the_screen() {
    // provenance: derived: Esc closes the topmost thing, as the palette does; the screen is left
    // only when nothing is open.
    let mut s = GstScreen::new("November 2026", Some(gstin())).with_year(year());
    s.apply(GstCmd::PickPeriod);
    assert!(matches!(s.apply(GstCmd::Close), ScreenAction::Stay));
    assert!(!s.period_picker_open());
    assert!(matches!(s.apply(GstCmd::Close), ScreenAction::Pop));
}

#[test]
fn a_screen_with_no_financial_year_says_why_it_cannot_pick_a_period() {
    // provenance: derived: with no year to list months from, the picker would have to guess;
    // it says so instead.
    let mut s = GstScreen::new("November 2026", None);
    s.apply(GstCmd::PickPeriod);
    assert!(!s.period_picker_open());
    assert!(s.notice().is_some());
}

// ---- the line under the section table -----------------------------------------------------------

#[test]
fn the_line_under_the_table_says_whether_gstr1_agrees_with_gstr3b() {
    // provenance: index.html:1683: "Taxable value agrees with GSTR-3B table 3.1(a): " and the
    // figure. Derived for the other case: when the engine finds two figures it says both, and
    // never says "agrees". 35,322,500 paise is 3,53,225.00 in the Indian grouping.
    use ledgerly_api::TaxableAgreement;
    use ledgerly_app::screens::gst::look::agreement_text;
    let same = TaxableAgreement {
        gstr1: Paise::new(35_322_500),
        gstr3b: Paise::new(35_322_500),
    };
    assert_eq!(
        agreement_text(&same),
        "Taxable value agrees with GSTR-3B table 3.1(a): 3,53,225.00."
    );
    let off = TaxableAgreement {
        gstr1: Paise::new(35_322_500),
        gstr3b: Paise::new(35_322_400),
    };
    let text = agreement_text(&off);
    assert!(text.contains("differs"), "{text}");
    assert!(text.contains("3,53,225.00") && text.contains("3,53,224.00"), "{text}");
}
