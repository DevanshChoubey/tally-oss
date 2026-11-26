//! The Nudges card: what the GST calendar, the duplicate rule and the ageing report say needs a
//! look. Each nudge names its rule. The words are made here from the engine's answers; no
//! figure is worked out here (rule C4). The mock's cards are `NUDGES()`,
//! `design/mock-v2/index.html` lines 1032 to 1038.

use ledgerly_api::{
    CUSTOMER_LATE_DAYS, DuplicatePayment, Gstr1Due, HomeFigures, HomeNudges, PaymentRef,
};
use ledgerly_core::{Outcome, Paise};

use super::HomeTarget;
use crate::i18n::Tr;
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
    pub action_label: String,
    /// What the button does.
    pub action: NudgeAction,
}

/// The money with its rupee sign and Indian grouping: `₹1,23,456.00`.
fn rupees(amount: Paise) -> String {
    format!("\u{20B9}{}", format::amount(amount))
}

/// The GSTR-1 card: its due date and how many vouchers need correcting before export.
fn gstr1_nudge(due: &Gstr1Due, tr: &Tr) -> Nudge {
    let body = match &due.needs_correction {
        Outcome::Value(0) => tr.t("home.nudge.gstr1.none"),
        Outcome::Value(n) => {
            let key = if *n == 1 {
                "home.nudge.gstr1.one"
            } else {
                "home.nudge.gstr1.many"
            };
            tr.tf(key, &[("n", &n.to_string())])
        }
        Outcome::Unsupported(u) => tr.tf("home.nudge.gstr1.unknown", &[("reason", &u.reason)]),
    };
    Nudge {
        tone: NudgeTone::Warn,
        icon: Icon::Cal,
        title: tr.tf(
            "home.nudge.gstr1.title",
            &[
                (
                    "month",
                    &format::month_name_in(due.period.from().month(), tr),
                ),
                ("date", &format::date_in(due.due, tr)),
            ],
        ),
        body,
        source: tr.t("home.nudge.gstr1.source"),
        action_label: tr.t("home.nudge.gstr1.action"),
        action: NudgeAction::Open(HomeTarget::GstReturns {
            tab: "gstr1".to_owned(),
        }),
    }
}

/// The IMS card in English; see [`ims_nudge_in`].
#[must_use]
pub fn ims_nudge(count: &Outcome<Option<usize>>) -> Option<Nudge> {
    ims_nudge_in(count, &Tr::default())
}

/// The IMS card: how many 2B records have no action. Nothing to review, or no 2B file, raises no
/// card; a count the engine cannot give is a neutral card with its reason, so silence never reads
/// as all clear.
#[must_use]
pub fn ims_nudge_in(count: &Outcome<Option<usize>>, tr: &Tr) -> Option<Nudge> {
    let tab = || {
        NudgeAction::Open(HomeTarget::GstReturns {
            tab: "gstr2b-ims".to_owned(),
        })
    };
    match count {
        Outcome::Value(Some(n)) if *n > 0 => Some(Nudge {
            tone: NudgeTone::Warn,
            icon: Icon::Gst,
            title: tr.tf(
                if *n == 1 {
                    "home.nudge.ims.one"
                } else {
                    "home.nudge.ims.many"
                },
                &[("n", &n.to_string())],
            ),
            body: tr.t("home.nudge.ims.body"),
            source: tr.t("home.nudge.ims.source"),
            action_label: tr.t("home.nudge.ims.action"),
            action: tab(),
        }),
        Outcome::Value(Some(_) | None) => None,
        Outcome::Unsupported(u) => Some(Nudge {
            action: tab(),
            action_label: tr.t("home.nudge.ims.action"),
            ..not_run(
                &tr.t("home.nudge.ims.unknown"),
                &u.reason,
                &tr.t("home.nudge.ims.source"),
                tr,
            )
        }),
    }
}

/// How a payment is named in a sentence: its number, or "A payment" without one.
fn payment_name(p: &PaymentRef, tr: &Tr) -> String {
    p.number
        .clone()
        .unwrap_or_else(|| tr.t("home.nudge.a_payment"))
}

