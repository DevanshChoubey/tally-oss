//! The predefined ledgers a new company is created with (S22).
//!
//! TallyPrime help, "Ledgers in TallyPrime" (<https://help.tallysolutions.com/ledgers-in-tallyprime/>,
//! read 2026-11-27): "TallyPrime provides predefined ledgers such as Cash and Profit & Loss A/c",
//! and neither can be deleted. Cash sits under Cash-in-Hand (research R1, "Reserved ledgers").
//!
//! Profit & Loss A/c is "grouped under the Primary group", carries the previous year's profit or
//! loss as its opening balance, "cannot be deleted but it can be altered", and takes adjustment
//! journals (TallyPrime help, "Ledgers and Groups in TallyPrime",
//! <https://help.tallysolutions.com/tally-prime/accounting/ledgers-and-groups-in-tallyprime/>, read
//! 2026-11-27). So it is created with no group (`Ledger::group` is `None`, Primary), the only
//! ledger that may sit there. The balance sheet adds its balance to the profit row it computes at
//! query time, and the trial balance lists it after the primary groups (`ledgerly-reports`).

use ledgerly_core::{LedgerId, Paise, SignedAmount};

use crate::group::ReservedGroup;
use crate::ledger::{Ledger, ReservedLedger};
use crate::seed_groups::reserved_groups;

/// The id of the predefined Cash ledger in a new company (the first ledger).
pub const CASH: LedgerId = LedgerId(1);

/// The id of the predefined Profit & Loss A/c in a new company (the second ledger).
pub const PROFIT_AND_LOSS: LedgerId = LedgerId(2);

/// The ledgers a new company is created with, both with no opening balance: Cash under
/// Cash-in-Hand, and Profit & Loss A/c under Primary (no group).
/// Empty only if the reserved groups lack Cash-in-Hand, which they never do.
#[must_use]
pub fn predefined_ledgers() -> Vec<Ledger> {
    let Some(cash_in_hand) = reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(ReservedGroup::CashInHand))
    else {
        return Vec::new();
    };
    vec![
        Ledger {
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
        },
        Ledger {
            ineligible_itc: None,
            id: PROFIT_AND_LOSS,
            name: "Profit & Loss A/c".to_owned(),
            aliases: Vec::new(),
            group: None,
            opening: SignedAmount::dr(Paise::ZERO),
            bill_wise: None,
            cost_centres_applicable: false,
            party: None,
            bank: None,
            tax: None,
            gst: None,
            reserved: Some(ReservedLedger::ProfitAndLoss),
            tally: None,
        },
    ]
}
