//! The "Sales by month, before GST" bars. The engine gives each month's total
//! ([`ledgerly_api::MonthSales`]); this file only turns them into bar heights and words. A
//! height is a share of the tallest bar, used for drawing; no figure is derived (rule C4).

use ledgerly_api::MonthSales;
use ledgerly_core::Paise;

use crate::ui::format;

/// Words before the open month in the caption.
const SOURCE: &str = "From posted vouchers";

/// One bar of the chart.
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    /// The month's three-letter name, drawn under the bar.
    pub label: String,
    /// The engine's total for the month.
    pub value: Paise,
    /// The total with Indian digit grouping and the rupee sign, for the tooltip.
    pub value_text: String,
    /// Height as a share of the tallest bar, from 0 to 1. Drawing only.
    pub share: f32,
    /// `true` for a month that is not over yet; drawn hatched.
    pub to_date: bool,
}

/// The total as the tooltip writes it: `₹13,40,000.00`, or `-₹500.00` when negative.
fn value_text(value: Paise) -> String {
    match value.checked_abs() {
        Ok(abs) if value.is_negative() => format!("-\u{20B9}{}", format::amount(abs)),
        Ok(abs) => format!("\u{20B9}{}", format::amount(abs)),
        Err(_) => "amount out of range".to_owned(),
    }
}

/// The bars for `months`, in the order given. A month with no sales, or with a net debit, has a
/// share of 0; when no month has sales every share is 0.
#[must_use]
#[allow(
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    reason = "a bar's height as a share of the tallest; drawing only, no displayed figure"
)]
pub fn bars(months: &[MonthSales]) -> Vec<Bar> {
    let tallest = months.iter().map(|m| m.sales.get()).max().unwrap_or(0);
    months
        .iter()
        .map(|m| {
            let share = if tallest > 0 && m.sales.get() > 0 {
                m.sales.get() as f32 / tallest as f32
            } else {
                0.0
            };
            Bar {
                label: format::month_abbrev(m.month_start.month()).to_owned(),
                value: m.sales,
                value_text: value_text(m.sales),
                share,
                to_date: m.to_date,
            }
        })
        .collect()
}

/// The words at the right of the chart title: where the figures come from, and which month is
/// still open ("From posted vouchers · November to date (hatched)").
#[must_use]
pub fn caption(months: &[MonthSales]) -> String {
    match months.iter().rev().find(|m| m.to_date) {
        Some(open) => format!(
            "{SOURCE} \u{b7} {} to date (hatched)",
            format::month_name(open.month_start.month())
        ),
        None => SOURCE.to_owned(),
    }
}

/// What the chart card shows.
#[derive(Debug, Clone, PartialEq)]
pub enum Chart {
    /// The bars and the words beside the title.
    Bars {
        /// The bars, oldest month first.
        bars: Vec<Bar>,
        /// The caption ([`caption`]).
        caption: String,
    },
    /// The engine could not give the series; this says why.
    Unsupported(String),
}

/// The chart for the engine's monthly series.
#[must_use]
pub fn chart_of(series: &ledgerly_core::Outcome<Vec<MonthSales>>) -> Chart {
    match series {
        ledgerly_core::Outcome::Value(months) => Chart::Bars {
            bars: bars(months),
            caption: caption(months),
        },
        ledgerly_core::Outcome::Unsupported(u) => {
            Chart::Unsupported(format!("{}: {}", u.what, u.reason))
        }
    }
}
