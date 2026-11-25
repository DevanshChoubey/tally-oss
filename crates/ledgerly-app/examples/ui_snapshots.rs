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
//! Every shot starts on a fresh shell and a fresh company file ([`Snapshots::reopen`]): nothing a
//! shot opened (a dialog, a toast, a voucher draft, a posted voucher) can reach the next one.
//! Before its keys go in, the tool checks that the shell is on a clean Home ([`Snapshots::clean_home`]:
//! the screen stack at its root, no palette, no comparison dialog, no toast) and stops with a
//! message if it is not.

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
use ledgerly_app::worker::{ApiWorker, Hold};
use ledgerly_core::{
    Clock, CompanyId, Date, Decimal, GroupId, Gstin, Hsn, LedgerId, Paise, Qty, Rate, SignedAmount,
    StateCode, StockItemId, UnitId, VoucherId, gstin_check_character,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BankDetails, BaseVoucherKind, BillAllocation, BillRefKind, BillWiseConfig, Company,
    EntityKind, Features, GstClassification, GstRegistration, Ledger, LedgerLine,
    NegativeStockPolicy, PartyDetails, RegistrationType, ReservedGroup, StockItem, TaxDetails,
    TaxLedgerKind, Taxability, Unit, UnitKind, ValuationMethod, Voucher, VoucherMode,
    VoucherStatus,
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
    /// Godavari Drip & Pump Works, the sample company (`ledgerly-sample`): the company the mock
    /// shows on nearly every screen, so every shot that names no other company.
    Godavari,
    /// The sample company with one more sale on 19 Nov that takes the pickaxe 30 nos below zero,
    /// the situation of the mock's `balance-sheet.provisional` (index.html:1582, `pick: -30`).
    /// Capture-only: the sample's own books are untouched, and its company is made with the
    /// negative-stock policy Allow so the sale can post.
    GodavariShortStock,
    /// The sample company with its API worker held ([`ApiWorker::hold`]): the books are given to
    /// the shell up front, so the title bar and the side rail are as in any shot, and every
    /// figure a screen asks the worker for stays "loading" for the whole shot. This is how
    /// `home.loading` and `balance-sheet.loading` are photographed; the hold is dropped before the
    /// next shot opens its company.
    GodavariHeld,
    /// The sample company with no 2B file imported and no IMS action or filing recorded
    /// ([`ledgerly_sample::create_without_2b`]): the mock's `recon-empty` and `recon-error`.
    GodavariNo2b,
    /// Fictional Traders ([`company_file`]): the shots whose key paths or typed names need its
    /// data, or that show a state the sample does not have.
    Main,
    /// Fictional Traders' masters with no voucher yet ([`new_company_file`]).
    New,
    /// No company at all: the shell exactly as the app's `main` builds it
    /// (`Shell::cold_start`), with nothing opened. This is what a user sees on first launch.
    Cold,
}

impl Fixture {
    /// Creates the company file in `dir` and opens it. Each call starts the file again. `None`
    /// for [`Fixture::Cold`], which opens nothing.
    fn open(self, dir: &Path) -> Option<Api> {
        match self {
            Fixture::Godavari | Fixture::GodavariHeld => Some(godavari_company_file(dir)),
            Fixture::GodavariNo2b => Some(godavari_no_2b_file(dir)),
            Fixture::GodavariShortStock => Some(godavari_short_stock_file(dir)),
            Fixture::Main => Some(company_file(dir)),
            Fixture::New => Some(new_company_file(dir)),
            Fixture::Cold => None,
        }
    }

    /// The shell this fixture starts with and the stage the harness continues at: a company's
    /// shell waits for its books ([`Stage::Warm`]); the cold start has none to wait for. The
    /// third value is the hold on the worker, for the fixture that keeps screens loading.
    fn shell(self, dir: &Path) -> (Shell, Stage, Option<Hold>) {
        match self.open(dir) {
            Some(api) => {
                let held_lock = api.lock_date();
                let held_books = match self {
                    Fixture::GodavariHeld => match api.books() {
                        Ok(ledgerly_core::Outcome::Value(books)) => Some(books.clone()),
                        other => panic!("no books to hand the shell: {other:?}"),
                    },
                    Fixture::Godavari
                    | Fixture::GodavariNo2b
                    | Fixture::GodavariShortStock
                    | Fixture::Main
                    | Fixture::New
                    | Fixture::Cold => None,
                };
                let mut shell = Shell::new(Box::new(GatewayScreen::new()));
                let worker = ApiWorker::spawn(api).unwrap();
                let hold = held_books.map(|books| {
                    shell.app_ctx_mut().books = Some(books);
                    shell.app_ctx_mut().lock_date = held_lock;
                    worker.hold()
                });
                shell.app_ctx_mut().api = Some(worker);
                submit::note_changed();
                (shell, Stage::Warm, hold)
            }
            None => (Shell::cold_start(), Stage::Home, None),
        }
    }
}

/// The sample company, posted from its seed through the API in `dir`. Its clock is the sample's
/// own "today" (21 Nov 2026, noon IST), so audit stamps and dates agree with the mock.
fn godavari_company_file(dir: &Path) -> Api {
    let (api, _path) = ledgerly_sample::create_under(dir, sample_clock()).unwrap();
    api
}

