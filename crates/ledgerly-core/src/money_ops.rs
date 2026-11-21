//! Money arithmetic helpers. **Card C2** implements this module.
//!
//! Frozen interface (plan card C2):
//!
//! ```text
//! pub fn sum<I: IntoIterator<Item = Paise>>(amounts: I) -> Result<Paise, MoneyError>;
//! pub fn percent_of(base: Paise, percent: Rate, rule: RoundingRule) -> Result<Paise, MoneyError>;
//! pub fn split_evenly(total: Paise, parts: u32) -> Result<Vec<Paise>, MoneyError>; // remainder to the first parts
//! ```
//!
//! Every operation is checked (rule A3): overflow is [`MoneyError::Overflow`], never a wrap.

use rust_decimal::Decimal;

use crate::{MoneyError, Paise, Rate, RoundingRule, round_rupees_to_paise};

/// Most parts [`split_evenly`] will produce. A larger request is refused rather than risking an
/// allocation failure.
pub const MAX_SPLIT_PARTS: u32 = 1 << 20;

/// Adds amounts left to right with checked addition.
///
/// An empty iterator sums to [`Paise::ZERO`]. The error is raised if any running total overflows,
/// even when later negative amounts would have brought the total back in range.
///
/// # Errors
/// [`MoneyError::Overflow`] when a running total does not fit in `i64` paise.
pub fn sum<I: IntoIterator<Item = Paise>>(amounts: I) -> Result<Paise, MoneyError> {
    amounts
        .into_iter()
        .try_fold(Paise::ZERO, super::money::Paise::checked_add)
}

/// `percent` percent of `base`, rounded to paise under `rule`.
///
/// `percent` is a percentage, not a fraction: `Rate(18)` means 18%. The product is formed first
/// and divided once, so no precision is lost to an intermediate fraction.
///
/// # Errors
/// [`MoneyError::Overflow`] when the intermediate product or the rounded result does not fit.
pub fn percent_of(base: Paise, percent: Rate, rule: RoundingRule) -> Result<Paise, MoneyError> {
    // base paise * percent / 100 = result paise; / 100 again turns paise into rupees.
    let scaled = Decimal::from(base.get())
        .checked_mul(percent.0)
        .ok_or(MoneyError::Overflow)?;
    let rupees = scaled
        .checked_div(Decimal::from(10_000_u32))
        .ok_or(MoneyError::Overflow)?;
    round_rupees_to_paise(rupees, rule)
}

