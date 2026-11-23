//! Card U3 (shell chrome): the sidebar opens its targets, Ctrl+\ toggles the rail, the selected
//! entry follows the top of the stack, and no palette row is a dead command (rule B3).
//!
//! Provenance: the sizes are the mock's. `design/mock-v2/index.html:80` gives the `.app` grid
//! `grid-template-columns: var(--navw, 208px) 1fr` and `grid-template-rows: 48px 1fr 32px`, and
//! line 82 gives the rail `--navw: 60px`. Each test names the mutation it catches.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_app::keymap::{self, Action};
use ledgerly_app::palette::PaletteTarget;
use ledgerly_app::shell::navigate::NavError;
use ledgerly_app::shell::sidebar::{self, CommandRoute};
use ledgerly_app::shell::{KEY_BAR_HEIGHT, Shell, TOP_BAR_HEIGHT, palette_rows};
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{CompanyId, Gstin, LedgerId, Paise, SignedAmount, StateCode};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BankDetails, Books, Company, EntityKind, Features, GstRegistration, Ledger, Masters,
    NegativeStockPolicy, ReservedGroup,
};
use time::macros::date;

/// Does nothing but have a title.
struct Leaf(&'static str);

impl Screen for Leaf {
    fn title(&self) -> String {
        self.0.to_owned()
    }
    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut AppCtx) -> ScreenAction {
        ui.label(self.0);
        ScreenAction::Stay
    }
}

fn leaf(name: &'static str) -> Box<dyn Screen> {
    Box::new(Leaf(name))
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

fn frame(ctx: &egui::Context, shell: &mut Shell, input: RawInput) {
    let out = ctx.run_ui(input, |ui| shell.show(ui));
    out.drop_without_applying_deltas();
}

fn group(kind: ReservedGroup) -> ledgerly_core::GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(kind))
        .unwrap()
        .id
}

fn plain(id: LedgerId, name: &str, reserved: ReservedGroup) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: group(reserved),
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

