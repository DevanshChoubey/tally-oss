//! Developer tool: opens the real app window on a fictional company and saves a screenshot of
//! each registered screen state, so the app can be compared with the design mock
//! (`design/mock-v2/`). `design/app-captures/capture.sh` runs it and converts the output.
//!
//! Usage: `cargo run -p ledgerly-app --example ui_snapshots -- <output dir> [--only <substring>]`
//!
//! Each entry in [`shots`] is named by the mock state id it should match (`states.json`) and a
//! theme. It writes `<output dir>/<theme>/<state-id>.ppm` (theme `light`, `dark` or `hc`); convert
//! with `ffmpeg -i in.ppm out.webp`. `--only` keeps the entries whose `<theme>/<state-id>` label
//! contains the substring. The company file is created in the output directory. All names, GSTINs
//! and amounts are fictional; the figures on screen are whatever the engine computes, and this
//! tool asserts none of them.
//!
//! Between shots the tool returns to Home: it presses Esc, one press per key cycle, until the
//! active tab's screen stack is back at its root and the palette is closed. If [`HOME_PRESSES_MAX`]
//! presses do not get there, the stack and the palette are closed directly, with a warning printed.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    reason = "developer tool, not product code: it stops on any setup failure"
)]

use std::collections::{BTreeMap, VecDeque};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use egui::{Event, Key, Modifiers};
use ledgerly_api::{Api, Command};
use ledgerly_app::shell::{GatewayScreen, ScreenStack, Shell, submit};
use ledgerly_app::theme::ThemeKind;
use ledgerly_app::worker::ApiWorker;
use ledgerly_core::{
    Clock, CompanyId, Date, Decimal, GroupId, Gstin, Hsn, LedgerId, Paise, Qty, Rate, SignedAmount,
    StateCode, StockItemId, UnitId, VoucherId, gstin_check_character,
};
use ledgerly_model::seed_godowns::MAIN_LOCATION;
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BankDetails, BaseVoucherKind, BillAllocation, BillRefKind, BillWiseConfig, Company,
    EntityKind, Features, GodownAllocation, GstClassification, GstRegistration, InventoryLine,
    Ledger, LedgerLine, NegativeStockPolicy, PartyDetails, RegistrationType, ReservedGroup,
    StockItem, TaxDetails, TaxLedgerKind, Taxability, Unit, UnitKind, ValuationMethod, Voucher,
    VoucherMode, VoucherStatus,
};
use time::OffsetDateTime;
use time::macros::date;

const CASH: LedgerId = LedgerId(1);
const CAPITAL: LedgerId = LedgerId(2);
const BANK: LedgerId = LedgerId(3);
const BUYER: LedgerId = LedgerId(4);
const OTHER_BUYER: LedgerId = LedgerId(5);
const SALES: LedgerId = LedgerId(6);
const CGST: LedgerId = LedgerId(7);
const SGST: LedgerId = LedgerId(8);
const IGST: LedgerId = LedgerId(9);
const RENT: LedgerId = LedgerId(10);
const COIL: UnitId = UnitId(1);
const NOS: UnitId = UnitId(2);

fn gstin(first14: &str) -> Gstin {
    let check = gstin_check_character(first14).unwrap();
    Gstin::parse(&format!("{first14}{check}")).unwrap()
}

fn state(code: u8) -> StateCode {
    StateCode::new(code).unwrap()
}

fn group(kind: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(kind))
        .unwrap()
        .id
}

fn ledger(id: LedgerId, name: &str, under: ReservedGroup) -> Ledger {
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
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn buyer(id: LedgerId, name: &str, first14: &str, place: u8) -> Ledger {
    let mut l = ledger(id, name, ReservedGroup::SundryDebtors);
    l.party = Some(PartyDetails {
        registration: RegistrationType::Regular,
        gstin: Some(gstin(first14)),
        pan: None,
        state: Some(state(place)),
        address: Address::default(),
    });
    l
}

fn tax(id: LedgerId, name: &str, kind: TaxLedgerKind) -> Ledger {
    let mut l = ledger(id, name, ReservedGroup::DutiesAndTaxes);
    l.tax = Some(TaxDetails {
        kind,
        rate_percent: None,
    });
    l
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

fn dr(ledger: LedgerId, rupees: i64) -> LedgerLine {
    line(ledger, SignedAmount::dr(Paise::new(rupees * 100)))
}

/// `line` opening a new bill `name` for its whole amount, due `days` after the voucher date.
fn billed(mut line: LedgerLine, name: &str, days: u32) -> LedgerLine {
    line.bills = vec![BillAllocation {
        kind: BillRefKind::New,
        name: name.to_owned(),
        due_date: None,
        credit_days: Some(days),
        amount: line.amount,
    }];
    line
}

fn cr(ledger: LedgerId, rupees: i64) -> LedgerLine {
    line(ledger, SignedAmount::cr(Paise::new(rupees * 100)))
}

fn type_of(kind: BaseVoucherKind) -> ledgerly_core::VoucherTypeId {
    default_voucher_types()
        .into_iter()
        .find(|t| t.base == kind)
        .unwrap()
        .id
}

fn voucher(id: u64, kind: BaseVoucherKind, on: Date, lines: Vec<LedgerLine>) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id: type_of(kind),
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
        ledger_lines: lines,
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn sale(id: u64, on: Date, party: LedgerId, place: u8, lines: Vec<LedgerLine>) -> Voucher {
    let mut v = voucher(id, BaseVoucherKind::Sales, on, lines);
    v.mode = VoucherMode::AccountingInvoice;
    v.party = Some(party);
    v.place_of_supply = Some(state(place));
    v
}

fn run(api: &mut Api, command: Command) {
    match api.execute(command) {
        Ok(ledgerly_core::Outcome::Value(_)) => {}
        other => panic!("setup command failed: {other:?}"),
    }
}

/// A simple unit.
fn unit(id: UnitId, symbol: &str, formal: &str) -> Unit {
    Unit {
        id,
        symbol: symbol.to_owned(),
        formal_name: formal.to_owned(),
        decimals: 0,
        kind: UnitKind::Simple,
        uqc: None,
        tally: None,
    }
}

/// A taxable stock item at `rate` percent from the start of the year, with no opening stock.
fn stock_item(id: u64, name: &str, unit: UnitId, rate: i64) -> StockItem {
    StockItem {
        ineligible_itc: None,
        id: StockItemId(id),
        name: name.to_owned(),
        aliases: Vec::new(),
        group: None,
        unit,
        alt_unit: None,
        hsn: None,
        gst_history: vec![ledgerly_model::GstHistory {
            effective_from: date!(2026 - 04 - 01),
            taxability: Taxability::Taxable,
            rate_percent: Some(Rate(Decimal::from(rate))),
        }],
        valuation: ValuationMethod::AverageCost,
        batches_enabled: false,
        opening: Vec::new(),
        standard_costs: Vec::new(),
        tally: None,
    }
}

/// A fictional Maharashtra company registered for GST, named `name`.
fn maharashtra_company(
    name: &str,
    features: Features,
    negative_stock: NegativeStockPolicy,
) -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: name.to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: state(27),
        pan: None,
        registrations: vec![GstRegistration {
            gstin: gstin("27ABCDE1234F1Z"),
            state: state(27),
            effective_from: date!(2026 - 04 - 01),
            composition: false,
        }],
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features,
        negative_stock,
    }
}

