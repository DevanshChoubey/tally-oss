//! Card C3 tests: bank statements stored in the company file, CSV balances, the statement end
//! date, and the BRS over them.
//!
//! Expected values are `derived` by hand from the amounts and dates written in the statement
//! texts below and in the voucher table of `w5_bank.rs` (same company). The BRS rules are
//! TallyPrime's: <https://help.tallysolutions.com/docs/te9rel65/Reports/Display_Registers_Ledgers/Bank_Reconciliation.htm>
//! (accessed 2026-11-21): balances are as of the last date of the period; the page shows
//! "Balance as per Imported Bank Statement" and a Difference against the Balance as per Bank. The
//! page does not mention a statement end date; that part of the behaviour is ours (an MT940
//! `:62F:` date states that the file is complete up to that day) and is `pending` confirmation
//! against Tally.
//
// Books on 2026-05-03 (all seven vouchers of `w5_bank.rs`): opening 100_000 + receipts 50_000 +
// 120_000 + 20_000 + 77_000 - payments 30_000 - 45_000 - 25_000 = 267_000. With no bank date set,
// uncleared deposits are all four receipts (267_000) and uncleared payments all three payments
// (100_000), so the balance per bank is 267_000 - 267_000 + 100_000 = 100_000.
//
// The MT940 statement: opening 100_000, lines +50_000, +120_000, -30_000, -45_000, -1_000,
// +77_000, so its balance on 2026-05-03 is 100_000 + 50_000 + 120_000 - 30_000 - 45_000 -
// 1_000 + 77_000 = 271_000, which is its stated `:62F:` 2710,00.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use ledgerly_api::{Api, ApiError, Brs, Command, CommandResult};
use ledgerly_bank::statement::StatementError;
use ledgerly_core::{
    BankStatementId, Clock, CompanyId, Date, GroupId, LedgerId, Outcome, Paise, SignedAmount,
    StateCode, VoucherId, VoucherTypeId,
};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BankAllocation, BankDetails, BaseVoucherKind, Company, EntityKind, Features,
    InstrumentKind, Ledger, LedgerLine, NegativeStockPolicy, ReservedGroup, Voucher, VoucherMode,
    VoucherStatus,
};
use time::OffsetDateTime;
use time::macros::date;

/// The bank account (a bank ledger with bank details).
const BANK: LedgerId = LedgerId(10);
/// An expense, paid from the bank.
const RENT: LedgerId = LedgerId(11);
/// A customer, who pays into the bank.
const CUSTOMER: LedgerId = LedgerId(12);
/// Bank charges: the ledger the rule `BANK*` suggests.
const CHARGES: LedgerId = LedgerId(13);

/// The format registry id of the MT940 parser.
const MT940: &str = "generic-mt940";

/// A fictional MT940 statement for the bank account. Its lines, in row order (row = line number
/// in this text):
///
/// - row 6: 2026-04-05 credit 500,00 (money in)
/// - row 7: 2026-04-11 credit 1200,00
/// - row 8: 2026-04-12 debit 300,00
/// - row 9: 2026-04-21 debit 450,00
/// - row 10: 2026-04-28 debit 10,00, description "BANK CHARGES" from the `:86:` on row 11
/// - row 12: 2026-05-03 credit 770,00
///
/// Derived closing balance: opening 1000,00 + 500 + 1200 - 300 - 450 - 10 + 770 = 2710,00, which is
/// the stated `:62F:`. The statement covers dates from 2026-04-05 up to, not including, 2026-05-03.
const STATEMENT: &str = "{1:F01TESTINBBAXXX0000000000}{2:O9400000000000TESTINBBXXXX00000000000000000000N}{4:\r\n\
:20:STMT0001\r\n\
:25:0123456789\r\n\
:28C:00001/001\r\n\
:60F:C260401INR1000,00\r\n\
:61:260405C500,00NTRFNONREF//BK1\r\n\
:61:260411C1200,00NTRFNONREF//BK2\r\n\
:61:260412D300,00NTRFCHQ000123\r\n\
:61:260421D450,00NTRFNONREF//BK4\r\n\
:61:260428D10,00NTRFNONREF//BANKCHG\r\n\
:86:BANK CHARGES\r\n\
:61:260503C770,00NTRFNONREF//BK5\r\n\
:62F:C260503INR2710,00\r\n\
-}\r\n";

