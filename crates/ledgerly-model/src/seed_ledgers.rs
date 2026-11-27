//! The predefined ledgers a new company is created with (S22).
//!
//! TallyPrime help, "Ledgers in TallyPrime" (<https://help.tallysolutions.com/ledgers-in-tallyprime/>,
//! read 2026-11-27): "TallyPrime provides predefined ledgers such as Cash and Profit & Loss A/c",
//! and neither can be deleted. Cash sits under Cash-in-Hand (research R1, "Reserved ledgers").
//!
//! Profit & Loss A/c sits under Primary in that product. A ledger here always has a group, and the
//! books compute the profit and loss row at query time (`ledgerly-reports`, balance sheet), so it
//! is not created: a ledger of that name under some other group would be a wrong placement.

use ledgerly_core::{LedgerId, Paise, SignedAmount};

use crate::group::ReservedGroup;
use crate::ledger::{Ledger, ReservedLedger};
use crate::seed_groups::reserved_groups;

/// The id of the predefined Cash ledger in a new company (the first ledger).
pub const CASH: LedgerId = LedgerId(1);

/// The ledgers a new company is created with: Cash, under Cash-in-Hand, with no opening balance.
/// Empty only if the reserved groups lack Cash-in-Hand, which they never do.
#[must_use]
pub fn predefined_ledgers() -> Vec<Ledger> {
    let Some(cash_in_hand) = reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(ReservedGroup::CashInHand))
    else {
        return Vec::new();
    };
    vec![Ledger {
        ineligible_itc: None,
        id: CASH,
        name: "Cash".to_owned(),
        aliases: Vec::new(),
        group: Some(cash_in_hand.id),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank: None,
        tax: None,
        gst: None,
        reserved: Some(ReservedLedger::Cash),
        tally: None,
    }]
}
