//! Card s9: the words and bars Home draws from the engine's figures. Each expected value is
//! derived by hand (the working is beside it) or is the mock's own text
//! (`design/mock-v2/index.html`, `NUDGES()` line 1032 and `SCREENS.home` line 1061); none is the
//! output of the code under test.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_api::{
    DuplicatePayment, Gstr1Due, HomeFigures, HomeNudges, LateBills, MonthSales, PaymentRef,
};
use ledgerly_app::screens::home::HomeTarget;
use ledgerly_app::screens::home::chart::{bars, caption};
use ledgerly_app::screens::home::kpi::{cash_footnote, payables_footnote, receivables_footnote};
use ledgerly_app::screens::home::nudge::{NudgeAction, NudgeTone, nudges};
use ledgerly_core::{Outcome, Paise, Period, VoucherId};
use time::macros::date;

fn rs(rupees: i64) -> Paise {
    Paise::new(rupees * 100)
}

fn month(start: time::Date, sales: i64, to_date: bool) -> MonthSales {
    MonthSales {
        month_start: start,
        sales: rs(sales),
        to_date,
    }
}

fn unsupported<T>(reason: &str) -> Outcome<T> {
    Outcome::unsupported("x", reason)
}

fn figures(overdue: Outcome<LateBills>) -> HomeFigures {
    HomeFigures {
        cash_and_bank: unsupported("n/a"),
        receivables: unsupported("n/a"),
        payables: unsupported("n/a"),
        receivables_overdue: overdue,
        payables_past_due: unsupported("n/a"),
        sales_by_month: unsupported("n/a"),
    }
}

fn pair() -> DuplicatePayment {
    DuplicatePayment {
        first: PaymentRef {
            id: VoucherId(1),
            number: Some("PY/0304".to_owned()),
            date: date!(2026 - 10 - 28),
            locked: true,
            bank_date: None,
        },
        second: PaymentRef {
            id: VoucherId(2),
            number: Some("PY/0316".to_owned()),
            date: date!(2026 - 11 - 12),
            locked: false,
            bank_date: None,
        },
        ledger: "Electricity".to_owned(),
        payee: None,
        amount: rs(14_860),
        narration: "City Power, October bill".to_owned(),
        days_apart: 15,
    }
}

#[test]
fn the_cash_line_counts_bank_accounts() {
    // provenance: derived, the mock's line "Cash and 2 bank accounts"; one account is singular.
    assert_eq!(cash_footnote(2), "Cash and 2 bank accounts");
    assert_eq!(cash_footnote(1), "Cash and 1 bank account");
    assert_eq!(cash_footnote(0), "Cash and 0 bank accounts");
}

#[test]
fn the_bill_lines_use_the_engines_counts_and_never_a_made_up_zero() {
    // provenance: derived, the mock's lines "3 bills overdue more than 30 days" and "3 supplier
    // bills past due". A count the engine cannot give is said so, never shown as 0.
    let three = Outcome::Value(LateBills {
        count: 3,
        open: rs(1),
    });
    let one = Outcome::Value(LateBills {
        count: 1,
        open: rs(1),
    });
    assert_eq!(
        receivables_footnote(&three),
        "3 bills overdue more than 30 days"
    );
    assert_eq!(
        receivables_footnote(&one),
        "1 bill overdue more than 30 days"
    );
    assert_eq!(payables_footnote(&three), "3 supplier bills past due");
    assert_eq!(payables_footnote(&one), "1 supplier bill past due");
    let none = receivables_footnote(&unsupported("no bill detail"));
    assert_eq!(none, "Overdue bills not counted: no bill detail");
}

#[test]
fn bars_are_the_engines_months_with_heights_as_a_share_of_the_tallest() {
    // provenance: derived. Oct 13,40,000 is the tallest (1.0); Apr 10,20,000 / 13,40,000 =
    // 0.7612; Nov to date 3,06,085 / 13,40,000 = 0.2284. Labels are the month names; the tooltip
    // text is the amount in Indian grouping.
    let months = [
        month(date!(2026 - 04 - 01), 1_020_000, false),
        month(date!(2026 - 10 - 01), 1_340_000, false),
        month(date!(2026 - 11 - 01), 306_085, true),
    ];
    let b = bars(&months);
    assert_eq!(b.len(), 3);
    assert_eq!(b[0].label, "Apr");
    assert_eq!(b[1].label, "Oct");
    assert_eq!(b[2].label, "Nov");
    assert!((b[1].share - 1.0).abs() < 1e-6);
    assert!((b[0].share - 0.7612).abs() < 1e-3, "{}", b[0].share);
    assert!((b[2].share - 0.2284).abs() < 1e-3, "{}", b[2].share);
    assert_eq!(b[1].value_text, "\u{20B9}13,40,000.00");
    assert!(!b[1].to_date);
    assert!(b[2].to_date);
}

