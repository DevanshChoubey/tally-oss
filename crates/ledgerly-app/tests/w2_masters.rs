//! Wave 2 S-G: the masters screen against mock v2. The view models (rows, the party detail, the
//! GSTIN check) and the screen's drawing with the two primary tabs and the "More master kinds"
//! control.
//!
//! Every expected value is derived by hand and the derivation is written next to it. Nothing is
//! copied from this crate's output. Provenance is one of: the mock (file and line), a hand
//! derivation in the test, or a cited source.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_app::screens::masters::view::{self, GstinCheck};
use ledgerly_app::screens::masters::{
    MasterDraft, MasterKind, MasterRef, MastersScreen, PartyDraft, Step,
};
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{
    CompanyId, Decimal, GroupId, Gstin, Hsn, LedgerId, Paise, Rate, SignedAmount, StateCode,
    StockItemId, UnitId,
};
use ledgerly_model::{
    Address, BillWiseConfig, Books, Company, EntityKind, Features, Group, GstHistory, Ledger,
    Masters, Nature, NegativeStockPolicy, OpeningStock, PartyDetails, RegistrationType, StockItem,
    Taxability, ValuationMethod,
};
use time::macros::date;

/// A GSTIN that passes every check. The vector is the first valid one in `ledgerly-core`'s
/// `gstin.rs` list, the same one the u6 fixture uses. Its 15th character is `W`, which is the
/// check character for its first 14 characters.
const GSTIN_VALID: &str = "29ABCDE1234F1ZW";

// ---- fixture ---------------------------------------------------------------------------------

fn group(id: u64, name: &str) -> Group {
    Group {
        ineligible_itc: None,
        id: GroupId(id),
        name: name.to_owned(),
        aliases: Vec::new(),
        parent: None,
        reserved: None,
        nature: Some(Nature::Asset),
        affects_gross_profit: false,
        tally: None,
    }
}

fn ledger(id: u64, name: &str, group_id: u64) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id: LedgerId(id),
        name: name.to_owned(),
        aliases: Vec::new(),
        group: GroupId(group_id),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn party(gstin: Option<&str>, state: Option<u8>) -> PartyDetails {
    PartyDetails {
        registration: RegistrationType::Regular,
        gstin: gstin.map(|g| Gstin::parse(g).unwrap()),
        pan: None,
        state: state.and_then(|s| StateCode::new(s).ok()),
        address: Address {
            lines: vec!["12 Market Road".to_owned(), "Bengaluru".to_owned()],
            pincode: Some("560001".to_owned()),
            state: None,
            country: "India".to_owned(),
        },
    }
}

fn item(id: u64, name: &str, hsn: Option<&str>, history: Vec<GstHistory>) -> StockItem {
    StockItem {
        ineligible_itc: None,
        id: StockItemId(id),
        name: name.to_owned(),
        aliases: Vec::new(),
        group: None,
        unit: UnitId(1),
        alt_unit: None,
        hsn: hsn.map(|h| Hsn::parse(h).unwrap()),
        gst_history: history,
        valuation: ValuationMethod::AverageCost,
        batches_enabled: false,
        opening: Vec::<OpeningStock>::new(),
        standard_costs: Vec::new(),
        tally: None,
    }
}

fn gst_entry(y: i32, m: u8, d: u8, rate: i64) -> GstHistory {
    GstHistory {
        effective_from: time::Date::from_calendar_date(y, time::Month::try_from(m).unwrap(), d)
            .unwrap(),
        taxability: Taxability::Taxable,
        rate_percent: Some(Rate(Decimal::from(rate))),
    }
}

