//! Set-off of input tax credit against the output tax of a return period (round s16, engine
//! item E-1): how much of each tax head is paid from the electronic credit ledger and how much is
//! left to pay in cash.
//!
//! # The law
//!
//! Read 2026-11-26.
//!
//! * CGST Act s.49(5) (India Code, `A2017-12.pdf`, Act as amended to 2025): integrated tax credit
//!   pays integrated tax first, and what remains may pay central tax and State/UT tax; central tax
//!   credit pays central tax first and what remains may pay integrated tax; State/UT tax credit
//!   pays State/UT tax first and what remains may pay integrated tax, "only where the balance of
//!   the input tax credit on account of central tax is not available" (provisos to s.49(5)(c)
//!   and (d)); central tax credit never pays State/UT tax and State/UT tax credit never pays
//!   central tax (s.49(5)(e) and (f)).
//! * CGST Act s.49A: central, State and UT tax credit is used only after the integrated tax credit
//!   has been used up fully.
//! * CGST Act s.49B and Rule 88A of the CGST Rules (notification 16/2019-Central Tax, 29 March
//!   2019), explained by CBIC Circular 98/17/2019-GST of 23 April 2019 (para 4 and 5): after the
//!   integrated tax liability, the integrated tax credit may pay central tax and State/UT tax "in
//!   any order and in any proportion". The circular's illustration is the test vector in
//!   `tests/s16_setoff.rs`.
//! * Compensation cess: the credit of cess can be used only to pay cess (Goods and Services Tax
//!   (Compensation to States) Act, 2017 s.11(2) proviso; GST Council flyer "Compensation Cess",
//!   gstcouncil.gov.in, read 2026-11-26).
//! * CGST Act s.170: an amount payable is rounded to the nearest rupee, fifty paise or more going
//!   up. Each tax head is a separate amount (central and State tax are due under separate Acts),
//!   so each head is rounded on its own.
//!
//! # The choice the law leaves open
//!
//! "In any order and in any proportion" leaves the split of spare integrated tax credit between
//! central and State/UT tax to the taxpayer. This engine uses the split that needs the least cash:
//! spare credit first covers what each of the two heads still lacks after its own credit
//! (central first, then State/UT), and any credit that is still spare, which must be used up
//! under s.49A, goes to central tax and then to State/UT tax. The result is always one of the
//! outcomes the circular allows. It is a stated policy, not a rule of the Act.
//!
//! # What is not modelled
//!
//! The balance already in the electronic credit ledger from earlier periods, the electronic cash
//! ledger, and the Rule 86B cap on credit use (see the API, which refuses a period it applies
//! to).

use ledgerly_core::{Date, Month, MoneyError, Paise};

use crate::GstError;
use crate::gstr3b::Gstr3bTaxAmounts;

/// `true` when s.49A is in force on `day`: the CGST (Amendment) Act, 2018 brought it in from
/// 1 February 2019 (Circular 98/17/2019-GST para 1). Set-off before it followed the older s.49(5)
/// order, which this module does not work out.
#[must_use]
pub fn section_49a_in_force(day: Date) -> bool {
    day.year() > 2019 || (day.year() == 2019 && day.month() != Month::January)
}

/// The result of setting credit off against liability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetOff {
    /// Tax left to pay after the credit was used, to the paisa. This is the liability not
    /// covered by credit.
    pub left_to_pay: Gstr3bTaxAmounts,
    /// Tax payable in cash, each head rounded to the rupee (CGST Act s.170).
    pub cash_payable: Gstr3bTaxAmounts,
    /// Credit that was not needed, by head.
    pub credit_unused: Gstr3bTaxAmounts,
}

impl SetOff {
    /// The four heads of [`SetOff::cash_payable`] added.
    ///
    /// # Errors
    /// [`GstError::Money`] on overflow.
    pub fn cash_total(&self) -> Result<Paise, GstError> {
        let c = self.cash_payable;
        Ok(c.integrated
            .checked_add(c.central)?
            .checked_add(c.state_ut)?
            .checked_add(c.cess)?)
    }
}

