use serde::{Deserialize, Serialize};

/// Identity of an object imported from Tally, kept so re-imports and syncs match objects.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TallyRef {
    /// Tally's GUID for the object.
    pub guid: String,
    /// Tally's ALTERID (change counter), when exported.
    pub alter_id: Option<u64>,
}
