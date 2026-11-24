//! Card U4: accounting voucher entry. Every expected value is derived by hand and the derivation
//! is written next to it; nothing is copied from this crate's output. The books are a small
//! fictional fixture built here (no Wave 0 fixture exists yet). No GST rate or tax is involved.
//!
//! Cases that are pending (no provenance yet, so the screen says `Unsupported`) are listed at the
//! end of the file as tests that assert `Unsupported`.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_app::screens::voucher_entry::{
    AmountError, Balance, DraftError, DraftRow, EntryDateError, EntryKind, Field, LedgerRef,
    OpenError, PickerItem, VoucherDraft, VoucherEntryScreen, eval_amount, format_entry_date,
    parse_entry_date, search_ledgers,
};
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{
    Date, DrCr, FinancialYear, GroupId, Gstin, LedgerId, MoneyError, Outcome, Paise, SignedAmount,
    StateCode, VoucherTypeId,
};
use ledgerly_model::{
    Address, BaseVoucherKind, BillWiseConfig, Books, Company, EntityKind, Features, Group, Ledger,
    Masters, NegativeStockPolicy, NumberingConfig, NumberingMethod, PartyDetails, RegistrationType,
    RestartPeriod, VoucherError, VoucherMode, VoucherStatus, VoucherType,
};
use time::macros::date;

// ---- fixture ---------------------------------------------------------------------------------

const CASH: LedgerId = LedgerId(1);
const HDFC: LedgerId = LedgerId(2);
const SHARMA: LedgerId = LedgerId(3);
const SHREE: LedgerId = LedgerId(4);
const RENT: LedgerId = LedgerId(5);
const BILLWISE: LedgerId = LedgerId(6);

const T_JOURNAL: VoucherTypeId = VoucherTypeId(1);
const T_PAYMENT: VoucherTypeId = VoucherTypeId(2);
const T_SALES: VoucherTypeId = VoucherTypeId(3);
const T_OFF: VoucherTypeId = VoucherTypeId(4);
const T_STOCK: VoucherTypeId = VoucherTypeId(5);
const GROUP: GroupId = GroupId(1);

fn ledger(id: LedgerId, name: &str, aliases: &[&str]) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: aliases.iter().map(|a| (*a).to_owned()).collect(),
        group: GROUP,
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

fn voucher_type(id: VoucherTypeId, name: &str, base: BaseVoucherKind, active: bool) -> VoucherType {
    VoucherType {
        id,
        name: name.to_owned(),
        base,
        abbreviation: name.chars().take(3).collect(),
        numbering: NumberingConfig {
            method: NumberingMethod::Automatic,
            prevent_renumbering: false,
            prefix: String::new(),
            suffix: String::new(),
            width: 4,
            restart: RestartPeriod::Yearly,
            series: Vec::new(),
        },
        active,
        tally: None,
    }
}

