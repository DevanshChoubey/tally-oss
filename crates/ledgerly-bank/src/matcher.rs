//! Matching statement lines to vouchers (card K2, spec 6.5).
//!
//! [`suggest_matches`] reads an imported [`BankStatement`] and the books and returns
//! [`Candidate`]s. A candidate is only ever a suggestion: this module never changes the books and
//! never sets a bank date. Only a user-confirmed match sets the bank date (spec 6.5), and that
//! happens elsewhere.
//!
//! Frozen interface (plan card K2):
//!
//! ```text
//! pub struct MatchRule { .. }   pub struct Candidate { .. }
//! pub fn suggest_matches(stmt: &BankStatement, books: &Books, rules: &[MatchRule]) -> Vec<Candidate>;   // never confirmed
//! ```
//!
//! # What is matched
//!
//! A voucher is a match target when it is `Regular` or `PostDated`, has at least one line on the
//! statement's bank ledger, and none of its bank lines already carries a bank date. Its bank
//! amount is the net of its lines on that ledger: a net debit is a deposit (a statement credit),
//! a net credit is a withdrawal (a statement debit).
//!
//! Passes, in this order; a statement line or voucher claimed without ambiguity by an earlier
//! pass is not offered again:
//!
//! 1. **Exact, one line to vouchers.** A statement line is matched to one voucher of the same
//!    direction and the same amount in paise whose date is within [`DATE_TOLERANCE_DAYS`] of the
//!    line's date; when no single voucher fits, to two to [`MAX_GROUP`] such vouchers whose
//!    amounts add up to the line exactly (one deposit, three receipts). When several single
//!    vouchers fit, an instrument or reference number equal to the line's reference narrows them;
//!    if still several, every alternative is returned with `ambiguous` set.
//! 2. **Exact, several lines to one voucher.** Two to [`MAX_GROUP`] unmatched lines of one
//!    direction, each within the tolerance of the voucher's date, whose amounts add up to the
//!    voucher's amount.
//! 3. **Rule.** A line that is still unmatched is tested against the rules in order; the first
//!    rule whose pattern matches the description gives a suggestion to post the line to the
//!    rule's ledger (no voucher exists yet).
//!
//! A search that would be unbounded is bounded (at most [`POOL_CAP`] vouchers nearest in date are
//! considered for a group, and at most [`MAX_ALTERNATIVES`] alternatives are returned). Omitting a
//! suggestion is safe; stating a wrong one is not, so no case is guessed.

use std::collections::BTreeSet;

use ledgerly_bankfmt::RawLine;
use ledgerly_core::{Date, LedgerId, Paise, VoucherId};
use ledgerly_model::{Books, MatchKind, Voucher, VoucherStatus};

use crate::statement::BankStatement;

/// A voucher date and a statement date match when they are at most this many days apart.
pub const DATE_TOLERANCE_DAYS: i64 = 3;

/// The most vouchers (or lines) combined into one many-to-many suggestion.
pub const MAX_GROUP: usize = 4;

/// The most vouchers (or lines) searched for a combination, nearest in date first.
pub const POOL_CAP: usize = 24;

/// The most alternative suggestions returned for one statement line or voucher.
pub const MAX_ALTERNATIVES: usize = 5;

/// Which way money moved on a statement line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineDirection {
    /// Money in: a statement credit, a debit to the bank ledger.
    Deposit,
    /// Money out: a statement debit, a credit to the bank ledger.
    Withdrawal,
}

/// A user rule: statement lines whose description matches `pattern` are suggested for `ledger`.
///
/// The pattern is matched against the whole description, ignoring case, and `*` stands for any
/// run of characters (including none). There are no other special characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchRule {
    /// Pattern such as `"NEFT-*-RENT"`.
    pub pattern: String,
    /// Ledger to suggest for a matching line.
    pub ledger: LedgerId,
    /// Restrict the rule to deposits or withdrawals; `None` applies to both.
    pub direction: Option<LineDirection>,
}

impl MatchRule {
    /// A rule that applies to deposits and withdrawals alike.
    pub fn new(pattern: impl Into<String>, ledger: LedgerId) -> Self {
        MatchRule {
            pattern: pattern.into(),
            ledger,
            direction: None,
        }
    }