/// A fictional CSV statement in the generic split layout with a running balance on every row.
/// Row 2: 2026-04-05 deposit 500.00, balance 1500.00. Row 3: 2026-04-12 withdrawal 300.00,
/// balance 1200.00. Derived: the opening balance is 1500.00 - 500.00 = 1000.00 (`100_000` paise) and
/// the closing balance is the last row's 1200.00 (`120_000` paise); 1000 + 500 - 300 = 1200 agrees.
const CSV: &str = "Date,Value Date,Narration,Ref,Withdrawal,Deposit,Balance\n05/04/2026,05/04/2026,NEFT RECEIPT,R1,,500.00,1500.00\n12/04/2026,12/04/2026,CHEQUE,R2,300.00,,1200.00\n";

/// The same rows, but the second running balance is 1300.00 where 1500 - 300 = 1200 is right.
const CSV_BAD_RUNNING: &str = "Date,Value Date,Narration,Ref,Withdrawal,Deposit,Balance\n05/04/2026,05/04/2026,NEFT RECEIPT,R1,,500.00,1500.00\n12/04/2026,12/04/2026,CHEQUE,R2,300.00,,1300.00\n";

/// A CSV with no balance on its rows: nothing states the opening or closing balance.
const CSV_NO_BALANCE: &str = "Date,Value Date,Narration,Ref,Withdrawal,Deposit,Balance\n05/04/2026,05/04/2026,NEFT RECEIPT,R1,,500.00,\n";

/// A second MT940 statement: opening 2710,00 (the first statement's closing), one line on
/// 2026-05-10 credit 100,00, closing 2810,00 dated 2026-05-12 (two days after the last line).
const SECOND: &str = ":20:STMT0002\r\n:25:0123456789\r\n:60F:C260503INR2710,00\r\n:61:260510C100,00NTRFNONREF//BK9\r\n:62F:C260512INR2810,00\r\n";

/// Like [`SECOND`] but the `:62F:` date (2026-05-09) is before the last line (2026-05-10).
const END_BEFORE_LINE: &str = ":20:STMT0003\r\n:25:0123456789\r\n:60F:C260503INR2710,00\r\n:61:260510C100,00NTRFNONREF//BK9\r\n:62F:C260509INR2810,00\r\n";

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

/// The id of a reserved group, from the seed the company is created with.
fn reserved(group: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(group))
        .unwrap()
        .id
}

fn voucher_type(kind: BaseVoucherKind) -> VoucherTypeId {
    default_voucher_types()
        .into_iter()
        .find(|t| t.base == kind)
        .unwrap()
        .id
}

fn master(
    id: LedgerId,
    name: &str,
    group: GroupId,
    bank: Option<BankDetails>,
    opening: SignedAmount,
) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group: Some(group),
        opening,
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

/// A line on an ordinary ledger, with no bank allocation.
fn plain(ledger: LedgerId, amount: SignedAmount) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger,
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: None,
    }
}

/// A line on the bank ledger, with a bank allocation that no bank date has been set on.
fn bank_side(amount: SignedAmount) -> LedgerLine {
    LedgerLine {
        ineligible_itc: None,
        ledger: BANK,
        amount,
        bills: Vec::new(),
        cost: Vec::new(),
        bank: Some(BankAllocation {
            instrument: InstrumentKind::Neft,
            instrument_number: None,
            instrument_date: None,
            bank_date: None,
            favouring: None,
        }),
    }
}

