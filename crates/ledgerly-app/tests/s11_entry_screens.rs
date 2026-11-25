//! Entry screens beyond sales, payment and receipt (round s11): the keys that open purchase,
//! journal, credit note and debit note, and what each screen answers.
//!
//! Provenance:
//! - Keys: TallyPrime's keyboard shortcut page lists F9 Purchase, F7 Journal, Alt+F6 Credit Note
//!   and Alt+F5 Debit Note (help.tallysolutions.com/tally-prime/keyboard-shortcuts/
//!   keyboard-shortcuts-tally-prime/, read 2026-11-23), and the classic key map binds them
//!   (`w2_purchase_invoice.rs`). The mock's Home key table has F9 as "Purchase" and its sidebar
//!   row is Purchase F9 (design/mock-v2/index.html:888); the app's sidebar row opens the typed
//!   purchase form, so Home's F9 opens the same form (the bill reader is not built).
//! - Every expected value is derived by hand and the derivation is written next to it.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::time::{Duration, Instant};

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::{Api, ApiError, Command, CommandResult};
use ledgerly_app::Screen;
use ledgerly_app::shell::submit::{self};
use ledgerly_app::shell::{GatewayScreen, ScreenStack, Shell};
use ledgerly_app::worker::ApiWorker;
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
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("s11_entry_screens");
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

/// The breadcrumb trail after `key` is pressed on Home.
fn trail_after(name: &str, key: Key, modifiers: Modifiers) -> (Vec<String>, Option<String>) {
    let ctx = egui::Context::default();
    let mut shell = ready(&ctx, setup(name, 0));
    frame(&ctx, &mut shell, press(key, modifiers));
    frame(&ctx, &mut shell, RawInput::default());
    (titles(&shell), shell.notice().map(str::to_owned))
}

#[test]
fn f9_on_home_opens_the_purchase_form_and_says_nothing_is_missing() {
    // derived: the sidebar's Purchase row (F9, mock index.html:888) opens the purchase form
    // (tests/w2_purchase_invoice.rs); Home's F9 is the same key and must open the same form.
    // Before the fix it left the notice "a purchase from a photo of a bill ... not built yet".
    let (trail, notice) = trail_after("f9_home", Key::F9, Modifiers::NONE);
    assert_eq!(notice, None, "{trail:?}");
    assert_eq!(trail.len(), 2, "{trail:?}");
    assert_eq!(trail[0], "Home");
    assert!(trail[1].starts_with("Purchase"), "{trail:?}");
}

#[test]
fn f7_on_home_opens_the_journal_form() {
    // derived: the classic key map binds F7 to a new journal (keymap-classic.toml new-journal).
    let (trail, notice) = trail_after("f7_home", Key::F7, Modifiers::NONE);
    assert_eq!(notice, None, "{trail:?}");
    assert_eq!(trail.len(), 2, "{trail:?}");
    assert!(trail[1].starts_with("Journal"), "{trail:?}");
}

#[test]
fn alt_f6_and_alt_f5_open_the_credit_and_debit_notes() {
    // derived: Alt+F6 Credit Note and Alt+F5 Debit Note, the keyboard shortcut page cited above.
    let (trail, notice) = trail_after("altf6_home", Key::F6, Modifiers::ALT);
    assert_eq!(notice, None, "{trail:?}");
    assert!(
        trail.get(1).is_some_and(|t| t.starts_with("Credit")),
        "{trail:?}"
    );
    let (trail, notice) = trail_after("altf5_home", Key::F5, Modifiers::ALT);
    assert_eq!(notice, None, "{trail:?}");
    assert!(
        trail.get(1).is_some_and(|t| t.starts_with("Debit")),
        "{trail:?}"
    );
}

// ---- the item invoice screen's wording for purchases and notes ----------------------------------

use ledgerly_app::screens::item_invoice::{
    InvoiceDraft, InvoiceError, InvoiceKind, place_note_parts,
};
use ledgerly_core::{FinancialYear, VoucherTypeId};

fn state(code: u8) -> StateCode {
    StateCode::new(code).unwrap()
}

#[test]
fn a_purchase_and_a_debit_note_name_the_supplier_and_the_bill() {
    // derived: the mock's manual purchase form (design/mock-v2/index.html:1504, `manualPurchase`)
    // captions its fields "Supplier", "Supplier bill no." and "Bill date"; a sale keeps "Party".
    // A debit note against a supplier's bill and a credit note against our invoice carry the
    // original invoice's number and date: rule 53(1A) CGST Rules asks for "the serial number and
    // date of the original tax invoice" (taxguru.in/goods-and-service-tax/
    // credit-note-debit-note-section-34-rule-53-cgst-act-2017.html, read 2026-11-25).
    assert_eq!(InvoiceKind::Purchase.party_label(), "Supplier");
    assert_eq!(InvoiceKind::DebitNote.party_label(), "Supplier");
    assert_eq!(InvoiceKind::Sales.party_label(), "Party");
    assert_eq!(InvoiceKind::CreditNote.party_label(), "Party");
    assert_eq!(InvoiceKind::Purchase.reference_label(), "Supplier bill no.");
    assert_eq!(
        InvoiceKind::Purchase.reference_date_label(),
        Some("Bill date")
    );
    assert_eq!(
        InvoiceKind::CreditNote.reference_date_label(),
        Some("Original invoice date")
    );
    assert_eq!(
        InvoiceKind::DebitNote.reference_date_label(),
        Some("Original invoice date")
    );
    // A sale has no such date: its slot holds the e-Invoice status.
    assert_eq!(InvoiceKind::Sales.reference_date_label(), None);
}

