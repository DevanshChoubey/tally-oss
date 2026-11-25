//! Wave 2 S-G: Enter moves to the next field in the masters form (`design/mock-v2/index.html:1831`,
//! the form keys "Enter Next field").
//!
//! Every expected value is derived by hand and the derivation is written next to it. Nothing is
//! copied from this crate's output.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_app::screens::masters::{
    MasterDraft, MastersScreen, PartyDraft, line_ids, set_more_details,
};
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{CompanyId, StateCode};
use ledgerly_model::{Address, Books, Company, EntityKind, Features, Masters, NegativeStockPolicy};
use time::macros::date;

fn books() -> Books {
    Books {
        company: Company {
            ineligible_itc_default: None,
            id: CompanyId(1),
            name: "Enter Co".to_owned(),
            mailing_name: None,
            address: Address::default(),
            state: StateCode::new(27).unwrap(),
            pan: None,
            registrations: Vec::new(),
            fy_start: date!(2026 - 04 - 01),
            books_begin: date!(2026 - 04 - 01),
            entity_kind: EntityKind::Company,
            features: Features::default(),
            negative_stock: NegativeStockPolicy::Block,
        },
        masters: Masters::default(),
        vouchers: BTreeMap::new(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

fn frame(ctx: &egui::Context, screen: &mut MastersScreen, app: &mut AppCtx, input: RawInput) {
    let out = ctx.run_ui(input, |ui| {
        screen.ui(ui, app);
    });
    out.drop_without_applying_deltas();
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

fn typed(text: &str) -> RawInput {
    RawInput {
        events: vec![Event::Text(text.to_owned())],
        ..RawInput::default()
    }
}

/// A screen with the create-ledger form open, Party details on, drawn once.
fn open_form() -> (egui::Context, MastersScreen, AppCtx) {
    let ctx = egui::Context::default();
    let mut app = AppCtx {
        books: Some(books()),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    let mut s = MastersScreen::new();
    s.begin_create(app.books.as_ref().unwrap()).unwrap();
    let Some(MasterDraft::Ledger(d)) = s.draft_mut() else {
        panic!("Alt+C on ledgers opens a ledger form");
    };
    d.party = Some(PartyDraft::default());
    // Alias and the address sit under "More details"; these tests walk them.
    set_more_details(&ctx, true);
    frame(&ctx, &mut s, &mut app, RawInput::default());
    (ctx, s, app)
}

#[test]
fn enter_in_a_one_line_field_moves_to_the_next_one() {
    // provenance: index.html:1831, the masters form keys: "Enter" Next field. With the details
    // open the one-line fields are Name, GSTIN, Alias, Opening balance (the compact form puts
    // Under, a list, after Name, so Alias and Opening balance are the adjacent pair). Derived:
    // Enter from Alias gives Opening balance the keyboard, the form stays open (Enter is not
    // Save, which is Ctrl+A), and the alias is typed as given. Catches Enter only dropping the
    // keyboard, as egui's one-line field does by default.
    let (ctx, mut s, mut app) = open_form();
    let lines = line_ids(&ctx);
    assert!(lines.len() >= 4, "the form has Name, GSTIN, Alias, Opening");
    ctx.memory_mut(|m| m.request_focus(lines[2]));
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, typed("Ramesh"));
    frame(&ctx, &mut s, &mut app, key(Key::Enter, Modifiers::NONE));
    frame(&ctx, &mut s, &mut app, RawInput::default());
    assert_eq!(ctx.memory(egui::Memory::focused), Some(lines[3]));
    assert!(s.is_form_open(), "Enter closed the form");
    let Some(MasterDraft::Ledger(d)) = s.draft() else {
        panic!("no ledger form");
    };
    assert_eq!(d.aliases, "Ramesh");
}

#[test]
fn enter_twice_walks_two_fields() {
    // provenance: index.html:1831 as above. Derived: each Enter moves one field on, so two
    // Enters from the first one-line field land on the third. Catches a move that works once.
    let (ctx, mut s, mut app) = open_form();
    let lines = line_ids(&ctx);
    assert!(
        lines.len() >= 3,
        "the form has at least three one-line fields"
    );
    ctx.memory_mut(|m| m.request_focus(lines[0]));
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, key(Key::Enter, Modifiers::NONE));
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, key(Key::Enter, Modifiers::NONE));
    frame(&ctx, &mut s, &mut app, RawInput::default());
    let focused = ctx.memory(egui::Memory::focused);
    assert_ne!(focused, Some(lines[0]));
    assert_ne!(focused, Some(lines[1]));
    assert!(focused.is_some(), "the second Enter dropped the keyboard");
}

#[test]
fn enter_in_the_address_starts_a_new_line() {
    // provenance: derived. The address is a multi-line field (the ledger master's address
    // lines), where Enter is a new line, so it is not one of the fields Enter walks; it keeps
    // the keyboard. Catches the rule breaking multi-line fields.
    let (ctx, mut s, mut app) = open_form();
    let lines = line_ids(&ctx);
    // The address is the only multi-line field: give the keyboard to it by its id, which is
    // not among the one-line fields.
    let address = ledgerly_app::screens::masters::address_id();
    assert!(!lines.contains(&address));
    ctx.memory_mut(|m| m.request_focus(address));
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, typed("12 MG Road"));
    frame(&ctx, &mut s, &mut app, key(Key::Enter, Modifiers::NONE));
    frame(&ctx, &mut s, &mut app, typed("Pune"));
    frame(&ctx, &mut s, &mut app, RawInput::default());
    assert_eq!(ctx.memory(egui::Memory::focused), Some(address));
    let Some(MasterDraft::Ledger(d)) = s.draft() else {
        panic!("no ledger form");
    };
    assert_eq!(d.party.as_ref().unwrap().address_lines, "12 MG Road\nPune");
}

#[test]
fn shift_enter_in_a_one_line_field_goes_back() {
    // provenance: the mock's entry keys pair "Enter Next field" with "Shift+Enter Previous"
    // (index.html:1344). Derived: Shift+Enter from Opening balance gives Alias (the field before it)
    // the keyboard. Catches Shift+Enter doing nothing or moving forward.
    let (ctx, mut s, mut app) = open_form();
    let lines = line_ids(&ctx);
    ctx.memory_mut(|m| m.request_focus(lines[3]));
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, key(Key::Enter, Modifiers::SHIFT));
    frame(&ctx, &mut s, &mut app, RawInput::default());
    assert_eq!(ctx.memory(egui::Memory::focused), Some(lines[2]));
}