/// Creates `company` in `dir/file`, removing the file a last run left.
fn create_file(dir: &Path, file: &str, company: &Company) -> Api {
    let path = dir.join(file);
    let _removed = std::fs::remove_file(&path);
    Api::create(&path, company, Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap()
}

/// The GST-only company of [`company_file`] and [`new_company_file`].
fn traders() -> Company {
    let features = Features {
        gst: true,
        ..Features::default()
    };
    maharashtra_company("Fictional Traders", features, NegativeStockPolicy::Block)
}

/// A fictional Maharashtra trader with a bank, two GST buyers and a few November vouchers.
fn company_file(dir: &Path) -> Api {
    let mut api = create_file(dir, "snapshot-company.ledgerly", &traders());
    create_masters(&mut api);
    post_november(&mut api);
    api
}

/// Fictional Traders' masters with no voucher: a company just set up (mock `home.empty`).
fn new_company_file(dir: &Path) -> Api {
    let mut api = create_file(dir, "snapshot-new.ledgerly", &traders());
    create_masters(&mut api);
    api
}

/// Which company file a shot opens.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fixture {
    /// Fictional Traders ([`company_file`]): every shot that names no other company.
    Main,
    /// Fictional Traders' masters with no voucher yet ([`new_company_file`]).
    New,
    /// Fictional Pumps ([`stocked_company_file`]): stock sold before any was bought, an item with
    /// no HSN sold, a stored bank statement and the books locked to 31 Oct 2026.
    Stocked,
    /// No company at all: the shell exactly as the app's `main` builds it
    /// (`Shell::cold_start`), with nothing opened. This is what a user sees on first launch.
    Cold,
}

impl Fixture {
    /// Creates the company file in `dir` and opens it. Each call starts the file again. `None`
    /// for [`Fixture::Cold`], which opens nothing.
    fn open(self, dir: &Path) -> Option<Api> {
        match self {
            Fixture::Main => Some(company_file(dir)),
            Fixture::New => Some(new_company_file(dir)),
            Fixture::Stocked => Some(stocked_company_file(dir)),
            Fixture::Cold => None,
        }
    }

    /// The shell this fixture starts with and the stage the harness continues at: a company's
    /// shell waits for its books ([`Stage::Warm`]); the cold start has none to wait for.
    fn shell(self, dir: &Path) -> (Shell, Stage) {
        match self.open(dir) {
            Some(api) => {
                let mut shell = Shell::new(Box::new(GatewayScreen::new()));
                shell.app_ctx_mut().api = Some(ApiWorker::spawn(api).unwrap());
                submit::note_changed();
                (shell, Stage::Warm)
            }
            None => (Shell::cold_start(), Stage::Home),
        }
    }
}

/// A Maharashtra buyer of Fictional Pumps that is not bill-wise (the item invoice does not take
/// bill-wise parties yet).
const PUMP_BUYER: LedgerId = LedgerId(11);
/// A Gujarat buyer of Fictional Pumps, so an invoice to it is inter-state.
const GUJARAT_BUYER: LedgerId = LedgerId(12);
/// The Maharashtra supplier Fictional Pumps buys its stock from.
const SUPPLIER: LedgerId = LedgerId(13);
/// Fictional Pumps' purchase ledger.
const PURCHASES: LedgerId = LedgerId(14);

/// A statement for Example Bank in the generic split-column CSV layout: the capital paid in on
/// 15 Oct and the rent paid on 20 Nov (both in the books, so the matcher suggests them) and a bank
/// charge on 21 Nov that is not. Each running balance is the one before plus or minus the line.
const STOCKED_STATEMENT: &str = "Date,Value Date,Narration,Ref,Withdrawal,Deposit,Balance\n\
15/10/2026,15/10/2026,CAPITAL INTRODUCED,R1,,500000.00,500000.00\n\
20/11/2026,20/11/2026,NEFT RENT NOVEMBER,R2,25000.00,,475000.00\n\
21/11/2026,21/11/2026,BANK CHARGES,R3,118.00,,474882.00\n";

/// An item invoice line of `qty` units of `item` at `rate` rupees each, in Main Location, posted
/// to `account` (the sales or purchase ledger).
fn item_line(
    item: StockItemId,
    unit: UnitId,
    qty: i64,
    rate: i64,
    account: LedgerId,
) -> InventoryLine {
    let amount = Paise::new(qty * rate * 100);
    InventoryLine {
        item,
        qty: Qty(Decimal::from(qty)),
        unit,
        rate: Rate(Decimal::from(rate)),
        discount_percent: None,
        amount,
        ledger: Some(account),
        godowns: vec![GodownAllocation {
            godown: MAIN_LOCATION,
            batch: None,
            qty: Qty(Decimal::from(qty)),
            amount,
        }],
    }
}

/// An item invoice of `kind` (sales or purchase) with `party`, with the ledger lines and tax the
/// engine works out for it.
fn item_invoice(
    api: &Api,
    id: u64,
    kind: BaseVoucherKind,
    on: Date,
    (party, place): (LedgerId, u8),
    lines: Vec<InventoryLine>,
) -> Voucher {
    let mut v = voucher(id, kind, on, Vec::new());
    v.mode = VoucherMode::ItemInvoice;
    v.party = Some(party);
    v.place_of_supply = Some(state(place));
    v.inventory_lines = lines;
    match api.invoice_gst(&v) {
        Ok(ledgerly_core::Outcome::Value(built)) => {
            v.ledger_lines = built.ledger_lines;
            v.gst = Some(built.stored);
        }
        other => panic!("the engine did not build the invoice: {other:?}"),
    }
    v
}