fn books() -> Books {
    let mut masters = Masters::default();
    masters.groups.insert(
        GROUP,
        Group {
            ineligible_itc: None,
            id: GROUP,
            name: "Current Assets".to_owned(),
            aliases: Vec::new(),
            parent: None,
            reserved: None,
            nature: None,
            affects_gross_profit: false,
            tally: None,
        },
    );
    let mut sharma = ledger(SHARMA, "Sharma Traders", &["ST"]);
    sharma.party = Some(PartyDetails {
        registration: RegistrationType::Regular,
        // A valid GSTIN from ledgerly-core's own tested vectors (gstin.rs `VALID`).
        gstin: Some(Gstin::parse("29ABCDE1234F1ZW").unwrap()),
        pan: None,
        state: None,
        address: Address::default(),
    });
    let mut billwise = ledger(BILLWISE, "Kaveri Packaging", &[]);
    billwise.bill_wise = Some(BillWiseConfig {
        default_credit_days: Some(30),
    });
    for l in [
        ledger(CASH, "Cash", &[]),
        ledger(HDFC, "HDFC Bank", &[]),
        sharma,
        ledger(SHREE, "Shree Stationers", &[]),
        ledger(RENT, "Rent", &[]),
        billwise,
    ] {
        masters.ledgers.insert(l.id, l);
    }
    for t in [
        voucher_type(T_JOURNAL, "Journal", BaseVoucherKind::Journal, true),
        voucher_type(T_PAYMENT, "Payment", BaseVoucherKind::Payment, true),
        voucher_type(T_SALES, "Sales", BaseVoucherKind::Sales, true),
        voucher_type(T_OFF, "Contra", BaseVoucherKind::Contra, false),
        voucher_type(
            T_STOCK,
            "Stock Journal",
            BaseVoucherKind::StockJournal,
            true,
        ),
    ] {
        masters.voucher_types.insert(t.id, t);
    }
    Books {
        company: Company {
            ineligible_itc_default: None,
            id: ledgerly_core::CompanyId(1),
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
        },
        masters,
        vouchers: BTreeMap::new(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

fn fy() -> FinancialYear {
    FinancialYear::april(2026).unwrap()
}

const TODAY: Date = date!(2026 - 11 - 21);

fn open(books: &Books, t: VoucherTypeId) -> VoucherEntryScreen {
    match VoucherEntryScreen::open(books, t, fy(), TODAY).unwrap() {
        Outcome::Value(s) => s,
        Outcome::Unsupported(u) => panic!("unexpectedly unsupported: {u:?}"),
    }
}

fn rupees(r: i64) -> Paise {
    Paise::new(r * 100)
}

// ---- inline arithmetic -----------------------------------------------------------------------

#[test]
fn percent_of_a_base() {
    // provenance: derived; the card's example `1200*18%`: 18% = 18/100 = 0.18, 1200 x 0.18 = 216,
    // 216 rupees = 21,600 paise.
    assert_eq!(eval_amount("1200*18%"), Ok(Paise::new(21_600)));
}

#[test]
fn plain_arithmetic_and_parentheses() {
    // provenance: derived; 1200 + 216 = 1416 rupees = 141,600 paise.
    assert_eq!(eval_amount("1200+216"), Ok(Paise::new(141_600)));
    // provenance: derived; (100 + 50) x 2 = 300 rupees = 30,000 paise.
    assert_eq!(eval_amount("(100+50)*2"), Ok(Paise::new(30_000)));
    // provenance: derived; multiplication binds tighter: 2 + 3 x 4 = 14 rupees = 1,400 paise.
    assert_eq!(eval_amount(" 2 + 3 * 4 "), Ok(Paise::new(1_400)));
    // provenance: derived; 1000 - 250.50 = 749.50 rupees = 74,950 paise.
    assert_eq!(eval_amount("1000-250.50"), Ok(Paise::new(74_950)));
}

#[test]
fn fractional_percent() {
    // provenance: derived; 18.5% = 0.185, 1200 x 0.185 = 222 rupees = 22,200 paise.
    assert_eq!(eval_amount("1200*18.5%"), Ok(Paise::new(22_200)));
}

#[test]
fn a_lone_percent_is_refused() {
    // Review fix. Someone who types 18% in an amount field means neither 18 paise nor 18 rupees,
    // and a plausible wrong number is worse than an error (rule A1), the same reasoning that
    // refuses `a+b%`. Percent stays valid only as an operand of * and / (`1200*18%`).
    // provenance: design decision (spec: never a wrong number); pending observed provenance.
    assert!(matches!(
        eval_amount("10%"),
        Err(AmountError::LonePercent { .. })
    ));
    assert!(matches!(
        eval_amount("(18%)"),
        Err(AmountError::LonePercent { .. })
    ));
    assert!(matches!(
        eval_amount("10%+5"),
        Err(AmountError::LonePercent { .. })
    ));
    // A percentage on the right of * or / is still an operand: 1200 x 0.18 = 216 rupees, and
    // 5 / 50% = 5 / 0.5 = 10 rupees = 1,000 paise (derived by hand).
    assert_eq!(eval_amount("1200*18%"), Ok(Paise::new(21_600)));
    assert_eq!(eval_amount("5/50%"), Ok(Paise::new(1_000)));
}

#[test]
fn pasted_nesting_and_length_are_bounded_not_a_stack_overflow() {
    // Rule B2: no panic or abort on any input. 10,000 parentheses are far beyond what an amount
    // needs: refused by the length cap before the parser recurses.
    let long = "(".repeat(10_000);
    assert!(matches!(
        eval_amount(&long),
        Err(AmountError::TooLong { .. })
    ));
    let long_minus = "-".repeat(10_000);
    assert!(matches!(
        eval_amount(&long_minus),
        Err(AmountError::TooLong { .. })
    ));
    // Within the length cap but deeper than 64 levels: refused by the depth limit.
    let deep = format!("{}1{}", "(".repeat(100), ")".repeat(100));
    assert!(matches!(
        eval_amount(&deep),
        Err(AmountError::TooDeep { .. })
    ));
    let deep_minus = format!("{}1", "-".repeat(100));
    assert!(matches!(
        eval_amount(&deep_minus),
        Err(AmountError::TooDeep { .. })
    ));
    // Ordinary nesting still works: ((1+2)*3) = 9 rupees = 900 paise.
    assert_eq!(eval_amount("((1+2)*3)"), Ok(Paise::new(900)));
}

#[test]
fn percent_after_plus_is_refused_as_ambiguous() {
    // `100+10%` could mean 100.10 or 110; the evaluator refuses to choose (never a wrong number).
    assert!(matches!(
        eval_amount("100+10%"),
        Err(AmountError::AmbiguousPercent { .. })
    ));
    assert!(matches!(
        eval_amount("100-10%"),
        Err(AmountError::AmbiguousPercent { .. })
    ));
    // provenance: derived; the explicit form: 100 + 100 x 0.10 = 110 rupees = 11,000 paise.
    assert_eq!(eval_amount("100+100*10%"), Ok(Paise::new(11_000)));
}

#[test]
fn division_rounds_to_the_nearest_paisa() {
    // provenance: derived; 100/3 = 33.3333... rupees; nearest paisa is 33.33 = 3,333 paise.
    assert_eq!(eval_amount("100/3"), Ok(Paise::new(3_333)));
    // provenance: derived; 200/3 = 66.6666... rupees; nearest paisa is 66.67 = 6,667 paise.
    assert_eq!(eval_amount("200/3"), Ok(Paise::new(6_667)));
    // provenance: derived; 0.005 rupee is exactly half a paisa; halves round away from zero
    // (RoundingRule::HalfUpToPaise), so 1 paisa.
    assert_eq!(eval_amount("0.005"), Ok(Paise::new(1)));
}

#[test]
fn bad_input_is_a_typed_error() {
    assert_eq!(eval_amount(""), Err(AmountError::Empty));
    assert_eq!(eval_amount("   "), Err(AmountError::Empty));
    assert_eq!(eval_amount("1/0"), Err(AmountError::DivideByZero));
    assert_eq!(eval_amount("12a"), Err(AmountError::BadChar { ch: 'a' }));
    // Grouping commas are refused rather than guessed at.
    assert_eq!(eval_amount("1,200"), Err(AmountError::BadChar { ch: ',' }));
    assert!(matches!(
        eval_amount("12+"),
        Err(AmountError::Syntax { .. })
    ));
    assert!(matches!(
        eval_amount("(1+2"),
        Err(AmountError::Syntax { .. })
    ));
    assert!(matches!(
        eval_amount("1+2)"),
        Err(AmountError::Syntax { .. })
    ));
    assert!(matches!(
        eval_amount("1..2"),
        Err(AmountError::BadNumber { .. })
    ));
}

#[test]
fn a_negative_result_is_refused() {
    // 2 - 5 = -3: the side (Dr/Cr) carries the sign, so a negative amount is an error.
    assert_eq!(eval_amount("2-5"), Err(AmountError::Negative));
}

#[test]
fn overflow_is_an_error_not_a_wrap() {
    // 20-digit factors multiply to about 1e40, beyond the 28-digit decimal range.
    assert_eq!(
        eval_amount("99999999999999999999*99999999999999999999"),
        Err(AmountError::Overflow)
    );
    // provenance: derived; i64::MAX = 9,223,372,036,854,775,807 paise. 92,233,720,368,547,759
    // rupees x 100 = 9,223,372,036,854,775,900 paise, which is larger.
    assert_eq!(
        eval_amount("92233720368547759"),
        Err(AmountError::Money(MoneyError::Overflow))
    );
}

// ---- dates -----------------------------------------------------------------------------------

#[test]
fn dates_without_a_year_follow_the_financial_year() {
    // provenance: derived; FY 2026-27 runs 1 April 2026 to 31 March 2027, so month 4..=12 is 2026
    // and month 1..=3 is 2027.
    assert_eq!(parse_entry_date("15-4", fy()), Ok(date!(2026 - 04 - 15)));
    assert_eq!(parse_entry_date("15/4", fy()), Ok(date!(2026 - 04 - 15)));
    assert_eq!(parse_entry_date("5-1", fy()), Ok(date!(2027 - 01 - 05)));
    assert_eq!(parse_entry_date("31-3", fy()), Ok(date!(2027 - 03 - 31)));
}

#[test]
fn dates_with_a_year() {
    assert_eq!(
        parse_entry_date("15-04-2026", fy()),
        Ok(date!(2026 - 04 - 15))
    );
    assert_eq!(
        parse_entry_date("2026-04-15", fy()),
        Ok(date!(2026 - 04 - 15))
    );
    assert_eq!(
        parse_entry_date(" 1/12/2026 ", fy()),
        Ok(date!(2026 - 12 - 01))
    );
}

#[test]
fn bad_dates_are_typed_errors() {
    assert_eq!(parse_entry_date("", fy()), Err(EntryDateError::Empty));
    assert!(matches!(
        parse_entry_date("abc", fy()),
        Err(EntryDateError::Format(_))
    ));
    // Two-digit years are ambiguous and refused.
    assert!(matches!(
        parse_entry_date("15-4-26", fy()),
        Err(EntryDateError::Format(_))
    ));
    // provenance: derived; month 13 does not exist, and February 2027 has 28 days.
    assert!(matches!(
        parse_entry_date("15-13", fy()),
        Err(EntryDateError::NotADate(_))
    ));
    assert!(matches!(
        parse_entry_date("29-2", fy()),
        Err(EntryDateError::NotADate(_))
    ));
}

#[test]
fn date_display_is_day_month_year() {
    assert_eq!(format_entry_date(date!(2026 - 04 - 05)), "05-04-2026");
}

// ---- type-ahead ------------------------------------------------------------------------------

fn hit_names(items: &[PickerItem]) -> Vec<String> {
    items
        .iter()
        .map(|i| match i {
            PickerItem::Ledger(h) => h.name.clone(),
            PickerItem::Create { name } => format!("+{name}"),
        })
        .collect()
}

#[test]
fn an_exact_name_comes_first_and_offers_no_create_item() {
    let b = books();
    let items = search_ledgers(&b.masters, "rent", &[], &BTreeMap::new());
    // "rent" is an exact (case-insensitive) match of Rent; no other ledger contains r-e-n-t in
    // order ("Shree Stationers" has r,e,e,... but no 'n' after 'e' before 't'? see below).
    // Derivation of the second hit: letters r,e,n,t in order inside "shree stationers":
    // r(3) e(4) then n(11) then t(?) - after n at "stationers" index 11 there is 't'? No: "st-a-t-i-o-n-e-r-s"
    // has t before n, and nothing after n is t, so the subsequence fails.
    assert_eq!(hit_names(&items), ["Rent"]);
}

#[test]
fn prefix_then_in_order_letters_and_a_create_item_last() {
    let b = books();
    let items = search_ledgers(&b.masters, "sha", &[], &BTreeMap::new());
    // The ranking order (exact, recents, match quality) is a design choice of this screen,
    // pending observed provenance; the match facts below are derived by hand.
    // derived: "sharma traders" starts with "sha" (rank 1). "shree stationers" does not start,
    // has no word starting, does not contain "sha", but s,h,a appear in order (s,h in "shree",
    // a in "stationers"), rank 4. "cash" contains "sha"? c-a-s-h: no ("sh" but then no 'a').
    // s,h,a in order in "cash": s(2) h(3) then no 'a' after: no. "hdfc bank": no 's'. "rent": no.
    assert_eq!(
        hit_names(&items),
        ["Sharma Traders", "Shree Stationers", "+sha"]
    );
}

#[test]
fn an_alias_match_is_exact_and_reports_the_alias() {
    let b = books();
    let items = search_ledgers(&b.masters, "st", &[], &BTreeMap::new());
    // The ranking order is a design choice of this screen (see above); the match facts are
    // derived by hand.
    // derived: alias "ST" equals "st" case-insensitively (rank 0, listed first); "shree
    // stationers" has a word starting with "st" (rank 2). "sharma traders" by name alone would
    // only match in order (rank 4) so the alias wins. "cash", "hdfc bank", "rent" have no 's'
    // followed by 't'... "rent" has 't' but no 's'; "cash" has 's' but no 't'.
    assert_eq!(
        hit_names(&items),
        ["Sharma Traders", "Shree Stationers", "+st"]
    );
    let PickerItem::Ledger(first) = items.first().unwrap() else {
        panic!("first item is a ledger");
    };
    assert_eq!(first.via_alias.as_deref(), Some("ST"));
}

#[test]
fn recents_come_first_but_after_an_exact_match() {
    let b = books();
    // Recents-first is spec 7's "recents first"; the order inside it (by lowercase name) is a
    // design choice of this screen, pending observed provenance.
    // Empty query lists all ledgers: the recent one first, then by name (case-insensitive):
    // cash, hdfc bank, kaveri packaging, sharma traders, shree stationers.
    let items = search_ledgers(&b.masters, "", &[RENT], &BTreeMap::new());
    assert_eq!(
        hit_names(&items),
        [
            "Rent",
            "Cash",
            "HDFC Bank",
            "Kaveri Packaging",
            "Sharma Traders",
            "Shree Stationers"
        ]
    );
    // Query "s": Shree is recent so it goes first; Sharma is a prefix match (rank 1) like Shree;
    // "cash" contains an 's' (rank 3). "kaveri packaging", "hdfc bank", "rent": no 's'.
    let items = search_ledgers(&b.masters, "s", &[SHREE], &BTreeMap::new());
    assert_eq!(
        hit_names(&items),
        ["Shree Stationers", "Sharma Traders", "Cash", "+s"]
    );
    // An exact match beats a recent one: "cash" is exact, Shree is recent.
    let items = search_ledgers(&b.masters, "cash", &[SHREE], &BTreeMap::new());
    assert_eq!(hit_names(&items), ["Cash"]);
}

#[test]
fn hits_carry_the_supplied_balance_and_gstin_and_never_invent_a_balance() {
    let b = books();
    let mut balances = BTreeMap::new();
    // Input handed in by the host; the screen only passes it through.
    balances.insert(SHARMA, SignedAmount::dr(Paise::new(15_000_000)));
    let items = search_ledgers(&b.masters, "sharma", &[], &balances);
    let PickerItem::Ledger(sharma) = items.first().unwrap() else {
        panic!("first item is a ledger");
    };
    assert_eq!(
        sharma.balance,
        Some(SignedAmount::dr(Paise::new(15_000_000)))
    );
    assert_eq!(sharma.gstin.as_deref(), Some("29ABCDE1234F1ZW"));
    let items = search_ledgers(&b.masters, "shree", &[], &balances);
    let PickerItem::Ledger(shree) = items.first().unwrap() else {
        panic!("first item is a ledger");
    };
    assert_eq!(shree.balance, None);
    assert_eq!(shree.gstin, None);
}

// ---- the draft: balancing line, building the voucher -----------------------------------------

fn row(ledger: LedgerId, side: DrCr, amount: &str) -> DraftRow {
    DraftRow {
        ledger: Some(LedgerRef::Chosen(ledger)),
        ledger_text: String::new(),
        side,
        amount_text: amount.to_owned(),
    }
}

fn draft(kind: EntryKind, date_text: &str, rows: Vec<DraftRow>) -> VoucherDraft {
    VoucherDraft {
        type_id: T_JOURNAL,
        kind,
        date_text: date_text.to_owned(),
        narration: String::new(),
        rows,
    }
}

#[test]
fn the_balancing_line_is_prefilled_on_the_opposite_side() {
    // provenance: derived; spec 5 invariant 1 (sum Dr = sum Cr). A single credit of 5000 needs a
    // debit of 5000: 5000 rupees prints as "5000.00".
    // Changed in wave 2 (S-H): a payment is single entry now, where the account line's amount is
    // the total of the particulars and is never typed, so a lone typed account line is not a
    // double-entry draft. The balancing-line prefill is double-entry behaviour, so the same
    // expectations are exercised on a journal, which keeps double entry.
    let mut d = draft(
        EntryKind::Journal,
        "15-4",
        vec![row(CASH, DrCr::Cr, "5000")],
    );
    assert_eq!(
        d.balance(),
        Ok(Balance::Needs {
            side: DrCr::Dr,
            amount: rupees(5000)
        })
    );
    d.push_prefilled_row().unwrap();
    let last = d.rows.last().unwrap();
    assert_eq!(last.side, DrCr::Dr);
    assert_eq!(last.amount_text, "5000.00");
    assert_eq!(last.ledger, None);
}

#[test]
fn the_balancing_line_uses_inline_arithmetic_totals() {
    // provenance: derived; Dr Rent 1200 and Dr Shree 1200*18% = 216 total 1416 debit, so the
    // credit that balances is 1416.00.
    let mut d = draft(
        EntryKind::Journal,
        "15-4",
        vec![
            row(RENT, DrCr::Dr, "1200"),
            row(SHREE, DrCr::Dr, "1200*18%"),
        ],
    );
    d.push_prefilled_row().unwrap();
    let last = d.rows.last().unwrap();
    assert_eq!(last.side, DrCr::Cr);
    assert_eq!(last.amount_text, "1416.00");
}

#[test]
fn a_balanced_draft_gets_an_empty_line_on_the_last_side() {
    let mut d = draft(
        EntryKind::Journal,
        "15-4",
        vec![row(RENT, DrCr::Dr, "10"), row(CASH, DrCr::Cr, "10")],
    );
    assert_eq!(d.balance(), Ok(Balance::Balanced));
    d.push_prefilled_row().unwrap();
    let last = d.rows.last().unwrap();
    assert_eq!(last.side, DrCr::Cr);
    assert_eq!(last.amount_text, "");
}

#[test]
fn a_bad_amount_names_its_line() {
    let d = draft(
        EntryKind::Journal,
        "15-4",
        vec![row(RENT, DrCr::Dr, "10"), row(CASH, DrCr::Cr, "1/0")],
    );
    assert_eq!(
        d.balance(),
        Err(DraftError::Amount {
            line: 2,
            source: AmountError::DivideByZero
        })
    );
}

#[test]
fn a_good_draft_becomes_a_voucher() {
    let b = books();
    let mut d = draft(
        EntryKind::Journal,
        "15-4",
        vec![row(RENT, DrCr::Dr, "1200*18%"), row(CASH, DrCr::Cr, "216")],
    );
    d.narration = "  office rent tax  ".to_owned();
    let Outcome::Value(v) = d
        .build_voucher(&b.masters, fy(), b.company.books_begin)
        .unwrap()
    else {
        panic!("expected a voucher");
    };
    // provenance: derived; 1200 x 18% = 216 rupees = 21,600 paise on each side.
    assert_eq!(v.date, date!(2026 - 04 - 15));
    assert_eq!(v.ledger_lines.len(), 2);
    assert_eq!(v.ledger_lines[0].ledger, RENT);
    assert_eq!(
        v.ledger_lines[0].amount,
        SignedAmount::dr(Paise::new(21_600))
    );
    assert_eq!(v.ledger_lines[1].ledger, CASH);
    assert_eq!(
        v.ledger_lines[1].amount,
        SignedAmount::cr(Paise::new(21_600))
    );
    // Number is the store's to allocate (spec 5 invariant 7); status and mode are the plain ones.
    assert_eq!(v.number, None);
    assert_eq!(v.status, VoucherStatus::Regular);
    assert_eq!(v.mode, VoucherMode::AsVoucher);
    assert_eq!(v.narration, "office rent tax");
    assert_eq!(v.inventory_lines, [] as [ledgerly_model::InventoryLine; 0]);
    assert!(v.gst.is_none());
}

#[test]
fn blank_lines_are_skipped_when_building() {
    let b = books();
    let d = draft(
        EntryKind::Journal,
        "15-4",
        vec![
            row(RENT, DrCr::Dr, "10"),
            DraftRow::empty(DrCr::Cr),
            row(CASH, DrCr::Cr, "10"),
        ],
    );
    let Outcome::Value(v) = d
        .build_voucher(&b.masters, fy(), b.company.books_begin)
        .unwrap()
    else {
        panic!("expected a voucher");
    };
    assert_eq!(v.ledger_lines.len(), 2);
}

#[test]
fn an_unbalanced_draft_reports_both_totals() {
    let b = books();
    let d = draft(
        EntryKind::Journal,
        "15-4",
        vec![row(RENT, DrCr::Dr, "100"), row(CASH, DrCr::Cr, "99.50")],
    );
    // provenance: derived; Dr 100.00 = 10,000 paise, Cr 99.50 = 9,950 paise.
    assert_eq!(
        d.build_voucher(&b.masters, fy(), b.company.books_begin),
        Err(DraftError::Voucher(VoucherError::Unbalanced {
            dr: Paise::new(10_000),
            cr: Paise::new(9_950)
        }))
    );
}

#[test]
fn a_date_before_books_begin_is_rejected_naming_both_dates() {
    let b = books();
    let d = draft(
        EntryKind::Journal,
        "2026-03-31",
        vec![row(RENT, DrCr::Dr, "10"), row(CASH, DrCr::Cr, "10")],
    );
    // spec 5 invariant 6: voucher date >= books_begin (1 April 2026 in this fixture).
    assert_eq!(
        d.build_voucher(&b.masters, fy(), b.company.books_begin),
        Err(DraftError::Voucher(VoucherError::BeforeBooksBegin {
            date: date!(2026 - 03 - 31),
            books_begin: date!(2026 - 04 - 01)
        }))
    );
}

#[test]
fn incomplete_zero_unknown_and_pending_lines_are_errors() {
    let b = books();
    let build = |rows: Vec<DraftRow>| {
        draft(EntryKind::Journal, "15-4", rows).build_voucher(
            &b.masters,
            fy(),
            b.company.books_begin,
        )
    };
    // An amount without a ledger (line 1).
    let mut no_ledger = DraftRow::empty(DrCr::Dr);
    no_ledger.amount_text = "10".to_owned();
    assert_eq!(
        build(vec![no_ledger]),
        Err(DraftError::IncompleteLine { line: 1 })
    );
    // A ledger without an amount (line 1).
    assert_eq!(
        build(vec![row(RENT, DrCr::Dr, "")]),
        Err(DraftError::IncompleteLine { line: 1 })
    );
    // A zero amount.
    assert_eq!(
        build(vec![row(RENT, DrCr::Dr, "0")]),
        Err(DraftError::ZeroAmount { line: 1 })
    );
    // A ledger id that is not in the masters.
    assert_eq!(
        build(vec![row(LedgerId(999), DrCr::Dr, "5")]),
        Err(DraftError::Voucher(VoucherError::UnknownLedger(LedgerId(
            999
        ))))
    );
    // A ledger the host has not created yet.
    let mut pending = DraftRow::empty(DrCr::Dr);
    pending.ledger = Some(LedgerRef::Pending("Petty".to_owned()));
    pending.amount_text = "5".to_owned();
    assert_eq!(
        build(vec![pending]),
        Err(DraftError::PendingLedger {
            line: 1,
            name: "Petty".to_owned()
        })
    );
    // Nothing entered.
    assert_eq!(
        build(vec![DraftRow::empty(DrCr::Dr)]),
        Err(DraftError::Voucher(VoucherError::Empty))
    );
}

// ---- the screen: keyboard flow ---------------------------------------------------------------

#[test]
fn opening_checks_the_voucher_type() {
    let b = books();
    assert!(matches!(
        VoucherEntryScreen::open(&b, T_PAYMENT, fy(), TODAY),
        Ok(Outcome::Value(_))
    ));
    // Sales and stock kinds are not accounting-line vouchers: Unsupported, not a guess.
    assert!(matches!(
        VoucherEntryScreen::open(&b, T_SALES, fy(), TODAY),
        Ok(Outcome::Unsupported(_))
    ));
    assert!(matches!(
        VoucherEntryScreen::open(&b, T_STOCK, fy(), TODAY),
        Ok(Outcome::Unsupported(_))
    ));
    assert!(matches!(
        VoucherEntryScreen::open(&b, VoucherTypeId(404), fy(), TODAY),
        Err(OpenError::UnknownVoucherType(VoucherTypeId(404)))
    ));
    assert!(matches!(
        VoucherEntryScreen::open(&b, T_OFF, fy(), TODAY),
        Err(OpenError::Inactive(_))
    ));
}

#[test]
fn first_side_follows_double_entry_for_payment_and_receipt() {
    // design choice; pending observed provenance. Double entry fixes that the voucher has a debit
    // side and a credit side, not which line is entered first. The screen starts a payment on the
    // paying account's credit and a receipt on the receiving account's debit.
    assert_eq!(EntryKind::Payment.first_side(), DrCr::Cr);
    assert_eq!(EntryKind::Receipt.first_side(), DrCr::Dr);
    assert_eq!(EntryKind::Journal.first_side(), DrCr::Dr);
}

#[test]
fn enter_walks_ledger_amount_balancing_line_and_saves() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_JOURNAL);
    assert_eq!(s.focus(), Field::Ledger(0));

    // Line 1: type-ahead picks Rent, then the amount is an expression.
    s.set_ledger_text(0, "rent");
    s.submit(m);
    assert_eq!(s.focus(), Field::Amount(0));
    assert_eq!(s.draft().rows[0].ledger, Some(LedgerRef::Chosen(RENT)));
    s.set_amount_text(0, "1200*18%");
    s.submit(m);
    // The expression is replaced by its value, and the balancing line is prefilled
    // (derived: 1200 x 18% = 216.00; a debit of 216.00 needs a credit of 216.00).
    assert_eq!(s.draft().rows[0].amount_text, "216.00");
    assert_eq!(s.draft().rows.len(), 2);
    assert_eq!(s.draft().rows[1].side, DrCr::Cr);
    assert_eq!(s.draft().rows[1].amount_text, "216.00");
    assert_eq!(s.focus(), Field::Ledger(1));

    // Line 2: pick Cash, accept the prefilled amount; the voucher balances, so Narration is next.
    s.set_ledger_text(1, "cash");
    s.submit(m);
    assert_eq!(s.focus(), Field::Amount(1));
    s.submit(m);
    assert_eq!(s.draft().rows.len(), 2);
    assert_eq!(s.focus(), Field::Narration);

    // Enter on the narration saves: the voucher goes to the outbox, once.
    s.submit(m);
    assert_eq!(s.error_text(), None);
    let v = s.take_save_request().unwrap();
    assert_eq!(v.ledger_lines.len(), 2);
    assert_eq!(
        v.ledger_lines[0].amount,
        SignedAmount::dr(Paise::new(21_600))
    );
    assert_eq!(
        v.ledger_lines[1].amount,
        SignedAmount::cr(Paise::new(21_600))
    );
    // 21-11-2026 is TODAY in the fixture.
    assert_eq!(v.date, TODAY);
    assert!(s.take_save_request().is_none());
}

#[test]
fn enter_on_an_empty_ledger_field_is_an_error_not_a_skip() {
    let b = books();
    let mut s = open(&b, T_JOURNAL);
    s.submit(&b.masters);
    assert!(s.error_text().is_some());
    assert_eq!(s.focus(), Field::Ledger(0));
}

#[test]
fn a_bad_amount_keeps_the_cursor_and_says_why() {
    let b = books();
    let mut s = open(&b, T_JOURNAL);
    s.set_ledger_text(0, "rent");
    s.submit(&b.masters);
    s.set_amount_text(0, "12a");
    s.submit(&b.masters);
    assert_eq!(s.focus(), Field::Amount(0));
    assert!(s.error_text().unwrap().contains("line 1"));
    s.set_amount_text(0, "0");
    s.submit(&b.masters);
    assert!(s.error_text().unwrap().contains("zero"));
}

#[test]
fn enter_on_an_empty_ledger_field_after_a_balanced_voucher_ends_the_list() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_JOURNAL);
    s.set_ledger_text(0, "rent");
    s.submit(m);
    s.set_amount_text(0, "10");
    s.submit(m);
    s.set_ledger_text(1, "cash");
    s.submit(m);
    s.submit(m);
    assert_eq!(s.focus(), Field::Narration);
    // Going back and adding a third, empty line by hand, then Enter on its empty ledger ends the
    // list and drops it.
    s.draft_mut().rows.push(DraftRow::empty(DrCr::Cr));
    s.step_back();
    assert_eq!(s.focus(), Field::Amount(2));
    s.step_back();
    assert_eq!(s.focus(), Field::Ledger(2));
    s.submit(m);
    assert_eq!(s.draft().rows.len(), 2);
    assert_eq!(s.focus(), Field::Narration);
}

