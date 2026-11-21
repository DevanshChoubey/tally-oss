//! Card W5 tests: banking over the facade (spec 6.5). Statement import, match suggestions,
//! confirmation of a match, and the bank reconciliation statement (BRS).
//!
//! Expected values are `derived`: worked out by hand from the amounts and dates written in each
//! test and in the statement text, never copied from the code under test. The BRS rules are the
//! K3 rules of `docs/status/2026-11-21-round3-tally-decisions.md`, from the TallyPrime bank
//! reconciliation help page <https://help.tallysolutions.com/docs/te9rel65/Reports/Display_Registers_Ledgers/Bank_Reconciliation.htm>
//! (accessed 2026-11-21): balance per bank = balance per books - uncleared deposits + uncleared
//! payments. Amounts are paise: Rs 1,000 is `100_000`.
//!
//! Money in the test company, all in paise:
//!
//! | Voucher | Date       | Kind     | Amount |
//! |---------|------------|----------|--------|
//! | 1       | 2026-04-05 | receipt  | `50_000` |
//! | 2       | 2026-04-10 | receipt  | `120_000` |
//! | 3       | 2026-04-12 | payment  | `30_000` |
//! | 4       | 2026-04-20 | payment  | `45_000` |
//! | 5       | 2026-04-25 | receipt  | `20_000` |
//! | 6       | 2026-05-02 | receipt  | `77_000` |
//! | 7       | 2026-04-27 | payment  | `25_000` |
//!
//! The bank ledger opens with a debit balance of `100_000` (Rs 1,000.00).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use ledgerly_api::{Api, ApiError, Brs, Candidate, Command, CommandResult, MatchRule};
use ledgerly_bank::BankError;
use ledgerly_bank::statement::StatementError;
use ledgerly_core::{
    BankStatementId, Clock, CompanyId, Date, GroupId, LedgerId, Outcome, Paise, SignedAmount,
    StateCode, VoucherId, VoucherTypeId,
};
use ledgerly_engine::EngineError;
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_voucher_types::default_voucher_types;
use ledgerly_model::{
    Address, BankAllocation, BankDetails, BaseVoucherKind, Company, EntityKind, Features,
    InstrumentKind, Ledger, LedgerLine, MatchKind, NegativeStockPolicy, ReservedGroup, Voucher,
    VoucherMode, VoucherStatus,
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

/// A fictional CSV statement in the generic split layout. It has no opening or closing balance
/// line, which the CSV parser cannot supply.
const CSV: &str = "Date,Value Date,Narration,Ref,Withdrawal,Deposit,Balance\n05/04/2026,05/04/2026,NEFT RECEIPT,R1,,500.00,1500.00\n";

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
        group,
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

/// Imports [`STATEMENT`] for the bank ledger and returns its id.
fn import_statement(api: &mut Api) -> BankStatementId {
    match api
        .import_bank_statement(STATEMENT.as_bytes(), MT940, BANK)
        .unwrap()
    {
        Outcome::Value(id) => id,
        other @ Outcome::Unsupported(_) => {
            panic!("expected an imported statement, got {other:?}")
        }
    }
}

/// The match rules the tests use: a `BANK*` description suggests the charges ledger.
fn rules() -> Vec<MatchRule> {
    vec![MatchRule::new("BANK*", CHARGES)]
}

fn suggestions(api: &Api, id: BankStatementId) -> Vec<Candidate> {
    api.bank_suggestions(id, &rules()).unwrap().value().unwrap()
}

/// An exact suggestion of `vouchers` for the statement `lines`, dated `on`, not ambiguous.
fn exact(lines: &[u32], vouchers: &[u64], on: Date) -> Candidate {
    Candidate {
        statement_lines: lines.to_vec(),
        vouchers: vouchers.iter().map(|v| VoucherId(*v)).collect(),
        kind: MatchKind::Exact,
        suggested_ledger: None,
        proposed_bank_date: on,
        ambiguous: false,
    }
}

/// A rule suggestion for one statement line, to `ledger`.
fn rule(line: u32, ledger: LedgerId, on: Date) -> Candidate {
    Candidate {
        statement_lines: vec![line],
        vouchers: Vec::new(),
        kind: MatchKind::Rule,
        suggested_ledger: Some(ledger),
        proposed_bank_date: on,
        ambiguous: false,
    }
}

/// Confirms every exact suggestion for the statement.
fn confirm_exact(api: &mut Api, id: BankStatementId) {
    for candidate in suggestions(api, id) {
        if candidate.kind == MatchKind::Exact {
            value(api.confirm_bank_match(id, &candidate));
        }
    }
}

fn brs_at(api: &Api, as_of: Date) -> Brs {
    api.brs(BANK, as_of).unwrap().value().unwrap()
}

#[test]
fn suggestions_pair_statement_lines_with_vouchers_by_amount_and_date() {
    let (_dir, _path, mut api) = setup();
    let id = import_statement(&mut api);
    // derived: a line matches the one voucher of the same direction and amount dated within 3
    // days. Row 6 (5 Apr, 500) is voucher 1 (5 Apr). Row 7 (11 Apr, 1200) is voucher 2 (10 Apr,
    // one day). Row 8 (12 Apr, 300) is voucher 3 (12 Apr). Row 9 (21 Apr, 450) is voucher 4 (20
    // Apr). Row 12 (3 May, 770) is voucher 6 (2 May). Row 10 (28 Apr, 10 debit) has no voucher
    // of 10 rupees near it (voucher 7 is 250 on 27 Apr), so the rule `BANK*` suggests the charges
    // ledger. Voucher 5 (200, 25 Apr) and voucher 7 have no line of their amount, so no candidate.
    assert_eq!(
        suggestions(&api, id),
        vec![
            exact(&[6], &[1], date!(2026 - 04 - 05)),
            exact(&[7], &[2], date!(2026 - 04 - 11)),
            exact(&[8], &[3], date!(2026 - 04 - 12)),
            exact(&[9], &[4], date!(2026 - 04 - 21)),
            rule(10, CHARGES, date!(2026 - 04 - 28)),
            exact(&[12], &[6], date!(2026 - 05 - 03)),
        ]
    );
}

#[test]
fn confirmed_matches_set_bank_dates_and_brs_follows() {
    let (_dir, _path, mut api) = setup();
    let id = import_statement(&mut api);
    confirm_exact(&mut api, id);
    // derived: the five exact matches now carry a bank date, so only the rule suggestion is left.
    assert_eq!(
        suggestions(&api, id),
        vec![rule(10, CHARGES, date!(2026 - 04 - 28))]
    );
    let brs = brs_at(&api, date!(2026 - 04 - 30));
    // derived: books balance = opening 100_000 + receipts 50_000 + 120_000 + 20_000 - payments
    // 30_000 - 45_000 - 25_000 (vouchers 1 to 5 and 7; voucher 6 is after 30 Apr) = 190_000.
    assert_eq!(brs.books_balance, Paise::new(190_000));
    // derived: uncleared on 30 Apr are the bank-dated-later-or-blank lines: voucher 5 (20_000, no
    // bank date) as a deposit, and voucher 7 (25_000, no bank date) as a payment.
    assert_eq!(
        brs.uncleared_deposits
            .iter()
            .map(|i| (i.voucher, i.amount))
            .collect::<Vec<_>>(),
        vec![(VoucherId(5), Paise::new(20_000))]
    );
    assert_eq!(
        brs.uncleared_payments
            .iter()
            .map(|i| (i.voucher, i.amount))
            .collect::<Vec<_>>(),
        vec![(VoucherId(7), Paise::new(25_000))]
    );
    assert_eq!(brs.uncleared_deposits_total, Paise::new(20_000));
    assert_eq!(brs.uncleared_payments_total, Paise::new(25_000));
    // derived: balance per bank = 190_000 - 20_000 + 25_000 = 195_000.
    assert_eq!(brs.balance_as_per_bank, Paise::new(195_000));
    // derived: statement balance at 30 Apr = opening 100_000 + 500,00 + 1200,00 - 300,00 - 450,00
    // - 10,00 (lines dated 5 Apr to 28 Apr) = 194_000. Covered: 5 Apr <= 30 Apr < 3 May.
    assert_eq!(
        brs.statement_balance.clone().value(),
        Some(Paise::new(194_000))
    );
    // derived: difference = 194_000 - 195_000 = -1_000 (Rs 10.00: the bank charges are not in the
    // books yet, which is what the rule suggestion would fix).
    assert_eq!(brs.difference.clone().value(), Some(Paise::new(-1_000)));
    assert_eq!(brs.is_reconciled().value(), Some(false));
}

#[test]
fn brs_before_any_confirmation_has_balance_per_bank_equal_to_opening() {
    let (_dir, _path, mut api) = setup();
    import_statement(&mut api);
    let brs = brs_at(&api, date!(2026 - 04 - 30));
    // derived: nothing is cleared, so every posting on or before 30 Apr is uncleared. Deposits
    // are vouchers 1, 2 and 5 = 50_000 + 120_000 + 20_000 = 190_000. Payments are vouchers 3, 4
    // and 7 = 30_000 + 45_000 + 25_000 = 100_000. Balance per bank = 190_000 - 190_000 + 100_000 =
    // 100_000, which is the opening balance: with no bank dates, the bank still holds it.
    assert_eq!(brs.books_balance, Paise::new(190_000));
    assert_eq!(brs.uncleared_deposits_total, Paise::new(190_000));
    assert_eq!(brs.uncleared_payments_total, Paise::new(100_000));
    assert_eq!(brs.balance_as_per_bank, Paise::new(100_000));
    // derived: statement 194_000 - balance per bank 100_000 = 94_000, unmatched.
    assert_eq!(brs.difference.clone().value(), Some(Paise::new(94_000)));
}

#[test]
fn brs_as_of_the_last_statement_line_leaves_the_statement_side_unsupported() {
    let (_dir, _path, mut api) = setup();
    let id = import_statement(&mut api);
    confirm_exact(&mut api, id);
    let brs = brs_at(&api, date!(2026 - 05 - 03));
    // derived: books on 3 May include voucher 6 (77_000): 190_000 + 77_000 = 267_000.
    assert_eq!(brs.books_balance, Paise::new(267_000));
    // derived: voucher 6 has bank date 3 May, which is not after the as-of date, so it is cleared.
    // Uncleared: voucher 5 (20_000) and voucher 7 (25_000). Balance per bank =
    // 267_000 - 20_000 + 25_000 = 272_000.
    assert_eq!(brs.uncleared_deposits_total, Paise::new(20_000));
    assert_eq!(brs.uncleared_payments_total, Paise::new(25_000));
    assert_eq!(brs.balance_as_per_bank, Paise::new(272_000));
    // The statement's last line is dated 3 May, and a date is covered only before it (the end
    // date of the file is not stored). So the statement side is unsupported.
    assert!(brs.statement_balance.is_unsupported());
    assert!(brs.difference.is_unsupported());
    assert_eq!(brs.is_reconciled().value(), None);
}

#[test]
fn brs_without_an_imported_statement_has_no_statement_side() {
    let (_dir, _path, api) = setup();
    let brs = brs_at(&api, date!(2026 - 04 - 30));
    // derived: nothing cleared, so balance per bank = opening 100_000, as in the test above.
    assert_eq!(brs.balance_as_per_bank, Paise::new(100_000));
    assert!(brs.statement_balance.is_unsupported());
    assert!(brs.difference.is_unsupported());
}

#[test]
fn a_rule_suggestion_cannot_be_confirmed() {
    let (_dir, _path, mut api) = setup();
    let id = import_statement(&mut api);
    let rule_candidate = rule(10, CHARGES, date!(2026 - 04 - 28));
    let out = api.confirm_bank_match(id, &rule_candidate).unwrap();
    assert!(out.is_unsupported());
    // Nothing was written: the rule is still the only suggestion.
    assert_eq!(suggestions(&api, id), vec![rule_candidate]);
}

#[test]
fn a_match_that_is_not_suggested_is_refused() {
    let (_dir, _path, mut api) = setup();
    let id = import_statement(&mut api);
    // derived: row 6 is 500,00 credit; voucher 5 is 200,00 receipt. The amounts differ, so the
    // suggestion engine never lists this pair, and confirming it must be refused.
    let invented = exact(&[6], &[5], date!(2026 - 04 - 05));
    match api.confirm_bank_match(id, &invented) {
        Err(ApiError::MatchNotSuggested { lines }) => assert_eq!(lines, vec![6]),
        other => panic!("expected MatchNotSuggested, got {other:?}"),
    }
}

#[test]
fn a_locked_period_refuses_the_bank_match() {
    let (_dir, _path, mut api) = setup();
    let id = import_statement(&mut api);
    value(api.execute(Command::LockPeriod(date!(2026 - 04 - 10))));
    let first = suggestions(&api, id).remove(0);
    // derived: voucher 1 is dated 5 Apr, inside the lock of 10 Apr, so the period lock refuses.
    match api.confirm_bank_match(id, &first) {
        Err(ApiError::Engine(EngineError::PeriodLocked { until })) => {
            assert_eq!(until, date!(2026 - 04 - 10));
        }
        other => panic!("expected PeriodLocked, got {other:?}"),
    }
    // Nothing was written: the same match is still the first suggestion.
    assert_eq!(suggestions(&api, id).first(), Some(&first));
}

#[test]
fn the_same_file_cannot_be_imported_twice() {
    let (_dir, _path, mut api) = setup();
    import_statement(&mut api);
    match api.import_bank_statement(STATEMENT.as_bytes(), MT940, BANK) {
        Err(ApiError::StatementImport(StatementError::Duplicate { file_hash })) => {
            // A SHA-256 digest in lower-case hex has 64 characters.
            assert_eq!(file_hash.len(), 64);
        }
        other => panic!("expected Duplicate, got {other:?}"),
    }
}

#[test]
fn a_statement_that_does_not_balance_is_refused_and_nothing_is_held() {
    let (_dir, _path, mut api) = setup();
    // The stated closing is 2711,00 instead of the derived 2710,00.
    let text = STATEMENT.replace(":62F:C260503INR2710,00", ":62F:C260503INR2711,00");
    assert_ne!(text, STATEMENT);
    match api.import_bank_statement(text.as_bytes(), MT940, BANK) {
        Err(ApiError::StatementImport(StatementError::BalanceMismatch {
            computed_closing,
            stated_closing,
            ..
        })) => {
            // derived: opening 100_000 + net of the lines 171_000 = 271_000; stated 271_100.
            assert_eq!(computed_closing, Paise::new(271_000));
            assert_eq!(stated_closing, Paise::new(271_100));
        }
        other => panic!("expected BalanceMismatch, got {other:?}"),
    }
    // Nothing was kept, so statement 1 is unknown to this session.
    assert!(matches!(
        api.bank_suggestions(BankStatementId(1), &rules()),
        Err(ApiError::UnknownBankStatement { id: 1 })
    ));
}

#[test]
fn a_ledger_that_is_not_a_bank_account_is_refused() {
    let (_dir, _path, mut api) = setup();
    match api.import_bank_statement(STATEMENT.as_bytes(), MT940, RENT) {
        Err(ApiError::Bank(BankError::NotABankLedger(ledger))) => assert_eq!(ledger, RENT),
        other => panic!("expected NotABankLedger, got {other:?}"),
    }
}

#[test]
fn an_unknown_format_is_refused() {
    let (_dir, _path, mut api) = setup();
    match api.import_bank_statement(STATEMENT.as_bytes(), "no-such-format", BANK) {
        Err(ApiError::UnknownBankFormat(name)) => assert_eq!(name, "no-such-format"),
        other => panic!("expected UnknownBankFormat, got {other:?}"),
    }
}

#[test]
fn a_csv_statement_is_refused_until_the_parser_states_the_balances() {
    let (_dir, _path, mut api) = setup();
    // The CSV parser leaves opening and closing balances unset (ledgerly-bankfmt csv.rs), so the
    // balance check (spec 6.5) cannot pass and the file is refused, not guessed at.
    match api.import_bank_statement(CSV.as_bytes(), "generic-csv-split", BANK) {
        Err(ApiError::StatementImport(StatementError::MissingBalance { which })) => {
            assert_eq!(which, "opening");
        }
        other => panic!("expected MissingBalance, got {other:?}"),
    }
}

#[test]
fn reopening_keeps_confirmed_matches_but_not_the_statement() {
    let (_dir, path, mut api) = setup();
    let id = import_statement(&mut api);
    confirm_exact(&mut api, id);
    drop(api);

    let mut api = Api::open(&path).unwrap();
    // The statement itself is not in the company file.
    assert_eq!(api.bank_statements(BANK).len(), 0);
    let brs = brs_at(&api, date!(2026 - 04 - 30));
    // The confirmed bank dates were stored as events, so the books still show them cleared.
    assert_eq!(brs.uncleared_deposits_total, Paise::new(20_000));
    assert_eq!(brs.uncleared_payments_total, Paise::new(25_000));
    assert!(brs.statement_balance.is_unsupported());

    // Importing the file again is accepted (this session does not know it) and takes the next id.
    let again = match api
        .import_bank_statement(STATEMENT.as_bytes(), MT940, BANK)
        .unwrap()
    {
        Outcome::Value(id) => id,
        other @ Outcome::Unsupported(_) => {
            panic!("expected an imported statement, got {other:?}")
        }
    };
    assert_eq!(again, BankStatementId(2));
    // derived: the same statement gives the same difference as before the reopen: -1_000.
    let brs = brs_at(&api, date!(2026 - 04 - 30));
    assert_eq!(brs.difference.clone().value(), Some(Paise::new(-1_000)));
}
