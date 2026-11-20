//! GSTR-1 builder with exception buckets. **Card G8** implements this module.
//!
//! Frozen interface (plan card G8):
//!
//! ```text
//! pub enum Bucket { Ready, NotRelevant, Mismatch, NeedsCorrection }
//! pub struct BucketedVoucher { pub voucher: VoucherId, pub bucket: Bucket, pub reason: Option<String> }
//! pub fn build_gstr1(books: &Books, gstin: &Gstin, period: Period, pack: &RulePack) -> Result<Outcome<(Gstr1, Vec<BucketedVoucher>)>, GstError>;
//! ```
