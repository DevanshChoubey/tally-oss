//! Upgrading the books of a company file made by an older build (round s23).
//!
//! Round s22 made Cash and Profit & Loss A/c predefined ledgers of every new company, as
//! TallyPrime has them: "TallyPrime provides predefined ledgers such as Cash and Profit & Loss
//! A/c" (TallyPrime help, "Ledgers in TallyPrime",
//! <https://help.tallysolutions.com/ledgers-in-tallyprime/>, read 2026-11-27). A company file made
//! before that has neither. [`predefined_ledgers_upgrade`] works out, from the books alone, the
//! events that give such a company its predefined ledgers. The caller commits them as one audited
//! change.
//!
//! For each predefined ledger ([`ledgerly_model::seed_ledgers::predefined_ledgers`]):
//! - a ledger that already has the role is left alone;
//! - otherwise, a ledger of exactly that name, in the group the predefined ledger belongs to and
//!   with no role, is the company's own Cash made by hand before the ledger was predefined: it
//!   takes the role (one `MasterAltered` event), keeping its id, opening balance and vouchers, so
//!   no figure changes;
//! - otherwise, a ledger of that name somewhere else is left as it is and the step is reported as
//!   not done, with why: taking the role would move it, and its balance, to another group;
//! - otherwise, the predefined ledger is created with the next free ledger id, through
//!   [`crate::masters_accounts::create_ledger`].
//!
//! Running it again on the upgraded books gives no events.

use ledgerly_core::{LedgerId, Outcome};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::seed_ledgers::predefined_ledgers;
use ledgerly_model::{Books, Ledger, MasterSnapshot, ReservedLedger};
use ledgerly_store::Event;

use crate::EngineError;
use crate::masters_accounts::create_ledger;

/// What happened to one predefined ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PredefinedStep {
    /// The company already has it.
    AlreadyThere {
        /// The role.
        role: ReservedLedger,
        /// Its ledger.
        id: LedgerId,
    },
    /// It was created.
    Created {
        /// The role.
        role: ReservedLedger,
        /// The new ledger's id.
        id: LedgerId,
        /// Its name.
        name: String,
    },
    /// The company's own ledger of that name, in the right group, took the role.
    Adopted {
        /// The role.
        role: ReservedLedger,
        /// The ledger that took it.
        id: LedgerId,
        /// Its name.
        name: String,
    },
    /// Not done; the company keeps going without it.
    NotDone {
        /// The role.
        role: ReservedLedger,
        /// Why, in words for the user.
        reason: String,
    },
}

/// The events of the upgrade and what each predefined ledger came to.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PredefinedUpgrade {
    /// The events to commit, in order. Empty when nothing changes.
    pub events: Vec<Event>,
    /// One step per predefined ledger, in the order of
    /// [`ledgerly_model::seed_ledgers::predefined_ledgers`].
    pub steps: Vec<PredefinedStep>,
}

/// Works out the events that give a company made before round s22 its predefined ledgers. See
/// the module documentation.
///
/// `Outcome::Unsupported` when the engine cannot compute a ledger's creation.
///
/// # Errors
/// [`EngineError`] when a predefined ledger cannot be created (for example the ledger ids are
/// exhausted).
pub fn predefined_ledgers_upgrade(
    books: &Books,
) -> Result<Outcome<PredefinedUpgrade>, EngineError> {
    let mut upgrade = PredefinedUpgrade::default();
    let mut next = next_ledger_id(books)?;
    for template in predefined_ledgers() {
        let Some(role) = template.reserved else {
            continue;
        };
        if let Some(have) = books
            .masters
            .ledgers
            .values()
            .find(|l| l.reserved == Some(role))
        {
            upgrade
                .steps
                .push(PredefinedStep::AlreadyThere { role, id: have.id });
            continue;
        }
        let template = match in_this_company(books, template) {
            Ok(template) => template,
            Err(reason) => {
                upgrade.steps.push(PredefinedStep::NotDone { role, reason });
                continue;
            }
        };
        let name = template.name.trim();
        if let Some(same_name) = books
            .masters
            .ledgers
            .values()
            .find(|l| l.name.trim() == name)
        {
            upgrade.steps.push(adopt_or_explain(
                books,
                same_name,
                &template,
                role,
                &mut upgrade.events,
            ));
            continue;
        }
        let id = next;
        next = LedgerId(id.0.checked_add(1).ok_or_else(no_free_id)?);
        let ledger = Ledger { id, ..template };
        let name = ledger.name.clone();
        match create_ledger(ledger, books)? {
            Outcome::Value(events) => upgrade.events.extend(events),
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        }
        upgrade
            .steps
            .push(PredefinedStep::Created { role, id, name });
    }
    Ok(Outcome::Value(upgrade))
}

