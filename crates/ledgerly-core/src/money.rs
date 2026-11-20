use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// An amount of Indian rupees held as a whole number of paise (1 rupee = 100 paise).
///
/// There are deliberately no `+`/`-` operators: every operation is checked and returns
/// [`MoneyError::Overflow`] instead of wrapping (rule A3).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Paise(i64);

/// Errors from money arithmetic.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MoneyError {
    /// The result does not fit in an `i64` number of paise.
    #[error("amount overflow: the result is outside the representable range of paise")]
    Overflow,
    /// A decimal value could not be represented as paise under the requested rounding rule.
    #[error("{value} cannot be represented as paise")]
    NotRepresentable {
        /// The value that could not be converted, as text.
        value: String,
    },
}

impl Paise {
    /// Zero rupees.
    pub const ZERO: Paise = Paise(0);

    /// Wraps a number of paise.
    pub const fn new(paise: i64) -> Self {
        Paise(paise)
    }

    /// The number of paise.
    pub const fn get(self) -> i64 {
        self.0
    }

    /// `true` when the amount is zero.
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// `true` when the amount is below zero.
    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    /// Checked addition.
    ///
    /// # Errors
    /// [`MoneyError::Overflow`] when the sum does not fit.
    pub fn checked_add(self, other: Paise) -> Result<Paise, MoneyError> {
        self.0
            .checked_add(other.0)
            .map(Paise)
            .ok_or(MoneyError::Overflow)
    }

    /// Checked subtraction.
    ///
    /// # Errors
    /// [`MoneyError::Overflow`] when the difference does not fit.
    pub fn checked_sub(self, other: Paise) -> Result<Paise, MoneyError> {
        self.0
            .checked_sub(other.0)
            .map(Paise)
            .ok_or(MoneyError::Overflow)
    }

    /// Checked negation.
    ///
    /// # Errors
    /// [`MoneyError::Overflow`] for `i64::MIN` paise.
    pub fn checked_neg(self) -> Result<Paise, MoneyError> {
        self.0.checked_neg().map(Paise).ok_or(MoneyError::Overflow)
    }

    /// Absolute value.
    ///
    /// # Errors
    /// [`MoneyError::Overflow`] for `i64::MIN` paise.
    pub fn checked_abs(self) -> Result<Paise, MoneyError> {
        self.0.checked_abs().map(Paise).ok_or(MoneyError::Overflow)
    }

    /// Multiplies by a decimal factor (a quantity, a rate as a fraction) and rounds to paise.
    ///
    /// # Errors
    /// [`MoneyError::Overflow`] when the product does not fit.
    pub fn checked_mul_decimal(
        self,
        factor: Decimal,
        rule: RoundingRule,
    ) -> Result<Paise, MoneyError> {
        let paise = Decimal::from(self.0)
            .checked_mul(factor)
            .ok_or(MoneyError::Overflow)?;
        let rupees = paise
            .checked_div(Decimal::ONE_HUNDRED)
            .ok_or(MoneyError::Overflow)?;
        round_rupees_to_paise(rupees, rule)
    }

    /// The amount in rupees as an exact decimal (for display and export, never for floats).
    pub fn to_rupees(self) -> Decimal {
        Decimal::new(self.0, 2)
    }
}

/// How a decimal amount of rupees becomes paise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RoundingRule {
    /// To the nearest paisa; exact halves round away from zero.
    HalfUpToPaise,
    /// To the nearest whole rupee; exact halves round away from zero (CGST Act s.170 style).
    HalfUpToRupee,
    /// Toward zero, to the paisa.
    Truncate,
    /// Away from zero, to the paisa.
    Up,
}

/// Converts a decimal number of **rupees** to paise under `rule`.
///
/// # Errors
/// [`MoneyError::Overflow`] when the result does not fit in `i64` paise.
pub fn round_rupees_to_paise(rupees: Decimal, rule: RoundingRule) -> Result<Paise, MoneyError> {
    let rounded_rupees = match rule {
        RoundingRule::HalfUpToPaise => {
            rupees.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
        }
        RoundingRule::HalfUpToRupee => {
            rupees.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        }
        RoundingRule::Truncate => rupees.round_dp_with_strategy(2, RoundingStrategy::ToZero),
        RoundingRule::Up => rupees.round_dp_with_strategy(2, RoundingStrategy::AwayFromZero),
    };
    let paise = rounded_rupees
        .checked_mul(Decimal::ONE_HUNDRED)
        .ok_or(MoneyError::Overflow)?;
    i64::try_from(paise)
        .map(Paise)
        .map_err(|_| MoneyError::Overflow)
}