/// A fictional Maharashtra pump dealer with inventory: the masters of [`create_masters`], two
/// buyers that are not bill-wise, a supplier, capital paid in on 15 Oct, pipe coils sold on
/// 10 Nov and pickaxes sold to Gujarat on 12 Nov before any were bought, both bought on 16 Nov
/// (stock went negative during the period and closes positive, so closing stock is provisional;
/// the pickaxe has no HSN, so GSTR-1 holds its sale for correction), rent paid on 20 Nov, a
/// stored bank statement, and the books locked to 31 Oct 2026 last.
fn stocked_company_file(dir: &Path) -> Api {
    let features = Features {
        gst: true,
        inventory: true,
        ..Features::default()
    };
    let company = maharashtra_company("Fictional Pumps", features, NegativeStockPolicy::Allow);
    let mut api = create_file(dir, "snapshot-stocked.ledgerly", &company);
    create_masters(&mut api);
    for l in [
        buyer(PUMP_BUYER, "Patil Agro Stores", "27PQRST6789U1Z", 27),
        buyer(GUJARAT_BUYER, "Narmada Farm Supplies", "24KLMNO2345P1Z", 24),
        {
            let mut supplier = buyer(SUPPLIER, "Deccan Polymers", "27DEFGH4321J1Z", 27);
            supplier.group = group(ReservedGroup::SundryCreditors);
            supplier
        },
        ledger(PURCHASES, "Purchases", ReservedGroup::PurchaseAccounts),
    ] {
        run(&mut api, Command::CreateLedger(l));
    }
    run(
        &mut api,
        Command::PostVoucher(voucher(
            1,
            BaseVoucherKind::Journal,
            date!(2026 - 10 - 15),
            vec![dr(BANK, 500_000), cr(CAPITAL, 500_000)],
        )),
    );
    let (pipe, pickaxe) = (StockItemId(1), StockItemId(2));
    let sales = BaseVoucherKind::Sales;
    for v in [
        (
            2,
            date!(2026 - 11 - 10),
            (PUMP_BUYER, 27),
            pipe,
            COIL,
            10,
            1_500,
        ),
        (
            3,
            date!(2026 - 11 - 12),
            (GUJARAT_BUYER, 24),
            pickaxe,
            NOS,
            4,
            800,
        ),
    ]
    .map(|(id, on, party, item, unit, qty, rate)| {
        let line = item_line(item, unit, qty, rate, SALES);
        item_invoice(&api, id, sales, on, party, vec![line])
    }) {
        run(&mut api, Command::PostVoucher(v));
    }
    let bought = item_invoice(
        &api,
        4,
        BaseVoucherKind::Purchase,
        date!(2026 - 11 - 16),
        (SUPPLIER, 27),
        vec![
            item_line(pipe, COIL, 15, 1_000, PURCHASES),
            item_line(pickaxe, NOS, 10, 500, PURCHASES),
        ],
    );
    run(&mut api, Command::PostVoucher(bought));
    run(
        &mut api,
        Command::PostVoucher(voucher(
            5,
            BaseVoucherKind::Payment,
            date!(2026 - 11 - 20),
            vec![dr(RENT, 25_000), cr(BANK, 25_000)],
        )),
    );
    match api.import_bank_statement(STOCKED_STATEMENT.as_bytes(), "generic-csv-split", BANK) {
        Ok(ledgerly_core::Outcome::Value(_)) => {}
        other => panic!("the statement was not imported: {other:?}"),
    }
    run(&mut api, Command::LockPeriod(date!(2026 - 10 - 31)));
    api
}

/// The ledgers (ids 1 to 10), units and stock items every capture company starts with.
fn create_masters(api: &mut Api) {
    let mut bank = ledger(
        BANK,
        "Example Bank Current A/c",
        ReservedGroup::BankAccounts,
    );
    bank.bank = Some(BankDetails {
        account_number: "000123456789".to_owned(),
        ifsc: "EXMP0000001".to_owned(),
        bank_name: "Example Bank".to_owned(),
        branch: "Main".to_owned(),
    });
    let mut sales = ledger(SALES, "Sales", ReservedGroup::SalesAccounts);
    sales.gst = Some(GstClassification {
        hsn: Some(Hsn::parse("998313").unwrap()),
        taxability: Taxability::Taxable,
        rate_percent: Some(Rate(Decimal::from(18))),
    });
    // Sample Buyer is bill-wise with 30 credit days, so its detail has an Open bills card.
    let mut sample_buyer = buyer(BUYER, "Sample Buyer Pvt Ltd", "27FGHIJ5678K1Z", 27);
    sample_buyer.bill_wise = Some(BillWiseConfig {
        default_credit_days: Some(30),
    });
    for l in [
        ledger(CASH, "Cash", ReservedGroup::CashInHand),
        ledger(CAPITAL, "Capital", ReservedGroup::CapitalAccount),
        bank,
        sample_buyer,
        buyer(OTHER_BUYER, "Other State Buyer", "29RSTUV3456W1Z", 29),
        sales,
        tax(CGST, "Output CGST", TaxLedgerKind::Cgst),
        tax(SGST, "Output SGST", TaxLedgerKind::Sgst),
        tax(IGST, "Output IGST", TaxLedgerKind::Igst),
        ledger(RENT, "Rent", ReservedGroup::IndirectExpenses),
    ] {
        run(api, Command::CreateLedger(l));
    }
    // Two stock items with no opening stock and no movement, so no other screen's figures move:
    // the items list shows the engine's 0 quantities and 0.00 values, one item without an HSN.
    for u in [unit(COIL, "coil", "Coils"), unit(NOS, "nos", "Numbers")] {
        run(api, Command::CreateUnit(u));
    }
    let mut pipe = stock_item(1, "HDPE pipe coil 32 mm", COIL, 18);
    pipe.hsn = Some(Hsn::parse("3917").unwrap());
    for it in [pipe, stock_item(2, "Steel pickaxe 2.5 kg", NOS, 5)] {
        run(api, Command::CreateStockItem(it));
    }
}

/// Fictional Traders' November vouchers.
fn post_november(api: &mut Api) {
    let journal = BaseVoucherKind::Journal;
    for v in [
        voucher(
            1,
            journal,
            date!(2026 - 11 - 02),
            vec![dr(BANK, 500_000), cr(CAPITAL, 500_000)],
        ),
        voucher(
            2,
            journal,
            date!(2026 - 11 - 03),
            vec![dr(CASH, 25_000), cr(BANK, 25_000)],
        ),
        sale(
            3,
            date!(2026 - 11 - 10),
            BUYER,
            27,
            vec![
                billed(dr(BUYER, 118_000), "SI-3", 30),
                cr(SALES, 100_000),
                cr(CGST, 9_000),
                cr(SGST, 9_000),
            ],
        ),
        sale(
            4,
            date!(2026 - 11 - 18),
            OTHER_BUYER,
            29,
            vec![dr(OTHER_BUYER, 59_000), cr(SALES, 50_000), cr(IGST, 9_000)],
        ),
        voucher(
            5,
            BaseVoucherKind::Payment,
            date!(2026 - 11 - 20),
            vec![dr(RENT, 25_000), cr(BANK, 25_000)],
        ),
    ] {
        run(api, Command::PostVoucher(v));
    }
}