/// A ledger named as the predefined one already exists without the role: it takes the role when
/// it sits in the predefined ledger's group, and is otherwise left alone with a reason.
fn adopt_or_explain(
    books: &Books,
    same_name: &Ledger,
    template: &Ledger,
    role: ReservedLedger,
    events: &mut Vec<Event>,
) -> PredefinedStep {
    if same_name.reserved.is_none() && same_name.group == template.group {
        let after = Ledger {
            reserved: Some(role),
            ..same_name.clone()
        };
        events.push(Event::MasterAltered {
            before: MasterSnapshot::Ledger(same_name.clone()),
            after: MasterSnapshot::Ledger(after),
        });
        return PredefinedStep::Adopted {
            role,
            id: same_name.id,
            name: same_name.name.clone(),
        };
    }
    let place = |group: Option<ledgerly_core::GroupId>| match group {
        None => "Primary".to_owned(),
        Some(id) => books
            .masters
            .groups
            .get(&id)
            .map_or_else(|| format!("group {}", id.0), |g| format!("'{}'", g.name)),
    };
    let reason = if same_name.reserved.is_some() {
        format!(
            "the ledger '{}' already has another predefined role, so the predefined {} was not added",
            same_name.name, template.name
        )
    } else {
        format!(
            "the company's own ledger '{}' sits under {}, not under {} where the predefined one belongs; it was left as it is, because giving it the role would move it and its balance. Rename it and open the company again to add the predefined {}",
            same_name.name,
            place(same_name.group),
            place(template.group),
            template.name
        )
    };
    PredefinedStep::NotDone { role, reason }
}

/// One more than the highest ledger id in the books, or 1 when there are none.
fn next_ledger_id(books: &Books) -> Result<LedgerId, EngineError> {
    let highest = books
        .masters
        .ledgers
        .keys()
        .next_back()
        .map_or(0, |id| id.0);
    Ok(LedgerId(highest.checked_add(1).ok_or_else(no_free_id)?))
}

fn no_free_id() -> EngineError {
    EngineError::Invalid {
        field: "id".to_owned(),
        reason: "the company file has no free ledger id left".to_owned(),
    }
}

/// The predefined ledger with its group taken from this company: the seed names the group by
/// the seed's id, and the company's group with the same reserved role is used. Primary (no
/// group) stays Primary.
fn in_this_company(books: &Books, template: Ledger) -> Result<Ledger, String> {
    let Some(seed_group) = template.group else {
        return Ok(template);
    };
    let role = reserved_groups()
        .into_iter()
        .find(|g| g.id == seed_group)
        .and_then(|g| g.reserved);
    let here = role.and_then(|role| {
        books
            .masters
            .groups
            .values()
            .find(|g| g.reserved == Some(role))
            .map(|g| g.id)
    });
    match here {
        Some(group) => Ok(Ledger {
            group: Some(group),
            ..template
        }),
        None => Err(format!(
            "the company has no group for the predefined {}, so it was not added",
            template.name
        )),
    }
}
