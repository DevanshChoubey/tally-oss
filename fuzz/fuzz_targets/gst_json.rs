//! Fuzz target `gst_json` (card H2, rule D4): every GST JSON reader must return `Ok` or `Err`
//! on arbitrary bytes, and never panic, hang or allocate without bound.
//!
//! Readers covered: GSTR-2B (`parse_gstr2b`) and IMS (`parse_ims`, both card F3),
//! against their frozen interfaces. The GSTR-1, 3B, e-invoice and e-way bill modules only write
//! JSON (no reader in their frozen interface), so there is nothing to feed them untrusted bytes.
#![no_main]

use ledgerly_gst_formats::{gstr2b, ims};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    // The readers take `&str`; a file that is not UTF-8 is rejected by the caller before parsing.
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    // The outcome is irrelevant: the property under test is "returns, does not panic".
    let _ = gstr2b::parse_gstr2b(text);
    let _ = ims::parse_ims(text);
});
