//! Card S7 (accessibility): every text input on the masters, voucher entry, company setup and
//! welcome screens reads its visible caption. Expected names are the captions the screens draw
//! (derived from the source: `screens/masters/form_ui.rs` `text_row` labels, `screens/voucher_entry/draw.rs`
//! captions, `screens/setup/mod.rs` field captions, `shell/welcome.rs` captions).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    reason = "test code: rule B2 lets tests unwrap and panic; fixtures are long"
)]

use std::collections::BTreeMap;

use egui::accesskit::{Node, NodeId, Role, TreeUpdate};
use egui::{Context, Event, Key, Modifiers, Pos2, RawInput, Rect, Ui, Vec2};
use ledgerly_app::screens::masters::{MastersScreen, set_more_details};
use ledgerly_app::screens::setup::SetupScreen;
use ledgerly_app::screens::voucher_entry::VoucherEntryScreen;
use ledgerly_app::shell::WelcomeScreen;
use ledgerly_app::theme::{self, ThemeKind};
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{
    Clock, Date, FinancialYear, GroupId, Gstin, LedgerId, Outcome, Paise, SignedAmount, StateCode,
    VoucherTypeId,
};
use ledgerly_model::{
    Address, BaseVoucherKind, BillWiseConfig, Books, Company, EntityKind, Features, Group, Ledger,
    Masters, NegativeStockPolicy, NumberingConfig, NumberingMethod, PartyDetails, RegistrationType,
    RestartPeriod, VoucherType,
};
use time::OffsetDateTime;
use time::macros::date;

// ---- fixture (copied from tests/u4_voucher_entry.rs) -----------------------------------------

const CASH: LedgerId = LedgerId(1);
const HDFC: LedgerId = LedgerId(2);
const SHARMA: LedgerId = LedgerId(3);
const SHREE: LedgerId = LedgerId(4);
const RENT: LedgerId = LedgerId(5);
const BILLWISE: LedgerId = LedgerId(6);

const T_JOURNAL: VoucherTypeId = VoucherTypeId(1);
const T_PAYMENT: VoucherTypeId = VoucherTypeId(2);
const T_SALES: VoucherTypeId = VoucherTypeId(3);
const T_OFF: VoucherTypeId = VoucherTypeId(4);
const T_STOCK: VoucherTypeId = VoucherTypeId(5);
const GROUP: GroupId = GroupId(1);

fn ledger(id: LedgerId, name: &str, aliases: &[&str]) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: aliases.iter().map(|a| (*a).to_owned()).collect(),
        group: GROUP,
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

fn voucher_type(id: VoucherTypeId, name: &str, base: BaseVoucherKind, active: bool) -> VoucherType {
    VoucherType {
        id,
        name: name.to_owned(),
        base,
        abbreviation: name.chars().take(3).collect(),
        numbering: NumberingConfig {
            method: NumberingMethod::Automatic,
            prevent_renumbering: false,
            prefix: String::new(),
            suffix: String::new(),
            width: 4,
            restart: RestartPeriod::Yearly,
            series: Vec::new(),
        },
        active,
        tally: None,
    }
}

