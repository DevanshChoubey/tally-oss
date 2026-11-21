//! Coordinator tests (Wave 3): the shell wiring. The palette (Ctrl+K) opens over any screen, the
//! home screen of an open company turns its links into screens, the KPI tiles follow a posting,
//! and the welcome screen hands over to the home screen once a company is open.
//!
//! Every test drives `Shell::show` frame by frame against a real company file.
//!
//! Provenance:
//! - Figures are `derived` by hand from the vouchers posted here; the arithmetic sits beside each
//!   one. The tile is the closing balance of the Cash-in-Hand group, as card C4 defined it
//!   (`c4_home.rs` header), as of the end of the default period. Tally's dashboard page names a
//!   Cash/Bank tile and does not define the figure
//!   (<https://help.tallysolutions.com/tally-prime/reports-in-tallyprime/dashboard/>, read
//!   2026-11-21).
//! - Keys are the design mock's and the classic key map's: Ctrl+K opens the palette, F6 a
//!   receipt, M the masters, Esc goes back, Ctrl+T opens a tab. TallyPrime's Go To is Alt+G, which
//!   the classic map also binds (`u2_keymap.rs`).
//! - The default period is the financial year from its start to the last voucher dated in it
//!   (`navigate::default_period`), which is `pending` a Tally source.

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
use ledgerly_app::screens::home::KpiView;
use ledgerly_app::shell::submit::{self, CommandReply};
use ledgerly_app::shell::{GatewayScreen, ScreenStack, Shell, WelcomeScreen};
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
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("c9_shell");
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
    for l in ledgers {
        committed(api.execute(Command::CreateLedger(l)));
    }
    committed(api.execute(Command::PostVoucher(journal(
        1,
        date!(2026 - 04 - 10),
        rs(100_000),
    ))));
    api
}

/// Shows a shared home screen, so a test can read its tiles while the shell owns the screen.
struct Shared(Rc<RefCell<GatewayScreen>>);

impl Screen for Shared {
    fn title(&self) -> String {
        self.0.borrow().title()
    }
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut AppCtx) -> ScreenAction {
        self.0.borrow_mut().ui(ui, ctx)
    }
}