/// Splits `total` into `parts` amounts that differ by at most one paisa and add up to `total`.
///
/// The remainder is handed out one paisa at a time to the first parts, in the direction of the
/// total's sign (a negative total gives the first parts the extra negative paisa). With
/// `parts == 0` there is nothing to split into and the call is refused.
///
/// # Errors
/// [`MoneyError::NotRepresentable`] when `parts` is zero or above [`MAX_SPLIT_PARTS`];
/// [`MoneyError::Overflow`] if an internal step overflows (not reachable for valid input).
pub fn split_evenly(total: Paise, parts: u32) -> Result<Vec<Paise>, MoneyError> {
    if parts == 0 {
        return Err(MoneyError::NotRepresentable {
            value: "a split into 0 parts".to_owned(),
        });
    }
    if parts > MAX_SPLIT_PARTS {
        return Err(MoneyError::NotRepresentable {
            value: format!("a split into {parts} parts (limit {MAX_SPLIT_PARTS})"),
        });
    }
    let count = i64::from(parts);
    let base = total.get().checked_div(count).ok_or(MoneyError::Overflow)?;
    // Truncating division: the remainder has the sign of the total and |remainder| < parts.
    let remainder = total.get().checked_rem(count).ok_or(MoneyError::Overflow)?;
    let extra_parts = remainder.unsigned_abs();
    // Only needed when there is a remainder, which keeps `i64::MAX` over one part from
    // overflowing on an unused `base + 1`.
    let with_extra = if remainder == 0 {
        base
    } else {
        let step: i64 = if remainder < 0 { -1 } else { 1 };
        base.checked_add(step).ok_or(MoneyError::Overflow)?
    };
    let mut out = Vec::new();
    for index in 0..u64::from(parts) {
        out.push(Paise::new(if index < extra_parts {
            with_extra
        } else {
            base
        }));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::format_inr;

    #[test]
    fn sum_empty_and_basic() {
        // provenance: derived — 100 + 250 + (-50) = 300 paise; empty sum is the additive identity.
        assert_eq!(sum(Vec::new()), Ok(Paise::ZERO));
        assert_eq!(
            sum([Paise::new(100), Paise::new(250), Paise::new(-50)]),
            Ok(Paise::new(300))
        );
    }

    #[test]
    fn checked_add_overflow_errors() {
        // provenance: derived — i64::MAX + 1 is outside i64 (rule A3: must error, not wrap).
        assert_eq!(
            sum([Paise::new(i64::MAX), Paise::new(1)]),
            Err(MoneyError::Overflow)
        );
        assert_eq!(
            sum([Paise::new(i64::MIN), Paise::new(-1)]),
            Err(MoneyError::Overflow)
        );
    }

    #[test]
    fn neg_zero_displays_as_zero() {
        // provenance: derived — integers have no negative zero; negating 0 or subtracting an
        // amount from itself gives 0 paise, which must print "0.00" with no sign.
        let negated = Paise::ZERO.checked_neg().unwrap();
        assert_eq!(format_inr(negated), "0.00");
        let diff = Paise::new(500).checked_sub(Paise::new(500)).unwrap();
        assert_eq!(format_inr(diff), "0.00");
        // -0.4 paise truncates toward zero to 0 paise, not "-0.00".
        let tiny = percent_of(
            Paise::new(-1),
            Rate(Decimal::new(40, 0)),
            RoundingRule::Truncate,
        )
        .unwrap();
        assert_eq!(format_inr(tiny), "0.00");
    }

    #[test]
    fn percent_of_18_percent() {
        // provenance: derived — 18% of Rs 10,000.00 = Rs 1,800.00 = 180_000 paise.
        assert_eq!(
            percent_of(
                Paise::new(1_000_000),
                Rate(Decimal::new(18, 0)),
                RoundingRule::HalfUpToPaise
            ),
            Ok(Paise::new(180_000))
        );
    }

    #[test]
    fn percent_of_rounding_rules() {
        // provenance: derived — 2.5% of 99 paise = 2.475 paise = Rs 0.02475.
        // HalfUpToPaise: nearest paisa 2; Truncate: 2; Up: 3.
        // HalfUpToRupee: Rs 0.02475 rounds to Rs 0 = 0 paise.
        let base = Paise::new(99);
        let pct = Rate(Decimal::new(25, 1));
        assert_eq!(
            percent_of(base, pct, RoundingRule::HalfUpToPaise),
            Ok(Paise::new(2))
        );
        assert_eq!(
            percent_of(base, pct, RoundingRule::Truncate),
            Ok(Paise::new(2))
        );
        assert_eq!(percent_of(base, pct, RoundingRule::Up), Ok(Paise::new(3)));
        assert_eq!(
            percent_of(base, pct, RoundingRule::HalfUpToRupee),
            Ok(Paise::ZERO)
        );
    }

    #[test]
    fn percent_of_exact_half_rounds_away_from_zero() {
        // provenance: derived — 50% of 5 paise = 2.5 paise; the rule rounds exact halves away
        // from zero (see RoundingRule::HalfUpToPaise docs): 3, and -3 for the negative base.
        // That is 50% of Rs 0.05 = Rs 0.025, half-way between 0.02 and 0.03 rupees.
        let half = Rate(Decimal::new(50, 0));
        assert_eq!(
            percent_of(Paise::new(5), half, RoundingRule::HalfUpToPaise),
            Ok(Paise::new(3))
        );
        assert_eq!(
            percent_of(Paise::new(-5), half, RoundingRule::HalfUpToPaise),
            Ok(Paise::new(-3))
        );
    }

    #[test]
    fn percent_of_zero_percent_and_zero_base() {
        // provenance: derived — 0 * x = x * 0 = 0.
        assert_eq!(
            percent_of(Paise::new(12_345), Rate(Decimal::ZERO), RoundingRule::Up),
            Ok(Paise::ZERO)
        );
        assert_eq!(
            percent_of(
                Paise::ZERO,
                Rate(Decimal::new(18, 0)),
                RoundingRule::HalfUpToPaise
            ),
            Ok(Paise::ZERO)
        );
    }

    #[test]
    fn percent_of_overflow_errors() {
        // provenance: derived — 200% of i64::MAX paise is 2 * i64::MAX, outside i64.
        assert_eq!(
            percent_of(
                Paise::new(i64::MAX),
                Rate(Decimal::new(200, 0)),
                RoundingRule::HalfUpToPaise
            ),
            Err(MoneyError::Overflow)
        );
    }

    #[test]
    fn split_evenly_remainder_to_first_parts() {
        // provenance: derived — Rs 1.00 = 100 paise; 100 = 3 * 33 + 1, so the single extra
        // paisa goes to the first part: [34, 33, 33].
        assert_eq!(
            split_evenly(Paise::new(100), 3),
            Ok(vec![Paise::new(34), Paise::new(33), Paise::new(33)])
        );
    }

    #[test]
    fn split_evenly_two_extra_paise() {
        // provenance: derived — 101 = 3 * 33 + 2: first two parts get 34.
        assert_eq!(
            split_evenly(Paise::new(101), 3),
            Ok(vec![Paise::new(34), Paise::new(34), Paise::new(33)])
        );
    }

    #[test]
    fn split_evenly_exact_and_single() {
        // provenance: derived — 90 / 3 = 30 with no remainder; one part is the whole total.
        assert_eq!(split_evenly(Paise::new(90), 3), Ok(vec![Paise::new(30); 3]));
        assert_eq!(split_evenly(Paise::new(7), 1), Ok(vec![Paise::new(7)]));
        assert_eq!(
            split_evenly(Paise::ZERO, 2),
            Ok(vec![Paise::ZERO, Paise::ZERO])
        );
    }

    #[test]
    fn split_evenly_negative_total() {
        // provenance: derived — -100 = 3 * (-33) + (-1): the extra negative paisa goes to the
        // first part, giving [-34, -33, -33], which adds to -100.
        assert_eq!(
            split_evenly(Paise::new(-100), 3),
            Ok(vec![Paise::new(-34), Paise::new(-33), Paise::new(-33)])
        );
    }

    #[test]
    fn split_evenly_more_parts_than_paise() {
        // provenance: derived — 2 paise over 5 parts: base 0, remainder 2: [1, 1, 0, 0, 0].
        assert_eq!(
            split_evenly(Paise::new(2), 5),
            Ok(vec![
                Paise::new(1),
                Paise::new(1),
                Paise::ZERO,
                Paise::ZERO,
                Paise::ZERO
            ])
        );
    }

    #[test]
    fn split_evenly_extremes_do_not_overflow() {
        // provenance: derived — i64::MIN = 2 * (-2^62), exact; i64::MAX = 2 * (2^62 - 1) + 1.
        let half = i64::MIN / 2;
        assert_eq!(
            split_evenly(Paise::new(i64::MIN), 2),
            Ok(vec![Paise::new(half), Paise::new(half)])
        );
        let max_half = i64::MAX / 2;
        assert_eq!(
            split_evenly(Paise::new(i64::MAX), 2),
            Ok(vec![Paise::new(max_half + 1), Paise::new(max_half)])
        );
        assert_eq!(
            split_evenly(Paise::new(i64::MAX), 1),
            Ok(vec![Paise::new(i64::MAX)])
        );
        assert_eq!(
            split_evenly(Paise::new(i64::MIN), 1),
            Ok(vec![Paise::new(i64::MIN)])
        );
    }

    #[test]
    fn split_evenly_refuses_zero_and_huge_part_counts() {
        // provenance: derived — dividing by zero parts has no answer; the cap is this module's
        // documented MAX_SPLIT_PARTS.
        assert!(matches!(
            split_evenly(Paise::new(100), 0),
            Err(MoneyError::NotRepresentable { .. })
        ));
        assert!(matches!(
            split_evenly(Paise::new(100), MAX_SPLIT_PARTS + 1),
            Err(MoneyError::NotRepresentable { .. })
        ));
    }
}
