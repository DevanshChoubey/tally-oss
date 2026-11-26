//! The Nudges card: what the GST calendar, the duplicate rule and the ageing report say needs a
//! look. Each nudge names its rule. The words are made here from the engine's answers; no
//! figure is worked out here (rule C4). The mock's cards are `NUDGES()`,
//! `design/mock-v2/index.html` lines 1032 to 1038.

use ledgerly_api::{
    CUSTOMER_LATE_DAYS, DuplicatePayment, Gstr1Due, HomeFigures, HomeNudges, PaymentRef,
};
use ledgerly_core::{Outcome, Paise};

use super::HomeTarget;
use crate::ui::format;
use crate::ui::icon::Icon;

/// How loudly a nudge is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NudgeTone {
    /// Amber: something is coming up.
    Warn,
    /// Red: something looks wrong.
    Bad,
    /// Plain: for information.
    Neutral,
}

/// What the nudge's button does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NudgeAction {
    /// Open a screen.
    Open(HomeTarget),
    /// Open the comparison of the possible duplicate at this index of the screen's list.
    Compare(usize),
}

/// One card in the Nudges list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nudge {
    /// How it is drawn.
    pub tone: NudgeTone,
    /// The icon before the title.
    pub icon: Icon,
    /// Bold first line.
    pub title: String,
    /// What the rule found.
    pub body: String,
    /// The rule that raised it, as the card's source line says.
    pub source: String,
    /// The button's label.
    pub action_label: &'static str,
    /// What the button does.
    pub action: NudgeAction,
}

/// The money with its rupee sign and Indian grouping: `₹1,23,456.00`.
fn rupees(amount: Paise) -> String {
    format!("\u{20B9}{}", format::amount(amount))
}

/// "s" for any count but one.
fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

/// The GSTR-1 card: its due date and how many vouchers need correcting before export.
fn gstr1_nudge(due: &Gstr1Due) -> Nudge {
    let body = match &due.needs_correction {
        Outcome::Value(0) => "No voucher needs correction before export.".to_owned(),
        Outcome::Value(n) => {
            let verb = if *n == 1 { "needs" } else { "need" };
            format!("{n} voucher{} {verb} correction before export.", plural(*n))
        }
        Outcome::Unsupported(u) => format!("Corrections not counted: {}.", u.reason),
    };
    Nudge {
        tone: NudgeTone::Warn,
        icon: Icon::Cal,
        title: format!(
            "GSTR-1 for {} is due {}",
            format::month_name(due.period.from().month()),
            format::date(due.due)
        ),
        body,
        source: "GST calendar, due date 11th of the next month".to_owned(),
        action_label: "Open GSTR-1",
        action: NudgeAction::Open(HomeTarget::GstReturns {
            tab: "gstr1".to_owned(),
        }),
    }
}

/// The IMS card: how many 2B records have no action. Nothing to review, or no 2B file, raises no
/// card; a count the engine cannot give is a neutral card with its reason, so silence never reads
/// as all clear.
#[must_use]
pub fn ims_nudge(count: &Outcome<Option<usize>>) -> Option<Nudge> {
    let tab = || {
        NudgeAction::Open(HomeTarget::GstReturns {
            tab: "gstr2b-ims".to_owned(),
        })
    };
    match count {
        Outcome::Value(Some(n)) if *n > 0 => Some(Nudge {
            tone: NudgeTone::Warn,
            icon: Icon::Gst,
            title: format!(
                "{n} 2B record{} {} no IMS action",
                plural(*n),
                if *n == 1 { "has" } else { "have" }
            ),
            body: "With no action they are deemed accepted when 2B is generated. Review them before GSTR-3B."
                .to_owned(),
            source: "IMS rule".to_owned(),
            action_label: "Review 2B",
            action: tab(),
        }),
        Outcome::Value(Some(_) | None) => None,
        Outcome::Unsupported(u) => Some(Nudge {
            action: tab(),
            action_label: "Review 2B",
            ..not_run("2B records not checked for IMS action", &u.reason, "IMS rule")
        }),
    }
}

/// How a payment is named in a sentence: its number, or "A payment" without one.
fn payment_name(p: &PaymentRef) -> String {
    p.number.clone().unwrap_or_else(|| "A payment".to_owned())
}

/// "12 Nov", the date without its year, as the mock's duplicate nudge writes it.
fn short_date(p: &PaymentRef) -> String {
    format!("{} {}", p.date.day(), format::month_abbrev(p.date.month()))
}

/// The duplicate card for the pair at `index` of the list.
fn duplicate_nudge(index: usize, pair: &DuplicatePayment) -> Nudge {
    Nudge {
        tone: NudgeTone::Bad,
        icon: Icon::Alert,
        title: format!(
            "Possible double payment to {}",
            pair.payee.as_deref().unwrap_or(&pair.ledger)
        ),
        body: format!(
            "{} on {} repeats {} on {}: same ledger, amount and narration.",
            payment_name(&pair.second),
            short_date(&pair.second),
            payment_name(&pair.first),
            short_date(&pair.first)
        ),
        source: "Duplicate rule D-02".to_owned(),
        action_label: "Compare",
        action: NudgeAction::Compare(index),
    }
}

/// A card for a rule that could not run: neutral, giving the reason, so silence never means
/// "all clear".
fn not_run(title: &str, reason: &str, source: &str) -> Nudge {
    Nudge {
        tone: NudgeTone::Neutral,
        icon: Icon::Info,
        title: title.to_owned(),
        body: reason.to_owned(),
        source: source.to_owned(),
        action_label: "Day book",
        action: NudgeAction::Open(HomeTarget::DayBook),
    }
}

/// The nudges for the engine's answers, in the mock's order: GSTR-1, duplicates, late bills.
/// The IMS card is added after the GSTR-1 card by [`ims_nudge`].
#[must_use]
pub fn nudges(figures: &HomeFigures, found: &HomeNudges) -> Vec<Nudge> {
    let mut out = Vec::new();
    if let Some(due) = &found.gstr1 {
        out.push(gstr1_nudge(due));
    }
    match &found.duplicates {
        Outcome::Value(pairs) => {
            out.extend(pairs.iter().enumerate().map(|(i, p)| duplicate_nudge(i, p)));
        }
        Outcome::Unsupported(u) => out.push(not_run(
            "Duplicate payments not checked",
            &u.reason,
            "Duplicate rule D-02",
        )),
    }
    match &figures.receivables_overdue {
        Outcome::Value(late) if late.count > 0 => out.push(Nudge {
            tone: NudgeTone::Neutral,
            icon: Icon::Clock,
            title: format!(
                "{} customer bill{} overdue more than {CUSTOMER_LATE_DAYS} days",
                late.count,
                plural(late.count)
            ),
            body: format!("Open amount {}.", rupees(late.open)),
            source: "Ageing report".to_owned(),
            action_label: "Show list",
            action: NudgeAction::Open(HomeTarget::OutstandingReceivables),
        }),
        Outcome::Value(_) => {}
        Outcome::Unsupported(u) => out.push(Nudge {
            action: NudgeAction::Open(HomeTarget::OutstandingReceivables),
            action_label: "Show list",
            ..not_run(
                "Overdue customer bills not counted",
                &u.reason,
                "Ageing report",
            )
        }),
    }
    out
}
