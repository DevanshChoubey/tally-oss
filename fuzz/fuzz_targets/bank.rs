//! Fuzz target `bank`. **Card H2** points it at the real parser.
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = data;
});
