//! Amount text for the entry screen: Indian grouping and the Dr / Cr suffix.

use ledgerly_core::{DrCr, Paise, SignedAmount};

/// Digits grouped the Indian way (`1234567` -> `12,34,567`).
pub(super) fn group_indian(rupees: u64) -> String {
    let mut out: Vec<char> = Vec::new();
    for (i, digit) in rupees.to_string().chars().rev().enumerate() {
        if i == 3 || (i > 3 && i.saturating_sub(3) % 2 == 0) {
            out.push(',');
        }
        out.push(digit);
    }
    out.iter().rev().collect()
}

/// `12,34,567.00` (a leading `-` for negative amounts). Card C1 owns the shared formatter
/// (`ledgerly_core::format::format_inr`); this private copy exists only so this card does not
/// depend on a card that is not merged yet, and is to be replaced by it in Wave 2.
pub(super) fn format_amount(amount: Paise) -> String {
    let abs = amount.get().unsigned_abs();
    let sign = if amount.is_negative() { "-" } else { "" };
    format!("{sign}{}.{:02}", group_indian(abs / 100), abs % 100)
}

pub(super) fn side_label(side: DrCr) -> &'static str {
    match side {
        DrCr::Dr => "Dr",
        DrCr::Cr => "Cr",
    }
}

pub(super) fn format_signed(amount: SignedAmount) -> String {
    format!(
        "{} {}",
        format_amount(amount.amount),
        side_label(amount.side)
    )
}

/// A balance the way the mock's foot box writes it (`inr(now)`, index.html line 1409): the
/// amount alone for a debit balance, a leading minus for a credit one. The side is not named; the
/// account line above says Dr or Cr.
pub(super) fn format_balance(amount: SignedAmount) -> String {
    match amount.side {
        DrCr::Cr if !amount.amount.is_zero() => format!("-{}", format_amount(amount.amount)),
        DrCr::Cr | DrCr::Dr => format_amount(amount.amount),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // provenance: the mock's overdraft state (index.html payScreen, line 1409 and the capture
    // payment.overdraft): a balance of 36,000 Cr after the payment is written -36,000.00, and the
    // Saptagiri balance of 7,27,524.50 Dr is written 7,27,524.50. derived: lakh grouping is 3
    // digits then pairs.
    #[test]
    fn the_foot_box_writes_a_debit_plain_and_a_credit_with_a_minus() {
        assert_eq!(
            format_balance(SignedAmount::dr(Paise::new(72_752_450))),
            "7,27,524.50"
        );
        assert_eq!(
            format_balance(SignedAmount::cr(Paise::new(3_600_000))),
            "-36,000.00"
        );
    }
}