/// Rounds an amount of paise to the nearest rupee, fifty paise or more going up (s.170).
///
/// # Errors
/// [`GstError::Money`] on overflow.
pub fn round_to_rupee(amount: Paise) -> Result<Paise, GstError> {
    let paise = amount.get();
    let rem = paise.rem_euclid(100);
    let base = paise.checked_sub(rem).ok_or(MoneyError::Overflow)?;
    let rounded = if rem >= 50 {
        base.checked_add(100).ok_or(MoneyError::Overflow)?
    } else {
        base
    };
    Ok(Paise::new(rounded))
}

fn min(a: Paise, b: Paise) -> Paise {
    if a <= b { a } else { b }
}

/// Moves `amount` from `credit` to `liability`, never more than either holds.
fn pay(credit: &mut Paise, liability: &mut Paise, amount: Paise) -> Result<(), GstError> {
    let used = min(min(amount, *credit), *liability);
    *credit = credit.checked_sub(used)?;
    *liability = liability.checked_sub(used)?;
    Ok(())
}

/// Moves as much of `credit` to `liability` as `liability` holds.
fn pay_all(credit: &mut Paise, liability: &mut Paise) -> Result<(), GstError> {
    let all = *credit;
    pay(credit, liability, all)
}

/// What a head still lacks after its own credit pays it.
fn shortfall(liability: Paise, own_credit: Paise) -> Result<Paise, GstError> {
    if liability > own_credit {
        Ok(liability.checked_sub(own_credit)?)
    } else {
        Ok(Paise::ZERO)
    }
}

/// Sets `credit` off against `liability` in the order of CGST Act s.49(5), s.49A and Rule 88A
/// (see the module docs), and rounds what is left to pay under s.170.
///
/// Both arguments must hold no negative amount.
///
/// # Errors
/// [`GstError::Invalid`] when an amount is negative, [`GstError::Money`] on overflow.
pub fn set_off(liability: Gstr3bTaxAmounts, credit: Gstr3bTaxAmounts) -> Result<SetOff, GstError> {
    for (name, amount) in [
        ("liability.integrated", liability.integrated),
        ("liability.central", liability.central),
        ("liability.state_ut", liability.state_ut),
        ("liability.cess", liability.cess),
        ("credit.integrated", credit.integrated),
        ("credit.central", credit.central),
        ("credit.state_ut", credit.state_ut),
        ("credit.cess", credit.cess),
    ] {
        if amount.is_negative() {
            return Err(GstError::Invalid {
                field: name.to_owned(),
                reason: "set-off takes amounts that are not negative".to_owned(),
            });
        }
    }
    let (mut li, mut lc, mut ls, mut lx) = (
        liability.integrated,
        liability.central,
        liability.state_ut,
        liability.cess,
    );
    let (mut ci, mut cc, mut cs, mut cx) = (
        credit.integrated,
        credit.central,
        credit.state_ut,
        credit.cess,
    );
    // s.49(5)(a): integrated tax credit pays integrated tax first.
    pay_all(&mut ci, &mut li)?;
    // s.49A and Rule 88A: what is left of it must be used up before any other credit, on central
    // and State/UT tax in any order and proportion. Least-cash split (module docs).
    let for_central = min(ci, shortfall(lc, cc)?);
    pay(&mut ci, &mut lc, for_central)?;
    let for_state = min(ci, shortfall(ls, cs)?);
    pay(&mut ci, &mut ls, for_state)?;
    pay_all(&mut ci, &mut lc)?;
    pay_all(&mut ci, &mut ls)?;
    // s.49(5)(b) to (d): central and State/UT credit pay their own tax first, then integrated
    // tax; State/UT credit pays integrated tax only when no central credit is left.
    pay_all(&mut cc, &mut lc)?;
    pay_all(&mut cs, &mut ls)?;
    pay_all(&mut cc, &mut li)?;
    pay_all(&mut cs, &mut li)?;
    // Compensation cess credit pays cess only.
    pay_all(&mut cx, &mut lx)?;
    let left_to_pay = Gstr3bTaxAmounts {
        integrated: li,
        central: lc,
        state_ut: ls,
        cess: lx,
    };
    Ok(SetOff {
        left_to_pay,
        cash_payable: Gstr3bTaxAmounts {
            integrated: round_to_rupee(li)?,
            central: round_to_rupee(lc)?,
            state_ut: round_to_rupee(ls)?,
            cess: round_to_rupee(lx)?,
        },
        credit_unused: Gstr3bTaxAmounts {
            integrated: ci,
            central: cc,
            state_ut: cs,
            cess: cx,
        },
    })
}