/// "12 Nov", the date without its year, as the mock's duplicate nudge writes it.
fn short_date(p: &PaymentRef, tr: &Tr) -> String {
    format!(
        "{} {}",
        p.date.day(),
        format::month_abbrev_in(p.date.month(), tr)
    )
}

/// The duplicate card for the pair at `index` of the list.
fn duplicate_nudge(index: usize, pair: &DuplicatePayment, tr: &Tr) -> Nudge {
    Nudge {
        tone: NudgeTone::Bad,
        icon: Icon::Alert,
        title: tr.tf(
            "home.nudge.dup.title",
            &[("payee", pair.payee.as_deref().unwrap_or(&pair.ledger))],
        ),
        body: tr.tf(
            "home.nudge.dup.body",
            &[
                ("second", &payment_name(&pair.second, tr)),
                ("second_date", &short_date(&pair.second, tr)),
                ("first", &payment_name(&pair.first, tr)),
                ("first_date", &short_date(&pair.first, tr)),
            ],
        ),
        source: tr.t("home.nudge.dup.source"),
        action_label: tr.t("home.nudge.dup.action"),
        action: NudgeAction::Compare(index),
    }
}

/// A card for a rule that could not run: neutral, giving the reason, so silence never means
/// "all clear".
fn not_run(title: &str, reason: &str, source: &str, tr: &Tr) -> Nudge {
    Nudge {
        tone: NudgeTone::Neutral,
        icon: Icon::Info,
        title: title.to_owned(),
        body: reason.to_owned(),
        source: source.to_owned(),
        action_label: tr.t("sidebar.day_book"),
        action: NudgeAction::Open(HomeTarget::DayBook),
    }
}

/// The nudges in English; see [`nudges_in`].
#[must_use]
pub fn nudges(figures: &HomeFigures, found: &HomeNudges) -> Vec<Nudge> {
    nudges_in(figures, found, &Tr::default())
}

/// The nudges for the engine's answers, in the mock's order: GSTR-1, duplicates, late bills.
/// The IMS card is added after the GSTR-1 card by [`ims_nudge_in`].
#[must_use]
pub fn nudges_in(figures: &HomeFigures, found: &HomeNudges, tr: &Tr) -> Vec<Nudge> {
    let mut out = Vec::new();
    if let Some(due) = &found.gstr1 {
        out.push(gstr1_nudge(due, tr));
    }
    match &found.duplicates {
        Outcome::Value(pairs) => {
            out.extend(
                pairs
                    .iter()
                    .enumerate()
                    .map(|(i, p)| duplicate_nudge(i, p, tr)),
            );
        }
        Outcome::Unsupported(u) => out.push(not_run(
            &tr.t("home.nudge.dup.unknown"),
            &u.reason,
            &tr.t("home.nudge.dup.source"),
            tr,
        )),
    }
    match &figures.receivables_overdue {
        Outcome::Value(late) if late.count > 0 => out.push(Nudge {
            tone: NudgeTone::Neutral,
            icon: Icon::Clock,
            title: tr.tf(
                if late.count == 1 {
                    "home.nudge.late.one"
                } else {
                    "home.nudge.late.many"
                },
                &[
                    ("n", &late.count.to_string()),
                    ("days", &CUSTOMER_LATE_DAYS.to_string()),
                ],
            ),
            body: tr.tf("home.nudge.late.body", &[("amount", &rupees(late.open))]),
            source: tr.t("home.nudge.late.source"),
            action_label: tr.t("home.nudge.late.action"),
            action: NudgeAction::Open(HomeTarget::OutstandingReceivables),
        }),
        Outcome::Value(_) => {}
        Outcome::Unsupported(u) => out.push(Nudge {
            action: NudgeAction::Open(HomeTarget::OutstandingReceivables),
            action_label: tr.t("home.nudge.late.action"),
            ..not_run(
                &tr.t("home.nudge.late.unknown"),
                &u.reason,
                &tr.t("home.nudge.late.source"),
                tr,
            )
        }),
    }
    out
}
