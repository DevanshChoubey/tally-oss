//! Card S16: printing the vouchers that are not tax invoices (payment, receipt, journal, contra,
//! purchase) and printing with the paper, copies and pages a person set.
//!
//! Provenance: the figures are `derived`: Rs 9,12,000 is `91_200_000` paise and prints in Indian
//! grouping as "9,12,000.00" (ten lakh grouping: the last three digits, then pairs). The help
//! pages (<https://help.tallysolutions.com/print-documents/>,
//! <https://help.tallysolutions.com/configure-for-print-export-share/>, read 2026-11-26) say the
//! product prints vouchers but give no layout for these types, so the layout under test is
//! Ledgerly's own: company, "<type> Voucher", number and date, then Particulars with Debit and
//! Credit, a Total row, and the narration. Every figure is a stored line amount; the totals are
//! the sums of the lines and must agree (a voucher posts only when Dr equals Cr).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use ledgerly_api::{Api, ApiError, Command, CommandResult};
use ledgerly_core::{
    Clock, CompanyId, Date, GroupId, LedgerId, Outcome, Paise, Period, SignedAmount, StateCode,
    VoucherId, VoucherTypeId,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BaseVoucherKind, Company, EntityKind, Features, Ledger, LedgerLine,
    NegativeStockPolicy, ReservedGroup, Scenario, Voucher, VoucherMode, VoucherStatus,
};
use ledgerly_reports::{ColumnSpec, Detail, ReportFilter, ReportKind, ReportParams};
use time::OffsetDateTime;
use time::macros::date;

const CASH: LedgerId = LedgerId(1);
const RENT: LedgerId = LedgerId(2);

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
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
    }
}

fn clock() -> Clock {
    Clock::Fixed(OffsetDateTime::UNIX_EPOCH)
}

fn reserved(group: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(group))
        .unwrap()
        .id
}

fn payment_type() -> VoucherTypeId {
    default_voucher_types()
        .into_iter()
        .find(|t| t.base == BaseVoucherKind::Payment)
        .unwrap()
        .id
}

fn ledger(id: LedgerId, name: &str, group: GroupId) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: Some(group),
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

/// A payment of `rupees`: Dr Rent, Cr Cash. It carries no GST block.
fn payment(id: u64, on: Date, rupees: i64) -> Voucher {
    let paise = Paise::new(rupees * 100);
    Voucher {
        id: VoucherId(id),
        type_id: payment_type(),
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
            line(RENT, SignedAmount::dr(paise)),
            line(CASH, SignedAmount::cr(paise)),
        ],
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn setup() -> (tempfile::TempDir, PathBuf, Api) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("company.ledgerly");
    let mut api = Api::create(&path, &company(), clock()).unwrap();
    value(api.execute(Command::CreateLedger(ledger(
        CASH,
        "Cash",
        reserved(ReservedGroup::CashInHand),
    ))));
    value(api.execute(Command::CreateLedger(ledger(
        RENT,
        "Rent",
        reserved(ReservedGroup::IndirectExpenses),
    ))));
    (dir, path, api)
}

fn value(out: Result<Outcome<CommandResult>, ApiError>) -> CommandResult {
    match out {
        Ok(Outcome::Value(result)) => result,
        other => panic!("expected a committed value, got {other:?}"),
    }
}

fn report_params(kind: ReportKind) -> ReportParams {
    ReportParams {
        kind,
        period: Period::new(date!(2026 - 04 - 01), date!(2027 - 03 - 31)).unwrap(),
        as_of: date!(2026 - 06 - 30),
        scenario: Scenario::Actual,
        detail: Detail::Detailed,
        columns: ColumnSpec::Single,
        filter: ReportFilter::default(),
        exceptions_only: false,
    }
}

fn type_of(kind: BaseVoucherKind) -> VoucherTypeId {
    default_voucher_types()
        .into_iter()
        .find(|t| t.base == kind)
        .unwrap()
        .id
}

/// A voucher of `kind` for `rupees`: Dr Rent, Cr Cash (Dr Cash, Cr Rent for a receipt).
fn voucher_of(kind: BaseVoucherKind, id: u64, rupees: i64) -> Voucher {
    let mut v = payment(id, date!(2026 - 06 - 01), rupees);
    v.type_id = type_of(kind);
    v.number = Some(format!("N/{id:04}"));
    if kind == BaseVoucherKind::Purchase {
        v.party = Some(CASH);
    }
    if kind == BaseVoucherKind::Receipt {
        let paise = Paise::new(rupees * 100);
        v.ledger_lines = vec![
            line(CASH, SignedAmount::dr(paise)),
            line(RENT, SignedAmount::cr(paise)),
        ];
    }
    v
}

fn page_texts(doc: &ledgerly_api::PrintedDocument) -> Vec<String> {
    doc.pages
        .iter()
        .flat_map(|p| p.items.iter())
        .filter_map(|i| match i {
            ledgerly_api::PageItem::Text { text, .. } => Some(text.clone()),
            ledgerly_api::PageItem::Rule { .. } | ledgerly_api::PageItem::Fill { .. } => None,
        })
        .collect()
}

fn printed(api: &Api, id: u64) -> ledgerly_api::PrintedDocument {
    match api.print_voucher_document(VoucherId(id)) {
        Ok(Outcome::Value(doc)) => doc,
        other => panic!("voucher {id} should print, got {other:?}"),
    }
}