#[test]
fn shift_enter_steps_back() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_JOURNAL);
    s.set_ledger_text(0, "rent");
    s.submit(m);
    assert_eq!(s.focus(), Field::Amount(0));
    s.step_back();
    assert_eq!(s.focus(), Field::Ledger(0));
    s.step_back();
    assert_eq!(s.focus(), Field::Date);
    s.step_back();
    assert_eq!(s.focus(), Field::Date);
}

#[test]
fn the_date_field_is_normalised_on_enter_and_errors_stay_put() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_JOURNAL);
    s.step_back();
    assert_eq!(s.focus(), Field::Date);
    s.draft_mut().date_text = "5-5".to_owned();
    s.submit(m);
    assert_eq!(s.draft().date_text, "05-05-2026");
    assert_eq!(s.focus(), Field::Ledger(0));
    s.step_back();
    s.draft_mut().date_text = "31-2".to_owned();
    s.submit(m);
    assert_eq!(s.focus(), Field::Date);
    assert!(s.error_text().is_some());
}

#[test]
fn arrow_keys_move_the_highlight_inside_the_list() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_JOURNAL);
    s.set_ledger_text(0, "s");
    // Items: Shree/Sharma (prefix), Cash, then the create item: 4 entries, indexes 0..=3.
    let len = s.picker_items(m).len();
    assert_eq!(len, 4);
    s.move_pick(m, -1);
    assert_eq!(s.pick_index(), 0);
    for _ in 0..10 {
        s.move_pick(m, 1);
    }
    assert_eq!(s.pick_index(), 3);
    // Enter on the create item opens the create form instead of picking a ledger.
    s.submit(m);
    assert!(s.is_creating());
}