#[test]
fn a_month_with_a_credit_balance_or_no_sales_has_no_height() {
    // provenance: derived. A net debit month (sales returns above sales) has nothing to draw
    // above the baseline; the tooltip still carries the engine's signed amount. With every month
    // at nil there is no tallest bar to compare to, so every share is 0, not a division by zero.
    let months = [month(date!(2026 - 04 - 01), 0, false)];
    assert!((bars(&months)[0].share).abs() < 1e-9);
    let negative = [
        month(date!(2026 - 04 - 01), -500, false),
        month(date!(2026 - 05 - 01), 1000, false),
    ];
    let b = bars(&negative);
    assert!((b[0].share).abs() < 1e-9);
    assert!((b[1].share - 1.0).abs() < 1e-6);
    assert_eq!(b[0].value_text, "-\u{20B9}500.00");
}

#[test]
fn the_caption_names_the_open_month() {
    // provenance: derived from the mock's caption "From posted vouchers · November to date
    // (hatched)"; with no open month the part after the dot is not shown.
    let open = [
        month(date!(2026 - 10 - 01), 1, false),
        month(date!(2026 - 11 - 01), 1, true),
    ];
    assert_eq!(
        caption(&open),
        "From posted vouchers \u{b7} November to date (hatched)"
    );
    let closed = [month(date!(2026 - 10 - 01), 1, false)];
    assert_eq!(caption(&closed), "From posted vouchers");
}

#[test]
fn the_gstr1_nudge_reads_like_the_mock() {
    // provenance: the mock's first nudge: tone warn, title "GSTR-1 for November is due 11 Dec
    // 2026", body "1 item line needs correction before export.", source line naming the rule,
    // button "Open GSTR-1". The unit here is a voucher, which is what the engine counts.
    let found = HomeNudges {
        gstr1: Some(Gstr1Due {
            period: Period::new(date!(2026 - 11 - 01), date!(2026 - 11 - 30)).unwrap(),
            due: date!(2026 - 12 - 11),
            needs_correction: Outcome::Value(1),
        }),
        duplicates: Outcome::Value(Vec::new()),
    };
    let n = nudges(
        &figures(Outcome::Value(LateBills {
            count: 0,
            open: Paise::ZERO,
        })),
        &found,
    );
    assert_eq!(n.len(), 1);
    assert_eq!(n[0].tone, NudgeTone::Warn);
    assert_eq!(n[0].title, "GSTR-1 for November is due 11 Dec 2026");
    assert_eq!(n[0].body, "1 voucher needs correction before export.");
    assert_eq!(n[0].source, "GST calendar, due date 11th of the next month");
    assert_eq!(n[0].action_label, "Open GSTR-1");
    assert_eq!(
        n[0].action,
        NudgeAction::Open(HomeTarget::GstReturns {
            tab: "gstr1".to_owned()
        })
    );
}

#[test]
fn a_gstr1_count_the_engine_cannot_give_is_said_not_shown_as_zero() {
    // provenance: derived. Rule A1: the nudge stays (the due date is certain) and its body says
    // the count was not worked out, with the engine's reason.
    let found = HomeNudges {
        gstr1: Some(Gstr1Due {
            period: Period::new(date!(2026 - 11 - 01), date!(2026 - 11 - 30)).unwrap(),
            due: date!(2026 - 12 - 11),
            needs_correction: unsupported("the return could not be worked out"),
        }),
        duplicates: Outcome::Value(Vec::new()),
    };
    let n = nudges(
        &figures(Outcome::Value(LateBills {
            count: 0,
            open: Paise::ZERO,
        })),
        &found,
    );
    assert_eq!(
        n[0].body,
        "Corrections not counted: the return could not be worked out."
    );
}