/// Debit or credit side of an amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DrCr {
    /// Debit.
    Dr,
    /// Credit.
    Cr,
}

impl DrCr {
    /// The other side.
    #[must_use]
    pub const fn opposite(self) -> DrCr {
        match self {
            DrCr::Dr => DrCr::Cr,
            DrCr::Cr => DrCr::Dr,
        }
    }
}

/// A non-negative amount with its side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SignedAmount {
    /// The amount; never negative for a well-formed value.
    pub amount: Paise,
    /// Debit or credit.
    pub side: DrCr,
}

impl SignedAmount {
    /// Debit amount.
    pub const fn dr(amount: Paise) -> Self {
        SignedAmount {
            amount,
            side: DrCr::Dr,
        }
    }

    /// Credit amount.
    pub const fn cr(amount: Paise) -> Self {
        SignedAmount {
            amount,
            side: DrCr::Cr,
        }
    }

    /// The amount with debit positive and credit negative.
    ///
    /// # Errors
    /// [`MoneyError::Overflow`] when negating overflows.
    pub fn to_signed(self) -> Result<Paise, MoneyError> {
        match self.side {
            DrCr::Dr => Ok(self.amount),
            DrCr::Cr => self.amount.checked_neg(),
        }
    }

    /// Builds a side-tagged amount from a debit-positive signed value. Zero is a debit.
    ///
    /// # Errors
    /// [`MoneyError::Overflow`] for `i64::MIN` paise.
    pub fn from_signed(value: Paise) -> Result<Self, MoneyError> {
        if value.is_negative() {
            Ok(SignedAmount::cr(value.checked_neg()?))
        } else {
            Ok(SignedAmount::dr(value))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn checked_add_overflow_errors() {
        assert_eq!(
            Paise::new(i64::MAX).checked_add(Paise::new(1)),
            Err(MoneyError::Overflow)
        );
    }

    #[test]
    fn neg_of_min_errors() {
        assert_eq!(
            Paise::new(i64::MIN).checked_neg(),
            Err(MoneyError::Overflow)
        );
    }

    #[test]
    fn rounding_rules() {
        // provenance: derived — definitions of the rules in this file; 12.345 rupees.
        let v = Decimal::new(12_345, 3);
        assert_eq!(
            round_rupees_to_paise(v, RoundingRule::HalfUpToPaise),
            Ok(Paise::new(1235))
        );
        assert_eq!(
            round_rupees_to_paise(v, RoundingRule::Truncate),
            Ok(Paise::new(1234))
        );
        assert_eq!(
            round_rupees_to_paise(v, RoundingRule::Up),
            Ok(Paise::new(1235))
        );
        assert_eq!(
            round_rupees_to_paise(v, RoundingRule::HalfUpToRupee),
            Ok(Paise::new(1200))
        );
        // 12.50 rupees rounds away from zero to 13.
        let half = Decimal::new(1250, 2);
        assert_eq!(
            round_rupees_to_paise(half, RoundingRule::HalfUpToRupee),
            Ok(Paise::new(1300))
        );
        let neg = Decimal::new(-1250, 2);
        assert_eq!(
            round_rupees_to_paise(neg, RoundingRule::HalfUpToRupee),
            Ok(Paise::new(-1300))
        );
    }

    #[test]
    fn mul_decimal_18_percent() {
        // provenance: derived — 18% of ₹10,000.00 = ₹1,800.00.
        let base = Paise::new(1_000_000);
        let rate = Decimal::new(18, 2);
        assert_eq!(
            base.checked_mul_decimal(rate, RoundingRule::HalfUpToPaise),
            Ok(Paise::new(180_000))
        );
    }

    #[test]
    fn signed_round_trip() {
        let cr = SignedAmount::cr(Paise::new(500));
        assert_eq!(cr.to_signed(), Ok(Paise::new(-500)));
        assert_eq!(SignedAmount::from_signed(Paise::new(-500)), Ok(cr));
        assert_eq!(
            SignedAmount::from_signed(Paise::ZERO),
            Ok(SignedAmount::dr(Paise::ZERO))
        );
    }

    #[test]
    fn to_rupees_is_exact() {
        assert_eq!(Paise::new(123_456).to_rupees().to_string(), "1234.56");
    }
}