#[test]
fn the_highlighted_item_is_the_one_picked() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_JOURNAL);
    s.set_ledger_text(0, "sha");
    // [Sharma Traders, Shree Stationers, Create]: move down once and pick Shree.
    s.move_pick(m, 1);
    s.submit(m);
    assert_eq!(s.draft().rows[0].ledger, Some(LedgerRef::Chosen(SHREE)));
    assert_eq!(s.draft().rows[0].ledger_text, "Shree Stationers");
}

#[test]
fn switching_a_side_and_deleting_a_line() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_JOURNAL);
    assert_eq!(s.draft().rows[0].side, DrCr::Dr);
    s.toggle_side(0);
    assert_eq!(s.draft().rows[0].side, DrCr::Cr);

    s.set_ledger_text(0, "rent");
    s.submit(m);
    s.set_amount_text(0, "10");
    s.submit(m);
    assert_eq!(s.draft().rows.len(), 2);
    // Delete line 2 while its ledger field has the keyboard.
    assert_eq!(s.focus(), Field::Ledger(1));
    s.delete_line();
    assert_eq!(s.draft().rows.len(), 1);
    assert_eq!(s.focus(), Field::Ledger(0));
    // Deleting the only line empties it instead.
    s.delete_line();
    assert_eq!(s.draft().rows.len(), 1);
    assert!(s.draft().is_blank());
}