fn voucher(id: u64, kind: BaseVoucherKind, on: Date, lines: Vec<LedgerLine>) -> Voucher {
    Voucher {
        id: VoucherId(id),
        type_id: voucher_type(kind),
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

/// A receipt: debit the bank, credit the customer.
fn receipt(id: u64, on: Date, paise: i64) -> Voucher {
    voucher(
        id,
        BaseVoucherKind::Receipt,
        on,
        vec![
            bank_side(SignedAmount::dr(Paise::new(paise))),
            plain(CUSTOMER, SignedAmount::cr(Paise::new(paise))),
        ],
    )
}

/// A payment: debit rent, credit the bank.
fn payment(id: u64, on: Date, paise: i64) -> Voucher {
    voucher(
        id,
        BaseVoucherKind::Payment,
        on,
        vec![
            plain(RENT, SignedAmount::dr(Paise::new(paise))),
            bank_side(SignedAmount::cr(Paise::new(paise))),
        ],
    )
}

fn value(out: Result<Outcome<CommandResult>, ApiError>) -> CommandResult {
    match out {
        Ok(Outcome::Value(result)) => result,
        other => panic!("expected a committed value, got {other:?}"),
    }
}

/// A company file with the bank, rent, customer and charges ledgers and the seven vouchers.
fn setup() -> (tempfile::TempDir, PathBuf, Api) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("company.ledgerly");
    let mut api = Api::create(&path, &company(), clock()).unwrap();
    let bank_details = BankDetails {
        account_number: "0123456789".to_owned(),
        ifsc: "TEST0000001".to_owned(),
        bank_name: "Test Bank".to_owned(),
        branch: "Main".to_owned(),
    };
    value(api.execute(Command::CreateLedger(master(
        BANK,
        "Bank Current Account",
        reserved(ReservedGroup::BankAccounts),
        Some(bank_details),
        SignedAmount::dr(Paise::new(100_000)),
    ))));
    value(api.execute(Command::CreateLedger(master(
        RENT,
        "Rent",
        reserved(ReservedGroup::IndirectExpenses),
        None,
        SignedAmount::dr(Paise::ZERO),
    ))));
    value(api.execute(Command::CreateLedger(master(
        CUSTOMER,
        "Customer A",
        reserved(ReservedGroup::SundryDebtors),
        None,
        SignedAmount::dr(Paise::ZERO),
    ))));
    value(api.execute(Command::CreateLedger(master(
        CHARGES,
        "Bank Charges",
        reserved(ReservedGroup::IndirectExpenses),
        None,
        SignedAmount::dr(Paise::ZERO),
    ))));
    let vouchers = vec![
        receipt(1, date!(2026 - 04 - 05), 50_000),
        receipt(2, date!(2026 - 04 - 10), 120_000),
        payment(3, date!(2026 - 04 - 12), 30_000),
        payment(4, date!(2026 - 04 - 20), 45_000),
        receipt(5, date!(2026 - 04 - 25), 20_000),
        receipt(6, date!(2026 - 05 - 02), 77_000),
        payment(7, date!(2026 - 04 - 27), 25_000),
    ];
    for v in vouchers {
        value(api.execute(Command::PostVoucher(v)));
    }
    (dir, path, api)
}

fn import(api: &mut Api, file: &str, format: &str) -> BankStatementId {
    match api
        .import_bank_statement(file.as_bytes(), format, BANK)
        .unwrap()
    {
        Outcome::Value(id) => id,
        other @ Outcome::Unsupported(_) => panic!("expected an imported statement, got {other:?}"),
    }
}

fn brs_at(api: &Api, as_of: Date) -> Brs {
    api.brs(BANK, as_of).unwrap().value().unwrap()
}

#[test]
fn a_statement_survives_reopen_and_the_brs_keeps_its_statement_side() {
    let (_dir, path, mut api) = setup();
    assert_eq!(import(&mut api, STATEMENT, MT940), BankStatementId(1));
    drop(api);

    let api = Api::open(&path).unwrap();
    let held = api.bank_statements(BANK);
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].0, BankStatementId(1));
    // derived: the statement states opening 100_000, closing 271_000 and six lines.
    assert_eq!(held[0].1.opening(), Paise::new(100_000));
    assert_eq!(held[0].1.closing(), Paise::new(271_000));
    assert_eq!(held[0].1.lines().len(), 6);
    // derived: balance on 30 Apr = 100_000 + 50_000 + 120_000 - 30_000 - 45_000 - 1_000 = 194_000;
    // balance per bank with nothing cleared is 100_000 (header), so the difference is 94_000.
    let brs = brs_at(&api, date!(2026 - 04 - 30));
    assert_eq!(
        brs.statement_balance.clone().value(),
        Some(Paise::new(194_000))
    );
    assert_eq!(brs.difference.clone().value(), Some(Paise::new(94_000)));
}

#[test]
fn a_file_imported_before_the_reopen_is_a_duplicate_afterwards() {
    let (_dir, path, mut api) = setup();
    import(&mut api, STATEMENT, MT940);
    drop(api);

    let mut api = Api::open(&path).unwrap();
    match api.import_bank_statement(STATEMENT.as_bytes(), MT940, BANK) {
        Err(ApiError::StatementImport(StatementError::Duplicate { .. })) => {}
        other => panic!("expected a duplicate refusal, got {other:?}"),
    }
    // A different file takes the next id: ids are not reused after a reopen.
    assert_eq!(import(&mut api, SECOND, MT940), BankStatementId(2));
    assert_eq!(api.bank_statements(BANK).len(), 2);
}

