//! Fuzz target `tally_xml` (card H1, rule D4): arbitrary bytes into every Tally XML parser that
//! exists on this branch. The property is "never panics, never hangs, never allocates without
//! bound"; the parse result (value or typed error) is ignored.
//!
//! Run: `cargo +nightly fuzz run tally_xml -- -max_total_time=600 -timeout=10 -rss_limit_mb=2048`
//! from `fuzz/`. A crash input goes into `fuzz/corpus/tally_xml/` as a regression seed and into a
//! unit test of the parser it broke.
//!
//! Parsers covered: masters (raw bytes: UTF-8, BOM, UTF-16LE/BE decoding included) and vouchers
//! (lossy-decoded text). Cards X1 (`envelope`), X4 (`writer`) and X5 (`reports`) are not built
//! when this target was written; add their entry points here when they land.
#![no_main]

use ledgerly_tally_xml::{masters, vouchers};

/// Inputs above this size are skipped: it bounds a single run and keeps the fuzzer on small,
/// structure-heavy documents, where the interesting bugs are.
const MAX_INPUT_BYTES: usize = 1 << 20;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }
    // Masters from raw bytes: exercises the decoder (BOMs, UTF-16, invalid sequences) too.
    // The Ok and Err values are dropped on purpose; only a panic or hang is a finding.
    drop(masters::parse_masters_bytes(data));
    // Vouchers from text: invalid UTF-8 becomes U+FFFD so the XML layer sees every input.
    let text = String::from_utf8_lossy(data);
    drop(vouchers::parse_vouchers_xml(&text));
});