#[test]
fn a_typed_bill_date_is_read_and_a_wrong_one_is_refused() {
    // derived: the text "17-11-2026" is 17 November 2026 (day-month-year, as every date field of
    // the app); 31 November does not exist; nothing typed means no date (the field is optional,
    // as TallyPrime's supplier invoice date is not demanded by the voucher).
    let fy = FinancialYear::april(2026).unwrap();
    let mut draft = InvoiceDraft::new(
        VoucherTypeId(1),
        InvoiceKind::Purchase,
        false,
        "21-11-2026".to_owned(),
    );
    assert_eq!(draft.reference_date(fy).unwrap(), None);
    draft.reference_date_text = "17-11-2026".to_owned();
    assert_eq!(
        draft.reference_date(fy).unwrap(),
        Some(date!(2026 - 11 - 17))
    );
    draft.reference_date_text = "31-11-2026".to_owned();
    assert!(matches!(
        draft.reference_date(fy),
        Err(InvoiceError::Date(_))
    ));
}

#[test]
fn a_purchase_note_names_the_supplier_state_not_ours() {
    // derived: IGST Act 2017 s.7 and s.8: a supply is inter-State when the supplier's location and
    // the place of supply are in different States. On a purchase the supplier is the party.
    // Rupa Polymers is in Gujarat (24); goods delivered to us in Maharashtra (27): inter-State.
    let (lead, bold, _rest) = place_note_parts(
        InvoiceKind::Purchase,
        &["IGST"],
        state(27),
        state(27),
        Some(state(24)),
        "p",
    )
    .unwrap();
    assert_eq!(
        lead,
        "Supplier in Gujarat (24), place of supply Maharashtra (27). "
    );
    assert_eq!(bold, "IGST applies.");
    // A Karnataka (29) supplier delivering in Karnataka: intra-State, CGST and SGST.
    let (lead, bold, _rest) = place_note_parts(
        InvoiceKind::Purchase,
        &["CGST", "SGST"],
        state(29),
        state(27),
        Some(state(29)),
        "p",
    )
    .unwrap();
    assert_eq!(
        lead,
        "Supplier and place of supply are both Karnataka (29). "
    );
    assert_eq!(bold, "Tax splits into CGST and SGST.");
    // The same applies to a debit note, which goes back to the supplier.
    let debit = place_note_parts(
        InvoiceKind::DebitNote,
        &["IGST"],
        state(27),
        state(27),
        Some(state(24)),
        "p",
    )
    .unwrap();
    assert!(debit.0.starts_with("Supplier in Gujarat (24)"));
    // Without the supplier's state there is nothing honest to say.
    assert_eq!(
        place_note_parts(
            InvoiceKind::Purchase,
            &["IGST"],
            state(27),
            state(27),
            None,
            "p"
        ),
        None
    );
}

#[test]
fn a_sale_note_still_names_our_state() {
    // derived: the company is the supplier on a sale (and on a credit note); the wording the
    // mock's sales-invoice.default shows (index.html, "Place of supply and your state are both
    // Maharashtra (27)").
    let (lead, bold, _rest) = place_note_parts(
        InvoiceKind::Sales,
        &["CGST", "SGST"],
        state(27),
        state(27),
        None,
        "p",
    )
    .unwrap();
    assert_eq!(
        lead,
        "Place of supply and your state are both Maharashtra (27). "
    );
    assert_eq!(bold, "Tax splits into CGST and SGST.");
    let (lead, bold, _rest) = place_note_parts(
        InvoiceKind::CreditNote,
        &["IGST"],
        state(24),
        state(27),
        None,
        "p",
    )
    .unwrap();
    assert_eq!(
        lead,
        "Place of supply Gujarat (24), you are in Maharashtra (27). "
    );
    assert_eq!(bold, "IGST applies.");
}

#[test]
fn a_credit_note_is_titled_as_a_note_not_as_an_invoice() {
    // derived: the voucher type is "Credit Note" (TallyPrime's name, seeded by
    // `default_voucher_types`); a sale and a purchase are invoices, a note is not one (CGST Act
    // s.34 calls it a credit note, not an invoice).
    let (trail, _) = trail_after("title_credit", Key::F6, Modifiers::ALT);
    assert_eq!(trail.get(1).map(String::as_str), Some("Credit Note"));
    let (trail, _) = trail_after("title_debit", Key::F5, Modifiers::ALT);
    assert_eq!(trail.get(1).map(String::as_str), Some("Debit Note"));
    let (trail, _) = trail_after("title_purchase", Key::F9, Modifiers::NONE);
    assert_eq!(trail.get(1).map(String::as_str), Some("Purchase invoice"));
}
