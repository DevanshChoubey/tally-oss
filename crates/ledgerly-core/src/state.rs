use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A GST state or union-territory code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct StateCode(u8);

/// The code is not in the table.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{0} is not a GST state code")]
pub struct StateCodeError(pub u8);

/// GST state codes and names.
///
/// Provenance: recalled from the GSTN state code list, **not yet checked against the published
/// table**. Verify each row and cite the source here before release (rule A4). Codes 25 and 28
/// are pre-merger codes kept so that old GSTINs and invoices parse.
const STATES: &[(u8, &str)] = &[
    (1, "Jammu and Kashmir"),
    (2, "Himachal Pradesh"),
    (3, "Punjab"),
    (4, "Chandigarh"),
    (5, "Uttarakhand"),
    (6, "Haryana"),
    (7, "Delhi"),
    (8, "Rajasthan"),
    (9, "Uttar Pradesh"),
    (10, "Bihar"),
    (11, "Sikkim"),
    (12, "Arunachal Pradesh"),
    (13, "Nagaland"),
    (14, "Manipur"),
    (15, "Mizoram"),
    (16, "Tripura"),
    (17, "Meghalaya"),
    (18, "Assam"),
    (19, "West Bengal"),
    (20, "Jharkhand"),
    (21, "Odisha"),
    (22, "Chhattisgarh"),
    (23, "Madhya Pradesh"),
    (24, "Gujarat"),
    (25, "Daman and Diu (before merger)"),
    (26, "Dadra and Nagar Haveli and Daman and Diu"),
    (27, "Maharashtra"),
    (28, "Andhra Pradesh (before reorganisation)"),
    (29, "Karnataka"),
    (30, "Goa"),
    (31, "Lakshadweep"),
    (32, "Kerala"),
    (33, "Tamil Nadu"),
    (34, "Puducherry"),
    (35, "Andaman and Nicobar Islands"),
    (36, "Telangana"),
    (37, "Andhra Pradesh"),
    (38, "Ladakh"),
    (97, "Other Territory"),
    (99, "Centre Jurisdiction"),
];

impl StateCode {
    /// Validates a code against the table.
    ///
    /// # Errors
    /// [`StateCodeError`] for a code not in the table.
    pub fn new(code: u8) -> Result<Self, StateCodeError> {
        if STATES.iter().any(|(c, _)| *c == code) {
            Ok(StateCode(code))
        } else {
            Err(StateCodeError(code))
        }
    }

    /// The numeric code.
    pub const fn get(self) -> u8 {
        self.0
    }

    /// The state or territory name.
    pub fn name(self) -> &'static str {
        STATES
            .iter()
            .find(|(c, _)| *c == self.0)
            .map_or("", |(_, n)| n)
    }

    /// `true` for a union territory without its own legislature, where UTGST replaces SGST.
    ///
    /// Provenance: recalled (UTGST Act s.2(8)); verify before release. Delhi, Puducherry and
    /// Jammu and Kashmir have legislatures and use SGST.
    pub const fn is_utgst_territory(self) -> bool {
        matches!(self.0, 4 | 25 | 26 | 31 | 35 | 38 | 97)
    }

    /// Every code in the table, ascending.
    pub fn all() -> impl Iterator<Item = StateCode> {
        STATES.iter().map(|(c, _)| StateCode(*c))
    }
}

impl TryFrom<u8> for StateCode {
    type Error = StateCodeError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        StateCode::new(value)
    }
}

impl From<StateCode> for u8 {
    fn from(value: StateCode) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table() {
        assert_eq!(StateCode::new(27).unwrap().name(), "Maharashtra");
        assert_eq!(StateCode::new(0), Err(StateCodeError(0)));
        assert_eq!(StateCode::new(39), Err(StateCodeError(39)));
        assert_eq!(StateCode::all().count(), 40);
    }
}