/// The sample company with no 2B file, no IMS action and no filing recorded.
fn godavari_no_2b_file(dir: &Path) -> Api {
    let path = ledgerly_sample::path_under(dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let _removed = std::fs::remove_file(&path);
    ledgerly_sample::create_without_2b(&path, sample_clock()).unwrap()
}

/// The sample company plus two vouchers that leave the pickaxe's stock negative for a day: a
/// counter sale on 19 Nov (SI/0415's voucher copied) of 174 nos where 144 are on hand, so 30 nos
/// short, and a purchase on 20 Nov (the 15 Oct stock purchase copied) of 100 nos, so the item
/// closes positive. The stock engine cannot value the period at average cost, so closing stock is
/// provisional, as in the mock's `balance-sheet.provisional` (index.html:1582, `pick: -30`).
/// The books hold 150 bought and 6 sold before, so 144 are on hand.
fn godavari_short_stock_file(dir: &Path) -> Api {
    let path = ledgerly_sample::path_under(dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let _removed = std::fs::remove_file(&path);
    let mut api = ledgerly_sample::create_allowing_negative_stock(&path, sample_clock()).unwrap();
    let books = match api.books() {
        Ok(ledgerly_core::Outcome::Value(b)) => b,
        other => panic!("no books: {other:?}"),
    };
    let pick = books
        .masters
        .stock_items
        .values()
        .find(|i| i.name.starts_with("Steel pickaxe"))
        .map(|i| i.id)
        .unwrap();
    let next_id = books.vouchers.keys().map(|k| k.0).max().unwrap() + 1;
    let copy = |number: &str, copied: &str, id: u64, on: Date, qty: i64, rate: i64| {
        let mut v = books
            .vouchers
            .values()
            .find(|v| v.number.as_deref() == Some(copied))
            .cloned()
            .unwrap();
        v.id = VoucherId(id);
        v.number = Some(number.to_owned());
        v.date = on;
        v.supply_date = None;
        let party_bills = v
            .ledger_lines
            .iter()
            .find(|l| !l.bills.is_empty())
            .map(|l| l.bills.clone());
        v.ledger_lines.clear();
        v.gst = None;
        v.inventory_lines.retain(|l| l.item == pick);
        for line in &mut v.inventory_lines {
            let (q, amount) = (Decimal::from(qty), Paise::new(qty * rate * 100));
            line.qty = Qty(q);
            line.rate = Rate(Decimal::from(rate));
            line.amount = amount;
            for g in &mut line.godowns {
                g.qty = Qty(q);
                g.amount = amount;
            }
        }
        match api.invoice_gst(&v) {
            Ok(ledgerly_core::Outcome::Value(built)) => {
                v.ledger_lines = built.ledger_lines;
                v.gst = Some(built.stored);
                // A bill-wise party opens the bill for the whole party line.
                if let (Some(mut bills), Some(first)) = (party_bills, v.ledger_lines.first_mut()) {
                    for b in &mut bills {
                        number.clone_into(&mut b.name);
                        b.amount = first.amount;
                    }
                    first.bills = bills;
                }
            }
            other => panic!("the engine did not build the invoice: {other:?}"),
        }
        v
    };
    let sale = copy(
        "SI/0416",
        "SI/0415",
        next_id,
        date!(2026 - 11 - 19),
        174,
        640,
    );
    let purchase = copy(
        "PB/0146",
        "PB/0125",
        next_id + 1,
        date!(2026 - 11 - 20),
        100,
        470,
    );
    run(&mut api, Command::PostVoucher(sale));
    run(&mut api, Command::PostVoucher(purchase));
    api
}

/// The clock the sample company is made on and the screens read: 21 Nov 2026, 10:00 IST.
fn sample_clock() -> Clock {
    let today = ledgerly_sample::today().unwrap();
    let at = today.with_hms(4, 30, 0).unwrap().assume_utc();
    Clock::Fixed(at)
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
const SAVE_SETTLE_FRAMES: u32 = 240;

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
    /// Nothing for this many frames, for an answer from the worker that the next input needs (the
    /// open bills of a bill-wise party arrive some frames after the amount's Enter).
    Wait(u32),
    /// Press J until the cursor row of the top screen is the one with this label
    /// ([`ledgerly_app::Screen::cursor_label`]). A flow picks a named row this way and never by a
    /// count of key presses, so a change to the row order cannot move a capture to another row.
    /// Panics when the row is not reached.
    Seek(&'static str),
    /// Writes a file and makes the next open-file dialog answer with it, since no native dialog
    /// can be driven here ([`ledgerly_app::ui::dialogs::script_open_file`]). The key that opens the
    /// dialog goes in after it.
    ScriptFile(ScriptedFile),
    /// Panics unless the top screen is the expected one (see [`Expect`]). Every flow that drills
    /// or picks a row ends in one, so a capture of the wrong screen stops the run.
    Expect(Expect),
}

/// A file a flow hands to the next open-file dialog.
#[derive(Clone, Copy)]
enum ScriptedFile {
    /// The sample's November statement with its printed closing balance 640.00 too high, so the
    /// file does not add up (the mock's `bank.import-error`).
    StatementThatDoesNotAdd,
    /// The sample's November 2B file with another company's GSTIN as its recipient, so the engine
    /// refuses it (the mock's `recon-error`).
    TwoBForAnotherGstin,
}

impl ScriptedFile {
    /// Writes the file under `dir` and returns its path.
    fn write(self, dir: &Path) -> PathBuf {
        let dir = dir.join("scripted-files");
        std::fs::create_dir_all(&dir).unwrap();
        match self {
            ScriptedFile::StatementThatDoesNotAdd => {
                let sample = include_str!("../../ledgerly-sample/assets/saptagiri-nov-2026.mt940");
                // The file prints 8,23,305.50 and its lines add to exactly that; printing
                // 8,23,945.50 makes the printed figure 640.00 more than the lines give.
                let broken = sample.replace(":62F:C261120INR823305,50", ":62F:C261120INR823945,50");
                assert_ne!(
                    broken, sample,
                    "the sample statement no longer prints that closing"
                );
                let path = dir.join("saptagiri-nov-2026-cut.mt940");
                std::fs::write(&path, broken).unwrap();
                path
            }
            ScriptedFile::TwoBForAnotherGstin => {
                let sample = ledgerly_sample::GSTR2B_NOVEMBER;
                let other = sample.replace("27ZZZZZ0000Z1ZQ", "27ZZZZZ0105Z1ZK");
                assert_ne!(
                    other, sample,
                    "the sample 2B file no longer names the company"
                );
                let path = dir.join("gstr2b-nov-2026-other.json");
                std::fs::write(&path, other).unwrap();
                path
            }
        }
    }
}

/// What the screen a flow reached must be. The harness waits for it to settle, then panics with
/// what was shown instead.
#[derive(Clone, Copy)]
struct Expect {
    /// The title of the top screen (its breadcrumb).
    title: &'static str,
    /// The key bar name of the top screen, when the flow must also be in a given mode (the
    /// masters list names itself "Ledger" only while its form is open).
    key_bar: Option<&'static str>,
    /// The cursor row's label, when the screen has one.
    cursor: Option<&'static str>,
}

/// Frames a `Seek` or `Expect` waits for a worker-loaded screen before it gives up.
const REACH_FRAMES_MAX: u32 = 300;

/// J presses a `Seek` may make before it gives up (more than any list of the sample company).
const SEEK_PRESSES_MAX: u32 = 200;

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

    /// The same shot with nothing sent for `frames` frames after what came before it.
    fn wait(mut self, frames: u32) -> Self {
        self.after.push(Inject::Wait(frames));
        self
    }

    /// The same shot with the cursor moved to the row labelled `label` after what came before it.
    fn seek(mut self, label: &'static str) -> Self {
        self.after.push(Inject::Seek(label));
        self
    }

    /// The same shot with a check, after what came before it, that the top screen is titled
    /// `title` and its cursor is on `cursor` (when given).
    fn expect(mut self, title: &'static str, cursor: Option<&'static str>) -> Self {
        self.after.push(Inject::Expect(Expect {
            title,
            key_bar: None,
            cursor,
        }));
        self
    }

    /// As [`Shot::expect`], also requiring the key bar name `key_bar`.
    fn expect_bar(mut self, title: &'static str, key_bar: &'static str) -> Self {
        self.after.push(Inject::Expect(Expect {
            title,
            key_bar: Some(key_bar),
            cursor: None,
        }));
        self
    }

    /// As [`Shot::expect_bar`], also requiring the cursor row.
    fn expect_in(
        mut self,
        title: &'static str,
        key_bar: &'static str,
        cursor: &'static str,
    ) -> Self {
        self.after.push(Inject::Expect(Expect {
            title,
            key_bar: Some(key_bar),
            cursor: Some(cursor),
        }));
        self
    }

    /// The same shot with `file` handed to the next open-file dialog, after what came before it.
    fn script_file(mut self, file: ScriptedFile) -> Self {
        self.after.push(Inject::ScriptFile(file));
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
        company: Fixture::Godavari,
    }
}

const NONE: Modifiers = Modifiers::NONE;
const ESC: (Key, Modifiers) = (Key::Escape, Modifiers::NONE);

/// The title Home takes while its comparison dialog (`home.nudge-open`) is open.
const HOME_COMPARE_TITLE: &str = "Compare vouchers";

/// Frames a fresh shell may take to show a clean Home before the run stops.
const HOME_FRAMES_MAX: u32 = 600;

/// The balance sheet's Sundry Debtors group (the mock's `balance-sheet.group`): B, the Current
/// Assets line, Enter, the Sundry Debtors line, Enter. Rows are picked by name, so a change to the
/// statement's row order cannot change the capture.
fn debtors_group(state: &'static str) -> Shot {
    shot(state, &[(Key::B, NONE)])
        .seek("Current Assets")
        .press(&[(Key::Enter, NONE)])
        .seek("Sundry Debtors")
        .press(&[(Key::Enter, NONE)])
        .expect("Sundry Debtors", Some("Shree Ganesh Traders"))
}

/// The ledger drill under [`debtors_group`] (the mock's `balance-sheet.ledger`): Shree Ganesh
/// Traders' vouchers.
fn debtors_ledger(state: &'static str) -> Shot {
    debtors_group(state)
        .seek("Shree Ganesh Traders")
        .press(&[(Key::Enter, NONE)])
        .expect("Shree Ganesh Traders", None)
}

/// The label of the voucher the mock's `balance-sheet.voucher` opens.
const DEBTORS_VOUCHER_ROW: &str = "SI/0412";

/// The voucher drill under [`debtors_ledger`] (the mock's `balance-sheet.voucher`): SI/0412.
fn debtors_voucher(state: &'static str) -> Shot {
    debtors_ledger(state)
        .seek(DEBTORS_VOUCHER_ROW)
        .press(&[(Key::Enter, NONE)])
        .expect("SI/0412", None)
}

/// The company name the setup captures type (the mock's sample company, index.html:489).
const SETUP_NAME: &str = "Godavari Drip & Pump Works";
/// The GSTIN the setup captures type; its first two digits fill the state (index.html:1868).
const SETUP_GSTIN: &str = "27ZZZZZ0000Z1ZQ";
/// The city the setup captures type (the mock's, index.html:1867).
const SETUP_CITY: &str = "Pune";
/// The home folder the setup captures run with, relative to the repository root, so the default
/// company file (`~/Documents/Ledgerly/<name>.ledgerly`) is made here and never in the real
/// Documents folder. The capture shows the path as `~/Documents/...`, as a user sees it.
const SETUP_HOME: &str = "target/setup-capture/home";
/// The company file the setup captures create (the default under [`SETUP_HOME`]). The harness
/// removes it before a run, because the wizard does not overwrite a file.
const SETUP_FILE: &str =
    "target/setup-capture/home/Documents/Ledgerly/godavari-drip-pump-works.ledgerly";

/// A company setup capture, as a first-time user reaches it: the cold start (no company open) shows
/// the wizard, Enter goes from Start to Company (where the name field takes focus by itself), the
/// company form is filled by typing (Tab moves focus between its fields), then `after_company`
/// Enters move on from the company step.
fn setup_shot(state: &'static str, after_company: usize) -> Shot {
    let mut shot = shot(state, &[])
        .on(Fixture::Cold)
        .then(&[(Key::Enter, NONE)])
        .type_text(SETUP_NAME)
        .press(&[(Key::Tab, NONE), (Key::Tab, NONE)])
        .type_text(SETUP_CITY)
        .press(&[(Key::Tab, NONE)])
        .type_text(SETUP_GSTIN)
        // State, GST filing, Books begin, then the Company file field, which stays blank: the
        // capture shows the default path under the user's Documents folder.
        .press(&[
            (Key::Tab, NONE),
            (Key::Tab, NONE),
            (Key::Tab, NONE),
            (Key::Tab, NONE),
        ])
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

/// The mock's invoice head on Godavari: F8 opens an item invoice, `party` and `place` typed, Sales
/// as the sales ledger and `order` as the buyer's order number, so the keyboard is on the first
/// item (design/mock-v2/index.html `INV_SEED`, lines 1261-1269).
fn godavari_invoice(
    state: &'static str,
    party: &'static str,
    place: &'static str,
    order: &'static str,
) -> Shot {
    let enter = (Key::Enter, NONE);
    let mut s = shot(state, &[(Key::F8, NONE)])
        .on(Fixture::Godavari)
        .type_text(party)
        .press(&[enter])
        .type_text(place)
        .press(&[enter])
        .type_text("Sales")
        .press(&[enter]);
    if !order.is_empty() {
        s = s.type_text(order);
    }
    s.press(&[enter])
}

/// The purchase form as a user reaches it from Home: F9.
fn purchase_start(state: &'static str) -> Shot {
    shot(state, &[(Key::F9, NONE)]).on(Fixture::Godavari)
}

/// An item invoice head on Godavari, opened by `start`, whose top screen is titled `title`:
/// `party` and `place` typed, `ledger` as the sales or purchase ledger and `reference` as the
/// supplier's or original invoice number dated `reference_date`, so the keyboard is on the first item.
fn godavari_bill(
    start: Shot,
    title: &'static str,
    party: &'static str,
    place: &'static str,
    ledger: &'static str,
    reference: &'static str,
    reference_date: &'static str,
) -> Shot {
    let enter = (Key::Enter, NONE);
    start
        .expect(title, None)
        .type_text(party)
        .press(&[enter])
        .type_text(place)
        .press(&[enter])
        .type_text(ledger)
        .press(&[enter])
        .type_text(reference)
        .press(&[enter])
        .type_text(reference_date)
        .press(&[enter])
}

/// One line typed into the keyboard's current line: item, quantity, rate, and Enter on the rate
/// goes on to the next line (the discount column is hidden).
fn invoice_line(shot: Shot, item: &'static str, qty: &'static str, rate: &'static str) -> Shot {
    let enter = (Key::Enter, NONE);
    shot.type_text(item)
        .press(&[enter])
        .type_text(qty)
        .press(&[enter])
        .type_text(rate)
        .press(&[enter])
}

/// The create-ledger form filled as the mock draws it (index.html:1790-1810), by clicks, since no
/// key opens the Under list or ticks its boxes: Alt+C opens the form, the name is typed, Under is
/// Sundry Debtors (the list is scrolled to its end, where the scroll stops, so the row is always
/// at the same place), Bill-wise and Party details are on, `gstin` is typed and the credit days
/// are 30. The clicks are at points of the 1280 x 800 window; the form's rows do not move until
/// Party details adds the GSTIN row.
fn jadhav_form(state: &'static str, gstin: &'static str) -> Shot {
    shot(state, &[(Key::M, NONE), (Key::C, Modifiers::ALT)])
        .click(456.0, 263.0)
        .type_text("Jadhav Irrigation Services")
        .click(780.0, 257.0)
        .scroll(780.0, 400.0, -1200.0)
        .click(744.0, 474.0)
        .click(902.0, 263.0)
        .click(262.0, 330.0)
        // The credit days come before the GSTIN: its check line moves the rows below it.
        .click(995.0, 327.0)
        .type_text("30")
        .click(457.0, 327.0)
        .type_text(gstin)
        .expect("Masters", None)
}

/// The mock's three lines of SI/0418: 25 rolls of lateral, 67 nozzle sets, 40 sprayers.
fn three_lines(shot: Shot) -> Shot {
    let shot = invoice_line(shot, "Drip lateral", "25", "3850");
    let shot = invoice_line(shot, "Brass nozzle", "67", "505");
    invoice_line(shot, "Hand sprayer", "40", "1450")
}

/// The mock's narration of SI/0418, typed once the keyboard is past the last line.
const SI_NARRATION: &str = "Against PO SGT/PO/1131";

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
/// Prefixes of the entry states whose figures come from the engine one answer after another
/// (voucher number, tax, balance): the capture waits longer for them.
const ENGINE_SETTLED: [&str; 5] = [
    "purchase.",
    "journal.",
    "credit-note.",
    "debit-note.",
    "contra.",
];

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
        // The possible-double-payment nudge's Compare button (rule D-02) opens the comparison.
        shot("home.nudge-open", &[]).click(1176.0, 570.0),
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
            .then(&[(Key::Enter, NONE)])
            .on(Fixture::Main),
        shot("palette.no-results", &[palette]).query("zzqx"),
        shot("balance-sheet.default", &[(Key::B, NONE)]),
        shot(
            "balance-sheet.detailed",
            &[(Key::B, NONE), (Key::F1, Modifiers::ALT)],
        ),
        // The mock drills from the balance sheet into Sundry debtors (index.html:1594), then into
        // its first ledger (index.html:1600). Current Assets is the fourth line of the capture
        // company's condensed statement and Sundry Debtors the third line of its summary.
        debtors_group("balance-sheet.group"),
        debtors_ledger("balance-sheet.ledger"),
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
        // The mock selects G2, Nilgiri's NST/1145 (index.html:1695).
        shot("gst.recon", &[(Key::G, NONE), (Key::Num3, NONE)])
            .seek("NST/1145")
            .expect("GST returns", Some("NST/1145")),
        shot("gst.recon-empty", &[(Key::G, NONE), (Key::Num3, NONE)])
            .on(Fixture::GodavariNo2b)
            .expect("GST returns", None),
        // Ctrl+O is handed another company's 2B file: the engine refuses it and the screen says why.
        shot("gst.recon-error", &[(Key::G, NONE), (Key::Num3, NONE)])
            .on(Fixture::GodavariNo2b)
            .expect("GST returns", None)
            .script_file(ScriptedFile::TwoBForAnotherGstin)
            .press(&[(Key::O, Modifiers::COMMAND)])
            .wait(60)
            .expect("GST returns", None),
        // Alt+F2 opens the period picker on November; one step back is October, checked before
        // Enter chooses it. October's 3B was filed on 20 Nov, so its actions are final.
        shot("gst.recon-locked", &[(Key::G, NONE), (Key::Num3, NONE)])
            .expect("GST returns", None)
            .press(&[(Key::F2, Modifiers::ALT), (Key::K, NONE)])
            .expect("GST returns", Some("October 2026"))
            .press(&[(Key::Enter, NONE)])
            .seek("KP/2205")
            .expect("GST returns", Some("KP/2205")),
        shot("masters.ledgers", &[(Key::M, NONE)]),
        // The mock's party detail is the alter form of a bill-wise party ledger (index.html:1803).
        // The cursor goes to Shree Ganesh Traders by name, then Ctrl+Enter alters it.
        shot("masters.ledger-detail", &[(Key::M, NONE)])
            .seek("Shree Ganesh Traders")
            .press(&[(Key::Enter, Modifiers::COMMAND)])
            .expect_in("Masters", "Ledger", "Shree Ganesh Traders"),
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
        // The mock's new party ledger (index.html:1790-1810), typed through the real form: the
        // GSTIN's check character is right here and wrong in `masters.gstin-error`.
        jadhav_form("masters.create-ledger", "27ZZZZZ0100Z1ZP"),
        shot("keys.classic", &[palette])
            .query("Keyboard map")
            .then(&[(Key::Enter, NONE)]),
        shot("keys.standard", &[palette])
            .query("Keyboard map")
            .then(&[(Key::Enter, NONE), (Key::Num2, NONE)]),
        // SI/0418 to Shree Ganesh Traders as the mock's default shows: three lines, the buyer's
        // order number and the narration. A blank line after the last one moves to the narration.
        three_lines(godavari_invoice(
            "sales-invoice.default",
            "Shree Ganesh",
            "27",
            "SGT/PO/1131",
        ))
        .press(&[(Key::Enter, NONE)])
        .type_text(SI_NARRATION),
        // The mock's receipt: 1,00,000 from Mehta Agro into the Saptagiri account. Mehta tracks
        // bills, so the bill-wise popup opens on the amount, as the mock draws it.
        shot("receipt.default", &[(Key::F6, NONE)])
            .type_text("Saptagiri")
            .press(&[(Key::Enter, NONE)])
            .type_text("NEFT")
            .press(&[(Key::Enter, NONE)])
            .type_text("Mehta")
            .press(&[(Key::Enter, NONE)])
            .type_text("100000")
            .press(&[(Key::Enter, NONE)]),
        // The mock's payment: 6,500 of freight from the Saptagiri account.
        shot("payment.default", &[(Key::F5, NONE)])
            .type_text("Saptagiri")
            .press(&[(Key::Enter, NONE)])
            .type_text("NEFT")
            .press(&[(Key::Enter, NONE)])
            .type_text("Freight inward")
            .press(&[(Key::Enter, NONE)])
            .type_text("6500")
            .press(&[(Key::Enter, NONE), (Key::Enter, NONE)])
            .type_text("Freight, Vardhaman Logistics, LR 4471"),
        // The mock's bill allocation: 1,50,000 to Vardhaman Logistics against VL/861.
        shot("payment.bill-wise", &[(Key::F5, NONE)])
            .type_text("Saptagiri")
            .press(&[(Key::Enter, NONE)])
            .type_text("NEFT")
            .press(&[(Key::Enter, NONE)])
            .type_text("Vardhaman")
            .press(&[(Key::Enter, NONE)])
            .type_text("150000")
            .press(&[(Key::Enter, NONE)]),
        // The mock's overdraft: 2,50,000 from the Mulacur account takes it below zero. The mock
        // pays Nilgiri Steel Traders; Nilgiri tracks bills here, which opens the bill popup first
        // (tested in `s9_voucher_screens`), so the capture pays a ledger that does not.
        shot("payment.overdraft", &[(Key::F5, NONE)])
            .type_text("Mulacur")
            .press(&[(Key::Enter, NONE)])
            .type_text("RTGS")
            .press(&[(Key::Enter, NONE)])
            .type_text("Nilgiri")
            .press(&[(Key::Enter, NONE)])
            .type_text("250000")
            .press(&[(Key::Enter, NONE)])
            // The bill popup opens with the first bill filled in full once the worker has sent
            // the open bills; the mock pays 2,00,000 against NST/1102 and 50,000 against NST/1145.
            .wait(120)
            .click(1127.0, 388.0)
            .press(&[(Key::End, NONE)])
            .press(&[(Key::Backspace, NONE); 12])
            .type_text("200000")
            .click(1127.0, 448.0)
            .type_text("50000")
            // Enter accepts the details; Enter on the empty second line goes to the narration.
            .press(&[(Key::Enter, NONE), (Key::Enter, NONE)])
            .type_text("RTGS to Nilgiri Steel Traders"),
        // Godavari has two bank accounts, so Bank Reconciliation opens the chooser first; the
        // mock's statement is Saptagiri's (the first row), so Enter on the chooser opens it.
        shot("bank-chooser.default", &[palette])
            .query("Bank Reconciliation")
            .then(&[(Key::Enter, NONE)])
            .expect_in(
                "Bank reconciliation",
                "Choose account",
                "Saptagiri Commercial Bank A/c",
            ),
        shot("bank.default", &[palette])
            .query("Bank Reconciliation")
            .then(&[(Key::Enter, NONE), (Key::Enter, NONE)]),
        // Import statement (Ctrl+O) is handed a copy of the sample's statement whose printed
        // closing balance is 640.00 too high: the engine refuses it and the screen shows why.
        shot("bank.import-error", &[palette])
            .query("Bank Reconciliation")
            .then(&[(Key::Enter, NONE)])
            .seek("Saptagiri Commercial Bank A/c")
            .press(&[(Key::Enter, NONE)])
            .expect("Bank reconciliation: Saptagiri Commercial Bank A/c", None)
            .script_file(ScriptedFile::StatementThatDoesNotAdd)
            .press(&[(Key::O, Modifiers::COMMAND)])
            .wait(60)
            .expect("Bank reconciliation: Saptagiri Commercial Bank A/c", None),
        shot("bank.brs", &[palette])
            .query("Bank Reconciliation")
            .then(&[(Key::Enter, NONE), (Key::Enter, NONE), (Key::B, NONE)]),
        // Company setup (mock SCREENS.setup). The palette opens the wizard at Start; Enter goes on.
        // The true first launch: the shell as `main` builds it (`Shell::cold_start`), no company.
        shot("setup.welcome", &[]).on(Fixture::Cold),
        setup_shot("setup.company", 0),
        setup_shot("setup.features", 1),
        setup_shot("setup.keyboard", 2),
        setup_shot("setup.ai", 3),
        setup_shot("setup.backup", 4),
        setup_shot("setup.done", 5),
        // The sample imports a statement for Saptagiri only, so the second row of the chooser (Mula
        // Valley Co-op Bank) is an account with no statement: the empty state
        // (index.html:434 "No statement imported for this account").
        shot("bank.empty", &[palette])
            .query("Bank Reconciliation")
            .then(&[(Key::Enter, NONE)])
            .seek("Mula Valley Co-op Bank, current A/c")
            .press(&[(Key::Enter, NONE)])
            .expect(
                "Bank reconciliation: Mula Valley Co-op Bank, current A/c",
                None,
            ),
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
        debtors_voucher("balance-sheet.voucher"),
        // AI and privacy stores Off (key 1, as ai-settings.off) and Esc goes back to Home.
        shot("home.ai-off", &[palette])
            .query("AI and privacy")
            .then(&[(Key::Enter, NONE), (Key::Num1, NONE), ESC]),
        // The create-ledger form (masters.create-ledger) filled by clicks, since no key opens the
        // Under list or ticks its boxes: the mock's name, Under = Sundry Debtors (the list scrolled
        // to its end, where the scroll stops, so the row is always at the same place), Party details
        // on, the mock's GSTIN with a wrong check character (index.html:1790: the check character of
        // 27ZZZZZ0100Z1Z is P, so the mock's bad GSTIN ends in 7), Bill-wise on and 30 credit days.
        jadhav_form("masters.gstin-error", "27ZZZZZ0100Z1Z7"),
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
        // The worker is held, so the first refresh of the tiles never answers: the skeleton.
        shot("home.loading", &[])
            .on(Fixture::GodavariHeld)
            .expect("Home", None),
        // The report's request waits behind the hold: the computing card.
        shot("balance-sheet.loading", &[(Key::B, NONE)])
            .on(Fixture::GodavariHeld)
            .expect_bar("Balance sheet", "Computing"),
        // Godavari with a late sale that takes the pickaxe 30 nos below zero, so closing stock is
        // provisional (the mock's `pick: -30`).
        shot("balance-sheet.provisional", &[(Key::B, NONE)]).on(Fixture::GodavariShortStock),
        // The sample company has an opening balance on an expense ledger, which the engine's
        // profit and loss refuses (see PARITY-GAPS): the screen shows that refusal.
        shot("profit-and-loss.default", &[palette])
            .query("Profit and Loss")
            .then(&[(Key::Enter, NONE)])
            .expect_bar("Profit and loss", "Profit and loss"),
        shot("profit-and-loss.provisional", &[palette])
            .query("Profit and Loss")
            .then(&[(Key::Enter, NONE)])
            .expect_bar("Profit and loss", "Profit and loss")
            .on(Fixture::GodavariShortStock),
        shot("trial-balance.default", &[palette])
            .query("Trial Balance")
            .then(&[(Key::Enter, NONE)])
            .expect_bar("Trial balance", "Trial balance"),
        shot("outstanding.receivables", &[palette])
            .query("Receivables Outstanding")
            .then(&[(Key::Enter, NONE)])
            .expect_bar("Receivables", "Outstanding bills"),
        shot("outstanding.payables", &[palette])
            .query("Payables Outstanding")
            .then(&[(Key::Enter, NONE)])
            .expect_bar("Payables", "Outstanding bills"),
        shot("stock-summary.default", &[palette])
            .query("Stock Summary")
            .then(&[(Key::Enter, NONE)])
            .expect_bar("Stock summary", "Stock summary"),
        shot("stock-summary.provisional", &[palette])
            .query("Stock Summary")
            .then(&[(Key::Enter, NONE)])
            .expect_bar("Stock summary", "Stock summary")
            .on(Fixture::GodavariShortStock),
        // The edit log has no palette row; it is the first tab of the audit screen.
        shot("audit.default", &[palette])
            .query("Integrity Certificate")
            .then(&[(Key::Enter, NONE)])
            .expect_bar("Audit", "Audit")
            .click(941.0, 127.0)
            .wait(30)
            .expect_bar("Audit", "Audit"),
        shot("audit.integrity", &[palette])
            .query("Integrity Certificate")
            .then(&[(Key::Enter, NONE)])
            .expect_bar("Audit", "Audit"),
        // Godavari as it is: the pickaxe has no HSN, so Table 12 needs correction. The cursor goes
        // to table 12 by its number, as the mock's `gstr1-correction` state selects it.
        shot("gst.gstr1-correction", &[(Key::G, NONE), (Key::Num1, NONE)])
            .seek("12")
            .expect("GST returns", Some("12")),
        // Fictional Pumps has a stored statement whose lines match two vouchers.
        shot("review.default", &[palette])
            .query("Review queue")
            .then(&[(Key::Enter, NONE)]),
        // The item list open on the first line, filtered by what is typed (index.html: type-ahead
        // over item masters).
        invoice_line(
            invoice_line(
                godavari_invoice("sales-invoice.item-list", "Shree Ganesh", "27", ""),
                "Drip lateral",
                "25",
                "3850",
            ),
            "Brass nozzle",
            "67",
            "505",
        )
        .type_text("p"),
        // A Gujarat party: the place of supply is 24, so the engine charges IGST.
        three_lines(godavari_invoice(
            "sales-invoice.gst-inter-state",
            "Mehta",
            "24",
            "",
        ))
        .press(&[(Key::Enter, NONE)]),
        // A cash sale: Cash has no GSTIN, so the sale is B2C.
        invoice_line(
            invoice_line(
                godavari_invoice("sales-invoice.b2c", "Cash sale", "27", ""),
                "Hand sprayer",
                "2",
                "1450",
            ),
            "Steel pick",
            "3",
            "640",
        )
        .press(&[(Key::Enter, NONE)]),
        // An item picked with no quantity, then Ctrl+A: the save is refused with the reason.
        invoice_line(
            godavari_invoice("sales-invoice.validation-error", "Shree Ganesh", "27", ""),
            "Drip lateral",
            "25",
            "3850",
        )
        .type_text("Brass nozzle")
        .press(&[(Key::Enter, NONE), (Key::A, Modifiers::COMMAND)]),
        // A click puts the keyboard in the date (Ctrl+A is Save on this screen, so the date is
        // cleared with End and Backspace), and 31 Oct is the lock date of Fictional Pumps
        // (index.html sales-invoice.period-locked). Enter then goes on to the customer.
        shot("sales-invoice.period-locked", &[(Key::F8, NONE)])
            .on(Fixture::Godavari)
            .click(360.0, 215.0)
            .press(&[(Key::End, NONE)])
            .press(&[(Key::Backspace, NONE); 12])
            .type_text("31-10-2026")
            .press(&[(Key::Enter, NONE)])
            .type_text("Mehta")
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
        // The mock's `purchase.default` (index.html:1960-1990): Nilgiri Steel Traders' bill NST/1150
        // of 21 Nov 2026, 10 HDPE pipe coils at 2,050.00 and 100 PVC ball valves at 228.40, hand
        // typed. The mock labels the place of supply "29 Karnataka, inter-state"; the engine takes
        // the place typed here (27, delivered in Maharashtra), so the tax is IGST either way.
        invoice_line(
            invoice_line(
                godavari_bill(
                    purchase_start("purchase.default"),
                    "Purchase invoice",
                    "Nilgiri",
                    "27",
                    "Purchases",
                    "NST/1150",
                    "21-11-2026",
                ),
                "HDPE",
                "10",
                "2050",
            ),
            "PVC ball",
            "100",
            "228.40",
        )
        .press(&[(Key::Enter, NONE)])
        .type_text("Hand-typed, Nilgiri quote 19 Nov"),
        // A line with an item and no quantity, then Ctrl+A: the save is refused with the reason.
        invoice_line(
            godavari_bill(
                purchase_start("purchase.validation-error"),
                "Purchase invoice",
                "Nilgiri",
                "27",
                "Purchases",
                "NST/1150",
                "21-11-2026",
            ),
            "HDPE",
            "10",
            "2050",
        )
        .type_text("PVC ball")
        .press(&[(Key::Enter, NONE), (Key::A, Modifiers::COMMAND)]),
        // The mock's `credit-note.default` (index.html:2004): 10 sets of brass nozzles (6 pc)
        // returned by Shree Ganesh Traders against SI/0412 of 14 Nov, at the invoiced 505.00.
        invoice_line(
            godavari_bill(
                shot("credit-note.default", &[(Key::F6, Modifiers::ALT)]).on(Fixture::Godavari),
                "Credit Note",
                "Shree Ganesh",
                "27",
                "Sales",
                "SI/0412",
                "14-11-2026",
            ),
            "Brass nozzle",
            "10",
            "505",
        )
        .press(&[(Key::Enter, NONE)])
        .type_text("Returned 10 sets, damaged in transit"),
        // STOPPED against the mock's `credit-note.validation-error` (70 returned against 67
        // invoiced): the engine has no rule that compares a return with the invoice it names, so
        // this is the app's own refusal (an item with no quantity), not the mock's error.
        invoice_line(
            godavari_bill(
                shot("credit-note.validation-error", &[(Key::F6, Modifiers::ALT)])
                    .on(Fixture::Godavari),
                "Credit Note",
                "Shree Ganesh",
                "27",
                "Sales",
                "SI/0412",
                "14-11-2026",
            ),
            "Drip lateral",
            "2",
            "3850",
        )
        .type_text("Brass nozzle")
        .press(&[(Key::Enter, NONE), (Key::A, Modifiers::COMMAND)]),
        // STOPPED against the mock's `debit-note.default` and `.validation-error`: the mock returns
        // 2 bundles of MS angles, an item the sample company does not have, and a return must link
        // to a bill line, which the engine does not do. This is the app's own debit note.
        // Debit note: two pickaxes of bill NST/1145 sent back to Nilgiri Steel Traders.
        invoice_line(
            godavari_bill(
                shot("debit-note.default", &[(Key::F5, Modifiers::ALT)]).on(Fixture::Godavari),
                "Debit Note",
                "Nilgiri",
                "27",
                "Purchases",
                "NST/1145",
                "20-11-2026",
            ),
            "Steel pickaxe",
            "2",
            "900",
        )
        .press(&[(Key::Enter, NONE)])
        .type_text("Two pickaxes returned, handles cracked"),
        invoice_line(
            godavari_bill(
                shot("debit-note.validation-error", &[(Key::F5, Modifiers::ALT)])
                    .on(Fixture::Godavari),
                "Debit Note",
                "Nilgiri",
                "27",
                "Purchases",
                "NST/1145",
                "20-11-2026",
            ),
            "Steel pickaxe",
            "2",
            "900",
        )
        .type_text("Hand sprayer")
        .press(&[(Key::Enter, NONE), (Key::A, Modifiers::COMMAND)]),
        // The mock's journal (`journal.default`): freight paid was booked as Purchases and is moved
        // to Freight inward, 1,840.00 on both sides. The credit line's amount is the prefilled
        // balancing one.
        shot("journal.default", &[(Key::F7, NONE)])
            .on(Fixture::Godavari)
            .type_text("Freight inward")
            .press(&[(Key::Enter, NONE)])
            .type_text("1840")
            .press(&[(Key::Enter, NONE)])
            .type_text("Purchases")
            .press(&[(Key::Enter, NONE), (Key::Enter, NONE)])
            .type_text("Freight inward on the Nilgiri consignment, moved from Purchases")
            .expect("Journal", None),
        // The mock's `journal.validation-error`: the credit is typed as 1,480.00 against the
        // debit of 1,840.00. The prefilled third line is deleted (Ctrl+D) and Ctrl+A is refused
        // with the difference.
        shot("journal.validation-error", &[(Key::F7, NONE)])
            .on(Fixture::Godavari)
            .type_text("Freight inward")
            .press(&[(Key::Enter, NONE)])
            .type_text("1840")
            .press(&[(Key::Enter, NONE)])
            .type_text("Purchases")
            .press(&[(Key::Enter, NONE)])
            // The prefilled amount ("1840.00") is cleared, then 1480 typed in its place.
            .wait(30)
            .press(&[(Key::End, NONE)])
            .press(&[(Key::Backspace, NONE); 8])
            .type_text("1480")
            .press(&[(Key::Enter, NONE)])
            .wait(30)
            .press(&[(Key::D, Modifiers::COMMAND)])
            .press(&[(Key::A, Modifiers::COMMAND)])
            .expect("Journal", None),
        // The mock's `contra.default`: 30,000.00 of cash paid into the Saptagiri account.
        shot("contra.default", &[(Key::F4, NONE)])
            .on(Fixture::Godavari)
            .type_text("Saptagiri")
            .press(&[(Key::Enter, NONE)])
            .type_text("30000")
            .press(&[(Key::Enter, NONE)])
            .type_text("Cash")
            .press(&[(Key::Enter, NONE), (Key::Enter, NONE)])
            .type_text("Cash deposited into the Saptagiri account")
            .expect("Contra", None),
        // The mock's `contra.validation-error`: nothing picked, nothing typed, Ctrl+A.
        shot("contra.validation-error", &[(Key::F4, NONE)])
            .on(Fixture::Godavari)
            .press(&[(Key::A, Modifiers::COMMAND)])
            // A click on the narration takes the keyboard off the ledger field, so its list no
            // longer covers the notes.
            .click(800.0, 373.0)
            .expect("Contra", None),
        // A blank line ends the lines; Enter on the narration asks to save.
        three_lines(godavari_invoice(
            "sales-invoice.save-confirm",
            "Shree Ganesh",
            "27",
            "SGT/PO/1131",
        ))
        .press(&[(Key::Enter, NONE)])
        .type_text(SI_NARRATION)
        .press(&[(Key::Enter, NONE)]),
        // Yes on the save question: the invoice is posted and a fresh one opens (index.html
        // sales-invoice.saved).
        three_lines(godavari_invoice(
            "sales-invoice.saved",
            "Shree Ganesh",
            "27",
            "SGT/PO/1131",
        ))
        .press(&[(Key::Enter, NONE)])
        .type_text(SI_NARRATION)
        .press(&[(Key::Enter, NONE), (Key::Enter, NONE)]),
    ]
}

/// Ctrl+A: in a text field, selects all of its text.
const SELECT_ALL: (Key, Modifiers) = (Key::A, Modifiers::COMMAND);

/// Where the snapshot run is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// Waiting for the books and the first figures.
    Warm,
    /// A fresh shell is checked for a clean Home before the shot's keys go in.
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
    /// The hold on the worker of a [`Fixture::GodavariHeld`] shot. Declared first so that it is
    /// dropped before the shell it belongs to, or the worker thread would never end.
    held: Option<Hold>,
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
    image: Arc<Mutex<Option<Arc<egui::ColorImage>>>>,
    /// The company file open in the shell.
    company: Fixture,
    /// Frames the front `Seek` or `Expect` has waited so far.
    reach_frames: u32,
    /// J presses the front `Seek` has made so far.
    seek_presses: u32,
}