/// Books with one cash ledger, one bank ledger and one GST registration, so that every sidebar
/// target that needs a company has one to open. The company is fictional.
fn books() -> Books {
    let company = Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(29).unwrap(),
        pan: None,
        registrations: vec![GstRegistration {
            gstin: Gstin::parse("29ABCDE1234F1ZW").unwrap(),
            state: StateCode::new(29).unwrap(),
            effective_from: date!(2026 - 04 - 01),
            composition: false,
        }],
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    };
    let mut bank = plain(LedgerId(3), "HDFC Current", ReservedGroup::BankAccounts);
    bank.bank = Some(BankDetails {
        account_number: "000123456789".to_owned(),
        ifsc: "FIC0000001".to_owned(),
        bank_name: "Fictional Bank".to_owned(),
        branch: "Test Branch".to_owned(),
    });
    let mut masters = Masters::default();
    for ledger in [plain(LedgerId(1), "Cash", ReservedGroup::CashInHand), bank] {
        masters.ledgers.insert(ledger.id, ledger);
    }
    for voucher_type in default_voucher_types() {
        masters.voucher_types.insert(voucher_type.id, voucher_type);
    }
    Books {
        company,
        masters,
        vouchers: BTreeMap::new(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

fn books_ctx() -> AppCtx {
    AppCtx {
        books: Some(books()),
        ..AppCtx::default()
    }
}

/// The labels of the visible sidebar entries, in order.
fn labels() -> Vec<&'static str> {
    sidebar::visible_groups()
        .iter()
        .flat_map(|group| group.entries.iter().map(|entry| entry.label))
        .collect()
}

// ---- layout sizes (the mock) ---------------------------------------------------------------

#[test]
fn the_chrome_sizes_are_the_mocks() {
    // provenance: design/mock-v2/index.html:80-82: 48 px top bar, 32 px key bar, 208 px side bar,
    // 60 px rail. Catches a changed height or width in the shell's constants.
    assert_eq!(TOP_BAR_HEIGHT, 48.0);
    assert_eq!(KEY_BAR_HEIGHT, 32.0);
    assert_eq!(sidebar::WIDTH, 208.0);
    assert_eq!(sidebar::RAIL_WIDTH, 60.0);
}

// ---- sidebar targets -----------------------------------------------------------------------

#[test]
fn each_sidebar_entry_opens_its_target_or_says_why_it_cannot() {
    // provenance: derived. The fixture has a bank ledger and a GST registration, so the bank and
    // GST entries open. Sales is the one entry that may refuse: the voucher entry screen does not
    // enter sales vouchers, so a company without inventory gets the reason, not a blank screen.
    // Catches a sidebar row whose target has no route (the B3 failure in the sidebar).
    let ctx = books_ctx();
    for group in sidebar::visible_groups() {
        for entry in &group.entries {
            match sidebar::screen_for(&entry.target, &ctx) {
                Ok(screen) => assert!(!screen.title().is_empty(), "\"{}\"", entry.label),
                Err(NavError::Unsupported { reason, .. }) => {
                    assert!(!reason.is_empty(), "\"{}\" gave no reason", entry.label);
                }
                Err(other) => panic!("\"{}\" did not open: {other}", entry.label),
            }
        }
    }
}

#[test]
fn the_sidebar_shows_the_mock_entries() {
    // provenance: derived from the sidebar groups of design/mock-v2/DESIGN.md ("Side bar groups":
    // Enter is Sales F8, Purchase F9, Payment F5, Receipt F6, Plain-words entry). Purchase opens
    // the invoice screen (tests/w2_purchase_invoice.rs), so its row shows, after Sales.
    let shown = labels();
    assert_eq!(
        shown,
        vec![
            "Home",
            "Review queue",
            "Sales",
            "Purchase",
            "Payment",
            "Receipt",
            "Plain-words entry",
            "Day book",
            "Balance sheet",
            "Masters",
            "GST returns",
            "Bank reconciliation",
            "AI and privacy",
            "Keyboard map",
            "Company setup",
        ]
    );
}

// ---- Ctrl+\ --------------------------------------------------------------------------------

#[test]
fn ctrl_backslash_toggles_the_rail() {
    // provenance: design/mock-v2/DESIGN.md (side bar, rail 60 px with Ctrl+\). Catches a missing
    // toggle, a toggle on the wrong key, or a toggle that does not stick across frames.
    let ctx = egui::Context::default();
    let mut shell = Shell::new(leaf("Home"));
    assert!(!shell.rail(), "the full sidebar is the default");
    frame(&ctx, &mut shell, key(Key::Backslash, Modifiers::COMMAND));
    assert!(shell.rail(), "Ctrl+\\ shows the rail");
    frame(&ctx, &mut shell, key(Key::Backslash, Modifiers::COMMAND));
    assert!(!shell.rail(), "Ctrl+\\ again shows the full sidebar");
}

// ---- selection -----------------------------------------------------------------------------

#[test]
fn the_selected_entry_follows_the_top_of_the_stack() {
    // The selected entry is the one for the screen on top. Titles: a report titles itself with
    // a colon and the kind it shows ("Masters: Ledgers", "Bank reconciliation: HDFC Current"),
    // and the day book's title differs only in case. Catches a selection fixed to the first
    // entry, or one that ignores the stack.
    assert_eq!(sidebar::selected_label("Masters: Ledgers"), Some("Masters"));
    assert_eq!(
        sidebar::selected_label("Bank reconciliation: HDFC Current"),
        Some("Bank reconciliation")
    );
    assert_eq!(sidebar::selected_label("Day Book"), Some("Day book"));
    assert_eq!(sidebar::selected_label("Some other screen"), None);
    // An invoice titles itself "<voucher type> invoice"; the mock's sidebar row Sales is the
    // `sales-invoice` screen (index.html:888, `NAV`), so it is selected while the invoice is open.
    // Purchase opens the same screen here. A credit note has no sidebar row.
    assert_eq!(sidebar::selected_label("Sales invoice"), Some("Sales"));
    assert_eq!(
        sidebar::selected_label("Purchase invoice"),
        Some("Purchase")
    );
    assert_eq!(sidebar::selected_label("Credit Note invoice"), None);

    let mut shell = Shell::new(leaf("Home"));
    assert_eq!(shell.selected_sidebar(), Some("Home"));
    shell
        .tabs_mut()
        .active_stack_mut()
        .unwrap()
        .push(leaf("Masters: Ledgers"));
    assert_eq!(shell.selected_sidebar(), Some("Masters"));
    shell
        .tabs_mut()
        .active_stack_mut()
        .unwrap()
        .push(leaf("Some other screen"));
    assert_eq!(shell.selected_sidebar(), None);
}

// ---- B3: no dead palette row ---------------------------------------------------------------

#[test]
fn no_palette_row_is_a_dead_command() {
    // B3: a command row either has a route (the shell runs it, or a screen opens for it) or is not
    // listed. Every listed command must open a screen or be one of the shell's own, and none may
    // raise "cannot run it yet" (the old generic notice). Catches a command row added without a
    // route, and a route removed from a listed command.
    let rows = palette_rows(&keymap::try_classic().unwrap());
    let ctx = books_ctx();
    let mut commands = Vec::new();
    for row in &rows {
        if let PaletteTarget::Command(action) = &row.target {
            commands.push(*action);
            match sidebar::command_route(*action) {
                CommandRoute::Hidden => panic!("\"{}\" is listed but has no route", row.title),
                CommandRoute::Shell => {}
                CommandRoute::Screen => {
                    if let Err(NavError::NotNavigation) = sidebar::screen_for(&row.target, &ctx) {
                        panic!("\"{}\" is listed but opens nothing", row.title);
                    }
                }
            }
        }
    }
    // The shell's own commands and Help are listed.
    for listed in [
        Action::NewTab,
        Action::CloseTab,
        Action::Back,
        Action::OpenPalette,
        Action::Help,
        Action::NewPayment,
        Action::NewPurchase,
        Action::NewCreditNote,
        Action::NewDebitNote,
    ] {
        assert!(commands.contains(&listed), "{listed:?} should be listed");
    }
    // The commands with no screen, and the keys that belong to a screen, are not.
    for hidden in [
        Action::ChangePeriod,
        Action::SwitchCompany,
        Action::CompanyFeatures,
        Action::Configure,
        Action::Save,
        Action::Print,
    ] {
        assert!(!commands.contains(&hidden), "{hidden:?} must not be listed");
    }
}
