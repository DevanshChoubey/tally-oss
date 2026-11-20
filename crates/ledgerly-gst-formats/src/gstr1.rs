//! GSTR-1 and GSTR-1A data shapes and JSON writer. **Card F1** implements this module.
//!
//! Frozen interface (plan card F1):
//!
//! ```text
//! pub struct Gstr1 { .. }  pub struct Gstr1a { .. }   // sections: b2b, b2cl, b2cs, cdnr, cdnur, exp, nil, hsn, doc_issue
//! pub fn gstr1_to_json(r: &Gstr1) -> Result<String, FormatError>;
//! pub fn gstr1a_to_json(r: &Gstr1a) -> Result<String, FormatError>;
//! ```