// ---- create a ledger inline (Alt+C) ----------------------------------------------------------

#[test]
fn alt_c_creates_a_ledger_request_and_the_line_waits_for_the_host() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_JOURNAL);
    s.set_ledger_text(0, "Petty Expenses");
    s.begin_create();
    assert!(s.is_creating());

    // A name that already exists is refused (case-insensitive), and nothing is requested.
    s.confirm_create(m, "cash", Some(GROUP));
    assert!(s.error_text().unwrap().contains("already exists"));
    assert_eq!(
        s.take_master_requests(),
        [] as [ledgerly_app::screens::voucher_entry::CreateLedgerRequest; 0]
    );
    // A group is required, and it must exist.
    s.confirm_create(m, "Petty Expenses", None);
    assert!(s.error_text().unwrap().contains("group"));
    s.confirm_create(m, "Petty Expenses", Some(GroupId(77)));
    assert!(s.error_text().unwrap().contains("does not exist"));
    assert!(s.is_creating());

    s.confirm_create(m, "Petty Expenses", Some(GROUP));
    assert!(!s.is_creating());
    assert_eq!(s.focus(), Field::Amount(0));
    assert_eq!(
        s.draft().rows[0].ledger,
        Some(LedgerRef::Pending("Petty Expenses".to_owned()))
    );
    let requests = s.take_master_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].name, "Petty Expenses");
    assert_eq!(requests[0].group, GROUP);

    // While pending, saving names the line and the ledger.
    s.set_amount_text(0, "5");
    s.draft_mut().rows.push(row(CASH, DrCr::Cr, "5"));
    s.save(m);
    assert!(s.error_text().unwrap().contains("Petty Expenses"));
    assert!(s.take_save_request().is_none());

    // The host creates it: the line now uses the new id, and it is among the recents.
    s.resolve_created_ledger("Petty Expenses", LedgerId(50));
    assert_eq!(
        s.draft().rows[0].ledger,
        Some(LedgerRef::Chosen(LedgerId(50)))
    );
}