impl Snapshots {
    /// One step of a `Seek`: done when the cursor is on `label`, otherwise one J press (after
    /// the screen had time to load its rows).
    fn seek_step(&mut self, label: &'static str, raw_input: &mut egui::RawInput) {
        let state = self.shots[self.index].label();
        let cursor = self.shell.top_cursor_label();
        if cursor.as_deref() == Some(label) {
            self.inject.pop_front();
            self.reach_frames = 0;
            self.seek_presses = 0;
            self.gap = 6;
            return;
        }
        self.reach_frames += 1;
        if cursor.is_none() && self.reach_frames < 30 {
            return;
        }
        assert!(
            self.seek_presses < SEEK_PRESSES_MAX,
            "{state}: the cursor never reached the row {label:?} on the screen {:?} (cursor now \
             {cursor:?}, {} J presses)",
            self.shell.tabs().active_stack().map(ScreenStack::top_title),
            self.seek_presses,
        );
        for pressed in [true, false] {
            raw_input.events.push(Event::Key {
                key: Key::J,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Modifiers::NONE,
            });
        }
        self.seek_presses += 1;
        self.gap = 6;
    }

    /// One step of an `Expect`: done when the top screen is the expected one, a panic when it is
    /// not after the screen had time to load.
    fn expect_step(&mut self, expect: Expect) {
        let title = self
            .shell
            .tabs()
            .active_stack()
            .map_or_default(ScreenStack::top_title);
        let key_bar = self.shell.top_key_bar_name();
        let cursor = self.shell.top_cursor_label();
        let reached = title == expect.title
            && expect.key_bar.is_none_or(|k| key_bar == k)
            && expect.cursor.is_none_or(|c| cursor.as_deref() == Some(c));
        if reached {
            self.inject.pop_front();
            self.reach_frames = 0;
            return;
        }
        self.reach_frames += 1;
        assert!(
            self.reach_frames < REACH_FRAMES_MAX,
            "{}: expected the screen {:?} (key bar {:?}, cursor {:?}) but the capture would show \
             {title:?} (key bar {key_bar:?}, cursor {cursor:?})",
            self.shots[self.index].label(),
            expect.title,
            expect.key_bar,
            expect.cursor,
        );
    }

