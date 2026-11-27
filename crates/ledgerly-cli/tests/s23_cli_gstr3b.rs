//! S23: `ledgerly export gstr3b` writes the working as tab-separated text.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::process::Command;

use ledgerly_core::{Clock, OffsetDateTime};

#[test]
fn the_sample_novembers_working_is_written_with_every_table() {
    // provenance: derived. Output tax of the sample's November sales (SI/0401, 0405, 0412, 0415,
    // 0417, see crates/ledgerly-sample/tests/s16_payment.rs): IGST 16,020.00 + 4,350.00 =
    // 20,370.00; CGST = SGST = 11,707.65 + 241.00 + 3,375.00 = 15,323.65. 3.1(d) and 4(A)(3) are
    // nil: November's purchases PB/0141 and PB/0144 are from regular suppliers whose invoices
    // charge tax (mode "taxed" in assets/godavari.toml), and none is marked reverse charge.
    // 3.1(c) and 3.1(e) are nil: every GST classification in assets/godavari.toml is taxable
    // (two ledgers `taxability = "Taxable"`, items with a `gst_rate` only).
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sample.ledgerly");
    drop(ledgerly_sample::create(&path, Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap());
    let out = Command::new(env!("CARGO_BIN_EXE_ledgerly"))
        .args([
            "export",
            "gstr3b",
            path.to_str().unwrap(),
            "--gstin",
            "27ZZZZZ0000Z1ZQ",
            "--from",
            "2026-11-01",
            "--to",
            "2026-11-30",
        ])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    for expected in [
        "3.1(a)\tintegrated tax\t20370.00",
        "3.1(a)\tcentral tax\t15323.65",
        "3.1(a)\tstate/UT tax\t15323.65",
        "3.1(d)\ttaxable value\t0.00",
        "3.1(d)\tcentral tax\t0.00",
        "4(A)(3)\tintegrated tax\t0.00",
        "3.1(c)\tnil rated and exempted value\t0.00",
        "3.1(e)\tnon-GST value\t0.00",
    ] {
        assert!(
            text.lines().any(|l| l == expected),
            "no line {expected:?} in\n{text}"
        );
    }
    assert!(!text.contains("not worked out"), "{text}");
}

#[test]
fn a_bad_gstin_exits_2_before_the_file_is_opened() {
    // provenance: derived — the CLI checks its arguments first (exit code 2, module docs).
    let out = Command::new(env!("CARGO_BIN_EXE_ledgerly"))
        .args([
            "export",
            "gstr3b",
            "missing.ledgerly",
            "--gstin",
            "BAD",
            "--from",
            "2026-11-01",
            "--to",
            "2026-11-30",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("GSTIN"));
}