#[test]
fn enter_on_the_create_item_opens_the_form_with_the_typed_name() {
    let b = books();
    let mut s = open(&b, T_JOURNAL);
    s.set_ledger_text(0, "Zzz Unknown");
    assert_eq!(
        s.picker_items(&b.masters),
        vec![PickerItem::Create {
            name: "Zzz Unknown".to_owned()
        }]
    );
    s.submit(&b.masters);
    assert!(s.is_creating());
}

#[test]
fn a_rejected_creation_clears_the_pending_line() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_JOURNAL);
    s.set_ledger_text(0, "Petty");
    s.begin_create();
    s.confirm_create(m, "Petty", Some(GROUP));
    s.reject_created_ledger("Petty", "the store is read-only");
    assert_eq!(s.draft().rows[0].ledger, None);
    assert!(s.error_text().unwrap().contains("read-only"));
}

// ---- save, fresh voucher, discard ------------------------------------------------------------

fn filled_screen(b: &Books) -> VoucherEntryScreen {
    let m = &b.masters;
    let mut s = open(b, T_JOURNAL);
    s.set_ledger_text(0, "rent");
    s.submit(m);
    s.set_amount_text(0, "10");
    s.submit(m);
    s.set_ledger_text(1, "cash");
    s.submit(m);
    s.submit(m);
    s
}