    fn applies_to(&self, direction: LineDirection) -> bool {
        match self.direction {
            None => true,
            Some(only) => only == direction,
        }
    }
}

/// A suggested match. It is never confirmed: it carries no authority to change a voucher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Row numbers (from the file) of the statement lines in the suggestion, ascending by line.
    pub statement_lines: Vec<u32>,
    /// Vouchers in the suggestion, ascending by id; empty for a rule suggestion.
    pub vouchers: Vec<VoucherId>,
    /// [`MatchKind::Exact`] or [`MatchKind::Rule`]; this module produces no other kind.
    pub kind: MatchKind,
    /// For a rule suggestion, the ledger to post the line to.
    pub suggested_ledger: Option<LedgerId>,
    /// The bank date that would be recorded if the user confirms: the date of the statement line
    /// (the latest one when several lines are matched). Not applied to anything.
    pub proposed_bank_date: Date,
    /// True when other candidates compete for the same line(s) or voucher(s); the user must pick.
    pub ambiguous: bool,
}

/// Suggests matches between the lines of `stmt` and the vouchers in `books`.
///
/// The result is ordered by the first statement line of each candidate, with alternatives for
/// the same line next to each other. Nothing is confirmed and `books` is not touched.
pub fn suggest_matches(stmt: &BankStatement, books: &Books, rules: &[MatchRule]) -> Vec<Candidate> {
    let entries = bank_entries(books, stmt.ledger());
    let lines: Vec<LineView<'_>> = stmt.lines().iter().filter_map(LineView::new).collect();
    let mut state = State::default();
    match_lines_to_vouchers(&lines, &entries, &mut state);
    match_vouchers_to_lines(&lines, &entries, &mut state);
    suggest_by_rules(&lines, rules, &mut state);
    let mut out = state.out;
    out.sort_by_key(|c| c.statement_lines.first().copied());
    out
}

/// Whether `text` matches `pattern`: whole string, ignoring case, `*` matches any run.
pub fn pattern_matches(pattern: &str, text: &str) -> bool {
    let pat: Vec<char> = pattern.to_lowercase().chars().collect();
    let txt: Vec<char> = text.to_lowercase().chars().collect();
    let (mut p_pos, mut t_pos) = (0_usize, 0_usize);
    // The last `*` seen: its place in the pattern, and the text position it has consumed up to.
    let mut star: Option<(usize, usize)> = None;
    while t_pos < txt.len() {
        match (pat.get(p_pos), txt.get(t_pos)) {
            (Some(&'*'), _) => {
                star = Some((p_pos, t_pos));
                p_pos += 1;
            }
            (Some(p), Some(t)) if p == t => {
                p_pos += 1;
                t_pos += 1;
            }
            (Some(_) | None, _) => match star {
                Some((star_p, star_t)) => {
                    p_pos = star_p + 1;
                    t_pos = star_t + 1;
                    star = Some((star_p, star_t + 1));
                }
                None => return false,
            },
        }
    }
    while pat.get(p_pos) == Some(&'*') {
        p_pos += 1;
    }
    p_pos == pat.len()
}

#[derive(Default)]
struct State {
    used_vouchers: BTreeSet<VoucherId>,
    matched_lines: BTreeSet<usize>,
    out: Vec<Candidate>,
}

/// One statement line reduced to what matching needs.
struct LineView<'a> {
    row: u32,
    date: Date,
    direction: LineDirection,
    amount: i64,
    description: &'a str,
    reference: Option<&'a str>,
}

impl<'a> LineView<'a> {
    /// `None` for a line that moves no money or has an impossible amount (K1 rejects those).
    fn new(line: &'a RawLine) -> Option<Self> {
        let (direction, amount) = if !line.credit.is_zero() && line.debit.is_zero() {
            (LineDirection::Deposit, line.credit)
        } else if !line.debit.is_zero() && line.credit.is_zero() {
            (LineDirection::Withdrawal, line.debit)
        } else {
            return None;
        };
        if amount.is_negative() {
            return None;
        }
        Some(LineView {
            row: line.row,
            date: line.date,
            direction,
            amount: amount.get(),
            description: &line.description,
            reference: line.reference.as_deref(),
        })
    }
}

/// A voucher's effect on the bank ledger.
struct Entry {
    id: VoucherId,
    date: Date,
    direction: LineDirection,
    amount: i64,
    refs: Vec<String>,
}