/// A colour theme a shot is taken in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Theme {
    Light,
    Dark,
    Hc,
    /// The audit set (`audit_shots`): light theme, written to `<out>/audit/`. Its states have no
    /// mock state of the same id, so `design/compare.mjs` leaves the folder alone.
    Audit,
}

impl Theme {
    /// The shell's theme for this entry.
    fn kind(self) -> ThemeKind {
        match self {
            Theme::Light | Theme::Audit => ThemeKind::Light,
            Theme::Dark => ThemeKind::Dark,
            Theme::Hc => ThemeKind::HighContrast,
        }
    }

    /// The directory name of the theme, the same as in `design/mock-v2/captures/states/`.
    fn dir(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
            Theme::Hc => "hc",
            Theme::Audit => "audit",
        }
    }
}

/// One screenshot: the mock state it matches, its theme, and the keys that reach it. The keys go in
/// after the return to Home, then an optional palette query, then the keys after the query.
#[derive(Clone)]
struct Shot {
    /// The mock state id (`design/mock-v2/states.json`), which also names the image.
    state: &'static str,
    theme: Theme,
    keys: Vec<(Key, Modifiers)>,
    query: Option<&'static str>,
    after: Vec<Inject>,
    /// The company file the shot is taken on.
    company: Fixture,
}

/// Frames the harness waits after a wheel scroll before its next input.
const SCROLL_SETTLE_FRAMES: u32 = 90;

/// Frames the `setup.done` shot waits after its keys, so the background company file job can
/// finish.
const SETUP_SETTLE_FRAMES: u32 = 900;

/// Frames the `sales-invoice.saved` shot waits after Ctrl+A: the post goes through the worker and
/// the screen says "Saved." only when it answers (60 frames showed "Saving...").
const SAVE_SETTLE_FRAMES: u32 = 900;

/// One thing the harness sends after a shot's setup: a key press or typed text (the company setup
/// wizard's name and GSTIN fields need text).
#[derive(Clone, Copy)]
enum Inject {
    /// A key press (down and up).
    Key(Key, Modifiers),
    /// Text typed into the focused field.
    Text(&'static str),
    /// A primary click at a point of the 1280 x 800 window, for a widget no key reaches (a combo
    /// box's list, a checkbox the form gives no key to).
    Click(f32, f32),
    /// The mouse wheel turned at a point by the given egui wheel delta in points: negative scrolls
    /// the list under it down, positive up.
    Scroll(f32, f32, f32),
}

impl Shot {
    /// The `<theme>/<state-id>` label that `--only` matches against.
    fn label(&self) -> String {
        format!("{}/{}", self.theme.dir(), self.state)
    }