#[test]
fn the_duplicate_nudge_names_both_vouchers() {
    // provenance: the mock's third nudge: tone bad, body "PY/0316 on 12 Nov repeats PY/0304 on
    // 28 Oct: same ledger, amount and narration.", source "Duplicate rule D-02", button
    // "Compare". The title names the ledger the engine found (Electricity), not a payee.
    let found = HomeNudges {
        gstr1: None,
        duplicates: Outcome::Value(vec![pair()]),
    };
    let n = nudges(
        &figures(Outcome::Value(LateBills {
            count: 0,
            open: Paise::ZERO,
        })),
        &found,
    );
    assert_eq!(n.len(), 1);
    assert_eq!(n[0].tone, NudgeTone::Bad);
    assert_eq!(n[0].title, "Possible double payment to Electricity");
    assert_eq!(
        n[0].body,
        "PY/0316 on 12 Nov repeats PY/0304 on 28 Oct: same ledger, amount and narration."
    );
    assert_eq!(n[0].source, "Duplicate rule D-02");
    assert_eq!(n[0].action_label, "Compare");
    assert_eq!(n[0].action, NudgeAction::Compare(0));
}

#[test]
fn the_late_bills_nudge_carries_the_open_amount_from_the_engine() {
    // provenance: the mock's fourth nudge: neutral, "3 customer bills overdue more than 30
    // days", "Open amount ₹..." , source "Ageing report", button "Show list". The amount is the
    // engine's: 1,23,456.00 here.
    let n = nudges(
        &figures(Outcome::Value(LateBills {
            count: 3,
            open: rs(123_456),
        })),
        &HomeNudges {
            gstr1: None,
            duplicates: Outcome::Value(Vec::new()),
        },
    );
    assert_eq!(n.len(), 1);
    assert_eq!(n[0].tone, NudgeTone::Neutral);
    assert_eq!(n[0].title, "3 customer bills overdue more than 30 days");
    assert_eq!(n[0].body, "Open amount \u{20B9}1,23,456.00.");
    assert_eq!(n[0].source, "Ageing report");
    assert_eq!(n[0].action_label, "Show list");
    assert_eq!(
        n[0].action,
        NudgeAction::Open(HomeTarget::OutstandingReceivables)
    );
}

#[test]
fn nothing_late_and_nothing_found_raises_no_nudge() {
    // provenance: derived. A rule that finds nothing says nothing.
    let n = nudges(
        &figures(Outcome::Value(LateBills {
            count: 0,
            open: Paise::ZERO,
        })),
        &HomeNudges {
            gstr1: None,
            duplicates: Outcome::Value(Vec::new()),
        },
    );
    assert_eq!(
        n.len(),
        0,
        "nothing late and nothing flagged raises no nudge"
    );
}

#[test]
fn a_rule_that_could_not_run_says_so() {
    // provenance: derived. Rule A1: a duplicate check or an ageing count that is unsupported
    // appears as a neutral nudge giving the reason, so silence never means "all clear".
    let n = nudges(
        &figures(unsupported("no bill detail")),
        &HomeNudges {
            gstr1: None,
            duplicates: unsupported("the books are not loaded"),
        },
    );
    assert_eq!(n.len(), 2);
    assert_eq!(n[0].title, "Duplicate payments not checked");
    assert_eq!(n[0].body, "the books are not loaded");
    assert_eq!(n[1].title, "Overdue customer bills not counted");
    assert_eq!(n[1].body, "no bill detail");
    assert!(n.iter().all(|x| x.tone == NudgeTone::Neutral));
}

#[test]
fn the_duplicate_nudge_names_the_payee_when_the_pair_has_one() {
    // provenance: derived from the card's own rule: the title names the payee the engine gives
    // (the party both vouchers name) and falls back to the ledger when there is none.
    let mut with_payee = pair();
    with_payee.payee = Some("City Power Distribution".to_owned());
    let found = |p: DuplicatePayment| {
        nudges(
            &figures(Outcome::Value(LateBills {
                count: 0,
                open: Paise::ZERO,
            })),
            &HomeNudges {
                gstr1: None,
                duplicates: Outcome::Value(vec![p]),
            },
        )
    };
    assert_eq!(
        found(with_payee)
            .iter()
            .find(|n| n.title.starts_with("Possible double payment"))
            .map(|n| n.title.as_str()),
        Some("Possible double payment to City Power Distribution")
    );
}