    /// Closes the open company and opens `company` on a new shell, as the start of the run does.
    fn reopen(&mut self, ctx: &egui::Context, company: Fixture) -> Stage {
        // Scroll offsets, open popups, collapsing headers and the focus live in the egui context,
        // not in the shell, so they are cleared too.
        ctx.memory_mut(|m| {
            m.data.clear();
            m.reset_areas();
            m.stop_text_input();
        });
        // The hold goes first: the old shell's worker thread ends only once it is released.
        self.held = None;
        let (shell, stage, held) = company.shell(&self.out);
        self.shell = shell;
        self.held = held;
        self.company = company;
        // The setup wizard never overwrites a company file; a shot that created one leaves none.
        let _removed = std::fs::remove_file(SETUP_FILE);
        stage
    }

    fn next_stage(&mut self, stage: Stage) {
        self.stage = stage;
        self.frames = 0;
    }

    /// True when the shell shows a clean Home: the active tab's stack at its root, the palette
    /// closed, Home's comparison dialog closed and no toast on screen.
    fn clean_home(&self) -> bool {
        !self.shell.palette().is_open()
            && self.shell.notice().is_none()
            && self
                .shell
                .tabs()
                .active_stack()
                .is_some_and(|stack| stack.depth() == 1 && stack.top_title() != HOME_COMPARE_TITLE)
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
        // No hover from the real pointer or an earlier click may reach a capture.
        ledgerly_app::ui::capture::park_pointer(raw_input);
        if self.gap > 0 {
            self.gap -= 1;
            return;
        }
        match self.inject.front().copied() {
            Some(Inject::Seek(label)) => {
                self.seek_step(label, raw_input);
                return;
            }
            Some(Inject::Expect(expect)) => {
                self.expect_step(expect);
                return;
            }
            Some(_) | None => {}
        }
        if let Some(item) = self.inject.pop_front() {
            // A wheel scroll is animated over several frames; the next click waits for it to end.
            self.gap = match item {
                Inject::Scroll(..) => SCROLL_SETTLE_FRAMES,
                Inject::Wait(frames) => frames,
                Inject::Seek(_) | Inject::Expect(_) => 0,
                Inject::Key(..) | Inject::Text(_) | Inject::Click(..) | Inject::ScriptFile(_) => 6,
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
                Inject::Wait(_) | Inject::Seek(_) | Inject::Expect(_) => {}
                Inject::ScriptFile(file) => {
                    ledgerly_app::ui::dialogs::script_open_file(file.write(&self.out));
                }
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
                    // The capture window has no window manager, so it is never given the keyboard;
                    // egui reports no widget as focused then, and a click could not move the
                    // keyboard. A click on a window is what gives it the keyboard.
                    raw_input.focused = true;
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
                if self.clean_home() {
                    self.next_stage(Stage::Keys);
                } else {
                    assert!(
                        self.frames < HOME_FRAMES_MAX,
                        "a fresh shell did not show a clean Home before {}: stack depth {:?}, \
                         palette open: {}, notice: {:?}",
                        self.shots[self.index].label(),
                        self.shell.tabs().active_stack().map(ScreenStack::depth),
                        self.shell.palette().is_open(),
                        self.shell.notice(),
                    );
                }
            }
            Stage::Keys => {
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
                } else if self.shots[self.index].state.starts_with("sales-invoice.")
                    || self.shots[self.index].state.starts_with("payment.")
                    || self.shots[self.index].state.starts_with("receipt.")
                    || ENGINE_SETTLED
                        .iter()
                        .any(|p| self.shots[self.index].state.starts_with(p))
                {
                    // The engine answers tax, the voucher number and the balance one after the
                    // other, each only once the one before it has settled.
                    240
                } else {
                    60
                };
                if self.inject.is_empty() && self.gap == 0 && self.frames > settle {
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
                        // The next shot starts on a fresh shell and company file.
                        let wanted = self.shots[self.index].company;
                        let stage = self.reopen(ui.ctx(), wanted);
                        self.next_stage(stage);
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
    let _ = std::fs::create_dir_all(format!("{SETUP_HOME}/Documents/Ledgerly"));
    if let Ok(home) = std::fs::canonicalize(SETUP_HOME) {
        ledgerly_app::screens::setup::set_home_override(home);
    }
    // Every screen reads "today" from the sample's own date, as the mock does (CO.today).
    // 15:00, so Home greets in the afternoon as the mock does (the mock's clock is a Saturday afternoon).
    ledgerly_app::shell::navigate::set_clock_override(ledgerly_sample::today().unwrap(), 15);
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
    let (shell, stage, held) = company.shell(&out);
    let app = Snapshots {
        shell,
        out,
        shots: entries,
        index: 0,
        stage,
        frames: 0,
        inject: VecDeque::new(),
        gap: 0,
        image: Arc::new(Mutex::new(None)),
        company,
        reach_frames: 0,
        seek_presses: 0,
        held,
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
