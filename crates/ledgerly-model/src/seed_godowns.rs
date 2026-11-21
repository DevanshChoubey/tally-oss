//! The godown every company starts with.
//!
//! The TallyPrime godowns FAQ
//! (<https://help.tallysolutions.com/tally-prime/godowns-locations/godowns-locations-faq-tally/>,
//! accessed 2026-11-21) says that, with "Maintain Multiple Godowns" off, all stock sits in an
//! internal godown named Main Location under the root Primary. With the feature on, the same
//! godown is listed for allocation. So Main Location exists in every company from creation.
//! Primary is the root of the tree, not a godown, so Main Location has no parent here.

use ledgerly_core::GodownId;

use crate::Godown;

/// The id of Main Location, the default godown of every company.
pub const MAIN_LOCATION: GodownId = GodownId(1);

/// The godowns a new company is created with: Main Location alone.
#[must_use]
pub fn default_godowns() -> Vec<Godown> {
    vec![Godown {
        id: MAIN_LOCATION,
        name: "Main Location".to_owned(),
        parent: None,
        tally: None,
    }]
}
