//! PDF rendering. **Card PR2** implements this module.
//!
//! Frozen interface (plan card PR2):
//!
//! ```text
//! pub fn render_invoice(t: &Template, v: &Voucher, books: &Books, gst: &StoredGst) -> Result<Vec<u8>, PrintError>;
//! ```