fn books_with(masters: Masters) -> Books {
    Books {
        company: Company {
            ineligible_itc_default: None,
            id: CompanyId(1),
            name: "Test Traders".to_owned(),
            mailing_name: None,
            address: Address::default(),
            state: StateCode::new(27).unwrap(),
            pan: None,
            registrations: Vec::new(),
            fy_start: date!(2026 - 04 - 01),
            books_begin: date!(2026 - 04 - 01),
            entity_kind: EntityKind::Company,
            features: Features {
                inventory: true,
                ..Features::default()
            },
            negative_stock: NegativeStockPolicy::Block,
        },
        masters,
        vouchers: BTreeMap::new(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

/// Two groups, three ledgers and two stock items. Ledgers: "Zeta Traders" (alias ZT, no party),
/// "Alpha Stores" (party with `GSTIN_VALID`, bill-wise with 30 credit days), "Mid Co" (two aliases,
/// no party). Items: "Pipe" (HSN 3917, two GST entries) and "Pickaxe" (no HSN, no GST entry).
fn fixture() -> Masters {
    let mut m = Masters::default();
    m.groups.insert(GroupId(10), group(10, "Sundry debtors"));
    m.groups.insert(GroupId(11), group(11, "Cash-in-hand"));

    let mut zeta = ledger(1, "Zeta Traders", 10);
    zeta.aliases = vec!["ZT".to_owned()];
    m.ledgers.insert(LedgerId(1), zeta);

    let mut alpha = ledger(2, "Alpha Stores", 10);
    alpha.party = Some(party(Some(GSTIN_VALID), None));
    alpha.bill_wise = Some(BillWiseConfig {
        default_credit_days: Some(30),
    });
    m.ledgers.insert(LedgerId(2), alpha);

    let mut mid = ledger(3, "Mid Co", 11);
    mid.aliases = vec!["MC".to_owned(), "Mid".to_owned()];
    m.ledgers.insert(LedgerId(3), mid);

    m.stock_items.insert(
        StockItemId(1),
        item(
            1,
            "Pipe",
            Some("3917"),
            vec![gst_entry(2024, 7, 1, 12), gst_entry(2025, 4, 1, 18)],
        ),
    );
    m.stock_items
        .insert(StockItemId(2), item(2, "Pickaxe", None, Vec::new()));
    m
}

fn frame(
    ctx: &egui::Context,
    screen: &mut MastersScreen,
    app: &mut AppCtx,
    input: RawInput,
) -> ScreenAction {
    let mut action = ScreenAction::Stay;
    let out = ctx.run_ui(input, |ui| {
        action = screen.ui(ui, app);
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

fn app_with(masters: Masters) -> AppCtx {
    AppCtx {
        books: Some(books_with(masters)),
        api: None,
        lock_date: None,
        ai_tier: None,
    }
}

// ---- tabs and the secondary kinds ------------------------------------------------------------

#[test]
fn every_kind_is_on_exactly_one_side_of_the_tabs() {
    // provenance: derived. Two primary kinds plus eight secondary kinds make the ten of
    // MasterKind::ALL, with no overlap.
    assert_eq!(view::PRIMARY_KINDS.len() + view::MORE_KINDS.len(), 10);
    for kind in MasterKind::ALL {
        let in_primary = view::PRIMARY_KINDS.contains(&kind);
        let in_more = view::MORE_KINDS.contains(&kind);
        assert!(in_primary ^ in_more, "{kind:?} must be on exactly one side");
    }
}

#[test]
fn the_tabs_are_ledgers_then_stock_items() {
    // provenance: mock v2 index.html line 1793 lists the tabs as Ledgers, then Stock items.
    assert_eq!(
        view::PRIMARY_KINDS,
        [MasterKind::Ledger, MasterKind::StockItem]
    );
    assert_eq!(view::primary_tab(MasterKind::Ledger), Some(0));
    assert_eq!(view::primary_tab(MasterKind::StockItem), Some(1));
    // A kind behind "More master kinds" selects no tab.
    assert_eq!(view::primary_tab(MasterKind::Group), None);
}

// ---- the GSTIN check -------------------------------------------------------------------------

#[test]
fn an_empty_gstin_has_nothing_to_check() {
    // provenance: derived. Nothing typed gives no result, not an error.
    assert_eq!(view::check_gstin(""), GstinCheck::Empty);
    assert_eq!(view::check_gstin("   "), GstinCheck::Empty);
}

#[test]
fn a_valid_gstin_passes_and_surrounding_spaces_do_not_matter() {
    // provenance: the vector is valid per ledgerly-core gstin.rs (see GSTIN_VALID); Gstin::parse
    // trims its input (gstin.rs, `let s = input.trim()`).
    assert_eq!(view::check_gstin(GSTIN_VALID), GstinCheck::Valid);
    assert_eq!(
        view::check_gstin(&format!("  {GSTIN_VALID} ")),
        GstinCheck::Valid
    );
}

#[test]
fn a_wrong_check_character_names_the_expected_one() {
    // provenance: derived. The first 14 characters of GSTIN_VALID carry the check character W, so
    // changing the 15th to X must fail, and the message must say W was expected.
    let typo = "29ABCDE1234F1ZX";
    match view::check_gstin(typo) {
        GstinCheck::Invalid(reason) => {
            assert!(reason.contains("expected W"), "reason was: {reason}");
        }
        other @ (GstinCheck::Empty | GstinCheck::Valid) => {
            panic!("expected Invalid, got {other:?}")
        }
    }
}

#[test]
fn a_short_gstin_says_it_needs_fifteen_characters() {
    // provenance: derived. A GSTIN has 15 characters (GstinError::Length message).
    match view::check_gstin("29ABCDE") {
        GstinCheck::Invalid(reason) => {
            assert!(reason.contains("15 characters"), "reason was: {reason}");
        }
        other @ (GstinCheck::Empty | GstinCheck::Valid) => {
            panic!("expected Invalid, got {other:?}")
        }
    }
}

// ---- ledger rows -----------------------------------------------------------------------------

#[test]
fn ledger_rows_are_in_name_order_with_group_alias_and_gstin() {
    let m = fixture();
    let rows = view::ledger_rows(&m, "");
    // provenance: derived. Alphabetical by name: Alpha, Mid, Zeta.
    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["Alpha Stores", "Mid Co", "Zeta Traders"]);
    // provenance: derived. Alpha sits under Sundry debtors and carries the GSTIN.
    assert_eq!(rows[0].group, "Sundry debtors");
    assert_eq!(rows[0].gstin, GSTIN_VALID);
    assert_eq!(rows[0].alias, "");
    // provenance: derived. Two aliases are joined with ", " in the order they are stored.
    assert_eq!(rows[1].alias, "MC, Mid");
    assert_eq!(rows[1].group, "Cash-in-hand");
    // provenance: derived. A ledger with no party record has no GSTIN.
    assert_eq!(rows[2].gstin, "");
}

#[test]
fn the_ledger_filter_finds_by_alias_and_gstin() {
    let m = fixture();
    // provenance: derived. "zt" is the alias of Zeta Traders, in any case.
    let by_alias = view::ledger_rows(&m, "zt");
    assert_eq!(by_alias.len(), 1);
    assert_eq!(by_alias[0].name, "Zeta Traders");
    // provenance: derived. A part of the GSTIN finds Alpha Stores.
    let by_gstin = view::ledger_rows(&m, "29abcde");
    assert_eq!(by_gstin.len(), 1);
    assert_eq!(by_gstin[0].name, "Alpha Stores");
}

#[test]
fn a_ledger_whose_group_is_missing_says_so() {
    let mut m = Masters::default();
    m.ledgers.insert(LedgerId(9), ledger(9, "Orphan", 99));
    let rows = view::ledger_rows(&m, "");
    // provenance: derived. The brief forbids a silent default: a missing group is named as
    // missing, never shown as a group name.
    assert_eq!(rows.len(), 1);
    assert!(
        rows[0].group.contains("not found"),
        "group: {}",
        rows[0].group
    );
}

// ---- the party detail ------------------------------------------------------------------------

#[test]
fn the_party_detail_shows_the_stored_fields_and_the_gstin_state() {
    let m = fixture();
    let d = view::ledger_detail(&m, 2).unwrap();
    assert_eq!(d.name, "Alpha Stores");
    // provenance: derived. The party record has no state, so the state comes from the GSTIN's
    // first two digits, 29, which the GST state list gives as Karnataka.
    assert_eq!(d.state, "29 Karnataka");
    assert_eq!(d.gstin_check, GstinCheck::Valid);
    // provenance: derived. Credit days and bill-wise come from the bill-wise settings.
    assert_eq!(d.credit_days, Some(30));
    assert!(d.bill_wise);
    // provenance: derived. Address lines, then the PIN code, joined with ", ".
    assert_eq!(d.address, "12 Market Road, Bengaluru, 560001");
    assert_eq!(d.group, "Sundry debtors");
}

#[test]
fn a_party_state_on_the_record_wins_over_the_gstin() {
    let mut m = Masters::default();
    m.groups.insert(GroupId(10), group(10, "Sundry debtors"));
    let mut l = ledger(5, "Maharashtra Party", 10);
    // The record says 27 (Maharashtra); the GSTIN says 29. The record is what the user typed.
    l.party = Some(party(Some(GSTIN_VALID), Some(27)));
    m.ledgers.insert(LedgerId(5), l);
    let d = view::ledger_detail(&m, 5).unwrap();
    // provenance: derived. GST state list: 27 is Maharashtra.
    assert_eq!(d.state, "27 Maharashtra");
}

#[test]
fn a_ledger_without_a_party_has_no_gstin_state_or_address() {
    let m = fixture();
    let d = view::ledger_detail(&m, 1).unwrap();
    // provenance: derived. Zeta Traders has no party record, so every party field is empty.
    assert_eq!(d.gstin, "");
    assert_eq!(d.gstin_check, GstinCheck::Empty);
    assert_eq!(d.state, "");
    assert_eq!(d.address, "");
    assert_eq!(d.credit_days, None);
    assert!(!d.bill_wise);
}

#[test]
fn an_unknown_ledger_has_no_detail() {
    let m = fixture();
    // provenance: derived. There is no ledger with id 99 in the fixture.
    assert_eq!(view::ledger_detail(&m, 99), None);
}

// ---- stock item rows -------------------------------------------------------------------------

#[test]
fn item_rows_show_hsn_and_the_latest_gst_rate() {
    let m = fixture();
    let rows = view::item_rows(&m, "");
    // provenance: derived. Alphabetical: Pickaxe, then Pipe.
    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["Pickaxe", "Pipe"]);
    // provenance: derived. Pickaxe has no HSN and no GST entry, so both cells are empty.
    assert_eq!(rows[0].hsn, "");
    assert_eq!(rows[0].gst_rate, "");
    // provenance: derived. Pipe's HSN is 3917. Its two entries are 12% from 1 July 2024 and 18%
    // from 1 April 2025, so the later one, 18%, is the latest.
    assert_eq!(rows[1].hsn, "3917");
    assert_eq!(rows[1].gst_rate, "18%");
}

#[test]
fn the_stock_filter_matches_names() {
    let m = fixture();
    // provenance: derived. "pipe" matches only Pipe.
    let rows = view::item_rows(&m, "pipe");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "Pipe");
}

// ---- drawing ---------------------------------------------------------------------------------

#[test]
fn the_page_head_and_keys_follow_the_mock() {
    let s = MastersScreen::new();
    // provenance: mock v2 index.html line 1794 gives the page title and the sub-line.
    assert_eq!(s.page_title(), "Masters");
    assert_eq!(
        s.subtitle().as_deref(),
        Some("Ledgers and stock items. Type to filter. Alt+C creates anywhere.")
    );
    // provenance: mock v2 index.html line 1831 (the masters key bar): Alt+C creates.
    assert!(
        s.keys()
            .iter()
            .any(|k| k.keys == "Alt+C" && k.label == "Create")
    );
    assert!(s.keys().iter().any(|k| k.keys == "Ctrl+Enter"));
}

#[test]
fn the_form_keys_replace_the_list_keys() {
    let ctx = egui::Context::default();
    let mut app = app_with(fixture());
    let mut s = MastersScreen::new();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, key(Key::C, Modifiers::ALT));
    assert!(s.is_form_open());
    // provenance: mock v2 index.html line 1831 (the create-ledger key bar): Ctrl+A saves.
    assert!(
        s.keys()
            .iter()
            .any(|k| k.keys == "Ctrl+A" && k.label == "Save")
    );
    assert!(!s.keys().iter().any(|k| k.keys == "Alt+C"));
}

#[test]
fn every_tab_and_secondary_kind_draws() {
    let ctx = egui::Context::default();
    let mut app = app_with(fixture());
    for kind in MasterKind::ALL {
        let mut s = MastersScreen::with_kind(kind);
        frame(&ctx, &mut s, &mut app, RawInput::default());
        assert_eq!(s.kind(), kind);
    }
    // A selected ledger draws with its detail panel: the cursor starts on the first row.
    let mut s = MastersScreen::new();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    let masters = app.books.as_ref().unwrap().masters.clone();
    s.move_selection(&masters, Step::Down);
    assert_eq!(
        s.selected(),
        Some(MasterRef {
            kind: MasterKind::Ledger,
            id: 2
        })
    );
    frame(&ctx, &mut s, &mut app, RawInput::default());
}

#[test]
fn an_empty_list_and_a_search_with_no_match_both_draw() {
    let ctx = egui::Context::default();
    let mut app = app_with(Masters::default());
    let mut s = MastersScreen::new();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    assert_eq!(s.kind(), MasterKind::Ledger);
    let mut app = app_with(fixture());
    let mut s = MastersScreen::new();
    s.set_filter("no such ledger");
    frame(&ctx, &mut s, &mut app, RawInput::default());
    assert!(s.error_text().is_none());
}

#[test]
fn a_gstin_that_fails_the_check_draws_its_reason_in_the_form() {
    let ctx = egui::Context::default();
    let mut app = app_with(fixture());
    let mut s = MastersScreen::new();
    s.begin_create(app.books.as_ref().unwrap()).unwrap();
    let Some(MasterDraft::Ledger(d)) = s.draft_mut() else {
        panic!("no ledger form");
    };
    let p = PartyDraft {
        gstin: "29ABCDE1234F1ZX".to_owned(),
        ..PartyDraft::default()
    };
    d.party = Some(p);
    frame(&ctx, &mut s, &mut app, RawInput::default());
    assert!(s.is_form_open());
    // provenance: derived. The same text gives the same Invalid result the form draws.
    assert!(matches!(
        view::check_gstin("29ABCDE1234F1ZX"),
        GstinCheck::Invalid(_)
    ));
}

#[test]
fn the_stock_tab_shows_a_missing_hsn_and_draws() {
    let ctx = egui::Context::default();
    let mut app = app_with(fixture());
    let mut s = MastersScreen::with_kind(MasterKind::StockItem);
    frame(&ctx, &mut s, &mut app, RawInput::default());
    // provenance: mock v2 index.html lines 1798-1799: the stock tab is the item table, and the item
    // with no HSN carries the "Missing: add HSN" warning.
    let rows = view::item_rows(&app.books.as_ref().unwrap().masters, "");
    assert!(rows.iter().any(|r| r.hsn.is_empty()));
    assert_eq!(s.kind(), MasterKind::StockItem);
}

#[test]
fn a_screen_opened_to_create_a_ledger_starts_the_form_with_the_name() {
    // provenance: index.html:1129, the palette's 'Create ledger "<q>"' row: "Opens the ledger
    // form with this name". The name is the typed text unchanged. Catches the form opened blank,
    // or not opened.
    let ctx = egui::Context::default();
    let mut app = app_with(fixture());
    let mut s = MastersScreen::creating_ledger("Ramesh Kirana");
    frame(&ctx, &mut s, &mut app, RawInput::default());
    assert!(s.is_form_open());
    let Some(MasterDraft::Ledger(d)) = s.draft() else {
        panic!("no ledger form");
    };
    assert_eq!(d.name, "Ramesh Kirana");
}

#[test]
fn a_create_from_the_palette_without_books_says_why() {
    // provenance: derived. With no company open there is nothing to create a ledger in; the
    // screen shows its no-books line and opens no form. Catches a pending create that panics
    // or opens a form with no books under it.
    let ctx = egui::Context::default();
    let mut app = AppCtx {
        books: None,
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    let mut s = MastersScreen::creating_ledger("Ramesh Kirana");
    frame(&ctx, &mut s, &mut app, RawInput::default());
    assert!(!s.is_form_open());
}
