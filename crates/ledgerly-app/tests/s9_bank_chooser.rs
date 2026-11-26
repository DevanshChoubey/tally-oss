//! Bank reconciliation with several bank ledgers (round s9, branch s9/bank): the screen asks which
//! account to open instead of refusing.
//!
//! Provenance:
//! - Behaviour: the help page "Bank Reconciliation reports" says a different bank is chosen from
//!   the bank ledgers (F4 "Bank Ledger" inside the reconciliation screens, a list of banks when
//!   the report is opened)
//!   (<https://help.tallysolutions.com/bank-recon-reports/>, read 2026-11-25). The key here is C,
//!   because F4 is Contra in this app (`shell::COMPANY_KEYS`).
//! - Figures: derived by hand. An opening balance with no voucher on the ledger is its books
//!   balance (First Bank 40,000.00 Dr, Second Bank 7,500.00 Dr, see `setup`).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::{Api, ApiError, Command, CommandResult};
use ledgerly_app::screens::bank::{BankChoice, BankChooserScreen};
use ledgerly_app::shell::submit;
use ledgerly_app::shell::{GatewayScreen, Shell};
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{
    Clock, CompanyId, Date, GroupId, LedgerId, Outcome, Paise, SignedAmount, StateCode, VoucherId,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BankDetails, BaseVoucherKind, Company, EntityKind, Features, Ledger, LedgerLine,
    NegativeStockPolicy, ReservedGroup, Voucher, VoucherMode, VoucherStatus,
};
use time::OffsetDateTime;
use time::macros::date;

const CASH: LedgerId = LedgerId(1);
const CAPITAL: LedgerId = LedgerId(2);
const BANK_ONE: LedgerId = LedgerId(3);
const BANK_TWO: LedgerId = LedgerId(4);

fn rs(rupees: i64) -> Paise {
    Paise::new(rupees * 100)
}

fn group(kind: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(kind))
        .unwrap()
        .id
}

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
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
    }
}

fn ledger(id: LedgerId, name: &str, under: ReservedGroup, bank: bool) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: group(under),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank: bank.then(|| BankDetails {
            account_number: format!("000{}", id.0),
            ifsc: "ABCD0000001".to_owned(),
            bank_name: name.to_owned(),
            branch: "Main".to_owned(),
        }),
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn line(ledger: LedgerId, amount: SignedAmount) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger,
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    }
}

fn journal(id: u64, on: Date, cash: Paise) -> Voucher {
    let type_id = default_voucher_types()
        .into_iter()
        .find(|t| t.base == BaseVoucherKind::Journal)
        .unwrap()
        .id;
    Voucher {
        id: VoucherId(id),
        type_id,
        number: None,
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
        narration: String::new(),
        ledger_lines: vec![
            line(CASH, SignedAmount::dr(cash)),
            line(CAPITAL, SignedAmount::cr(cash)),
        ],
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: std::collections::BTreeMap::new(),
    }
}

fn committed(out: Result<Outcome<CommandResult>, ApiError>) {
    match out {
        Ok(Outcome::Value(_)) => {}
        other => panic!("expected a committed value, got {other:?}"),
    }
}

fn file(name: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("s9_bank_chooser");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.ledgerly"));
    let _removed = std::fs::remove_file(&path);
    path
}

/// A company with Cash, Capital and `banks` bank ledgers, and one voucher: Capital of 1,00,000
/// paid into Cash on 10 April 2026.
fn setup(name: &str, banks: usize) -> Api {
    let mut api = Api::create(
        &file(name),
        &company(),
        Clock::Fixed(OffsetDateTime::UNIX_EPOCH),
    )
    .unwrap();
    let mut ledgers = vec![
        ledger(CASH, "Cash", ReservedGroup::CashInHand, false),
        ledger(CAPITAL, "Capital", ReservedGroup::CapitalAccount, false),
    ];
    for (id, name) in [(BANK_ONE, "First Bank"), (BANK_TWO, "Second Bank")]
        .into_iter()
        .take(banks)
    {
        ledgers.push(ledger(id, name, ReservedGroup::BankAccounts, true));
    }
    for mut l in ledgers {
        // provenance: derived. An opening balance is the ledger's books balance on any date from
        // the books' beginning with no voucher on it: First Bank 40,000 Dr, Second Bank 7,500 Dr.
        if l.id == BANK_ONE {
            l.opening = SignedAmount::dr(rs(40_000));
        }
        if l.id == BANK_TWO {
            l.opening = SignedAmount::dr(rs(7_500));
        }
        committed(api.execute(Command::CreateLedger(l)));
    }
    committed(api.execute(Command::PostVoucher(journal(
        1,
        date!(2026 - 04 - 10),
        rs(100_000),
    ))));
    api
}