    /// The same shot in another theme.
    fn themed(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// The same shot with a palette query typed after its keys.
    fn query(mut self, query: &'static str) -> Self {
        self.query = Some(query);
        self
    }

    /// The same shot with keys pressed after the query.
    fn then(mut self, keys: &[(Key, Modifiers)]) -> Self {
        self.after = keys.iter().map(|(k, m)| Inject::Key(*k, *m)).collect();
        self
    }

    /// The same shot with `text` typed after the keys of [`Shot::then`].
    fn type_text(mut self, text: &'static str) -> Self {
        self.after.push(Inject::Text(text));
        self
    }

    /// The same shot with a click at (`x`, `y`) after what came before it.
    fn click(mut self, x: f32, y: f32) -> Self {
        self.after.push(Inject::Click(x, y));
        self
    }

    /// The same shot with the wheel turned at (`x`, `y`) by `wheel` points (negative scrolls down).
    fn scroll(mut self, x: f32, y: f32, wheel: f32) -> Self {
        self.after.push(Inject::Scroll(x, y, wheel));
        self
    }

    /// The same shot taken on another company file.
    fn on(mut self, company: Fixture) -> Self {
        self.company = company;
        self
    }

    /// The same shot with `keys` pressed after what came before them.
    fn press(mut self, keys: &[(Key, Modifiers)]) -> Self {
        self.after
            .extend(keys.iter().map(|(k, m)| Inject::Key(*k, *m)));
        self
    }
}

fn shot(state: &'static str, keys: &[(Key, Modifiers)]) -> Shot {
    Shot {
        state,
        theme: Theme::Light,
        keys: keys.to_vec(),
        query: None,
        after: Vec::new(),
        company: Fixture::Main,
    }
}

const NONE: Modifiers = Modifiers::NONE;
const ESC: (Key, Modifiers) = (Key::Escape, Modifiers::NONE);

/// The most Escape presses one return to Home may take before the screens are closed directly.
const HOME_PRESSES_MAX: u32 = 12;

/// From Home to the balance sheet's Sundry Debtors drill: B, three lines down to Current Assets,
/// Enter, two lines down to Sundry Debtors, Enter.
fn to_debtors() -> Vec<(Key, Modifiers)> {
    vec![
        (Key::B, NONE),
        (Key::J, NONE),
        (Key::J, NONE),
        (Key::J, NONE),
        (Key::Enter, NONE),
        (Key::J, NONE),
        (Key::J, NONE),
        (Key::Enter, NONE),
    ]
}

/// The company name the setup captures type (the mock's sample company, index.html:489).
const SETUP_NAME: &str = "Godavari Drip & Pump Works";
/// The GSTIN the setup captures type; its first two digits fill the state (index.html:1868).
const SETUP_GSTIN: &str = "27AAGFG4417K1ZA";
/// Where the setup captures create their company file, relative to the repository root. The
/// harness removes it before a run, because the wizard does not overwrite a file.
const SETUP_FILE: &str = "target/setup-capture/godavari.ledgerly";

/// A company setup capture, as a first-time user reaches it: the cold start (no company open) shows
/// the wizard, Enter goes from Start to Company (where the name field takes focus by itself), the
/// company form is filled by typing (Tab moves focus between its fields), then `after_company`
/// Enters move on from the company step.
fn setup_shot(state: &'static str, after_company: usize) -> Shot {
    let mut shot = shot(state, &[])
        .on(Fixture::Cold)
        .then(&[(Key::Enter, NONE)])
        .type_text(SETUP_NAME)
        .press(&[(Key::Tab, NONE), (Key::Tab, NONE), (Key::Tab, NONE)])
        .type_text(SETUP_GSTIN)
        .press(&[(Key::Tab, NONE), (Key::Tab, NONE), (Key::Tab, NONE)])
        .type_text(SETUP_FILE)
        .press(&[(Key::Tab, NONE), (Key::Tab, NONE)]);
    for _ in 0..after_company {
        shot = shot.press(&[(Key::Enter, NONE)]);
    }
    shot
}

/// A capture of `screen` with AI off for the company: the palette opens AI and privacy, 1 stores
/// Off, Esc goes back to Home, and the palette opens `screen` by its title.
fn ai_off_shot(state: &'static str, screen: &'static str) -> Shot {
    let palette = (Key::K, Modifiers::COMMAND);
    shot(state, &[palette])
        .query("AI and privacy")
        .then(&[(Key::Enter, NONE), (Key::Num1, NONE), ESC, palette])
        .type_text(screen)
        .press(&[(Key::Enter, NONE)])
}

/// An item invoice on Fictional Pumps (F8 opens one, since the company keeps inventory) with
/// `party` picked, the place of supply typed as a state code (the field is not filled from the
/// party), Sales as the sales ledger and the reference left empty, so the keyboard is on the
/// first item.
fn pumps_invoice(state: &'static str, party: &'static str, place: &'static str) -> Shot {
    let enter = (Key::Enter, NONE);
    shot(state, &[(Key::F8, NONE)])
        .on(Fixture::Stocked)
        .type_text(party)
        .press(&[enter])
        .type_text(place)
        .press(&[enter])
        .type_text("Sales")
        .press(&[enter, enter])
}

/// [`pumps_invoice`] with one line of `item`, `qty` and `rate` entered and the discount left
/// empty, so the keyboard is on the item of a second line.
fn pumps_invoice_line(
    state: &'static str,
    (party, place): (&'static str, &'static str),
    item: &'static str,
    qty: &'static str,
    rate: &'static str,
) -> Shot {
    let enter = (Key::Enter, NONE);
    pumps_invoice(state, party, place)
        .type_text(item)
        .press(&[enter])
        .type_text(qty)
        .press(&[enter])
        .type_text(rate)
        .press(&[enter, enter])
}

/// The audit set: what a user reaches from the real cold start (no company open) by the sidebar's
/// entries, the keys and the palette, plus the first-run states with no mock state. Written to
/// `<out>/audit/<id>.ppm` with `--audit`; `design/REAL-PATH-AUDIT.md` judges each image.
fn audit_shots() -> Vec<Shot> {
    let palette = (Key::K, Modifiers::COMMAND);
    let enter = (Key::Enter, NONE);
    let cold = |id: &'static str, keys: &[(Key, Modifiers)]| {
        let mut s = shot(id, keys).on(Fixture::Cold);
        s.theme = Theme::Audit;
        s
    };
    let by_palette =
        |id: &'static str, query: &'static str| cold(id, &[palette]).query(query).then(&[enter]);
    vec![
        cold("cold.palette", &[palette]),
        cold("cold.start-open-selected", &[]).then(&[(Key::ArrowDown, NONE)]),
        cold("cold.rail", &[(Key::Backslash, Modifiers::COMMAND)]),
        cold("cold.new-tab", &[(Key::T, Modifiers::COMMAND)]),
        cold("cold.keys-help", &[(Key::Questionmark, NONE)]),
        cold("cold.sales-f8", &[(Key::F8, NONE)]),
        cold("cold.purchase-f9", &[(Key::F9, NONE)]),
        cold("cold.payment-f5", &[(Key::F5, NONE)]),
        cold("cold.receipt-f6", &[(Key::F6, NONE)]),
        cold("cold.masters-m", &[(Key::M, NONE)]),
        cold("cold.day-book-d", &[(Key::D, NONE)]),
        cold("cold.balance-sheet-b", &[(Key::B, NONE)]),
        cold("cold.gst-g", &[(Key::G, NONE)]),
        by_palette("cold.home", "Home"),
        by_palette("cold.review", "Review queue"),
        by_palette("cold.masters", "Masters"),
        by_palette("cold.quick-entry", "Plain-words entry"),
        by_palette("cold.ask", "Ask your books"),
        by_palette("cold.purchase-bill", "Purchase from bill photo"),
        by_palette("cold.import", "Import from another app"),
        by_palette("cold.bank", "Bank reconciliation"),
        by_palette("cold.ai-settings", "AI and privacy"),
        by_palette("cold.contra", "Contra"),
        by_palette("cold.journal", "Journal"),
    ]
}

/// The registry: every light shot of [`light_shots`], then a dark and a high-contrast copy of each
/// one the mock has a capture of in that theme (`design/mock-v2/captures/states/<theme>/`).
fn shots() -> Vec<Shot> {
    let light = light_shots();
    let mock = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../design/mock-v2/captures/states");
    let mut all = light.clone();
    for theme in [Theme::Dark, Theme::Hc] {
        all.extend(
            light
                .iter()
                .filter(|s| {
                    mock.join(theme.dir())
                        .join(format!("{}.webp", s.state))
                        .is_file()
                })
                .map(|s| s.clone().themed(theme)),
        );
    }
    all
}

/// The light registry: one entry per image, named by the mock state id it should match.
fn light_shots() -> Vec<Shot> {
    let palette = (Key::K, Modifiers::COMMAND);
    vec![
        shot("home.default", &[]),
        // Recent needs a choice first: open the balance sheet from the palette, go back, and
        // reopen the palette on the empty query (index.html:1115 lists recents there).
        shot("palette.default", &[palette])
            .query("Balance sheet")
            .then(&[(Key::Enter, NONE), ESC, palette]),
        shot("palette.search", &[palette]).query("bal"),
        shot("palette.voucher", &[palette]).query("#0412"),
        shot("palette.ask", &[palette]).query("?who owes us"),
        shot("palette.entry", &[palette]).query("+paid rent"),
        shot("palette.commands", &[palette]).query(">"),
        // The Review queue opens from the palette ("Review queue" is a screen row there).
        shot("review.empty", &[palette])
            .query("Review queue")
            .then(&[(Key::Enter, NONE)]),
        shot("palette.no-results", &[palette]).query("zzqx"),
        shot("balance-sheet.default", &[(Key::B, NONE)]),
        shot(
            "balance-sheet.detailed",
            &[(Key::B, NONE), (Key::F1, Modifiers::ALT)],
        ),
        // The mock drills from the balance sheet into Sundry debtors (index.html:1594), then into
        // its first ledger (index.html:1600). Current Assets is the fourth line of the capture
        // company's condensed statement and Sundry Debtors the third line of its summary.
        shot("balance-sheet.group", &to_debtors()),
        shot("balance-sheet.ledger", &to_debtors()).then(&[(Key::Enter, NONE)]),
        shot("day-book.default", &[(Key::D, NONE)]),
        shot("gst.gstr1", &[(Key::G, NONE), (Key::Num1, NONE)]),
        // AI and privacy is reached from the palette (index.html:1920 keys: 1 Off, 2 On this
        // computer, 3 Cloud). The capture company has no stored tier, so each state presses its key.
        shot("ai-settings.local", &[palette])
            .query("AI and privacy")
            .then(&[(Key::Enter, NONE), (Key::Num2, NONE)]),
        shot("ai-settings.off", &[palette])
            .query("AI and privacy")
            .then(&[(Key::Enter, NONE), (Key::Num1, NONE)]),
        shot("ai-settings.cloud", &[palette])
            .query("AI and privacy")
            .then(&[(Key::Enter, NONE), (Key::Num3, NONE)]),
        // The AI-off states of the assistant screens: AI and privacy stores Off (key 1, as
        // ai-settings.off), Esc goes back, and the palette opens the screen. The capture company
        // has no stored tier, so each state sets it first. quick-entry has no AI-off state in the
        // mock; its capture shows the screen as built (PARITY-GAPS).
        ai_off_shot("ask.offline-ai", "Ask your books"),
        ai_off_shot("purchase-bill.offline-ai", "Purchase from bill photo"),
        ai_off_shot("quick-entry.offline-ai", "Plain-words entry"),
        // The tabs are 1 GSTR-1, 2 GSTR-3B, 3 reconciliation (`screens/gst.rs`). The capture
        // company has no 2B data, so the reconciliation tab is the mock's empty state.
        shot("gst.gstr3b", &[(Key::G, NONE), (Key::Num2, NONE)]),
        shot("gst.recon-empty", &[(Key::G, NONE), (Key::Num3, NONE)]),
        shot("masters.ledgers", &[(Key::M, NONE)]),
        // The mock's party detail is the alter form of a bill-wise party ledger (index.html:1803).
        // The cursor starts on the first row; Sample Buyer Pvt Ltd is the tenth ledger of the
        // capture company, so nine steps down, then Ctrl+Enter alters it.
        shot(
            "masters.ledger-detail",
            &[
                (Key::M, NONE),
                (Key::J, NONE),
                (Key::J, NONE),
                (Key::J, NONE),
                (Key::J, NONE),
                (Key::J, NONE),
                (Key::J, NONE),
                (Key::J, NONE),
                (Key::J, NONE),
                (Key::J, NONE),
                (Key::Enter, Modifiers::COMMAND),
            ],
        ),
        // Alt+Right walks the kinds from Ledger; the fourth step is Stock item.
        shot(
            "masters.items",
            &[
                (Key::M, NONE),
                (Key::ArrowRight, Modifiers::ALT),
                (Key::ArrowRight, Modifiers::ALT),
                (Key::ArrowRight, Modifiers::ALT),
                (Key::ArrowRight, Modifiers::ALT),
            ],
        ),
        shot(
            "masters.create-ledger",
            &[(Key::M, NONE), (Key::C, Modifiers::ALT)],
        ),
        shot("keys.classic", &[palette])
            .query("Keyboard map")
            .then(&[(Key::Enter, NONE)]),
        shot("keys.standard", &[palette])
            .query("Keyboard map")
            .then(&[(Key::Enter, NONE), (Key::Num2, NONE)]),
        // A filled invoice to a Maharashtra buyer, as the mock's default shows.
        pumps_invoice_line(
            "sales-invoice.default",
            ("Patil", "27"),
            "HDPE",
            "10",
            "1500",
        ),
        shot("receipt.default", &[(Key::F6, NONE)]),
        shot("payment.default", &[(Key::F5, NONE)]),
        shot("bank.default", &[palette])
            .query("Bank Reconciliation")
            .then(&[(Key::Enter, NONE)]),
        shot("bank.brs", &[palette])
            .query("Bank Reconciliation")
            .then(&[(Key::Enter, NONE), (Key::B, NONE)]),
        // Company setup (mock SCREENS.setup). The palette opens the wizard at Start; Enter goes on.
        // The true first launch: the shell as `main` builds it (`Shell::cold_start`), no company.
        shot("setup.welcome", &[]).on(Fixture::Cold),
        setup_shot("setup.company", 0),
        setup_shot("setup.features", 1),
        setup_shot("setup.keyboard", 2),
        setup_shot("setup.ai", 3),
        setup_shot("setup.backup", 4),
        setup_shot("setup.done", 5),
        // The capture company has no imported statement, so the bank screen shows its empty state
        // (index.html:434 "No statement imported for this account").
        shot("bank.empty", &[palette])
            .query("Bank Reconciliation")
            .then(&[(Key::Enter, NONE)]),
        // F2 on the day book asks for one date; Ctrl+A selects the prefilled day so the typed one
        // replaces it. The capture company has no voucher on 21 Nov (index.html:1641 uses 21 Nov).
        shot("day-book.empty", &[(Key::D, NONE), (Key::F2, NONE)])
            .then(&[SELECT_ALL])
            .type_text("21-11-2026")
            .press(&[(Key::Enter, NONE)]),
        // The month tab has no key; Alt+F2 asks for a period, and 1 Nov to the shown day is the
        // month tab's period (`day_book::tab_of`).
        shot(
            "day-book.month",
            &[(Key::D, NONE), (Key::F2, Modifiers::ALT)],
        )
        .then(&[SELECT_ALL])
        .type_text("01-11-2026")
        .press(&[(Key::Tab, NONE), SELECT_ALL])
        .type_text("20-11-2026")
        .press(&[(Key::Enter, NONE)]),
        // From the ledger drill (balance-sheet.ledger), Enter opens the voucher on the cursor row.
        shot("balance-sheet.voucher", &to_debtors()).then(&[
            (Key::Enter, NONE),
            (Key::J, NONE),
            (Key::Enter, NONE),
        ]),
        // AI and privacy stores Off (key 1, as ai-settings.off) and Esc goes back to Home.
        shot("home.ai-off", &[palette])
            .query("AI and privacy")
            .then(&[(Key::Enter, NONE), (Key::Num1, NONE), ESC]),
        // The create-ledger form (masters.create-ledger) filled by clicks, since no key opens the
        // Under list or ticks its boxes: the mock's name, Under = Sundry Debtors (the list scrolled
        // to its end, where the scroll stops, so the row is always at the same place), Party details
        // on, the mock's GSTIN with a wrong check character (index.html:1790: the check character of
        // 27AAJFJ5521N1Z is Z, so the mock's bad GSTIN ends in 7), Bill-wise on and 30 credit days.
        shot(
            "masters.gstin-error",
            &[(Key::M, NONE), (Key::C, Modifiers::ALT)],
        )
        .click(456.0, 263.0)
        .type_text("Jadhav Irrigation Services")
        .click(995.0, 256.0)
        .scroll(995.0, 400.0, -400.0)
        .click(958.0, 492.0)
        .click(472.0, 389.0)
        .click(456.0, 326.0)
        .type_text("27AAJFJ5521N1Z7")
        .click(903.0, 326.0)
        .click(348.0, 413.0)
        .type_text("30")
        .click(456.0, 326.0),
        // The company step as setup.company, then the State list opened by a click (no key opens
        // it) and the mock's 24 Gujarat chosen against the GSTIN's 27. The list is scrolled to its
        // end (where the scroll stops) and back up, so 24 is always at the same row.
        setup_shot("setup.gstin-error", 0)
            .click(950.0, 286.0)
            .scroll(950.0, 400.0, -400.0)
            .scroll(950.0, 400.0, -400.0)
            .scroll(950.0, 400.0, 207.0)
            .click(930.0, 337.0),
        // A company with masters and no voucher: Home as it opens (index.html home.empty).
        shot("home.empty", &[]).on(Fixture::New),
        // Fictional Pumps sold pipe coils it never bought, so closing stock is provisional.
        shot("balance-sheet.provisional", &[(Key::B, NONE)]).on(Fixture::Stocked),
        // Fictional Pumps sold pickaxes, which have no HSN, so GSTR-1 holds them for correction.
        shot("gst.gstr1-correction", &[(Key::G, NONE), (Key::Num1, NONE)]).on(Fixture::Stocked),
        // Fictional Pumps has a stored statement whose lines match two vouchers.
        shot("review.default", &[palette])
            .on(Fixture::Stocked)
            .query("Review queue")
            .then(&[(Key::Enter, NONE)]),
        // The item list open on the first line, filtered by what is typed (index.html: type-ahead
        // over item masters).
        pumps_invoice("sales-invoice.item-list", "Narmada", "24").type_text("p"),
        // A Gujarat party: the place of supply is 24, so the engine charges IGST.
        pumps_invoice_line(
            "sales-invoice.gst-inter-state",
            ("Narmada", "24"),
            "HDPE",
            "10",
            "1500",
        ),
        // A cash sale: Cash has no GSTIN, so the sale is B2C.
        pumps_invoice_line("sales-invoice.b2c", ("Cash", "27"), "HDPE", "2", "1500"),
        // An item picked with no quantity, then Ctrl+A: the save is refused with the reason.
        pumps_invoice("sales-invoice.validation-error", "Narmada", "24")
            .type_text("HDPE")
            .press(&[(Key::Enter, NONE), (Key::A, Modifiers::COMMAND)]),
        // A click puts the keyboard in the date (Ctrl+A is Save on this screen, so the date is
        // cleared with End and Backspace), and 31 Oct is the lock date of Fictional Pumps
        // (index.html sales-invoice.period-locked). Enter then goes on to the customer.
        shot("sales-invoice.period-locked", &[(Key::F8, NONE)])
            .on(Fixture::Stocked)
            .click(360.0, 215.0)
            .press(&[(Key::End, NONE)])
            .press(&[(Key::Backspace, NONE); 12])
            .type_text("31-10-2026")
            .press(&[(Key::Enter, NONE)])
            .type_text("Narmada")
            .press(&[(Key::Enter, NONE)])
            .type_text("24")
            .press(&[(Key::Enter, NONE)])
            .type_text("Sales")
            .press(&[(Key::Enter, NONE), (Key::Enter, NONE)])
            .type_text("HDPE")
            .press(&[(Key::Enter, NONE)])
            .type_text("10")
            .press(&[(Key::Enter, NONE)])
            .type_text("1500")
            .press(&[(Key::Enter, NONE), (Key::Enter, NONE)]),
        // A blank second line ends the lines; Enter on the narration asks to save.
        pumps_invoice_line(
            "sales-invoice.save-confirm",
            ("Narmada", "24"),
            "HDPE",
            "10",
            "1500",
        )
        .press(&[(Key::Enter, NONE), (Key::Enter, NONE)]),
        // Yes on the save question: the invoice is posted and a fresh one opens (index.html
        // sales-invoice.saved).
        pumps_invoice_line(
            "sales-invoice.saved",
            ("Narmada", "24"),
            "HDPE",
            "10",
            "1500",
        )
        .press(&[(Key::Enter, NONE), (Key::Enter, NONE), (Key::Enter, NONE)]),
    ]
}

/// Ctrl+A: in a text field, selects all of its text.
const SELECT_ALL: (Key, Modifiers) = (Key::A, Modifiers::COMMAND);

/// Where the snapshot run is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// Waiting for the books and the first figures.
    Warm,
    /// Escape goes in, one press per key cycle, until the screen is back at Home.
    Home,
    /// Keys of the current shot go in on the next frame.
    Keys,
    /// The palette query goes in.
    Query,
    /// The keys after the query go in.
    After,
    /// Letting the screen load from the worker.
    Settle,
    /// A screenshot was asked for.
    Capture,
    /// All shots saved.
    Done,
}