#[test]
fn a_fresh_voucher_follows_a_confirmed_save_and_keeps_the_date() {
    let b = books();
    let mut s = filled_screen(&b);
    s.draft_mut().date_text = "05-05-2026".to_owned();
    s.save(&b.masters);
    assert!(s.take_save_request().is_some());
    s.confirm_saved();
    assert!(s.draft().is_blank());
    assert_eq!(s.draft().rows.len(), 1);
    assert_eq!(s.draft().date_text, "05-05-2026");
    assert_eq!(s.focus(), Field::Ledger(0));
    assert!(s.info_text().unwrap().contains("Saved"));
}

#[test]
fn a_failed_post_keeps_the_draft_and_shows_the_reason() {
    let b = books();
    let mut s = filled_screen(&b);
    s.save(&b.masters);
    assert!(s.take_save_request().is_some());
    s.save_failed("period is locked up to 30-06-2026");
    assert!(!s.draft().is_blank());
    assert!(s.error_text().unwrap().contains("locked"));
}

#[test]
fn leaving_a_started_voucher_asks_first() {
    let b = books();
    let mut s = open(&b, T_JOURNAL);
    assert!(s.request_leave(), "a blank voucher closes at once");
    s.draft_mut().narration = "x".to_owned();
    assert!(!s.request_leave());
    assert!(s.is_confirming_discard());
    assert!(!s.answer_discard(false));
    assert!(!s.is_confirming_discard());
    assert!(!s.request_leave());
    assert!(s.answer_discard(true));
}

// ---- pending / unsupported -------------------------------------------------------------------

#[test]
fn ledgers_that_need_bill_allocations_are_unsupported_not_posted_without_them() {
    // pending: the bill allocation sub-form (spec 7: "sub-forms pop automatically") is not built,
    // and a bill-wise line without allocations would break spec 5 invariant 2.
    let b = books();
    let d = draft(
        EntryKind::Journal,
        "15-4",
        vec![row(BILLWISE, DrCr::Dr, "10"), row(CASH, DrCr::Cr, "10")],
    );
    let out = d
        .build_voucher(&b.masters, fy(), b.company.books_begin)
        .unwrap();
    assert!(out.is_unsupported());
}

#[test]
fn saving_with_an_unsupported_ledger_says_so() {
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_JOURNAL);
    s.set_ledger_text(0, "kaveri");
    s.submit(m);
    s.set_amount_text(0, "10");
    s.submit(m);
    s.set_ledger_text(1, "cash");
    s.submit(m);
    s.submit(m);
    s.submit(m);
    assert!(s.error_text().unwrap().contains("Not supported yet"));
    assert!(s.take_save_request().is_none());
}

// ---- egui: one frame, keys ------------------------------------------------------------------

fn frame(
    ctx: &egui::Context,
    screen: &mut VoucherEntryScreen,
    app: &mut AppCtx,
    input: RawInput,
) -> ScreenAction {
    let mut action = ScreenAction::Stay;
    let out = ctx.run_ui(input, |ui| {
        action = screen.ui(ui, app);
    });
    out.drop_without_applying_deltas();
    action
}