/// A shell over `api` whose only tab shows `root`, as after the welcome screen opened the file.
fn shell_over(api: Api, root: Box<dyn Screen>) -> Shell {
    let mut shell = Shell::new(root);
    shell.app_ctx_mut().api = Some(ApiWorker::spawn(api).unwrap());
    submit::note_changed();
    shell
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

/// Runs empty frames until `done` holds (bounded at 10 seconds).
fn frames_until(ctx: &egui::Context, shell: &mut Shell, done: impl Fn(&mut Shell) -> bool) {
    let start = Instant::now();
    loop {
        frame(ctx, shell, RawInput::default());
        if done(shell) {
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "the condition did not hold within 10 seconds"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A shell whose books have arrived, after two warm-up frames (a first layout may re-run).
fn ready(ctx: &egui::Context, api: Api) -> Shell {
    let mut shell = shell_over(api, Box::new(GatewayScreen::new()));
    frames_until(ctx, &mut shell, |s| s.app_ctx_mut().books.is_some());
    frame(ctx, &mut shell, RawInput::default());
    frame(ctx, &mut shell, RawInput::default());
    shell
}

fn titles(shell: &Shell) -> Vec<String> {
    shell
        .tabs()
        .active_stack()
        .map_or_default(ScreenStack::breadcrumbs)
}

/// Opens the palette with Ctrl+K, types `query` and accepts the first row with Enter.
fn choose(ctx: &egui::Context, shell: &mut Shell, query: &str) {
    frame(ctx, shell, press(Key::K, Modifiers::COMMAND));
    assert!(shell.palette().is_open(), "Ctrl+K opens the palette");
    shell.palette_mut().set_query(query);
    frame(ctx, shell, RawInput::default());
    frame(ctx, shell, press(Key::Enter, Modifiers::NONE));
}

#[test]
fn ctrl_k_and_enter_open_the_balance_sheet_over_home() {
    let ctx = egui::Context::default();
    let mut shell = ready(&ctx, setup("palette_report", 0));
    assert_eq!(titles(&shell), vec!["Home".to_owned()]);

    frame(&ctx, &mut shell, press(Key::K, Modifiers::COMMAND));
    assert!(shell.palette().is_open());
    shell.palette_mut().set_query("Balance Sheet");
    frame(&ctx, &mut shell, RawInput::default());
    assert_eq!(shell.palette().results()[0].title, "Balance Sheet");
    frame(&ctx, &mut shell, press(Key::Enter, Modifiers::NONE));

    assert!(!shell.palette().is_open(), "choosing closes the palette");
    assert_eq!(shell.notice(), None);
    let trail = titles(&shell);
    assert_eq!(trail.len(), 2, "{trail:?}");
    assert_eq!(trail[0], "Home");
}

#[test]
fn esc_with_the_palette_open_closes_it_and_keeps_the_screen() {
    let ctx = egui::Context::default();
    let mut shell = ready(&ctx, setup("palette_esc", 0));
    choose(&ctx, &mut shell, "Balance Sheet");
    assert_eq!(titles(&shell).len(), 2);

    frame(&ctx, &mut shell, press(Key::K, Modifiers::COMMAND));
    assert!(shell.palette().is_open());
    frame(&ctx, &mut shell, press(Key::Escape, Modifiers::NONE));
    assert!(!shell.palette().is_open(), "Esc closes the palette");
    assert_eq!(titles(&shell).len(), 2, "the screen under it stays");

    // With the palette closed, Esc goes back as before.
    frame(&ctx, &mut shell, press(Key::Escape, Modifiers::NONE));
    assert_eq!(titles(&shell), vec!["Home".to_owned()]);
}

#[test]
fn the_palette_lists_the_home_gst_and_import_screens() {
    let ctx = egui::Context::default();
    let mut shell = ready(&ctx, setup("palette_screens", 0));
    choose(&ctx, &mut shell, "Import from another app");
    assert_eq!(shell.notice(), None);
    assert_eq!(
        titles(&shell),
        vec!["Home".to_owned(), "Import from Tally".to_owned()]
    );
}

#[test]
fn home_keys_open_masters_and_a_receipt() {
    let ctx = egui::Context::default();
    let mut shell = ready(&ctx, setup("home_keys", 0));

    frame(&ctx, &mut shell, press(Key::M, Modifiers::NONE));
    let trail = titles(&shell);
    assert_eq!(trail.len(), 2, "{trail:?}");
    assert!(trail[1].starts_with("Masters"), "{trail:?}");

    frame(&ctx, &mut shell, press(Key::Escape, Modifiers::NONE));
    assert_eq!(titles(&shell), vec!["Home".to_owned()]);

    frame(&ctx, &mut shell, press(Key::F6, Modifiers::NONE));
    let trail = titles(&shell);
    assert_eq!(trail.len(), 2, "{trail:?}");
    assert!(trail[1].starts_with("Receipt"), "{trail:?}");
}

#[test]
fn the_cash_tile_follows_a_posting() {
    let ctx = egui::Context::default();
    let home = Rc::new(RefCell::new(GatewayScreen::new()));
    let mut shell = shell_over(setup("kpi_follow", 0), Box::new(Shared(Rc::clone(&home))));
    let cash = |home: &Rc<RefCell<GatewayScreen>>| -> Option<String> {
        let views: Vec<KpiView> = home.borrow().home().kpi_views();
        views
            .into_iter()
            .find(|v| v.label == "Cash in hand")
            .map(|v| v.value_text)
    };

    // derived: Cash Dr 1,00,000 (V1, 10 April), so the tile as of 10 April is 1,00,000.00.
    frames_until(&ctx, &mut shell, |_| {
        cash(&home).as_deref() == Some("\u{20B9}1,00,000.00")
    });
    assert_eq!(
        home.borrow().home().data().subtitle,
        "Figures as of 2026-04-10"
    );

    // V2 on 12 April: Cash Dr 5,000. derived: 1,00,000 + 5,000 = 1,05,000.00, and the default
    // period now ends on 12 April.
    let mut ticket = submit::send(
        shell.app_ctx_mut().api.as_ref().unwrap(),
        Command::PostVoucher(journal(2, date!(2026 - 04 - 12), rs(5_000))),
    );
    let start = Instant::now();
    let reply = loop {
        if let Some(reply) = submit::settle(&mut ticket) {
            break reply;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "no answer");
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(matches!(reply, CommandReply::Booked(_)), "{reply:?}");

    frames_until(&ctx, &mut shell, |_| {
        cash(&home).as_deref() == Some("\u{20B9}1,05,000.00")
    });
    assert_eq!(
        home.borrow().home().data().subtitle,
        "Figures as of 2026-04-12"
    );
}

#[test]
fn two_bank_ledgers_give_two_palette_rows_and_the_home_link_says_choose() {
    let ctx = egui::Context::default();
    let home = Rc::new(RefCell::new(GatewayScreen::new()));
    let mut shell = shell_over(setup("two_banks", 2), Box::new(Shared(Rc::clone(&home))));
    frames_until(&ctx, &mut shell, |s| s.app_ctx_mut().books.is_some());
    frame(&ctx, &mut shell, RawInput::default());

    frame(&ctx, &mut shell, press(Key::K, Modifiers::COMMAND));
    shell.palette_mut().set_query("Bank Reconciliation");
    frame(&ctx, &mut shell, RawInput::default());
    let rows: Vec<String> = shell
        .palette()
        .results()
        .iter()
        .map(|e| e.title.clone())
        .filter(|t| t.starts_with("Bank Reconciliation"))
        .collect();
    assert_eq!(
        rows,
        vec![
            "Bank Reconciliation: First Bank".to_owned(),
            "Bank Reconciliation: Second Bank".to_owned(),
        ]
    );
    frame(&ctx, &mut shell, press(Key::Escape, Modifiers::NONE));

    home.borrow_mut()
        .home_mut()
        .request_open(ledgerly_app::screens::home::HomeTarget::UnmatchedBankLines);
    frame(&ctx, &mut shell, RawInput::default());
    assert_eq!(titles(&shell), vec!["Home".to_owned()], "nothing opens");
    let notice = home.borrow().notice().map(str::to_owned).unwrap();
    assert!(notice.contains("more than one bank ledger"), "{notice}");
}

#[test]
fn one_bank_ledger_opens_from_the_palette() {
    let ctx = egui::Context::default();
    let mut shell = ready(&ctx, setup("one_bank", 1));
    choose(&ctx, &mut shell, "Bank Reconciliation");
    assert_eq!(shell.notice(), None);
    assert_eq!(
        titles(&shell),
        vec![
            "Home".to_owned(),
            "Bank reconciliation: First Bank".to_owned()
        ]
    );
}

#[test]
fn gst_returns_without_a_registration_leave_a_notice() {
    let ctx = egui::Context::default();
    let mut shell = ready(&ctx, setup("no_gstin", 0));
    frame(&ctx, &mut shell, press(Key::G, Modifiers::NONE));
    assert_eq!(titles(&shell), vec!["Home".to_owned()], "nothing opens");

    choose(&ctx, &mut shell, "GST Returns");
    assert_eq!(titles(&shell), vec!["Home".to_owned()]);
    let notice = shell.notice().unwrap();
    assert!(notice.contains("no GST registration"), "{notice}");
}

#[test]
fn the_welcome_screen_hands_over_to_home_after_opening_a_file() {
    let path = file("welcome");
    drop(Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap());
    let mut welcome = WelcomeScreen::with_clock(Clock::Fixed(OffsetDateTime::UNIX_EPOCH));
    welcome.set_open_path(path.to_string_lossy());
    welcome.start_open();
    let ctx = egui::Context::default();
    let mut shell = Shell::new(Box::new(welcome));
    frames_until(&ctx, &mut shell, |s| s.app_ctx_mut().books.is_some());
    frame(&ctx, &mut shell, RawInput::default());

    // The home screen of the open company answers M; the welcome screen does not.
    frame(&ctx, &mut shell, press(Key::M, Modifiers::NONE));
    let trail = titles(&shell);
    assert_eq!(trail.len(), 2, "{trail:?}");
    assert!(trail[1].starts_with("Masters"), "{trail:?}");
}

#[test]
fn ctrl_t_opens_home_when_a_company_is_open() {
    let ctx = egui::Context::default();
    let mut shell = ready(&ctx, setup("new_tab", 0));
    frame(&ctx, &mut shell, press(Key::T, Modifiers::COMMAND));
    assert_eq!(shell.tabs().len(), 2);
    frame(&ctx, &mut shell, RawInput::default());
    frame(&ctx, &mut shell, press(Key::M, Modifiers::NONE));
    let trail = titles(&shell);
    assert_eq!(trail.len(), 2, "{trail:?}");
    assert!(trail[1].starts_with("Masters"), "{trail:?}");
}