impl Entry {
    fn reference_matches(&self, line: &LineView<'_>) -> bool {
        match line.reference.map(str::trim) {
            Some(wanted) if !wanted.is_empty() => self
                .refs
                .iter()
                .any(|r| r.trim().eq_ignore_ascii_case(wanted)),
            Some(_) | None => false,
        }
    }
}

/// An amount with the key of the thing that carries it, for the combination search.
#[derive(Clone, Copy)]
struct Item<K> {
    key: K,
    amount: i64,
}

fn bank_entries(books: &Books, bank: LedgerId) -> Vec<Entry> {
    books
        .vouchers
        .values()
        .filter_map(|v| entry_for(v, bank))
        .collect()
}

fn entry_for(v: &Voucher, bank: LedgerId) -> Option<Entry> {
    match v.status {
        VoucherStatus::Regular | VoucherStatus::PostDated => {}
        VoucherStatus::Optional | VoucherStatus::Cancelled => return None,
    }
    let mut net = Paise::ZERO;
    let mut refs = Vec::new();
    let mut on_bank = false;
    for line in v.ledger_lines.iter().filter(|l| l.ledger == bank) {
        on_bank = true;
        if let Some(alloc) = &line.bank {
            if alloc.bank_date.is_some() {
                return None;
            }
            if let Some(number) = &alloc.instrument_number {
                refs.push(number.clone());
            }
        }
        net = net.checked_add(line.amount.to_signed().ok()?).ok()?;
    }
    if !on_bank || net.is_zero() {
        return None;
    }
    if let Some(r) = &v.reference {
        refs.push(r.clone());
    }
    let (direction, amount) = if net.is_negative() {
        (LineDirection::Withdrawal, net.checked_neg().ok()?)
    } else {
        (LineDirection::Deposit, net)
    };
    Some(Entry {
        id: v.id,
        date: v.date,
        direction,
        amount: amount.get(),
        refs,
    })
}

fn day_gap(a: Date, b: Date) -> i64 {
    (a - b).whole_days().abs()
}

fn near(a: Date, b: Date) -> bool {
    day_gap(a, b) <= DATE_TOLERANCE_DAYS
}

