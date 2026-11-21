//! Developer tool: opens the real app window on a fictional company and saves a screenshot of
//! each main screen, so the app can be compared with the design mock (`design/mock/index.html`).
//!
//! Usage: `cargo run -p ledgerly-app --example ui_snapshots -- <output dir>`
//!
//! It writes `NN-<screen>.ppm` files (convert with `ffmpeg -i in.ppm out.png`). The company file
//! is created in the output directory. All names, GSTINs and amounts are fictional; the figures
//! on screen are whatever the engine computes, and this tool asserts none of them.

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
use ledgerly_app::shell::{GatewayScreen, Shell, submit};
use ledgerly_app::theme::ThemeKind;
use ledgerly_app::worker::ApiWorker;
use ledgerly_core::{
    Clock, CompanyId, Date, Decimal, GroupId, Gstin, Hsn, LedgerId, Paise, Rate, SignedAmount,
    StateCode, VoucherId, gstin_check_character,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BankDetails, BaseVoucherKind, Company, EntityKind, Features, GstClassification,
    GstRegistration, Ledger, LedgerLine, NegativeStockPolicy, PartyDetails, RegistrationType,
    ReservedGroup, TaxDetails, TaxLedgerKind, Taxability, Voucher, VoucherMode, VoucherStatus,
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
    for l in [
        ledger(CASH, "Cash", ReservedGroup::CashInHand),
        ledger(CAPITAL, "Capital", ReservedGroup::CapitalAccount),
        bank,
        buyer(BUYER, "Sample Buyer Pvt Ltd", "27FGHIJ5678K1Z", 27),
        buyer(OTHER_BUYER, "Other State Buyer", "29RSTUV3456W1Z", 29),
        sales,
        tax(CGST, "Output CGST", TaxLedgerKind::Cgst),
        tax(SGST, "Output SGST", TaxLedgerKind::Sgst),
        tax(IGST, "Output IGST", TaxLedgerKind::Igst),
        ledger(RENT, "Rent", ReservedGroup::IndirectExpenses),
    ] {
        run(&mut api, Command::CreateLedger(l));
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
                dr(BUYER, 118_000),
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

/// One screenshot: keys pressed first, then an optional palette query and the keys after it.
struct Shot {
    name: &'static str,
    keys: Vec<(Key, Modifiers)>,
    query: Option<&'static str>,
    after: Vec<(Key, Modifiers)>,
    theme: Option<ThemeKind>,
}

fn shot(name: &'static str, keys: &[(Key, Modifiers)]) -> Shot {
    Shot {
        name,
        keys: keys.to_vec(),
        query: None,
        after: Vec::new(),
        theme: None,
    }
}

const NONE: Modifiers = Modifiers::NONE;
const ESC: (Key, Modifiers) = (Key::Escape, Modifiers::NONE);

fn shots() -> Vec<Shot> {
    let palette = (Key::K, Modifiers::COMMAND);
    vec![
        shot("home", &[]),
        shot("palette", &[palette]),
        shot("balance-sheet", &[ESC, (Key::B, NONE)]),
        shot("day-book", &[ESC, (Key::D, NONE)]),
        shot("gst-returns", &[ESC, (Key::G, NONE)]),
        shot("masters", &[ESC, (Key::M, NONE)]),
        shot("sales-invoice", &[ESC, (Key::F8, NONE)]),
        shot("receipt", &[ESC, (Key::F6, NONE)]),
        Shot {
            name: "bank-reconciliation",
            keys: vec![ESC, palette],
            query: Some("Bank Reconciliation"),
            after: vec![(Key::Enter, NONE)],
            theme: None,
        },
        Shot {
            name: "home-dark",
            keys: vec![ESC],
            query: None,
            after: Vec::new(),
            theme: Some(ThemeKind::Dark),
        },
    ]
}

/// Where the snapshot run is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// Waiting for the books and the first figures.
    Warm,
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
    image: Arc<Mutex<Option<Arc<egui::ColorImage>>>>,
}

impl Snapshots {
    fn next_stage(&mut self, stage: Stage) {
        self.stage = stage;
        self.frames = 0;
    }

    fn write_shot(&self, image: &egui::ColorImage) {
        let name = self.shots[self.index].name;
        let path = self.out.join(format!("{:02}-{name}.ppm", self.index + 1));
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
                    self.next_stage(Stage::Keys);
                }
            }
            Stage::Keys => {
                let current = &self.shots[self.index];
                if let Some(theme) = current.theme {
                    self.shell.set_theme(theme);
                }
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
                    self.index += 1;
                    if self.index == self.shots.len() {
                        self.next_stage(Stage::Done);
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    } else {
                        self.next_stage(Stage::Keys);
                    }
                } else {
                    assert!(self.frames < 600, "no screenshot arrived");
                }
            }
            Stage::Done => {}
        }
    }
}

fn main() -> eframe::Result<()> {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("usage: ui_snapshots <output dir>"),
    );
    std::fs::create_dir_all(&out).unwrap();
    let api = company_file(&out);
    let mut shell = Shell::new(Box::new(GatewayScreen::new()));
    shell.app_ctx_mut().api = Some(ApiWorker::spawn(api).unwrap());
    submit::note_changed();
    let app = Snapshots {
        shell,
        out,
        shots: shots(),
        index: 0,
        stage: Stage::Warm,
        frames: 0,
        inject: VecDeque::new(),
        gap: 0,
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
