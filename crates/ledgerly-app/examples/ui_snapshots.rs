//! Developer tool: opens the real app window on a fictional company and saves a screenshot of
//! each registered screen state, so the app can be compared with the design mock
//! (`design/mock-v2/`). `design/app-captures/capture.sh` runs it and converts the output.
//!
//! Usage: `cargo run -p ledgerly-app --example ui_snapshots -- <output dir> [--only <substring>]`
//!
//! Each entry in [`shots`] is named by the mock state id it should match (`states.json`) and a
//! theme. It writes `<output dir>/<theme>/<state-id>.ppm` (theme `light`, `dark` or `hc`); convert
//! with `ffmpeg -i in.ppm out.webp`. `--only` keeps the entries whose `<theme>/<state-id>` label
//! contains the substring (several, separated by `|`, keep the entries that match any). The company file is created in the output directory. All names, GSTINs
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

/// A company with nothing in it but what a new file starts with (mock `masters.empty`).
fn blank_company_file(dir: &Path) -> Api {
    blank_company_with(dir, &traders())
}

/// [`blank_company_file`] for a company that keeps inventory and has no stock item yet (mock
/// `masters.items-empty`: a company without inventory is not offered the Stock items tab).
fn blank_inventory_company_file(dir: &Path) -> Api {
    let company = Company {
        features: Features {
            inventory: true,
            ..traders().features
        },
        ..traders()
    };
    blank_company_with(dir, &company)
}

fn blank_company_with(dir: &Path, company: &Company) -> Api {
    // On the screens' clock (21 Nov 2026), not the epoch: an import forms its period from the
    // books' beginning to the end of the current financial year.
    let path = dir.join("snapshot-blank.ledgerly");
    let _removed = std::fs::remove_file(&path);
    Api::create(&path, company, sample_clock()).unwrap()
}

/// A loopback server that answers the import's requests the way a running other app would: a
/// small company (groups, three ledgers with opening balances, the Journal type and no vouchers).
/// It runs for the life of the harness and returns the port it listens on, as text.
fn fake_import_server_port() -> &'static str {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::OnceLock;
    static PORT: OnceLock<String> = OnceLock::new();
    PORT.get_or_init(|| {
        // A fixed port when it is free, so the capture is the same on every run.
        let listener = TcpListener::bind("127.0.0.1:39417")
            .or_else(|_| TcpListener::bind("127.0.0.1:0"))
            .unwrap();
        let port = listener.local_addr().unwrap().port().to_string();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut raw = Vec::new();
                let mut buf = [0_u8; 4096];
                while let Ok(n) = stream.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    raw.extend_from_slice(&buf[..n]);
                    let text = String::from_utf8_lossy(&raw).into_owned();
                    if let Some((head, body)) = text.split_once("\r\n\r\n") {
                        let want = head
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .and_then(|v| v.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if body.len() >= want {
                            break;
                        }
                    }
                }
                let request = String::from_utf8_lossy(&raw).into_owned();
                let inner = if request.contains("SVFROMDATE") {
                    ""
                } else if request.contains("<ID>LedgerlyGroups</ID>") {
                    "<GROUP NAME=\"Current Assets\"><GUID>g-ca</GUID><ALTERID>1</ALTERID><NAME>Current Assets</NAME></GROUP>\
                     <GROUP NAME=\"Cash-in-Hand\"><GUID>g-cih</GUID><ALTERID>2</ALTERID><NAME>Cash-in-Hand</NAME><PARENT>Current Assets</PARENT></GROUP>"
                } else if request.contains("<ID>LedgerlyLedgers</ID>") {
                    "<LEDGER NAME=\"Cash\"><GUID>l-c</GUID><ALTERID>3</ALTERID><NAME>Cash</NAME><PARENT>Cash-in-Hand</PARENT><OPENINGBALANCE>-10000.00</OPENINGBALANCE></LEDGER>\
                     <LEDGER NAME=\"Rent\"><GUID>l-r</GUID><ALTERID>3</ALTERID><NAME>Rent</NAME><PARENT>Indirect Expenses</PARENT><OPENINGBALANCE>0</OPENINGBALANCE></LEDGER>\
                     <LEDGER NAME=\"Capital\"><GUID>l-k</GUID><ALTERID>3</ALTERID><NAME>Capital</NAME><PARENT>Capital Account</PARENT><OPENINGBALANCE>10000.00</OPENINGBALANCE></LEDGER>"
                } else if request.contains("<ID>LedgerlyVoucherTypes</ID>") {
                    "<VOUCHERTYPE NAME=\"Journal\"><GUID>vt-j</GUID><ALTERID>4</ALTERID><NAME>Journal</NAME></VOUCHERTYPE>"
                } else {
                    ""
                };
                let body = format!(
                    "<ENVELOPE><HEADER><VERSION>1</VERSION><STATUS>1</STATUS></HEADER><BODY><DATA><COLLECTION>{inner}</COLLECTION></DATA></BODY></ENVELOPE>"
                );
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _written = stream.write_all(reply.as_bytes());
            }
        });
        port
    })
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
    /// The sample company with the PIN code removed from Shree Ganesh Traders' address, the buyer
    /// of the mock's `sales-invoice.einvoice-not-ready` ("A registered buyer whose address has no
    /// PIN code"). Capture-only: the sample's own seed holds the PIN.
    GodavariBuyerNoPin,
    /// Fictional Traders ([`company_file`]): the shots whose key paths or typed names need its
    /// data, or that show a state the sample does not have.
    Main,
    /// Fictional Traders' masters with no voucher yet ([`new_company_file`]).
    New,
    /// A company just created, with no master and no voucher ([`blank_company_file`]).
    Blank,
    /// [`Fixture::Blank`] with inventory on ([`blank_inventory_company_file`]).
    BlankInventory,
    /// No company at all: the shell exactly as the app's `main` builds it
    /// (`Shell::cold_start`), with nothing opened. This is what a user sees on first launch.
    Cold,
    /// The cold start after a company was opened before: the Start screen with its list. The
    /// list is a real record of the sample company in a folder under the output folder; no
    /// company is opened and the user's config folder is never touched.
    ColdRecent,
}