/// Pass 1: each statement line against one voucher, or a group of vouchers.
fn match_lines_to_vouchers(lines: &[LineView<'_>], entries: &[Entry], state: &mut State) {
    for (i, line) in lines.iter().enumerate() {
        let pool: Vec<&Entry> = entries
            .iter()
            .filter(|e| {
                e.direction == line.direction
                    && !state.used_vouchers.contains(&e.id)
                    && near(e.date, line.date)
            })
            .collect();
        let mut groups = single_voucher_groups(&pool, line);
        if groups.is_empty() {
            groups = voucher_group_sums(&pool, line);
        }
        if groups.is_empty() {
            continue;
        }
        let ambiguous = groups.len() > 1;
        state.matched_lines.insert(i);
        for vouchers in groups {
            if !ambiguous {
                state.used_vouchers.extend(vouchers.iter().copied());
            }
            state.out.push(Candidate {
                statement_lines: vec![line.row],
                vouchers,
                kind: MatchKind::Exact,
                suggested_ledger: None,
                proposed_bank_date: line.date,
                ambiguous,
            });
        }
    }
}

/// Vouchers with exactly the line's amount, narrowed by reference, nearest date first.
fn single_voucher_groups(pool: &[&Entry], line: &LineView<'_>) -> Vec<Vec<VoucherId>> {
    let mut same: Vec<&Entry> = pool
        .iter()
        .copied()
        .filter(|e| e.amount == line.amount)
        .collect();
    let by_reference: Vec<&Entry> = same
        .iter()
        .copied()
        .filter(|e| e.reference_matches(line))
        .collect();
    if !by_reference.is_empty() {
        same = by_reference;
    }
    same.sort_by_key(|e| (day_gap(e.date, line.date), e.id));
    same.truncate(MAX_ALTERNATIVES);
    same.iter().map(|e| vec![e.id]).collect()
}

/// Combinations of two or more vouchers whose amounts add up to the line.
fn voucher_group_sums(pool: &[&Entry], line: &LineView<'_>) -> Vec<Vec<VoucherId>> {
    let mut nearest: Vec<&Entry> = pool.to_vec();
    nearest.sort_by_key(|e| (day_gap(e.date, line.date), e.id));
    nearest.truncate(POOL_CAP);
    let items: Vec<Item<VoucherId>> = nearest
        .iter()
        .map(|e| Item {
            key: e.id,
            amount: e.amount,
        })
        .collect();
    group_sums(&items, line.amount)
}

/// Pass 2: each still-free voucher against a group of still-unmatched lines.
fn match_vouchers_to_lines(lines: &[LineView<'_>], entries: &[Entry], state: &mut State) {
    for entry in entries {
        if state.used_vouchers.contains(&entry.id) {
            continue;
        }
        let mut nearest: Vec<(usize, &LineView<'_>)> = lines
            .iter()
            .enumerate()
            .filter(|(i, l)| {
                l.direction == entry.direction
                    && !state.matched_lines.contains(i)
                    && near(entry.date, l.date)
            })
            .collect();
        nearest.sort_by_key(|(i, l)| (day_gap(entry.date, l.date), *i));
        nearest.truncate(POOL_CAP);
        let items: Vec<Item<usize>> = nearest
            .iter()
            .map(|(i, l)| Item {
                key: *i,
                amount: l.amount,
            })
            .collect();
        let groups = group_sums(&items, entry.amount);
        let ambiguous = groups.len() > 1;
        for group in groups {
            let chosen: Vec<&LineView<'_>> = group.iter().filter_map(|i| lines.get(*i)).collect();
            let Some(bank_date) = chosen.iter().map(|l| l.date).max() else {
                continue;
            };
            if !ambiguous {
                state.used_vouchers.insert(entry.id);
                state.matched_lines.extend(group.iter().copied());
            }
            state.out.push(Candidate {
                statement_lines: chosen.iter().map(|l| l.row).collect(),
                vouchers: vec![entry.id],
                kind: MatchKind::Exact,
                suggested_ledger: None,
                proposed_bank_date: bank_date,
                ambiguous,
            });
        }
    }
}

/// Pass 3: lines nothing else claimed, against the user's rules in order.
fn suggest_by_rules(lines: &[LineView<'_>], rules: &[MatchRule], state: &mut State) {
    for (i, line) in lines.iter().enumerate() {
        if state.matched_lines.contains(&i) {
            continue;
        }
        let rule = rules.iter().find(|r| {
            r.applies_to(line.direction) && pattern_matches(&r.pattern, line.description)
        });
        if let Some(rule) = rule {
            state.out.push(Candidate {
                statement_lines: vec![line.row],
                vouchers: Vec::new(),
                kind: MatchKind::Rule,
                suggested_ledger: Some(rule.ledger),
                proposed_bank_date: line.date,
                ambiguous: false,
            });
        }
    }
}

/// Smallest-size combinations (two to [`MAX_GROUP`] items) whose amounts sum to `target`.
///
/// Amounts must be positive. Each combination is sorted by key, and the list is sorted, so the
/// result does not depend on the order of `items`.
fn group_sums<K: Copy + Ord>(items: &[Item<K>], target: i64) -> Vec<Vec<K>> {
    for size in 2..=MAX_GROUP {
        let mut found = Vec::new();
        collect_sums(items, size, target, &mut Vec::new(), &mut found);
        if !found.is_empty() {
            for combo in &mut found {
                combo.sort();
            }
            found.sort();
            found.dedup();
            return found;
        }
    }
    Vec::new()
}

fn collect_sums<K: Copy>(
    items: &[Item<K>],
    size: usize,
    target: i64,
    chosen: &mut Vec<K>,
    found: &mut Vec<Vec<K>>,
) {
    if found.len() >= MAX_ALTERNATIVES {
        return;
    }
    if size == 0 {
        if target == 0 {
            found.push(chosen.clone());
        }
        return;
    }
    for (i, item) in items.iter().enumerate() {
        let Some(rest) = items.get(i + 1..) else {
            continue;
        };
        let Some(remaining) = target.checked_sub(item.amount) else {
            continue;
        };
        if remaining < 0 {
            continue;
        }
        chosen.push(item.key);
        collect_sums(rest, size - 1, remaining, chosen, found);
        chosen.pop();
    }
}