fn books() -> Books {
    let mut masters = Masters::default();
    masters.groups.insert(
        GROUP,
        Group {
            ineligible_itc: None,
            id: GROUP,
            name: "Current Assets".to_owned(),
            aliases: Vec::new(),
            parent: None,
            reserved: None,
            nature: None,
            affects_gross_profit: false,
            tally: None,
        },
    );
    let mut sharma = ledger(SHARMA, "Sharma Traders", &["ST"]);
    sharma.party = Some(PartyDetails {
        registration: RegistrationType::Regular,
        // A valid GSTIN from ledgerly-core's own tested vectors (gstin.rs `VALID`).
        gstin: Some(Gstin::parse("29ABCDE1234F1ZW").unwrap()),
        pan: None,
        state: None,
        address: Address::default(),
    });
    let mut billwise = ledger(BILLWISE, "Kaveri Packaging", &[]);
    billwise.bill_wise = Some(BillWiseConfig {
        default_credit_days: Some(30),
    });
    for l in [
        ledger(CASH, "Cash", &[]),
        ledger(HDFC, "HDFC Bank", &[]),
        sharma,
        ledger(SHREE, "Shree Stationers", &[]),
        ledger(RENT, "Rent", &[]),
        billwise,
    ] {
        masters.ledgers.insert(l.id, l);
    }
    for t in [
        voucher_type(T_JOURNAL, "Journal", BaseVoucherKind::Journal, true),
        voucher_type(T_PAYMENT, "Payment", BaseVoucherKind::Payment, true),
        voucher_type(T_SALES, "Sales", BaseVoucherKind::Sales, true),
        voucher_type(T_OFF, "Contra", BaseVoucherKind::Contra, false),
        voucher_type(
            T_STOCK,
            "Stock Journal",
            BaseVoucherKind::StockJournal,
            true,
        ),
    ] {
        masters.voucher_types.insert(t.id, t);
    }
    Books {
        company: Company {
            ineligible_itc_default: None,
            id: ledgerly_core::CompanyId(1),
            name: "Test Traders".to_owned(),
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
        masters,
        vouchers: BTreeMap::new(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

fn fy() -> FinancialYear {
    FinancialYear::april(2026).unwrap()
}

const TODAY: Date = date!(2026 - 11 - 21);

fn open(books: &Books, t: VoucherTypeId) -> VoucherEntryScreen {
    match VoucherEntryScreen::open(books, t, fy(), TODAY).unwrap() {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("unexpectedly unsupported: {u:?}"),
    }
}

fn context() -> Context {
    let ctx = Context::default();
    ctx.enable_accesskit();
    theme::apply(&ctx, ThemeKind::Light);
    ctx
}

/// Runs `frames` frames of `body` with AccessKit on, and returns the last frame's tree.
fn tree(ctx: &Context, frames: usize, mut body: impl FnMut(&mut Ui)) -> TreeUpdate {
    let mut last = None;
    for _ in 0..frames {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            ..RawInput::default()
        };
        let mut out = ctx.run_ui(input, &mut body);
        last = out.platform_output.accesskit_update.take();
        out.drop_without_applying_deltas();
    }
    last.expect("AccessKit is enabled")
}

/// The names of every text input in `update`, in tree order.
fn text_inputs(update: &TreeUpdate) -> Vec<String> {
    update
        .nodes
        .iter()
        .filter(|(_, node): &&(_, Node)| {
            matches!(node.role(), Role::TextInput | Role::MultilineTextInput)
        })
        .filter_map(|(_, node)| node.label().map(str::to_owned))
        .collect()
}

fn assert_named(update: &TreeUpdate, names: &[&str]) {
    let have = text_inputs(update);
    for name in names {
        assert!(
            have.iter().any(|h| h == name),
            "a text input named {name:?}; have {have:?}"
        );
    }
}

fn fixture_books() -> Books {
    books()
}

#[test]
fn voucher_entry_fields_read_their_captions() {
    // derived: the voucher screen captions its Date and Narration fields and names each row's
    // ledger and amount (voucher_entry/draw.rs). Checked for a payment and a journal.
    let ctx = context();
    let b = fixture_books();
    for t in [T_PAYMENT, T_JOURNAL] {
        let mut screen = open(&b, t);
        let mut app = AppCtx {
            tr: ledgerly_app::i18n::Tr::default(),
            books: Some(b.clone()),
            api: None,
            lock_date: None,
            ai_tier: None,
        };
        let update = tree(&ctx, 3, |ui| {
            let _ = screen.ui(ui, &mut app);
        });
        assert_named(&update, &["Date", "Narration", "Ledger", "Amount"]);
    }
}

#[test]
fn masters_ledger_form_fields_read_their_captions() {
    // derived: the ledger form's captions (form_ui.rs text_row "Name", "Alias"; fld "Opening balance").
    let ctx = context();
    let b = fixture_books();
    let mut app = AppCtx {
        tr: ledgerly_app::i18n::Tr::default(),
        books: Some(b.clone()),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    let mut screen = MastersScreen::creating_ledger("Ramesh Kirana");
    // The create form is the mock's compact one; Alias and Opening balance sit under "More details".
    set_more_details(&ctx, true);
    let update = tree(&ctx, 3, |ui| {
        let _ = screen.ui(ui, &mut app);
    });
    assert_named(&update, &["Name", "Alias", "Opening balance"]);
}

#[test]
fn masters_create_ledger_form_is_compact_until_more_details_is_opened() {
    // provenance: design/mock-v2/index.html:1802-1812 (`create-ledger` state): the form shows
    // Name, Under, Bill-wise (and GSTIN, State, Credit days for a party), with no Alias and no
    // Opening balance. Derived: a new form names Name only among those inputs.
    let ctx = context();
    let b = fixture_books();
    let mut app = AppCtx {
        tr: ledgerly_app::i18n::Tr::default(),
        books: Some(b.clone()),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    let mut screen = MastersScreen::creating_ledger("Ramesh Kirana");
    let update = tree(&ctx, 3, |ui| {
        let _ = screen.ui(ui, &mut app);
    });
    let have = text_inputs(&update);
    assert!(
        have.iter().any(|h| h == "Name"),
        "Name is drawn; have {have:?}"
    );
    assert!(
        !have.iter().any(|h| h == "Alias" || h == "Opening balance"),
        "Alias and Opening balance wait under More details; have {have:?}"
    );
}

#[test]
fn setup_fields_read_their_captions() {
    // derived: the company page's captions (setup/mod.rs field(...) calls).
    let ctx = context();
    let mut screen = SetupScreen::with_clock(Clock::Fixed(OffsetDateTime::UNIX_EPOCH));
    let mut app = AppCtx::default();
    // The wizard opens on its welcome step; Enter continues to the company step, where the
    // company fields are drawn (script as in s5_setup_look.rs `company_script`).
    tree(&ctx, 2, |ui| {
        let _ = screen.ui(ui, &mut app);
    });
    let enter = egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
        events: vec![enter],
        ..RawInput::default()
    };
    let out = ctx.run_ui(input, |ui| {
        let _ = screen.ui(ui, &mut app);
    });
    out.drop_without_applying_deltas();
    let update = tree(&ctx, 3, |ui| {
        let _ = screen.ui(ui, &mut app);
    });
    assert_named(&update, &["Company name", "City", "GSTIN"]);
}

#[test]
fn welcome_fields_read_their_captions() {
    // derived: the welcome screen's captions (shell/welcome.rs: "Open a company file", "Name",
    // "File", "State", "Financial year from 1 April").
    let ctx = context();
    let mut screen = WelcomeScreen::with_clock(Clock::Fixed(OffsetDateTime::UNIX_EPOCH));
    let mut app = AppCtx::default();
    let update = tree(&ctx, 3, |ui| {
        let _ = screen.ui(ui, &mut app);
    });
    assert_named(
        &update,
        &[
            "Open a company file",
            "Name",
            "File",
            "State",
            "Financial year from 1 April",
        ],
    );
}

/// The role and name of the node that has focus in `update`.
fn focused(update: &TreeUpdate) -> Option<(Role, String)> {
    let focus: NodeId = update.focus;
    update
        .nodes
        .iter()
        .find(|(id, _)| *id == focus)
        .and_then(|(_, node): &(NodeId, Node)| Some((node.role(), node.label()?.to_owned())))
}

/// One Tab press, then a settling frame (egui moves focus on the frame after the key), returning
/// what has focus.
fn tab(ctx: &Context, body: &mut impl FnMut(&mut Ui)) -> Option<(Role, String)> {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
        events: vec![Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        ..RawInput::default()
    };
    let mut out = ctx.run_ui(input, &mut *body);
    let _ = out.platform_output.accesskit_update.take();
    out.drop_without_applying_deltas();
    focused(&tree(ctx, 1, &mut *body))
}

#[test]
fn tab_in_a_voucher_skips_the_entries_of_the_open_ledger_list() {
    // Tally: "To move to the next field" is Tab (help.tallysolutions.com/keyboard-shortcuts-tally-prime/,
    // read 2026-11-24, Vouchers and Masters table). derived: while the Ledger field of a voucher is
    // open with its list, no Tab press may land on one of the list's ledger entries, which are
    // buttons; Tab moves on to the next field instead.
    let ctx = context();
    let b = fixture_books();
    let mut screen = open(&b, T_PAYMENT);
    let mut app = AppCtx {
        tr: ledgerly_app::i18n::Tr::default(),
        books: Some(b.clone()),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    let mut body = |ui: &mut Ui| {
        let _ = screen.ui(ui, &mut app);
    };
    tree(&ctx, 3, &mut body);
    let ledger_names: Vec<String> = b.masters.ledgers.values().map(|l| l.name.clone()).collect();
    let stops: Vec<(Role, String)> = (0..12).filter_map(|_| tab(&ctx, &mut body)).collect();
    assert!(
        stops
            .iter()
            .any(|(role, name)| *role == Role::TextInput && name == "Ledger"),
        "Tab never reached the Ledger field: {stops:?}"
    );
    let on_entries: Vec<&(Role, String)> = stops
        .iter()
        .filter(|(role, name)| {
            *role == Role::Button && ledger_names.iter().any(|l| name.contains(l.as_str()))
        })
        .collect();
    assert!(
        on_entries.is_empty(),
        "Tab stopped on ledger list entries: {on_entries:?}"
    );
}