#[test]
fn a_payment_prints_with_particulars_debit_credit_and_total() {
    let (_dir, _path, mut api) = setup();
    let mut v = voucher_of(BaseVoucherKind::Payment, 1, 912_000);
    v.narration = "June rent".to_owned();
    value(api.execute(Command::PostVoucher(v)));
    let t = page_texts(&printed(&api, 1));
    for want in [
        "Test Traders",
        "Payment Voucher",
        "No. N/0001, dated 01-06-2026",
        "Particulars",
        "Debit",
        "Credit",
        "Rent",
        "Cash",
        "Total",
        "Narration: June rent",
    ] {
        assert!(t.iter().any(|s| s == want), "no {want:?} in {t:?}");
    }
    // Dr Rent 9,12,000.00 and Cr Cash 9,12,000.00, and the total row repeats each side once.
    assert_eq!(t.iter().filter(|s| *s == "9,12,000.00").count(), 4, "{t:?}");
}

#[test]
fn receipt_journal_contra_and_purchase_print_under_their_own_title() {
    let (_dir, _path, mut api) = setup();
    let kinds = [
        (BaseVoucherKind::Receipt, "Receipt Voucher"),
        (BaseVoucherKind::Journal, "Journal Voucher"),
        (BaseVoucherKind::Contra, "Contra Voucher"),
        (BaseVoucherKind::Purchase, "Purchase Voucher"),
    ];
    for (n, (kind, _)) in kinds.iter().enumerate() {
        let id = u64::try_from(n).unwrap() + 1;
        value(api.execute(Command::PostVoucher(voucher_of(*kind, id, 1_000))));
    }
    for (n, (_, title)) in kinds.iter().enumerate() {
        let id = u64::try_from(n).unwrap() + 1;
        let t = page_texts(&printed(&api, id));
        assert!(t.iter().any(|s| s == title), "no {title:?} in {t:?}");
        assert!(t.iter().any(|s| s == "1,000.00"), "{t:?}");
    }
}

#[test]
fn a_voucher_type_with_no_print_layout_is_unsupported_naming_the_type() {
    let (_dir, _path, mut api) = setup();
    let mut v = voucher_of(BaseVoucherKind::Memorandum, 1, 1_000);
    v.status = VoucherStatus::Regular;
    value(api.execute(Command::PostVoucher(v)));
    match api.print_voucher_document(VoucherId(1)) {
        Ok(Outcome::Unsupported(u)) => {
            assert!(u.reason.contains("Memorandum"), "{:?}", u.reason);
        }
        other => panic!("a memorandum has no print layout, got {other:?}"),
    }
}

#[test]
fn the_paper_pages_and_copies_reach_a_voucher_print() {
    let (_dir, _path, mut api) = setup();
    value(api.execute(Command::PostVoucher(voucher_of(
        BaseVoucherKind::Payment,
        1,
        500,
    ))));
    let setup = ledgerly_api::PrintSetup {
        paper: Some(ledgerly_api::Paper::A5),
        copies: 2,
        ..ledgerly_api::PrintSetup::default()
    };
    let Ok(Outcome::Value(doc)) = api.print_voucher_document_with(VoucherId(1), &setup) else {
        panic!("should print");
    };
    assert_eq!((doc.pages[0].width, doc.pages[0].height), (420, 595));
    assert_eq!(doc.copies, 2);
}

#[test]
fn the_paper_and_page_range_reach_a_report_print_and_a_bad_range_is_an_error() {
    let (_dir, _path, mut api) = setup();
    value(api.execute(Command::PostVoucher(voucher_of(
        BaseVoucherKind::Payment,
        1,
        500,
    ))));
    let params = report_params(ReportKind::DayBook);
    let a5 = ledgerly_api::PrintSetup {
        paper: Some(ledgerly_api::Paper::A5),
        ..ledgerly_api::PrintSetup::default()
    };
    let Ok(Outcome::Value(doc)) = api.print_report_document_with(&params, &a5) else {
        panic!("should print");
    };
    assert_eq!(doc.pages[0].width, 420);
    let past = ledgerly_api::PrintSetup {
        pages: ledgerly_api::PageSelection::parse("9").unwrap(),
        ..ledgerly_api::PrintSetup::default()
    };
    assert!(matches!(
        api.print_report_document_with(&params, &past),
        Err(ApiError::Print(_))
    ));
}

#[test]
fn the_day_book_prints_a_row_per_voucher_with_its_debit_and_credit() {
    // Derived: one payment of Rs 9,12,000 (Dr Rent, Cr Cash) is one day book entry; the screen's
    // columns are printed as the engine laid them out, with Indian grouping (as the trial
    // balance test in p1_print) and the voucher number in its column.
    let (_dir, _path, mut api) = setup();
    value(api.execute(Command::PostVoucher(voucher_of(
        BaseVoucherKind::Payment,
        1,
        912_000,
    ))));
    let Ok(Outcome::Value(doc)) = api.print_report_document(&report_params(ReportKind::DayBook))
    else {
        panic!("the day book should print");
    };
    let t = page_texts(&doc);
    assert!(t.iter().any(|s| s == "Day Book"), "{t:?}");
    assert!(t.iter().any(|s| s == "N/0001"), "{t:?}");
    assert!(t.iter().any(|s| s == "9,12,000.00 Dr"), "{t:?}");
    assert!(t.iter().any(|s| s == "Payment"), "{t:?}");
    // The audit columns of the screen are not printed (Entered by, Origin).
    assert!(
        !t.iter().any(|s| s == "Entered by" || s == "Origin"),
        "{t:?}"
    );
}
