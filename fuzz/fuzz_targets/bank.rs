//! Fuzz target `bank` (card H2, rule D4): the bank statement parsers must return `Ok` or `Err`
//! on arbitrary bytes, and never panic, hang or allocate without bound.
//!
//! Covered: the CSV parser (`parse_csv_with_layout`, real) under each amount layout and date
//! layout, and the MT940 parser (`parse_mt940`, card BF4 frozen interface). The XLSX parser needs
//! a `FormatSpec` (card BF1, not built yet); add it here once `FormatSpec` has a constructor.
#![no_main]

use ledgerly_bankfmt::csv::{AmountColumns, CsvLayout, DateFormat, parse_csv_with_layout};
use ledgerly_bankfmt::mt940::parse_mt940;

fn layout(
    amounts: AmountColumns,
    date_format: DateFormat,
    delimiter: u8,
    skip_rows: u32,
) -> CsvLayout {
    CsvLayout {
        format_id: "fuzz".to_owned(),
        delimiter,
        skip_rows,
        date_col: 0,
        value_date_col: Some(1),
        description_col: 2,
        reference_col: Some(3),
        amounts,
        balance_col: Some(6),
        date_format,
        date_separator: '/',
    }
}

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    // CSV takes raw bytes (BOM, invalid UTF-8 and ragged rows are all in scope).
    let layouts = [
        layout(
            AmountColumns::Split {
                debit: 4,
                credit: 5,
            },
            DateFormat::DdMmYyyy,
            b',',
            0,
        ),
        layout(
            AmountColumns::Signed { amount: 4 },
            DateFormat::YyyyMmDd,
            b';',
            1,
        ),
        layout(
            AmountColumns::WithIndicator {
                amount: 4,
                indicator: 5,
            },
            DateFormat::DdMonYyyy,
            b'\t',
            2,
        ),
        layout(
            AmountColumns::Split {
                debit: 4,
                credit: 5,
            },
            DateFormat::DdMmYy { century_base: 2000 },
            b',',
            0,
        ),
    ];
    for l in &layouts {
        // The outcome is irrelevant: the property under test is "returns, does not panic".
        let _ = parse_csv_with_layout(data, l);
    }
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = parse_mt940(text);
    }
});