fn key(key: Key, modifiers: Modifiers) -> RawInput {
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

#[test]
fn the_screen_draws_with_and_without_a_company() {
    let b = books();
    let ctx = egui::Context::default();
    let mut s = open(&b, T_JOURNAL);
    // provenance: the mock's crumbs for payment and receipt are Home > <type> only
    // (design/mock-v2/index.html line 1388, `[s.type]`); the other kinds follow the same pattern.
    assert_eq!(s.title(), "Journal");
    let mut closed = AppCtx::default();
    assert!(matches!(
        frame(&ctx, &mut s, &mut closed, RawInput::default()),
        ScreenAction::Stay
    ));
    let mut app = AppCtx {
        books: Some(b),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    assert!(matches!(
        frame(&ctx, &mut s, &mut app, RawInput::default()),
        ScreenAction::Stay
    ));
}

#[test]
fn ctrl_a_saves_and_reports_an_empty_voucher() {
    let b = books();
    let ctx = egui::Context::default();
    let mut s = open(&b, T_JOURNAL);
    let mut app = AppCtx {
        books: Some(b),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, key(Key::A, Modifiers::COMMAND));
    // spec 5 invariant 1 and VoucherError::Empty: nothing entered cannot be saved.
    assert!(s.error_text().unwrap().contains("no lines"));
}

#[test]
fn escape_on_a_started_voucher_asks_then_y_closes_the_screen() {
    let b = books();
    let ctx = egui::Context::default();
    let mut s = open(&b, T_JOURNAL);
    s.draft_mut().narration = "x".to_owned();
    let mut app = AppCtx {
        books: Some(b),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    frame(&ctx, &mut s, &mut app, RawInput::default());
    let action = frame(&ctx, &mut s, &mut app, key(Key::Escape, Modifiers::NONE));
    assert!(matches!(action, ScreenAction::Stay));
    assert!(s.is_confirming_discard());
    let action = frame(&ctx, &mut s, &mut app, key(Key::Y, Modifiers::NONE));
    assert!(matches!(action, ScreenAction::Pop));
}

#[test]
fn escape_on_a_blank_voucher_is_left_for_the_shell() {
    // The shell pops the screen on an Esc that no screen consumed (spec 7: "Esc back"), so a blank
    // voucher must not consume it and must not open the discard question.
    let b = books();
    let ctx = egui::Context::default();
    let mut s = open(&b, T_JOURNAL);
    let mut app = AppCtx {
        books: Some(b),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    frame(&ctx, &mut s, &mut app, RawInput::default());
    let action = frame(&ctx, &mut s, &mut app, key(Key::Escape, Modifiers::NONE));
    assert!(matches!(action, ScreenAction::Stay));
    assert!(!s.is_confirming_discard());
}

#[test]
fn alt_s_does_not_switch_the_side() {
    // provenance: derived. Read 2026-11-21: Tally's shortcut page
    // https://help.tallysolutions.com/tally-prime/keyboard-shortcuts/keyboard-shortcuts-tally-prime/
    // gives Alt+S to the Stock Query report for the selected stock item, so this screen cannot
    // use it. The same page lists no key that switches a voucher line between Dr and Cr. Two more
    // pages were read: the accounting entry page
    // https://help.tallysolutions.com/tally-prime/accounting/accounting-entry-tally/
    // covers only post-dated vouchers, and the contra page
    // https://help.tallysolutions.com/record-contra-entry/
    // sets a line's side by the Dr or Cr field the ledger is entered in, not by a key. So the
    // keyboard binding is pending. Replaces the round-2 default (Alt+S switched the side). The
    // on-screen Dr / Cr button still switches it: switching_a_side_and_deleting_a_line calls the
    // same method the button calls.
    let b = books();
    let ctx = egui::Context::default();
    let mut s = open(&b, T_JOURNAL);
    let mut app = AppCtx {
        books: Some(b),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, key(Key::S, Modifiers::ALT));
    assert_eq!(s.draft().rows[0].side, DrCr::Dr);
}

#[test]
fn alt_c_opens_the_create_form_from_the_keyboard() {
    let b = books();
    let ctx = egui::Context::default();
    let mut s = open(&b, T_JOURNAL);
    s.set_ledger_text(0, "Gupta Hardware");
    let mut app = AppCtx {
        books: Some(b),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    frame(&ctx, &mut s, &mut app, RawInput::default());
    assert!(!s.is_creating());
    frame(&ctx, &mut s, &mut app, key(Key::C, Modifiers::ALT));
    // spec 7: Alt+C creates a master inline.
    assert!(s.is_creating());
}

#[test]
fn ctrl_s_saves_like_ctrl_a() {
    let b = books();
    let ctx = egui::Context::default();
    let mut s = open(&b, T_JOURNAL);
    let mut app = AppCtx {
        books: Some(b),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, key(Key::S, Modifiers::COMMAND));
    // Nothing entered: the same "no lines" refusal as Ctrl+A (spec 5 invariant 1).
    assert!(s.error_text().unwrap().contains("no lines"));
}

#[test]
fn ctrl_enter_does_not_save() {
    // Plan U2 section: "Ctrl+Enter alters the master under the cursor (the mock's Ctrl+Enter-to-
    // save is changed in U4/U5)". This screen has no master to alter, so it must not save, and
    // must not consume the key meant for the shell.
    let b = books();
    let ctx = egui::Context::default();
    let mut s = open(&b, T_JOURNAL);
    let mut app = AppCtx {
        books: Some(b),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    frame(&ctx, &mut s, &mut app, RawInput::default());
    frame(&ctx, &mut s, &mut app, key(Key::Enter, Modifiers::COMMAND));
    assert!(s.take_save_request().is_none());
    assert!(!s.error_text().is_some_and(|e| e.contains("no lines")));
}

#[test]
fn arrow_down_and_up_move_the_highlight_in_a_frame() {
    let b = books();
    let ctx = egui::Context::default();
    let mut s = open(&b, T_JOURNAL);
    s.set_ledger_text(0, "s");
    let mut app = AppCtx {
        books: Some(b),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    frame(&ctx, &mut s, &mut app, RawInput::default());
    assert_eq!(s.pick_index(), 0);
    frame(&ctx, &mut s, &mut app, key(Key::ArrowDown, Modifiers::NONE));
    assert_eq!(s.pick_index(), 1);
    frame(&ctx, &mut s, &mut app, key(Key::ArrowUp, Modifiers::NONE));
    assert_eq!(s.pick_index(), 0);
}

#[test]
fn a_ledger_that_needs_bill_allocations_is_refused_when_picked() {
    // pending (see above): the bill allocation sub-form is not built. The user hears it at the
    // pick, not after typing the whole voucher.
    let b = books();
    let m = &b.masters;
    let mut s = open(&b, T_JOURNAL);
    s.set_ledger_text(0, "kaveri");
    s.submit(m);
    assert_eq!(s.draft().rows[0].ledger, None);
    assert_eq!(s.focus(), Field::Ledger(0));
    let message = s.error_text().unwrap();
    assert!(message.contains("Not supported yet"));
    assert!(message.contains("Kaveri Packaging"));
}

#[test]
fn the_list_is_capped_at_fifty_and_recents_still_lead() {
    // Design choice of this screen (documented on search_ledgers), pending observed provenance:
    // at most 50 ledgers are listed. 60 ledgers named L00..L59 match the empty query with equal
    // rank, so they come in name order: L00..L49. A recent ledger beyond the cap (L59) is listed
    // first.
    let mut b = books();
    for n in 0..60_u64 {
        let id = LedgerId(100 + n);
        b.masters
            .ledgers
            .insert(id, ledger(id, &format!("L{n:02}"), &[]));
    }
    let items = search_ledgers(&b.masters, "L", &[LedgerId(159)], &BTreeMap::new());
    let names = hit_names(&items);
    assert_eq!(names.len(), 51, "50 ledgers and the create item");
    assert_eq!(names[0], "L59");
    assert_eq!(names[1], "L00");
    assert_eq!(names[49], "L48");
    assert_eq!(names[50], "+L");
}