impl Fixture {
    /// Creates the company file in `dir` and opens it. Each call starts the file again. `None`
    /// for [`Fixture::Cold`], which opens nothing.
    fn open(self, dir: &Path) -> Option<Api> {
        match self {
            Fixture::Godavari | Fixture::GodavariHeld => Some(godavari_company_file(dir)),
            Fixture::GodavariNo2b => Some(godavari_no_2b_file(dir)),
            Fixture::GodavariBuyerNoPin => Some(godavari_buyer_no_pin_file(dir)),
            Fixture::GodavariShortStock => Some(godavari_short_stock_file(dir)),
            Fixture::Main => Some(company_file(dir)),
            Fixture::New => Some(new_company_file(dir)),
            Fixture::Blank => Some(blank_company_file(dir)),
            Fixture::BlankInventory => Some(blank_inventory_company_file(dir)),
            Fixture::Cold | Fixture::ColdRecent => None,
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
                    | Fixture::GodavariBuyerNoPin
                    | Fixture::GodavariShortStock
                    | Fixture::Main
                    | Fixture::New
                    | Fixture::Blank
                    | Fixture::BlankInventory
                    | Fixture::Cold
                    | Fixture::ColdRecent => None,
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
            None if self == Fixture::ColdRecent => {
                let config = dir.join("recent-config");
                let _removed = std::fs::remove_dir_all(&config);
                let (api, path) = ledgerly_sample::create_under(dir, sample_clock()).unwrap();
                let entry = ledgerly_app::recent::describe(
                    &api,
                    &path,
                    ledgerly_core::Date::from_calendar_date(2026, time::Month::November, 21)
                        .unwrap(),
                )
                .expect("the sample's books are readable");
                drop(api);
                ledgerly_app::recent::record(&config, entry).unwrap();
                (
                    Shell::cold_start_with_recent(Some(config)),
                    Stage::Home,
                    None,
                )
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

/// The sample company with Shree Ganesh Traders' PIN code taken out of its address, so the
/// e-Invoice cell of an invoice to it says what is missing.
fn godavari_buyer_no_pin_file(dir: &Path) -> Api {
    let mut api = godavari_company_file(dir);
    let books = match api.books() {
        Ok(ledgerly_core::Outcome::Value(b)) => b,
        other => panic!("no books: {other:?}"),
    };
    let mut buyer = books
        .masters
        .ledgers
        .values()
        .find(|l| l.name == "Shree Ganesh Traders")
        .cloned()
        .unwrap();
    buyer.party.as_mut().unwrap().address.pincode = None;
    match api.execute(ledgerly_api::Command::AlterLedger(buyer)) {
        Ok(ledgerly_core::Outcome::Value(_)) => {}
        other => panic!("the ledger was not altered: {other:?}"),
    }
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
    /// The language code the shell is set to for this entry (`en` unless [`Shot::lang`]).
    lang: &'static str,
    keys: Vec<(Key, Modifiers)>,
    query: Option<&'static str>,
    after: Vec<Inject>,
    /// The company file the shot is taken on.
    company: Fixture,
}

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
    /// A primary click at the middle of the one widget with this accessible name (the name a
    /// screen reader is told), waited for up to [`REACH_FRAMES_MAX`] frames. A flow uses it for a
    /// widget no key reaches, so a change to the layout cannot move the click. Panics when no
    /// widget, or more than one, has the name.
    ClickNamed(&'static str),
    /// As `ClickNamed` for a row of a scrolling list: while no widget has the name, the wheel is
    /// turned one notch down over the widget named by the second string (a row that is on screen),
    /// until the row shows. The list is never scrolled by a fixed amount.
    ClickNamedScrolling(&'static str, &'static str),
    /// As `ClickNamed`, near the left end of the widget, for one whose middle a popup covers.
    ClickNamedStart(&'static str),
    /// Nothing for this many frames, for an answer from the worker that the next input needs (the
    /// open bills of a bill-wise party arrive some frames after the amount's Enter).
    Wait(u32),
    /// Press J until the cursor row of the top screen is the one with this label
    /// ([`ledgerly_app::Screen::cursor_label`]). A flow picks a named row this way and never by a
    /// count of key presses, so a change to the row order cannot move a capture to another row.
    /// Panics when the row is not reached.
    Seek(&'static str),
    /// As `Seek`, pressing `key` with `modifiers` in place of J (a kind tab list that has no rows
    /// to walk, such as the masters kinds on Alt+Right).
    SeekBy(Key, Modifiers, &'static str),
    /// Writes a file and makes the next open-file dialog answer with it, since no native dialog
    /// can be driven here ([`ledgerly_app::ui::dialogs::script_open_file`]). The key that opens the
    /// dialog goes in after it.
    ScriptFile(ScriptedFile),
    /// Makes the next save dialog answer with a path (see [`SaveTarget`]).
    ScriptSave(SaveTarget),
    /// Holds the API worker from now on: every job asked after this stays queued until the shot
    /// ends. A flow takes it after its screen has loaded, so the next request (Ctrl+P on a
    /// report) stays "preparing" for the photograph (`profit-and-loss.print-busy`).
    HoldWorker,
    /// Panics unless the top screen is the expected one (see [`Expect`]). Every flow that drills
    /// or picks a row ends in one, so a capture of the wrong screen stops the run.
    Expect(Expect),
}

/// Where a flow's save dialog answers.
#[derive(Clone, Copy)]
enum SaveTarget {
    /// A new file in a folder that exists, so the write succeeds.
    Writable,
    /// A file in a folder that does not exist, so the write fails with the system's reason.
    MissingFolder,
    /// A new `.csv` file in a folder that exists (the reconciliation statement's export).
    WritableCsv,
}

impl SaveTarget {
    /// The path under `dir` the dialog answers with.
    fn path(self, dir: &Path) -> PathBuf {
        let dir = dir.join("scripted-saves");
        std::fs::create_dir_all(&dir).unwrap();
        match self {
            SaveTarget::Writable => {
                let path = dir.join("Profit and loss.pdf");
                let _removed = std::fs::remove_file(&path);
                path
            }
            SaveTarget::MissingFolder => dir.join("no-such-folder").join("Profit and loss.pdf"),
            SaveTarget::WritableCsv => {
                let path = dir.join("bank-reconciliation-statement.csv");
                let _removed = std::fs::remove_file(&path);
                path
            }
        }
    }
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

/// Frames a list takes to settle after one notch of [`Inject::ClickNamedScrolling`].
const NOTCH_SETTLE_FRAMES: u32 = 30;

/// Points one wheel notch of [`Inject::ClickNamedScrolling`] turns a list (negative: down).
const SCROLL_NOTCH: f32 = -150.0;

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

    /// The same shot in another language, set before its keys (the shell's language, as the
    /// palette sets it).
    fn lang(mut self, code: &'static str) -> Self {
        self.lang = code;
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

    /// As [`Shot::seek`], pressing `key` with `modifiers` until the cursor label is `label`.
    fn seek_by(mut self, key: Key, modifiers: Modifiers, label: &'static str) -> Self {
        self.after.push(Inject::SeekBy(key, modifiers, label));
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

    /// The same shot with the next save dialog answering `target`, after what came before it.
    fn script_save(mut self, target: SaveTarget) -> Self {
        self.after.push(Inject::ScriptSave(target));
        self
    }

    /// The same shot with the API worker held after what came before it (see
    /// [`Inject::HoldWorker`]).
    fn hold_worker(mut self) -> Self {
        self.after.push(Inject::HoldWorker);
        self
    }

    /// The same shot with a click on the widget named `name` after what came before it (see
    /// [`Inject::ClickNamed`]).
    fn click_named(mut self, name: &'static str) -> Self {
        self.after.push(Inject::ClickNamed(name));
        self
    }

    /// The same shot with a click near the left end of the widget named `name` (see
    /// [`Inject::ClickNamedStart`]).
    fn click_named_start(mut self, name: &'static str) -> Self {
        self.after.push(Inject::ClickNamedStart(name));
        self
    }

    /// The same shot with a click on the row `name` of a scrolling list, in which the row
    /// `visible` is on screen (see [`Inject::ClickNamedScrolling`]).
    fn click_named_scrolling(mut self, name: &'static str, visible: &'static str) -> Self {
        self.after.push(Inject::ClickNamedScrolling(name, visible));
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
        lang: "en",
        keys: keys.to_vec(),
        query: None,
        after: Vec::new(),
        company: Fixture::Godavari,
    }
}

const NONE: Modifiers = Modifiers::NONE;
const ESC: (Key, Modifiers) = (Key::Escape, Modifiers::NONE);
/// Shift+Enter: back to the previous field of an entry screen.
const SHIFT_ENTER: (Key, Modifiers) = (Key::Enter, Modifiers::SHIFT);

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
    let start = start.expect(title, None);
    // The Enter order of the head follows the help pages (see `ItemInvoiceScreen::order`): a
    // purchase asks the supplier's invoice number and date, then the party, the purchase ledger
    // and the place; a note asks the party, the original invoice number and date, the ledger and
    // the place.
    if title == "Purchase" {
        start
            .type_text(reference)
            .press(&[enter])
            .type_text(reference_date)
            .press(&[enter])
            .type_text(party)
            .press(&[enter])
            .type_text(ledger)
            .press(&[enter])
            .type_text(place)
            .press(&[enter])
    } else {
        start
            .type_text(party)
            .press(&[enter])
            .type_text(reference)
            .press(&[enter])
            .type_text(reference_date)
            .press(&[enter])
            .type_text(ledger)
            .press(&[enter])
            .type_text(place)
            .press(&[enter])
    }
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

/// The create-ledger form filled as the mock draws it (index.html:1790-1810), by clicks on the
/// named widgets, since no key opens the Under list or ticks its boxes: Alt+C opens the form, the
/// name is typed, Under is Sundry Debtors (the list is scrolled until the row shows), Bill-wise and
/// Party details are on, the credit days are 30 and `gstin` is typed.
fn jadhav_form(state: &'static str, gstin: &'static str) -> Shot {
    shot(state, &[(Key::M, NONE), (Key::C, Modifiers::ALT)])
        .click_named("Name")
        .type_text("Jadhav Irrigation Services")
        .click_named("Under")
        .click_named_scrolling("Sundry Debtors", "Bank Accounts")
        .click_named("Bill-wise")
        .click_named("Party details")
        // The credit days come before the GSTIN: its check line moves the rows below it.
        .click_named("Credit days")
        .type_text("30")
        .click_named("GSTIN")
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

/// The Hindi set: the same flows as the English shots they copy, with the shell's language set to
/// Hindi before the keys. Each is named for its mock state (`hi.<screen>.<state>`, all in
/// `design/mock-v2/states.json`). Flows that pick rows by their English name keep working because
/// the names they pick are data (party and ledger names), not translated words.
fn hindi_shots(english: &[Shot]) -> Vec<Shot> {
    const COPIES: [(&str, &str); 8] = [
        ("home.default", "hi.home.default"),
        ("sales-invoice.default", "hi.sales-invoice.default"),
        (
            "sales-invoice.print-preview",
            "hi.sales-invoice.print-preview",
        ),
        ("masters.ledgers", "hi.masters.ledgers"),
        ("balance-sheet.default", "hi.balance-sheet.default"),
        ("gst.gstr1", "hi.gst.gstr1"),
        ("bank.default", "hi.bank.default"),
        ("setup.welcome", "hi.setup.welcome"),
    ];
    COPIES
        .iter()
        .filter_map(|(from, to)| {
            english.iter().find(|s| s.state == *from).map(|s| {
                let mut copy = s.clone().lang("hi");
                copy.state = to;
                copy
            })
        })
        .collect()
}

fn shots() -> Vec<Shot> {
    let mut light = light_shots();
    light.extend(s13_shots());
    light.extend(hindi_shots(&light));
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
        shot("home.default", &[]).wait(60).expect("Home", None),
        // The possible-double-payment nudge's Compare button (rule D-02) opens the comparison.
        shot("home.nudge-open", &[])
            .click_named("Compare")
            .expect_bar("Home", "Compare vouchers"),
        // Recent needs a choice first: open the balance sheet from the palette, go back, and
        // reopen the palette on the empty query (index.html:1115 lists recents there).
        shot("palette.default", &[palette])
            .query("Balance sheet")
            .then(&[(Key::Enter, NONE), ESC, palette]),
        shot("palette.search", &[palette]).query("bal"),
        shot("palette.voucher", &[palette]).query("#0412"),
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
        // AI and privacy is reached from the palette. Release 1.0.0 has no AI (owner decision of
        // 2026-11-26): Off is selected, Local and Cloud are disabled, and key 1 is the only tier
        // key. The mock's ai-settings.local and .cloud states, and the assistant screens' AI-off
        // states (ask, bill photo, plain-words entry), are not reachable and are not captured.
        shot("ai-settings.off", &[palette])
            .query("AI and privacy")
            .then(&[(Key::Enter, NONE), (Key::Num1, NONE)]),
        // The tabs are 1 GSTR-1, 2 GSTR-3B, 3 reconciliation (`screens/gst.rs`). The capture
        // company has no 2B data, so the reconciliation tab is the mock's empty state.
        shot("gst.gstr3b", &[(Key::G, NONE), (Key::Num2, NONE)])
            .wait(60)
            .expect("GST returns", None),
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
        // The mock's `sales-invoice.mode-list`: the default invoice with Ctrl+H pressed; the list of
        // modes is open on Item invoice. The list's highlighted row is the cursor, so the check
        // proves the list is showing.
        three_lines(godavari_invoice(
            "sales-invoice.mode-list",
            "Shree Ganesh",
            "27",
            "SGT/PO/1131",
        ))
        .press(&[(Key::Enter, NONE)])
        .type_text(SI_NARRATION)
        .press(&[(Key::H, Modifiers::COMMAND)])
        .expect_in("Sales invoice", "Sales invoice", "Item invoice"),
        // The mock's `sales-invoice.as-voucher`: Ctrl+H, Down, Down, Enter (As voucher in the list), then the party, the Sales ledger
        // for the taxable 1,30,085.00 (25 x 3,850.00 + 67 x 505.00) and the two GST ledgers with
        // no amount typed (the engine fills them). A blank ledger ends the lines; the narration is typed.
        shot("sales-invoice.as-voucher", &[(Key::F8, NONE)])
            .on(Fixture::Godavari)
            .press(&[
                (Key::H, Modifiers::COMMAND),
                (Key::ArrowDown, NONE),
                (Key::ArrowDown, NONE),
                (Key::Enter, NONE),
            ])
            .type_text("Shree Ganesh")
            .press(&[(Key::Enter, NONE)])
            .type_text("27")
            .press(&[(Key::Enter, NONE), (Key::Enter, NONE)])
            .type_text("Sales")
            .press(&[(Key::Enter, NONE)])
            .type_text("130085")
            .press(&[(Key::Enter, NONE)])
            .type_text("CGST")
            .press(&[(Key::Enter, NONE), (Key::Enter, NONE)])
            .type_text("SGST")
            .press(&[(Key::Enter, NONE), (Key::Enter, NONE), (Key::Enter, NONE)])
            .type_text(SI_NARRATION)
            .expect("Sales invoice", None),
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
            .type_text("Freight, Vardhaman Logistics, LR 4471")
            // The mock stops mid-entry with the keyboard back on row 1 (narration filled):
            // Shift+Enter steps back from the narration to the amount and then to the ledger of
            // the first particular.
            .press(&[SHIFT_ENTER, SHIFT_ENTER])
            .expect("Payment", None),
        // A payment just opened: the keyboard is on the Account, whose list holds only the cash
        // and bank ledgers (mock `payment.account-list`).
        shot("payment.account-list", &[(Key::F5, NONE)]).expect("Payment", None),
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
            // The keyboard is on the first bill's amount, which holds the offered default.
            .press(&[(Key::End, NONE)])
            .press(&[(Key::Backspace, NONE); 12])
            .type_text("200000")
            // Enter goes to the second bill, whose amount is empty.
            .press(&[(Key::Enter, NONE)])
            .type_text("50000")
            // Enter on the last bill accepts the details; Enter on the empty second line goes to
            // the narration.
            .press(&[(Key::Enter, NONE), (Key::Enter, NONE)])
            .type_text("RTGS to Nilgiri Steel Traders")
            // The mock keeps the keyboard on the first particular, as `payment.default`.
            .press(&[SHIFT_ENTER, SHIFT_ENTER])
            .expect("Payment", None),
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
            .then(&[(Key::Enter, NONE)])
            .seek("Saptagiri Commercial Bank A/c")
            .press(&[(Key::Enter, NONE)])
            .expect("Bank reconciliation: Saptagiri Commercial Bank A/c", None)
            .press(&[(Key::B, NONE)])
            .wait(30)
            .expect("Bank reconciliation: Saptagiri Commercial Bank A/c", None),
        // Ctrl+E on the Statement tab writes the reconciliation statement; the save dialog is
        // scripted to a new .csv file, so the screen says where it went.
        shot("bank.brs-exported", &[palette])
            .query("Bank Reconciliation")
            .then(&[(Key::Enter, NONE)])
            .seek("Saptagiri Commercial Bank A/c")
            .press(&[(Key::Enter, NONE)])
            .expect("Bank reconciliation: Saptagiri Commercial Bank A/c", None)
            .press(&[(Key::B, NONE)])
            .wait(30)
            .script_save(SaveTarget::WritableCsv)
            .press(&[(Key::E, Modifiers::COMMAND)])
            .wait(90)
            .expect("Bank reconciliation: Saptagiri Commercial Bank A/c", None),
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
            .click_named("State")
            .click_named_scrolling("24 Gujarat", "01 Jammu and Kashmir")
            .expect_bar("Company setup", "Setup, step 2 of 7"),
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
        // The mock's `profit-and-loss.detailed`: Alt+F1 on the profit and loss opens each group.
        shot("profit-and-loss.detailed", &[palette])
            .query("Profit and Loss")
            .then(&[(Key::Enter, NONE), (Key::F1, Modifiers::ALT)])
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
            .click_named("Edit log")
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
        // The buyer's address has no PIN code, which the e-invoice schema requires: the e-Invoice
        // cell says what is missing (the mock's `sales-invoice.einvoice-not-ready`).
        three_lines(godavari_invoice(
            "sales-invoice.einvoice-not-ready",
            "Shree Ganesh",
            "27",
            "",
        ))
        .on(Fixture::GodavariBuyerNoPin)
        .press(&[(Key::Enter, NONE)])
        .expect("Sales invoice", None),
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
        // Line 2 has an item and its quantity is cleared again, line 3 has a name that is no item
        // (Enter refuses it, so it stays typed in the row), then Ctrl+A: the save is refused and
        // both reasons are listed. Shift+Enter goes back from line 3 to line 2's quantity.
        invoice_line(
            invoice_line(
                godavari_invoice("sales-invoice.validation-error", "Shree Ganesh", "27", ""),
                "Drip lateral",
                "25",
                "3850",
            ),
            "Brass nozzle",
            "67",
            "505",
        )
        .type_text("Drip kit deluxe")
        .press(&[
            (Key::Enter, Modifiers::SHIFT),
            (Key::Enter, Modifiers::SHIFT),
        ])
        .press(&[(Key::End, NONE)])
        .press(&[(Key::Backspace, NONE); 4])
        .press(&[(Key::A, Modifiers::COMMAND)]),
        // The mock's `sales-invoice.period-locked` (index.html `SCREENS['sales-invoice']`, state
        // period-locked): SI/0418 to Shree Ganesh Traders dated 28 Oct 2026, two lines (25 drip
        // laterals at 3,850.00 and 67 brass nozzle sets at 505.00), no order number and no
        // narration; 31 Oct is the lock date of the company. A click puts the keyboard in the date
        // (Ctrl+A is Save on this screen, so the date is cleared with End and Backspace). Enter
        // then goes on to the party, and a blank line ends the lines.
        invoice_line(
            invoice_line(
                shot("sales-invoice.period-locked", &[(Key::F8, NONE)])
                    .on(Fixture::Godavari)
                    .click_named("Date")
                    .press(&[(Key::End, NONE)])
                    .press(&[(Key::Backspace, NONE); 12])
                    .type_text("28-10-2026")
                    .press(&[(Key::Enter, NONE)])
                    .type_text("Shree Ganesh")
                    .press(&[(Key::Enter, NONE)])
                    .type_text("27")
                    .press(&[(Key::Enter, NONE)])
                    .type_text("Sales")
                    .press(&[(Key::Enter, NONE), (Key::Enter, NONE)]),
                "Drip lateral",
                "25",
                "3850",
            ),
            "Brass nozzle",
            "67",
            "505",
        )
        .press(&[(Key::Enter, NONE)]),
        // The mock's `purchase.default` (index.html:1960-1990): Nilgiri Steel Traders' bill NST/1150
        // of 21 Nov 2026, 10 HDPE pipe coils at 2,050.00 and 100 PVC ball valves at 228.40, hand
        // typed. The mock labels the place of supply "29 Karnataka, inter-state"; the engine takes
        // the place typed here (27, delivered in Maharashtra), so the tax is IGST either way.
        invoice_line(
            invoice_line(
                godavari_bill(
                    purchase_start("purchase.default"),
                    "Purchase",
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
        // The mock's `purchase.validation-error`: line 1 is HDPE at 2,050.00 with the quantity
        // left out (Tab skips it; Enter on an empty quantity is refused), line 2 is a name no item
        // has, "Brass elbow 25 mm", 50 at 228.40. Ctrl+A is refused with both reasons.
        godavari_bill(
            purchase_start("purchase.validation-error"),
            "Purchase",
            "Nilgiri",
            "27",
            "Purchases",
            "NST/1150",
            "21-11-2026",
        )
        .type_text("HDPE")
        .press(&[(Key::Enter, NONE), (Key::Tab, NONE)])
        .type_text("2050")
        .press(&[(Key::Enter, NONE)])
        .type_text("Brass elbow 25 mm")
        .press(&[(Key::Tab, NONE)])
        .type_text("50")
        .press(&[(Key::Tab, NONE)])
        .type_text("228.40")
        .press(&[(Key::A, Modifiers::COMMAND)])
        .expect("Purchase", None),
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
            "Brass nozzle",
            "10",
            "505",
        )
        .type_text("Drip lateral")
        .press(&[(Key::Enter, NONE), (Key::A, Modifiers::COMMAND)]),
        // The mock's `debit-note.default`: 2 HDPE pipe coils of Rupa Polymers' bill RP/5398
        // (15 Oct 2026) returned at the bill's 2,050.00.
        invoice_line(
            godavari_bill(
                shot("debit-note.default", &[(Key::F5, Modifiers::ALT)]).on(Fixture::Godavari),
                "Debit Note",
                "Rupa",
                "27",
                "Purchases",
                "RP/5398",
                "15-10-2026",
            ),
            "HDPE",
            "2",
            "2050",
        )
        .press(&[(Key::Enter, NONE)])
        .type_text("Returned 2 coils, cracked in transit"),
        // The mock's `debit-note.validation-error`: a second line with an item and no quantity.
        invoice_line(
            godavari_bill(
                shot("debit-note.validation-error", &[(Key::F5, Modifiers::ALT)])
                    .on(Fixture::Godavari),
                "Debit Note",
                "Rupa",
                "27",
                "Purchases",
                "RP/5398",
                "15-10-2026",
            ),
            "HDPE",
            "2",
            "2050",
        )
        .type_text("PVC ball")
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
            .click_named_start("Narration")
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

/// Round S13: the states that had no app capture. Each is named by its mock state id.
fn s13_shots() -> Vec<Shot> {
    let palette = (Key::K, Modifiers::COMMAND);
    let new_tab = (Key::T, Modifiers::COMMAND);
    vec![
        // Ctrl+T with a company open: a second Home tab.
        // AI off (the AI and privacy screen, key 1), then the bank chooser and the sample's
        // Saptagiri account: exact and rule matches only.
        shot("bank.offline-ai", &[palette])
            .query("AI and privacy")
            .then(&[(Key::Enter, NONE), (Key::Num1, NONE), ESC, palette])
            .type_text("Bank Reconciliation")
            .press(&[(Key::Enter, NONE)])
            .seek("Saptagiri Commercial Bank A/c")
            .press(&[(Key::Enter, NONE)])
            .expect("Bank reconciliation: Saptagiri Commercial Bank A/c", None),
        shot("import.default", &[palette])
            .query("Import from another app")
            .then(&[(Key::Enter, NONE)])
            .on(Fixture::Blank)
            .expect_in(
                "Import from another app",
                "Import from another app",
                "Import in one step",
            ),
        // Field and button positions are from the capture of import.default (1280 x 800).
        shot("import.validation-error", &[palette])
            .query("Import from another app")
            .then(&[(Key::Enter, NONE)])
            .on(Fixture::Blank)
            .press(&[(Key::Enter, NONE)])
            .wait(20)
            .expect_in(
                "Import from another app",
                "Import from another app",
                "Error: enter the name of the company in the other app",
            ),
        shot("import.port-error", &[palette])
            .query("Import from another app")
            .then(&[(Key::Enter, NONE)])
            .on(Fixture::Blank)
            .click_named("Port")
            .press(&[(Key::A, Modifiers::COMMAND)])
            .type_text("99999")
            .click_named("Company in the other app")
            .type_text("Godavari Drip & Pump Works")
            .press(&[(Key::Enter, NONE)])
            .wait(20)
            .expect_in(
                "Import from another app",
                "Import from another app",
                "Error: \"99999\" is not a port number (1 to 65535)",
            ),
        shot("import.running", &[palette])
            .query("Import from another app")
            .then(&[(Key::Enter, NONE)])
            .on(Fixture::GodavariHeld)
            .click_named("Company in the other app")
            .type_text("Godavari Drip & Pump Works")
            .press(&[(Key::Enter, NONE)])
            .wait(20)
            .expect_in("Import from another app", "Importing", "Importing..."),
        shot("import.refused", &[palette])
            .query("Import from another app")
            .then(&[(Key::Enter, NONE)])
            .click_named("Company in the other app")
            .type_text("Godavari Drip & Pump Works")
            .press(&[(Key::Enter, NONE)])
            .wait(60)
            .expect_in("Import from another app", "Import from another app", "Not supported yet: Import from the other program into this company (the company already holds data, or a period is locked. An import needs a new company file, so that nothing is merged or overwritten. Nothing was imported)"),
        shot("import.imported", &[palette])
            .query("Import from another app")
            .then(&[(Key::Enter, NONE)])
            .on(Fixture::Blank)
            .click_named("Host")
            .press(&[(Key::A, Modifiers::COMMAND)])
            .type_text("127.0.0.1")
            .click_named("Port")
            .press(&[(Key::A, Modifiers::COMMAND)])
            .type_text(fake_import_server_port())
            .click_named("Company in the other app")
            .type_text("Godavari Drip & Pump Works")
            .press(&[(Key::Enter, NONE)])
            .wait(60)
            .expect_in("Import from another app", "Import from another app", "Imported from the other program's company \"Godavari Drip & Pump Works\": 6 events committed."),
        shot("import.failed", &[palette])
            .query("Import from another app")
            .then(&[(Key::Enter, NONE)])
            .on(Fixture::Blank)
            .click_named("Company in the other app")
            .type_text("Godavari Drip & Pump Works")
            .press(&[(Key::Enter, NONE)])
            .wait(60)
            .expect_in("Import from another app", "Import from another app", "Error: could not import \"Godavari Drip & Pump Works\" from the other app at localhost:9000: The other program is not running or not accepting connections on port 9000"),
        // Right from Create is the Import card (never Down, which reaches Open).
        shot(
            "import.no-company",
            &[(Key::ArrowRight, NONE), (Key::Enter, NONE)],
        )
        .on(Fixture::Cold)
        .expect_in(
            "Import from another app",
            "Import from another app",
            "No company is open yet.",
        ),
        shot("start.recent", &[])
            .on(Fixture::ColdRecent)
            .expect("Start", Some("Godavari Drip & Pump Works")),
        shot("home.new-tab", &[new_tab]).expect("Home", None),
        // Ctrl+T and Ctrl+\ before any company is open.
        shot("setup.new-tab", &[new_tab]).on(Fixture::Cold),
        shot("setup.rail", &[(Key::Backslash, Modifiers::COMMAND)]).on(Fixture::Cold),
        shot("home.gst-unsupported", &[]).on(Fixture::GodavariNo2b),
        // The worker is held, so each report's request never answers: the computing card.
        shot("trial-balance.loading", &[palette])
            .query("Trial Balance")
            .then(&[(Key::Enter, NONE)])
            .on(Fixture::GodavariHeld)
            .expect_bar("Trial balance", "Computing"),
        shot("profit-and-loss.loading", &[palette])
            .query("Profit and Loss")
            .then(&[(Key::Enter, NONE)])
            .on(Fixture::GodavariHeld)
            .expect_bar("Profit and loss", "Computing"),
        shot("outstanding.loading", &[palette])
            .query("Receivables Outstanding")
            .then(&[(Key::Enter, NONE)])
            .on(Fixture::GodavariHeld)
            .expect_bar("Receivables", "Computing"),
        shot("stock-summary.loading", &[palette])
            .query("Stock Summary")
            .then(&[(Key::Enter, NONE)])
            .on(Fixture::GodavariHeld)
            .expect_bar("Stock summary", "Computing"),
        shot("profit-and-loss.detailed", &[palette])
            .query("Profit and Loss")
            .then(&[(Key::Enter, NONE)])
            .expect_bar("Profit and loss", "Profit and loss")
            .press(&[(Key::F1, Modifiers::ALT)])
            .expect_bar("Profit and loss", "Profit and loss"),
        // Ctrl+P opens the preview of the engine's pages; Ctrl+P there saves, so the saved and
        // failed states are two presses, with the preview's key bar between them.
        shot("profit-and-loss.print-preview", &[palette])
            .query("Profit and Loss")
            .then(&[(Key::Enter, NONE)])
            .expect_bar("Profit and loss", "Profit and loss")
            .press(&[(Key::P, Modifiers::COMMAND)])
            .expect_bar("Profit and loss", "Print preview"),
        // The worker is held once the report is up, so the PDF stays "preparing".
        shot("profit-and-loss.print-busy", &[palette])
            .query("Profit and Loss")
            .then(&[(Key::Enter, NONE)])
            .expect_bar("Profit and loss", "Profit and loss")
            .hold_worker()
            .press(&[(Key::P, Modifiers::COMMAND)])
            .wait(30)
            .expect_bar("Profit and loss", "Profit and loss"),
        shot("profit-and-loss.print-saved", &[palette])
            .query("Profit and Loss")
            .then(&[(Key::Enter, NONE)])
            .expect_bar("Profit and loss", "Profit and loss")
            .press(&[(Key::P, Modifiers::COMMAND)])
            .expect_bar("Profit and loss", "Print preview")
            .script_save(SaveTarget::Writable)
            .press(&[(Key::P, Modifiers::COMMAND)])
            .wait(120)
            .expect_bar("Profit and loss", "Profit and loss"),
        shot("profit-and-loss.print-failed", &[palette])
            .query("Profit and Loss")
            .then(&[(Key::Enter, NONE)])
            .expect_bar("Profit and loss", "Profit and loss")
            .press(&[(Key::P, Modifiers::COMMAND)])
            .expect_bar("Profit and loss", "Print preview")
            .script_save(SaveTarget::MissingFolder)
            .press(&[(Key::P, Modifiers::COMMAND)])
            .wait(120)
            .expect_bar("Profit and loss", "Profit and loss"),
        // The settings in the preview's card: A5, 2 copies, page 1 only (the mock's
        // `profit-and-loss.print-config`), then a range past the last page.
        shot("profit-and-loss.print-config", &[palette])
            .query("Profit and Loss")
            .then(&[(Key::Enter, NONE)])
            .expect_bar("Profit and loss", "Profit and loss")
            .press(&[(Key::P, Modifiers::COMMAND)])
            .expect_bar("Profit and loss", "Print preview")
            .click_named("A5")
            .click_named("Copies")
            .press(&[SELECT_ALL])
            .type_text("2")
            .click_named("Pages to print")
            .type_text("1")
            .press(&[(Key::Enter, NONE)])
            .wait(60)
            .expect_bar("Profit and loss", "Print preview"),
        shot("profit-and-loss.print-range-error", &[palette])
            .query("Profit and Loss")
            .then(&[(Key::Enter, NONE)])
            .expect_bar("Profit and loss", "Profit and loss")
            .press(&[(Key::P, Modifiers::COMMAND)])
            .expect_bar("Profit and loss", "Print preview")
            .click_named("Pages to print")
            .type_text("5-6")
            .press(&[(Key::Enter, NONE)])
            .wait(60)
            .expect_bar("Profit and loss", "Print preview"),
        // The invoice, the day book and a journal printed through the page renderer.
        // SI/0412, two items with two HSN codes: the tax summary has one row for each code.
        shot(
            "sales-invoice.print-preview",
            &[(Key::D, NONE), (Key::F2, Modifiers::ALT)],
        )
        .then(&[SELECT_ALL])
        .type_text("01-11-2026")
        .press(&[(Key::Tab, NONE), SELECT_ALL])
        .type_text("20-11-2026")
        .press(&[(Key::Enter, NONE)])
        .seek_by(Key::K, NONE, "Shree Ganesh Traders")
        .press(&[(Key::Enter, NONE)])
        .expect("SI/0412", None)
        .press(&[(Key::P, Modifiers::COMMAND)])
        .wait(60)
        .expect_bar("Sales invoice", "Print preview"),
        shot("day-book.print-preview", &[(Key::D, NONE)])
            .expect("Day book", None)
            .press(&[(Key::P, Modifiers::COMMAND)])
            .wait(60)
            .expect_bar("Day book", "Print preview"),
        shot("journal.print-preview", &[(Key::D, NONE)])
            .expect("Day book", None)
            .seek("Depreciation")
            .press(&[(Key::Enter, NONE)])
            .expect("JV/0032", None)
            .press(&[(Key::P, Modifiers::COMMAND)])
            .wait(60)
            .expect_bar("Journal", "Print preview"),
        // The worker is held: the GST returns have not been asked for yet.
        shot("gst.gstr1-not-loaded", &[(Key::G, NONE), (Key::Num1, NONE)])
            .on(Fixture::GodavariHeld)
            .expect("GST returns", None),
        shot(
            "gst.gstr3b-not-loaded",
            &[(Key::G, NONE), (Key::Num2, NONE)],
        )
        .on(Fixture::GodavariHeld)
        .expect("GST returns", None),
        // October's GSTR-3B: the engine refuses it, because a ledger line has no GST
        // classification (a real refusal of the sample's books).
        shot(
            "gst.gstr3b-unsupported",
            &[(Key::G, NONE), (Key::Num2, NONE)],
        )
        .expect("GST returns", None)
        .press(&[(Key::F2, Modifiers::ALT), (Key::K, NONE)])
        .expect("GST returns", Some("October 2026"))
        .press(&[(Key::Enter, NONE)])
        .wait(90)
        .expect("GST returns", None),
        shot("audit.loading", &[palette])
            .query("Integrity Certificate")
            .then(&[(Key::Enter, NONE)])
            .on(Fixture::GodavariHeld)
            .expect("Audit", None),
        shot("audit.unsupported", &[palette])
            .query("Integrity Certificate")
            .then(&[(Key::Enter, NONE)])
            .expect_bar("Audit", "Audit")
            .click_named("Edits after lock")
            .wait(60)
            .expect_bar("Audit", "Audit"),
        shot("masters.loading", &[(Key::M, NONE)]).on(Fixture::GodavariHeld),
        shot("masters.no-match", &[(Key::M, NONE)]).type_text("zzqx"),
        shot("masters.empty", &[(Key::M, NONE)])
            .on(Fixture::Blank)
            .expect("Masters", Some("Ledgers")),
        shot("masters.items-empty", &[(Key::M, NONE)])
            .on(Fixture::BlankInventory)
            .seek_by(Key::ArrowRight, Modifiers::ALT, "Stock items")
            .expect("Masters", Some("Stock items")),
        shot("masters.create-item", &[(Key::M, NONE)])
            .seek_by(Key::ArrowRight, Modifiers::ALT, "Brass nozzle set (6 pc)")
            .press(&[(Key::C, Modifiers::ALT)])
            .expect_bar("Masters", "Stock item"),
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
    /// The named widgets of the last frame, for [`Inject::ClickNamed`].
    named: Vec<ledgerly_app::ui::capture::NamedRect>,
    /// Where the wheel turns for the front `ClickNamedScrolling`, once its anchor row was found.
    scroll_at: Option<egui::Pos2>,
    /// True while the modifiers of an injected key are still reported as held.
    modifiers_held: bool,
}

impl Snapshots {
    /// One step of a `Seek`: done when the cursor is on `label`, otherwise one J press (after
    /// the screen had time to load its rows).
    fn seek_step(
        &mut self,
        label: &'static str,
        key: Key,
        modifiers: Modifiers,
        raw_input: &mut egui::RawInput,
    ) {
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
            self.shell
                .tabs()
                .active_stack()
                .map(|stack| stack.top_title(&ledgerly_app::i18n::Tr::default())),
            self.seek_presses,
        );
        for pressed in [true, false] {
            raw_input.events.push(Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
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
            .map_or_default(|stack| stack.top_title(&ledgerly_app::i18n::Tr::default()));
        let key_bar = self.shell.top_key_bar_name();
        let cursor = self.shell.top_cursor_label();
        let reached = title == expect.title
            && expect
                .key_bar
                .is_none_or(|k| key_bar == k || key_bar == ledgerly_app::i18n::lit(k))
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

    /// One step of a `ClickNamed`: the click once exactly one widget has the name, otherwise a
    /// wait for the screen to draw it.
    fn click_named_step(
        &mut self,
        name: &'static str,
        scroll_over: Option<&'static str>,
        at_start: bool,
        raw_input: &mut egui::RawInput,
    ) {
        let found = if at_start {
            ledgerly_app::ui::capture::find_named_start(&self.named, name)
        } else {
            ledgerly_app::ui::capture::find_named(&self.named, name)
        };
        match found {
            Ok(pos) => {
                self.inject.pop_front();
                self.reach_frames = 0;
                self.scroll_at = None;
                self.gap = 6;
                push_click(raw_input, pos);
            }
            Err(e) => {
                self.reach_frames += 1;
                assert!(
                    self.reach_frames < REACH_FRAMES_MAX,
                    "{}: {e}",
                    self.shots[self.index].label(),
                );
                // A row of a list that has not scrolled to it yet: one notch, then a pause for
                // the scroll to settle.
                if let Some(over) = scroll_over
                    && let Some(pos) = self
                        .scroll_at
                        .or_else(|| ledgerly_app::ui::capture::find_named(&self.named, over).ok())
                {
                    // The wheel stays where the anchor row was found, over the list, after
                    // that row has itself scrolled out of view.
                    self.scroll_at = Some(pos);
                    raw_input.events.push(Event::PointerMoved(pos));
                    raw_input.events.push(Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, SCROLL_NOTCH),
                        phase: egui::TouchPhase::Move,
                        source: egui::MouseWheelSource::Unknown,
                        modifiers: Modifiers::NONE,
                    });
                    self.gap = NOTCH_SETTLE_FRAMES;
                }
            }
        }
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
            && self.shell.tabs().active_stack().is_some_and(|stack| {
                stack.depth() == 1
                    && stack.top_title(&ledgerly_app::i18n::Tr::default()) != HOME_COMPARE_TITLE
            })
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

/// A primary click at `pos`.
fn push_click(raw_input: &mut egui::RawInput, pos: egui::Pos2) {
    // The capture window has no window manager, so it is never given the keyboard; egui reports
    // no widget as focused then, and a click could not move the keyboard. A click on a window is
    // what gives it the keyboard.
    raw_input.focused = true;
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

impl eframe::App for Snapshots {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        // No hover from the real pointer or an earlier click may reach a capture.
        ledgerly_app::ui::capture::park_pointer(raw_input);
        if self.modifiers_held {
            // The modifiers of the last injected key are let go a frame after it.
            raw_input
                .events
                .push(Event::ModifiersChanged(Modifiers::NONE));
            self.modifiers_held = false;
        }
        if self.gap > 0 {
            self.gap -= 1;
            return;
        }
        match self.inject.front().copied() {
            Some(Inject::Seek(label)) => {
                self.seek_step(label, Key::J, Modifiers::NONE, raw_input);
                return;
            }
            Some(Inject::SeekBy(key, modifiers, label)) => {
                self.seek_step(label, key, modifiers, raw_input);
                return;
            }
            Some(Inject::Expect(expect)) => {
                self.expect_step(expect);
                return;
            }
            Some(Inject::ClickNamed(name)) => {
                self.click_named_step(name, None, false, raw_input);
                return;
            }
            Some(Inject::ClickNamedScrolling(name, visible)) => {
                self.click_named_step(name, Some(visible), false, raw_input);
                return;
            }
            Some(Inject::ClickNamedStart(name)) => {
                self.click_named_step(name, None, true, raw_input);
                return;
            }
            Some(_) | None => {}
        }
        if let Some(item) = self.inject.pop_front() {
            // A wheel scroll is animated over several frames; the next click waits for it to end.
            self.gap = match item {
                Inject::Wait(frames) => frames,
                Inject::Seek(_)
                | Inject::SeekBy(..)
                | Inject::Expect(_)
                | Inject::ClickNamed(_)
                | Inject::ClickNamedStart(_)
                | Inject::ClickNamedScrolling(..) => 0,
                Inject::HoldWorker
                | Inject::Key(..)
                | Inject::Text(_)
                | Inject::ScriptFile(_)
                | Inject::ScriptSave(_) => 6,
            };
            match item {
                Inject::Key(key, modifiers) => {
                    // A real keyboard reports the held modifiers as state too, and some screens
                    // read that state (Shift+Enter is "back" by `modifiers.shift`).
                    if modifiers != Modifiers::NONE {
                        raw_input.events.push(Event::ModifiersChanged(modifiers));
                        self.modifiers_held = true;
                    }
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
                Inject::Wait(_)
                | Inject::Seek(_)
                | Inject::SeekBy(..)
                | Inject::Expect(_)
                | Inject::ClickNamed(_)
                | Inject::ClickNamedStart(_)
                | Inject::ClickNamedScrolling(..) => {}
                Inject::HoldWorker => {
                    self.held = self.shell.app_ctx_mut().api.as_ref().map(ApiWorker::hold);
                }
                Inject::ScriptFile(file) => {
                    ledgerly_app::ui::dialogs::script_open_file(file.write(&self.out));
                }
                Inject::ScriptSave(target) => {
                    ledgerly_app::ui::dialogs::script_save_file(target.path(&self.out));
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.shell.show(ui);
        self.named = ledgerly_app::ui::capture::collect_named(ui.ctx());
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
                self.shell.set_language(current.lang);
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
                    self.shell.set_language("en");
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
                .is_none_or(|needles| needles.split('|').any(|needle| s.label().contains(needle)))
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
        named: Vec::new(),
        modifiers_held: false,
        scroll_at: None,
    };
    // CAPTURE_SIZE=<width>x<height> (points) captures at another window size, to check that
    // nothing is cut off on wide or small windows; the registry captures are 1280 x 800.
    let size = std::env::var("CAPTURE_SIZE")
        .ok()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Some([w.parse::<f32>().ok()?, h.parse::<f32>().ok()?])
        })
        .unwrap_or([1280.0, 800.0]);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Ledgerly snapshots")
            .with_inner_size(size),
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "Ledgerly snapshots",
        options,
        Box::new(|cc| {
            // Names the widgets, so a flow can click one by the name a screen reader is told.
            cc.egui_ctx.enable_accesskit();
            Ok(Box::new(app))
        }),
    )
}
