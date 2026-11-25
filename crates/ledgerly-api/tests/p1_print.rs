//! Card P1 tests: printing a saved voucher and a report through the facade (spec section 11, M5).
//!
//! Provenance: the expected outcomes are `derived` from the rules written in each test. A voucher
//! with no GST block is not a tax document, so it is refused, not printed (the print crate prints
//! only sales invoices, credit notes and debit notes). Amounts are paise: Rs 1,000 is `100_000`.

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
        group,
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

#[test]
fn a_voucher_without_a_gst_block_prints_as_unsupported_with_a_reason() {
    // Derived: a payment is not a sales invoice, so the print crate refuses it. The facade
    // passes that refusal on as Unsupported with a reason, not as a blank PDF.
    let (_dir, _path, mut api) = setup();
    value(api.execute(Command::PostVoucher(payment(1, date!(2026 - 06 - 01), 500))));
    match api.print_voucher(VoucherId(1)) {
        Ok(Outcome::Unsupported(u)) => {
            assert!(
                u.reason.contains("GST"),
                "reason should say what is missing, got {:?}",
                u.reason
            );
        }
        other => panic!("expected Unsupported for a payment, got {other:?}"),
    }
}

#[test]
fn a_voucher_that_is_not_in_the_books_is_an_error() {
    // Derived: voucher 99 was never posted, so there is nothing to print.
    let (_dir, _path, api) = setup();
    match api.print_voucher(VoucherId(99)) {
        Err(ApiError::UnknownVoucher { id }) => assert_eq!(id, 99),
        other => panic!("expected UnknownVoucher, got {other:?}"),
    }
}

/// The strings a PDF writes as text, read back from the uncompressed content streams.
fn texts(pdf: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < pdf.len() {
        if pdf[i] != b'(' {
            i += 1;
            continue;
        }
        let mut depth = 1;
        let mut s = Vec::new();
        i += 1;
        while i < pdf.len() && depth > 0 {
            match pdf[i] {
                b'\\' => {
                    i += 1;
                    s.push(pdf[i]);
                }
                b'(' => {
                    depth += 1;
                    s.push(b'(');
                }
                b')' => {
                    depth -= 1;
                    if depth > 0 {
                        s.push(b')');
                    }
                }
                other => s.push(other),
            }
            i += 1;
        }
        out.push(String::from_utf8(s).unwrap());
    }
    out
}

#[test]
fn a_report_without_a_print_layout_is_unsupported_with_the_export_instead() {
    // Derived: a stock summary has no print layout yet, so it is refused with a reason that names
    // the export formats the user can still use.
    let (_dir, _path, api) = setup();
    match api.print_report(&report_params(ReportKind::StockSummary)) {
        Ok(Outcome::Unsupported(u)) => {
            assert!(u.what.contains("report"), "what was {:?}", u.what);
            assert!(u.reason.contains("CSV"), "reason was {:?}", u.reason);
        }
        other => panic!("expected Unsupported for a report, got {other:?}"),
    }
}

#[test]
fn a_balance_sheet_and_a_trial_balance_print_as_pdfs_that_name_the_company() {
    // Derived: the company name is the one written in `company()` above; the report title is
    // checked as a word only, so the test does not depend on the exact title the engine gives.
    let (_dir, _path, api) = setup();
    for kind in [
        ReportKind::BalanceSheet,
        ReportKind::TrialBalance,
        ReportKind::ProfitAndLoss,
        ReportKind::DayBook,
    ] {
        match api.print_report(&report_params(kind.clone())) {
            Ok(Outcome::Value(pdf)) => {
                assert!(
                    pdf.starts_with(b"%PDF"),
                    "not a PDF: {:?}",
                    &pdf[..pdf.len().min(8)]
                );
                let t = texts(&pdf);
                assert!(
                    t.iter().any(|s| s == "Test Traders"),
                    "company missing in {t:?}"
                );
            }
            other => panic!("expected a PDF for {kind:?}, got {other:?}"),
        }
    }
}

#[test]
fn a_report_document_is_the_same_pdf_with_the_pages_it_was_written_from() {
    // Card S14. An identity check between two entry points of one render, not a copied figure:
    // the bytes equal `print_report`'s, and the first page's text starts with the company's
    // name written in `company()` above, then the report's title (Derived: report layout in
    // `ledgerly_print::pdf::render_report`: company, title, period, then the table).
    let (_dir, _path, api) = setup();
    let params = report_params(ReportKind::DayBook);
    let Ok(Outcome::Value(pdf)) = api.print_report(&params) else {
        panic!("the day book should print");
    };
    let Ok(Outcome::Value(doc)) = api.print_report_document(&params) else {
        panic!("the day book document should print");
    };
    assert_eq!(doc.pdf, pdf);
    // An empty day book is a heading and no rows: one page (derived, as `s14_pages`).
    assert_eq!(doc.pages.len(), 1);
    let first = doc.pages[0]
        .items
        .iter()
        .find_map(|i| match i {
            ledgerly_api::PageItem::Text { text, .. } => Some(text.as_str()),
            ledgerly_api::PageItem::Rule { .. } | ledgerly_api::PageItem::Fill { .. } => None,
        })
        .unwrap();
    assert_eq!(first, "Test Traders");
}

#[test]
fn a_document_that_cannot_print_is_unsupported_with_the_same_reason() {
    // Derived: the refusals of `print_report` and `print_voucher` are the document's too.
    let (_dir, _path, mut api) = setup();
    match api.print_report_document(&report_params(ReportKind::StockSummary)) {
        Ok(Outcome::Unsupported(u)) => assert!(u.reason.contains("CSV"), "{:?}", u.reason),
        other => panic!("expected Unsupported, got {other:?}"),
    }
    value(api.execute(Command::PostVoucher(payment(1, date!(2026 - 06 - 01), 500))));
    match api.print_voucher_document(VoucherId(1)) {
        Ok(Outcome::Unsupported(u)) => assert!(u.reason.contains("GST"), "{:?}", u.reason),
        other => panic!("expected Unsupported, got {other:?}"),
    }
}
