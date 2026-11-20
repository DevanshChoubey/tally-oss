//! Master parsing. **Card X2** implements this module.
//!
//! Frozen interface (plan card X2):
//!
//! ```text
//! pub enum TallyMaster { Group(TGroup), Ledger(TLedger), StockItem(..), Unit(..), Godown(..), VoucherType(..), CostCentre(..), CostCategory(..), StockGroup(..) }
//! pub fn parse_masters(env: &Envelope) -> Result<(Vec<TallyMaster>, Vec<XmlWarning>), TallyXmlError>;
//! ```