struct Snapshots {
    shell: Shell,
    out: PathBuf,
    shots: Vec<Shot>,
    index: usize,
    stage: Stage,
    frames: u32,
    /// Keys still to press, one per frame with a gap, so the screen reacts to each before the next.
    inject: VecDeque<Inject>,
    /// Frames to wait before the next queued key.
    gap: u32,
    /// Escape presses so far in the current return to Home.
    presses: u32,
    image: Arc<Mutex<Option<Arc<egui::ColorImage>>>>,
    /// The company file open in the shell.
    company: Fixture,
    /// The shell has not been driven since it was built. A cold-start shot needs this, since its
    /// keys move the wizard on and a shell remembers where it was.
    fresh: bool,
}

impl Snapshots {
    /// Closes the open company and opens `company` on a new shell, as the start of the run does.
    fn reopen(&mut self, company: Fixture) -> Stage {
        let (shell, stage) = company.shell(&self.out);
        self.shell = shell;
        self.company = company;
        self.fresh = true;
        stage
    }

    fn next_stage(&mut self, stage: Stage) {
        self.stage = stage;
        self.frames = 0;
    }

    /// True when the active tab's stack is at its root and the palette is closed.
    fn at_home(&self) -> bool {
        !self.shell.palette().is_open()
            && self
                .shell
                .tabs()
                .active_stack()
                .is_some_and(|stack| stack.depth() == 1)
    }