fn press(key: Key, modifiers: Modifiers) -> RawInput {
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

fn frames_until(ctx: &egui::Context, shell: &mut Shell, done: impl Fn(&mut Shell) -> bool) {
    let start = Instant::now();
    loop {
        frame(ctx, shell, RawInput::default());
        if done(shell) {
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn ready(ctx: &egui::Context, api: Api) -> Shell {
    let mut shell = Shell::new(Box::new(GatewayScreen::new()));
    shell.app_ctx_mut().api = Some(ApiWorker::spawn(api).unwrap());
    submit::note_changed();
    frames_until(ctx, &mut shell, |s| s.app_ctx_mut().books.is_some());
    frame(ctx, &mut shell, RawInput::default());
    frame(ctx, &mut shell, RawInput::default());
    shell
}

fn titles(shell: &Shell) -> Vec<String> {
    shell
        .tabs()
        .active_stack()
        .map_or_default(|stack| stack.breadcrumbs(&ledgerly_app::i18n::Tr::default()))
}

fn choose(ctx: &egui::Context, shell: &mut Shell, query: &str) {
    frame(ctx, shell, press(Key::K, Modifiers::COMMAND));
    shell.palette_mut().set_query(query);
    frame(ctx, shell, RawInput::default());
    frame(ctx, shell, press(Key::Enter, Modifiers::NONE));
}

/// A shell whose only screen is `root`, over `api`, with its books loaded.
fn shell_with(ctx: &egui::Context, api: Api, root: Box<dyn Screen>) -> Shell {
    let mut shell = Shell::new(root);
    shell.app_ctx_mut().api = Some(ApiWorker::spawn(api).unwrap());
    submit::note_changed();
    frames_until(ctx, &mut shell, |s| s.app_ctx_mut().books.is_some());
    frame(ctx, &mut shell, RawInput::default());
    shell
}

struct Shared<S: Screen>(Rc<RefCell<S>>);

impl<S: Screen> Screen for Shared<S> {
    fn title(&self, _tr: &ledgerly_app::i18n::Tr) -> String {
        self.0.borrow().title(&ledgerly_app::i18n::Tr::default())
    }
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut AppCtx) -> ScreenAction {
        self.0.borrow_mut().ui(ui, ctx)
    }
}

fn choices() -> Vec<BankChoice> {
    vec![
        BankChoice::new(BANK_ONE, "First Bank", "XXXX 0003"),
        BankChoice::new(BANK_TWO, "Second Bank", "XXXX 0004"),
    ]
}

#[test]
fn the_chooser_lists_each_bank_with_its_books_balance_from_the_engine() {
    let ctx = egui::Context::default();
    let chooser = Rc::new(RefCell::new(BankChooserScreen::new(
        choices(),
        date!(2026 - 11 - 20),
        None,
    )));
    let mut shell = shell_with(
        &ctx,
        setup("chooser_rows", 2),
        Box::new(Shared(Rc::clone(&chooser))),
    );
    frames_until(&ctx, &mut shell, |_| !chooser.borrow().is_loading());
    let rows: Vec<(String, Option<Paise>)> = chooser
        .borrow()
        .rows()
        .iter()
        .map(|r| (r.name().to_owned(), r.balance()))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("First Bank".to_owned(), Some(rs(40_000))),
            ("Second Bank".to_owned(), Some(rs(7_500))),
        ]
    );
}

#[test]
fn bank_reconciliation_with_two_banks_opens_the_chooser_then_the_chosen_bank() {
    let ctx = egui::Context::default();
    let mut shell = ready(&ctx, setup("chooser_open", 2));
    choose(&ctx, &mut shell, "Bank Reconciliation");
    assert_eq!(shell.notice(), None);
    assert_eq!(
        titles(&shell),
        vec!["Home".to_owned(), "Bank reconciliation".to_owned()]
    );
    // Down moves to the second bank, Enter opens it in place of the chooser.
    frame(&ctx, &mut shell, press(Key::ArrowDown, Modifiers::NONE));
    frame(&ctx, &mut shell, press(Key::Enter, Modifiers::NONE));
    assert_eq!(
        titles(&shell),
        vec![
            "Home".to_owned(),
            "Bank reconciliation: Second Bank".to_owned()
        ]
    );
}

#[test]
fn the_palette_row_for_one_bank_still_opens_it_directly() {
    let ctx = egui::Context::default();
    let mut shell = ready(&ctx, setup("chooser_palette_row", 2));
    choose(&ctx, &mut shell, "Bank Reconciliation: Second");
    assert_eq!(
        titles(&shell),
        vec![
            "Home".to_owned(),
            "Bank reconciliation: Second Bank".to_owned()
        ]
    );
}

#[test]
fn c_on_the_bank_screen_switches_account_through_the_chooser() {
    let ctx = egui::Context::default();
    let mut shell = ready(&ctx, setup("chooser_switch", 2));
    choose(&ctx, &mut shell, "Bank Reconciliation: First");
    assert_eq!(
        titles(&shell).last().map(String::as_str),
        Some("Bank reconciliation: First Bank")
    );
    frame(&ctx, &mut shell, press(Key::C, Modifiers::NONE));
    assert_eq!(
        titles(&shell),
        vec!["Home".to_owned(), "Bank reconciliation".to_owned()]
    );
}

#[test]
fn one_bank_ledger_opens_directly_and_has_nothing_to_switch_to() {
    let ctx = egui::Context::default();
    let mut shell = ready(&ctx, setup("chooser_one", 1));
    choose(&ctx, &mut shell, "Bank Reconciliation");
    let want = vec![
        "Home".to_owned(),
        "Bank reconciliation: First Bank".to_owned(),
    ];
    assert_eq!(titles(&shell), want);
    frame(&ctx, &mut shell, press(Key::C, Modifiers::NONE));
    assert_eq!(titles(&shell), want, "with one bank C does nothing");
}

#[test]
fn a_company_without_a_bank_ledger_still_says_so() {
    let ctx = egui::Context::default();
    let mut shell = ready(&ctx, setup("chooser_none", 0));
    choose(&ctx, &mut shell, "Bank Reconciliation");
    assert_eq!(titles(&shell), vec!["Home".to_owned()]);
    let notice = shell.notice().unwrap_or_default().to_owned();
    assert!(notice.contains("no bank ledger"), "{notice}");
}
