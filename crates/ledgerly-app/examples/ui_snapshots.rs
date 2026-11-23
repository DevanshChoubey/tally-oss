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
    Clock, CompanyId, Date, Decimal, GroupId, Gstin, Hsn, LedgerId, Paise, Rate, SignedAmount,
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

/// A fictional Maharashtra trader with a bank, two GST buyers and a few November vouchers.
fn company_file(dir: &Path) -> Api {
    let path = dir.join("snapshot-company.ledgerly");
    let _removed = std::fs::remove_file(&path);
    let company = Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Fictional Traders".to_owned(),
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
        features: Features {
            gst: true,
            ..Features::default()
        },
        negative_stock: NegativeStockPolicy::Block,
    };
    let mut api = Api::create(&path, &company, Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
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
        run(&mut api, Command::CreateLedger(l));
    }
    // Two stock items with no opening stock and no movement, so no other screen's figures move:
    // the items list shows the engine's 0 quantities and 0.00 values, one item without an HSN.
    for u in [unit(COIL, "coil", "Coils"), unit(NOS, "nos", "Numbers")] {
        run(&mut api, Command::CreateUnit(u));
    }
    let mut pipe = stock_item(1, "HDPE pipe coil 32 mm", COIL, 18);
    pipe.hsn = Some(Hsn::parse("3917").unwrap());
    for it in [pipe, stock_item(2, "Steel pickaxe 2.5 kg", NOS, 5)] {
        run(&mut api, Command::CreateStockItem(it));
    }
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
        run(&mut api, Command::PostVoucher(v));
    }
    api
}

/// A colour theme a shot is taken in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Theme {
    Light,
    Dark,
    Hc,
}

impl Theme {
    /// The shell's theme for this entry.
    fn kind(self) -> ThemeKind {
        match self {
            Theme::Light => ThemeKind::Light,
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
        }
    }
}

/// One screenshot: the mock state it matches, its theme, and the keys that reach it. The keys go in
/// after the return to Home, then an optional palette query, then the keys after the query.
struct Shot {
    /// The mock state id (`design/mock-v2/states.json`), which also names the image.
    state: &'static str,
    theme: Theme,
    keys: Vec<(Key, Modifiers)>,
    query: Option<&'static str>,
    after: Vec<(Key, Modifiers)>,
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
        self.after = keys.to_vec();
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

/// The registry: one entry per image, named by the mock state id it should match.
fn shots() -> Vec<Shot> {
    let palette = (Key::K, Modifiers::COMMAND);
    vec![
        shot("home.default", &[]),
        shot("home.default", &[]).themed(Theme::Dark),
        shot("home.default", &[]).themed(Theme::Hc),
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
        shot("sales-invoice.default", &[(Key::F8, NONE)]),
        shot("receipt.default", &[(Key::F6, NONE)]),
        shot("payment.default", &[(Key::F5, NONE)]),
        shot("bank.default", &[palette])
            .query("Bank Reconciliation")
            .then(&[(Key::Enter, NONE)]),
        shot("bank.brs", &[palette])
            .query("Bank Reconciliation")
            .then(&[(Key::Enter, NONE), (Key::B, NONE)]),
    ]
}

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
    inject: VecDeque<(Key, Modifiers)>,
    /// Frames to wait before the next queued key.
    gap: u32,
    /// Escape presses so far in the current return to Home.
    presses: u32,
    image: Arc<Mutex<Option<Arc<egui::ColorImage>>>>,
}

impl Snapshots {
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
        if let Some((key, modifiers)) = self.inject.pop_front() {
            for pressed in [true, false] {
                raw_input.events.push(Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers,
                });
            }
            self.gap = 6;
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
                        self.inject.push_back(ESC);
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
                let current = &self.shots[self.index];
                self.shell.set_theme(current.theme.kind());
                self.inject = current.keys.iter().copied().collect();
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
                if self.inject.is_empty() && self.frames > 60 {
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
fn parse_args() -> (PathBuf, Option<String>) {
    const USAGE: &str = "usage: ui_snapshots <output dir> [--only <substring>]";
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect(USAGE));
    let mut only = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--only" => only = Some(args.next().expect("--only needs a substring")),
            other => panic!("unknown argument {other}; {USAGE}"),
        }
    }
    (out, only)
}

fn main() -> eframe::Result<()> {
    let (out, only) = parse_args();
    let entries: Vec<Shot> = shots()
        .into_iter()
        .filter(|s| {
            only.as_deref()
                .is_none_or(|needle| s.label().contains(needle))
        })
        .collect();
    assert!(!entries.is_empty(), "no entry matches --only {only:?}");
    std::fs::create_dir_all(&out).unwrap();
    let api = company_file(&out);
    let mut shell = Shell::new(Box::new(GatewayScreen::new()));
    shell.app_ctx_mut().api = Some(ApiWorker::spawn(api).unwrap());
    submit::note_changed();
    let app = Snapshots {
        shell,
        out,
        shots: entries,
        index: 0,
        stage: Stage::Warm,
        frames: 0,
        inject: VecDeque::new(),
        gap: 0,
        presses: 0,
        image: Arc::new(Mutex::new(None)),
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