    fn write_shot(&self, image: &egui::ColorImage) {
        let current = &self.shots[self.index];
        let dir = self.out.join(current.theme.dir());
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{}.ppm", current.state));
        let [w, h] = image.size;
        let mut file = std::fs::File::create(&path).unwrap();
        write!(file, "P6\n{w} {h}\n255\n").unwrap();
        let mut bytes = Vec::with_capacity(w * h * 3);
        for px in &image.pixels {
            bytes.extend_from_slice(&[px.r(), px.g(), px.b()]);
        }
        file.write_all(&bytes).unwrap();
        println!("saved {}", path.display());
    }
}

impl eframe::App for Snapshots {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if self.gap > 0 {
            self.gap -= 1;
            return;
        }
        if let Some(item) = self.inject.pop_front() {
            // A wheel scroll is animated over several frames; the next click waits for it to end.
            self.gap = if matches!(item, Inject::Scroll(..)) {
                SCROLL_SETTLE_FRAMES
            } else {
                6
            };
            match item {
                Inject::Key(key, modifiers) => {
                    for pressed in [true, false] {
                        raw_input.events.push(Event::Key {
                            key,
                            physical_key: None,
                            pressed,
                            repeat: false,
                            modifiers,
                        });
                    }
                }
                Inject::Text(text) => raw_input.events.push(Event::Text(text.to_owned())),
                Inject::Scroll(x, y, wheel) => {
                    raw_input.events.push(Event::PointerMoved(egui::pos2(x, y)));
                    raw_input.events.push(Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, wheel),
                        phase: egui::TouchPhase::Move,
                        source: egui::MouseWheelSource::Unknown,
                        modifiers: Modifiers::NONE,
                    });
                }
                Inject::Click(x, y) => {
                    let pos = egui::pos2(x, y);
                    raw_input.events.push(Event::PointerMoved(pos));
                    for pressed in [true, false] {
                        raw_input.events.push(Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Modifiers::NONE,
                        });
                    }
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.shell.show(ui);
        ui.ctx().request_repaint();
        self.frames += 1;
        match self.stage {
            Stage::Warm => {
                if self.shell.app_ctx_mut().books.is_some() && self.frames > 90 {
                    self.next_stage(Stage::Home);
                }
            }
            Stage::Home => {
                // One Escape per key cycle: a key goes in only once the last one has been
                // handled (inject empty, gap spent), so the check below sees its effect.
                if self.inject.is_empty() && self.gap == 0 {
                    if self.at_home() {
                        self.next_stage(Stage::Keys);
                    } else if self.presses < HOME_PRESSES_MAX {
                        self.presses += 1;
                        self.inject.push_back(Inject::Key(ESC.0, ESC.1));
                    } else {
                        // Escape did not close what is open (a form that keeps its draft, say).
                        // Close the stack and the palette directly, and say so in the output.
                        println!(
                            "warning: Escape did not return to Home in {HOME_PRESSES_MAX} presses \
                             (stack depth {:?}, palette open: {}); closing them directly",
                            self.shell.tabs().active_stack().map(ScreenStack::depth),
                            self.shell.palette().is_open(),
                        );
                        if let Some(stack) = self.shell.tabs_mut().active_stack_mut() {
                            stack.truncate_to(1);
                        }
                        self.shell.palette_mut().close();
                        self.next_stage(Stage::Keys);
                    }
                }
            }
            Stage::Keys => {
                let wanted = self.shots[self.index].company;
                // A cold-start shot always begins from a fresh shell, since its keys move the
                // wizard on and a shell remembers where it was.
                if wanted != self.company || (wanted == Fixture::Cold && !self.fresh) {
                    // Another company file: open it and wait for its books as at the start.
                    let stage = self.reopen(wanted);
                    self.next_stage(stage);
                    return;
                }
                self.fresh = false;
                let current = &self.shots[self.index];
                self.shell.set_theme(current.theme.kind());
                self.inject = current
                    .keys
                    .iter()
                    .map(|(k, m)| Inject::Key(*k, *m))
                    .collect();
                self.next_stage(Stage::Query);
            }
            Stage::Query => {
                if self.inject.is_empty() && self.gap == 0 && self.frames > 5 {
                    if let Some(query) = self.shots[self.index].query {
                        self.shell.palette_mut().set_query(query);
                    }
                    self.next_stage(Stage::After);
                }
            }
            Stage::After => {
                if self.frames > 5 {
                    self.inject = self.shots[self.index].after.iter().copied().collect();
                    self.next_stage(Stage::Settle);
                }
            }
            Stage::Settle => {
                // The wizard's Finish creates a company file on a background thread, and the screen
                // moves to Done only when that finishes, so the Done shot waits longer. The other
                // setup steps draw at once and settle like any shot.
                let settle = if self.shots[self.index].state == "setup.done" {
                    SETUP_SETTLE_FRAMES
                } else if self.shots[self.index].state == "sales-invoice.saved" {
                    SAVE_SETTLE_FRAMES
                } else {
                    60
                };
                if self.inject.is_empty() && self.frames > settle {
                    let slot = Arc::clone(&self.image);
                    ui.ctx().request_screenshot(move |image| {
                        if let Ok(mut held) = slot.lock() {
                            *held = Some(image);
                        }
                    });
                    self.next_stage(Stage::Capture);
                }
            }
            Stage::Capture => {
                let taken = self.image.lock().ok().and_then(|mut held| held.take());
                if let Some(image) = taken {
                    self.write_shot(&image);
                    // Restore the default theme; the next entry sets its own.
                    self.shell.set_theme(ThemeKind::Light);
                    self.index += 1;
                    if self.index == self.shots.len() {
                        self.next_stage(Stage::Done);
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    } else {
                        self.presses = 0;
                        self.next_stage(Stage::Home);
                    }
                } else {
                    assert!(self.frames < 600, "no screenshot arrived");
                }
            }
            Stage::Done => {}
        }
    }
}

