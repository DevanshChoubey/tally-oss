//! A company that does not keep inventory is not offered inventory masters: no Stock items tab,
//! no Units, Stock groups, Godowns or Batches under "More master kinds", no "Create stock item"
//! button, and Alt+Left / Alt+Right skip them.
//!
//! provenance: observed in the product's own help, read 2026-11-25. "The Inventory Info. menu is
//! displayed in the Gateway of Tally, if you set Maintain Accounts only to No", and that menu
//! holds Stock Groups, Stock Items, Units of Measurement and Batches & Expiry Dates:
//! <https://help.tallysolutions.com/docs/te9rel65/Creating_Masters/Inventory_Info/Inventory_Information.htm>
//! Godowns are stock locations and exist only with stock, so they go with them (derived).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use egui::{Event, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use ledgerly_app::screens::masters::{MasterKind, MastersScreen, view};
use ledgerly_app::theme::{self, ThemeKind};
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{Clock, Outcome};
use ledgerly_model::{Books, Features};
use time::OffsetDateTime;

fn off() -> Features {
    Features {
        inventory: false,
        ..Features::default()
    }
}

fn on() -> Features {
    Features {
        inventory: true,
        ..Features::default()
    }
}

fn books(name: &str, inventory: bool) -> Books {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (api, _path) =
        ledgerly_sample::create_under(&dir, Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    let mut books = match api.books() {
        Ok(Outcome::Value(b)) => b,
        other => panic!("no books: {other:?}"),
    };
    books.company.features.inventory = inventory;
    books
}

fn frame(screen: &mut MastersScreen, ctx: &mut AppCtx, events: Vec<Event>) {
    let egui_ctx = egui::Context::default();
    theme::apply(&egui_ctx, ThemeKind::Light);
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
        events,
        ..RawInput::default()
    };
    let mut out = egui_ctx.run_ui(input, |ui| {
        screen.ui(ui, ctx);
    });
    out.textures_delta.clear();
}

fn alt(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::ALT,
    }
}

#[test]
fn the_tabs_follow_the_inventory_switch() {
    assert_eq!(
        view::primary_kinds(&on()),
        [MasterKind::Ledger, MasterKind::StockItem]
    );
    assert_eq!(view::primary_kinds(&off()), [MasterKind::Ledger]);
}

#[test]
fn inventory_kinds_leave_the_more_menu_when_inventory_is_off() {
    let hidden = [
        MasterKind::Unit,
        MasterKind::StockGroup,
        MasterKind::Godown,
        MasterKind::Batch,
    ];
    let shown = view::more_kinds(&off());
    for kind in hidden {
        assert!(!shown.contains(&kind), "{kind:?} is still offered");
    }
    for kind in [
        MasterKind::Group,
        MasterKind::VoucherType,
        MasterKind::CostCategory,
        MasterKind::CostCentre,
    ] {
        assert!(shown.contains(&kind), "{kind:?} went missing");
    }
    assert_eq!(view::more_kinds(&on()), view::MORE_KINDS);
}

#[test]
fn a_screen_asked_for_stock_items_shows_ledgers_when_inventory_is_off() {
    let mut ctx = AppCtx {
        books: Some(books("s13_inv_off_screen", false)),
        ..AppCtx::default()
    };
    let mut screen = MastersScreen::with_kind(MasterKind::StockItem);
    frame(&mut screen, &mut ctx, Vec::new());
    assert_eq!(screen.kind(), MasterKind::Ledger);
}

#[test]
fn a_screen_asked_for_stock_items_keeps_them_when_inventory_is_on() {
    let mut ctx = AppCtx {
        books: Some(books("s13_inv_on_screen", true)),
        ..AppCtx::default()
    };
    let mut screen = MastersScreen::with_kind(MasterKind::StockItem);
    frame(&mut screen, &mut ctx, Vec::new());
    assert_eq!(screen.kind(), MasterKind::StockItem);
}

#[test]
fn alt_arrows_skip_the_hidden_kinds() {
    let mut ctx = AppCtx {
        books: Some(books("s13_inv_off_keys", false)),
        ..AppCtx::default()
    };
    let mut screen = MastersScreen::with_kind(MasterKind::VoucherType);
    frame(&mut screen, &mut ctx, vec![alt(Key::ArrowRight)]);
    // The order is Group, Ledger, Voucher type, Unit, Stock group, Stock item, Godown, Batch,
    // Cost category, Cost centre; Unit to Batch are skipped.
    assert_eq!(screen.kind(), MasterKind::CostCategory);
    frame(&mut screen, &mut ctx, vec![alt(Key::ArrowLeft)]);
    assert_eq!(screen.kind(), MasterKind::VoucherType);
}