#[test]
fn an_mt940_end_date_covers_the_last_day_of_the_statement() {
    let (_dir, path, mut api) = setup();
    import(&mut api, STATEMENT, MT940);
    // derived: :62F: is 2026-05-03, the date of the last line, so 3 May is covered and the
    // statement balance is its stated closing 271_000. Books 267_000 (header), balance per bank
    // 100_000 with nothing cleared, difference 271_000 - 100_000 = 171_000.
    let brs = brs_at(&api, date!(2026 - 05 - 03));
    assert_eq!(brs.books_balance, Paise::new(267_000));
    assert_eq!(brs.balance_as_per_bank, Paise::new(100_000));
    assert_eq!(
        brs.statement_balance.clone().value(),
        Some(Paise::new(271_000))
    );
    assert_eq!(brs.difference.clone().value(), Some(Paise::new(171_000)));
    // The day after the end date is not covered.
    let after = brs_at(&api, date!(2026 - 05 - 04));
    assert!(after.statement_balance.is_unsupported());
    // The end date is stored: it still holds after a reopen.
    drop(api);
    let api = Api::open(&path).unwrap();
    assert_eq!(
        api.bank_statements(BANK)[0].1.end_date(),
        Some(date!(2026 - 05 - 03))
    );
    let brs = brs_at(&api, date!(2026 - 05 - 03));
    assert_eq!(brs.statement_balance.value(), Some(Paise::new(271_000)));
}

#[test]
fn an_end_date_before_the_last_line_is_refused_and_nothing_is_kept() {
    let (_dir, _path, mut api) = setup();
    import(&mut api, STATEMENT, MT940);
    match api.import_bank_statement(END_BEFORE_LINE.as_bytes(), MT940, BANK) {
        Err(ApiError::StatementImport(StatementError::EndBeforeLastLine { end, last_line })) => {
            assert_eq!(end, date!(2026 - 05 - 09));
            assert_eq!(last_line, date!(2026 - 05 - 10));
        }
        other => panic!("expected EndBeforeLastLine, got {other:?}"),
    }
    assert_eq!(api.bank_statements(BANK).len(), 1);
}

#[test]
fn a_csv_with_a_balance_column_is_verified_and_imported() {
    let (_dir, path, mut api) = setup();
    let id = import(&mut api, CSV, "generic-csv-split");
    let held = api.bank_statements(BANK);
    assert_eq!(held[0].0, id);
    // derived in the CSV constant: opening 1000.00, closing 1200.00, no stated end date.
    assert_eq!(held[0].1.opening(), Paise::new(100_000));
    assert_eq!(held[0].1.closing(), Paise::new(120_000));
    assert_eq!(held[0].1.end_date(), None);
    // derived: on 5 Apr the statement balance is the first row's 1500.00 = 150_000 (5 Apr is on or
    // after the first line and before the last line, 12 Apr).
    let brs = brs_at(&api, date!(2026 - 04 - 05));
    assert_eq!(brs.statement_balance.value(), Some(Paise::new(150_000)));
    // derived: with no end date the last line's day (12 Apr) is not covered.
    assert!(
        brs_at(&api, date!(2026 - 04 - 12))
            .statement_balance
            .is_unsupported()
    );
    drop(api);
    let api = Api::open(&path).unwrap();
    assert_eq!(api.bank_statements(BANK).len(), 1);
}

#[test]
fn a_csv_whose_running_balance_disagrees_is_refused() {
    let (_dir, _path, mut api) = setup();
    // derived: 1500.00 - 300.00 = 1200.00, but the file says 1300.00 on row 3.
    match api.import_bank_statement(CSV_BAD_RUNNING.as_bytes(), "generic-csv-split", BANK) {
        Err(ApiError::StatementImport(StatementError::RunningBalance {
            row,
            expected,
            found,
        })) => {
            assert_eq!(row, 3);
            assert_eq!(expected, Paise::new(120_000));
            assert_eq!(found, Paise::new(130_000));
        }
        other => panic!("expected RunningBalance, got {other:?}"),
    }
    assert_eq!(api.bank_statements(BANK).len(), 0);
}

#[test]
fn a_csv_with_no_balances_is_still_refused() {
    let (_dir, _path, mut api) = setup();
    match api.import_bank_statement(CSV_NO_BALANCE.as_bytes(), "generic-csv-split", BANK) {
        Err(ApiError::StatementImport(StatementError::MissingBalance { which })) => {
            assert_eq!(which, "opening");
        }
        other => panic!("expected MissingBalance, got {other:?}"),
    }
}