/// The output directory and the optional `--only` substring, from the command line.
fn parse_args() -> (PathBuf, Option<String>, bool) {
    const USAGE: &str = "usage: ui_snapshots <output dir> [--only <substring>] [--audit]";
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect(USAGE));
    let mut only = None;
    let mut audit = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--audit" => audit = true,
            "--only" => only = Some(args.next().expect("--only needs a substring")),
            other => panic!("unknown argument {other}; {USAGE}"),
        }
    }
    (out, only, audit)
}

fn main() -> eframe::Result<()> {
    // Names this build in the binary, so a script can tell a copy of it from another branch's
    // build of the same example in a shared target directory.
    std::hint::black_box("ledgerly-sheet-marker");
    // The setup captures create a company file; the wizard never overwrites one, so clear the last.
    let _ = std::fs::create_dir_all("target/setup-capture");
    let _ = std::fs::remove_file(SETUP_FILE);
    let (out, only, audit) = parse_args();
    let entries: Vec<Shot> = if audit { audit_shots() } else { shots() }
        .into_iter()
        .filter(|s| {
            only.as_deref()
                .is_none_or(|needle| s.label().contains(needle))
        })
        .collect();
    assert!(!entries.is_empty(), "no entry matches --only {only:?}");
    std::fs::create_dir_all(&out).unwrap();
    let company = entries[0].company;
    let (shell, stage) = company.shell(&out);
    let app = Snapshots {
        shell,
        out,
        shots: entries,
        index: 0,
        stage,
        frames: 0,
        inject: VecDeque::new(),
        gap: 0,
        presses: 0,
        image: Arc::new(Mutex::new(None)),
        company,
        fresh: true,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Ledgerly snapshots")
            .with_inner_size([1280.0, 800.0]),
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "Ledgerly snapshots",
        options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
}
