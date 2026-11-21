//! Card BF1 tests. Every expected value is `derived`: read off the TOML text written in the test
//! (a column index is the number written next to its key; ids and names are the strings written),
//! or worked out by hand from the CSV text. None comes from running the parser or registry.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_bankfmt::BankFmtError;
use ledgerly_bankfmt::csv::{AmountColumns, CsvLayout, DateFormat, parse_csv_with_layout};
use ledgerly_bankfmt::registry::{
    Columns, FileKind, FormatSpec, HeaderRows, Registry, builtin_registry, load_registry,
};
use ledgerly_core::{Date, Month, Paise};

fn files(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(n, t)| ((*n).to_owned(), (*t).to_owned()))
        .collect()
}

fn load(name: &str, text: &str) -> Result<Registry, BankFmtError> {
    load_registry(&files(&[(name, text)]))
}

/// The error's reason text, asserting it is a `Registry` error for `id`.
fn registry_reason(err: BankFmtError, expect_id: &str) -> String {
    match err {
        BankFmtError::Registry { id, reason } => {
            assert_eq!(id, expect_id);
            reason
        }
        other @ (BankFmtError::Row { .. } | BankFmtError::Format { .. }) => {
            panic!("expected a Registry error, got {other:?}")
        }
    }
}

const SPLIT: &str = r#"
id = "test-split"
name = "Test bank, CSV"
kind = "csv"
delimiter = ";"
skip_rows = 2
date_format = "dd-mm-yy"
century_base = 2000
amounts = "split"

[columns]
date = 0
value_date = 1
description = 2
reference = 3
debit = 4
credit = 5
balance = 6
"#;

#[test]
fn loads_columns_date_format_and_split_convention() {
    let reg = load("test-split.toml", SPLIT).unwrap();
    assert_eq!(reg.len(), 1);
    let spec = reg.get("test-split").unwrap();
    // derived: each expectation below is the value written in SPLIT above.
    assert_eq!(spec.id, "test-split");
    assert_eq!(spec.name, "Test bank, CSV");
    assert_eq!(spec.kind, FileKind::Csv);
    let table = spec.table.as_ref().unwrap();
    assert_eq!(table.delimiter, Some(b';'));
    assert_eq!(table.header, HeaderRows::Skip(2));
    assert_eq!(table.sheet, None);
    assert_eq!(table.date_format, DateFormat::DdMmYy { century_base: 2000 });
    assert_eq!(table.date_separator, '-');
    assert_eq!(
        table.columns,
        Columns {
            date: 0,
            value_date: Some(1),
            description: 2,
            reference: Some(3),
            balance: Some(6),
        }
    );
    assert_eq!(
        table.amounts,
        AmountColumns::Split {
            debit: 4,
            credit: 5
        }
    );
}

#[test]
fn signed_and_indicator_conventions_load() {
    let signed = r#"
id = "s"
name = "S"
kind = "csv"
delimiter = ","
skip_rows = 0
date_format = "yyyy-mm-dd"
amounts = "signed"
[columns]
date = 1
description = 0
amount = 2
"#;
    let spec = load("s.toml", signed).unwrap().get("s").unwrap().clone();
    let table = spec.table.unwrap();
    // derived: values written in the TOML above; optional columns not written are None.
    assert_eq!(table.amounts, AmountColumns::Signed { amount: 2 });
    assert_eq!(table.date_format, DateFormat::YyyyMmDd);
    assert_eq!(table.header, HeaderRows::Skip(0));
    assert_eq!(table.columns.date, 1);
    assert_eq!(table.columns.description, 0);
    assert_eq!(table.columns.value_date, None);
    assert_eq!(table.columns.reference, None);
    assert_eq!(table.columns.balance, None);

    let indicator = r#"
id = "i"
name = "I"
kind = "csv"
delimiter = "|"
skip_rows = 3
date_format = "dd/Mon/yyyy"
amounts = "indicator"
[columns]
date = 0
description = 1
amount = 2
indicator = 3
"#;
    let spec = load("i.toml", indicator).unwrap().get("i").unwrap().clone();
    let table = spec.table.unwrap();
    assert_eq!(table.delimiter, Some(b'|'));
    assert_eq!(table.date_format, DateFormat::DdMonYyyy);
    assert_eq!(table.date_separator, '/');
    assert_eq!(
        table.amounts,
        AmountColumns::WithIndicator {
            amount: 2,
            indicator: 3
        }
    );
}

#[test]
fn xlsx_header_cells_and_sheet() {
    let text = r#"
id = "x"
name = "X"
kind = "xlsx"
header_cells = ["Date", "Narration"]
sheet = "Statement"
date_format = "dd.mm.yyyy"
amounts = "signed"
[columns]
date = 0
description = 1
amount = 2
"#;
    let reg = load("x.toml", text).unwrap();
    let spec = reg.get("x").unwrap();
    assert_eq!(spec.kind, FileKind::Xlsx);
    let table = spec.table.as_ref().unwrap();
    // derived: values written in the TOML above.
    assert_eq!(
        table.header,
        HeaderRows::FindHeader(vec!["Date".to_owned(), "Narration".to_owned()])
    );
    assert_eq!(table.sheet.as_deref(), Some("Statement"));
    assert_eq!(table.delimiter, None);
    assert_eq!(table.date_separator, '.');
    // an xlsx format has no CSV layout
    assert!(matches!(
        spec.csv_layout(),
        Err(BankFmtError::Format { .. })
    ));
}

#[test]
fn mt940_takes_no_table_keys() {
    let ok = "id = \"m\"\nname = \"M\"\nkind = \"mt940\"\n";
    let reg = load("m.toml", ok).unwrap();
    let spec = reg.get("m").unwrap();
    assert_eq!(spec.kind, FileKind::Mt940);
    assert_eq!(spec.table, None);

    let with_columns = format!("{ok}skip_rows = 1\ndate_format = \"dd/mm/yyyy\"\n");
    let reason = registry_reason(load("m.toml", &with_columns).unwrap_err(), "m");
    assert!(reason.contains("skip_rows"), "{reason}");
    assert!(reason.contains("date_format"), "{reason}");
}

#[test]
fn unknown_top_level_key_errors() {
    let text = format!("{SPLIT}\nbank_colour = \"red\"\n");
    // The stray key lands in [columns] (last table), which is also strict.
    let reason = registry_reason(load("t.toml", &text).unwrap_err(), "t.toml");
    assert!(reason.contains("unknown field"), "{reason}");
    assert!(reason.contains("bank_colour"), "{reason}");

    let top = SPLIT.replacen("kind = \"csv\"", "kind = \"csv\"\nbank_colour = \"red\"", 1);
    let reason = registry_reason(load("t.toml", &top).unwrap_err(), "t.toml");
    assert!(reason.contains("unknown field"), "{reason}");
    assert!(reason.contains("bank_colour"), "{reason}");
}

#[test]
fn unknown_column_key_errors() {
    let text = SPLIT.replacen("balance = 6", "balance = 6\nnarration2 = 7", 1);
    let reason = registry_reason(load("t.toml", &text).unwrap_err(), "t.toml");
    assert!(reason.contains("unknown field"), "{reason}");
    assert!(reason.contains("narration2"), "{reason}");
}

#[test]
fn unknown_kind_or_convention_errors() {
    let kind = SPLIT.replacen("kind = \"csv\"", "kind = \"pdf\"", 1);
    let reason = registry_reason(load("t.toml", &kind).unwrap_err(), "t.toml");
    assert!(reason.contains("unknown variant"), "{reason}");

    let conv = SPLIT.replacen("amounts = \"split\"", "amounts = \"plusminus\"", 1);
    let reason = registry_reason(load("t.toml", &conv).unwrap_err(), "t.toml");
    assert!(reason.contains("unknown variant"), "{reason}");
}

#[test]
fn missing_required_keys_error() {
    // Each removal makes one required thing absent; the error names the format id.
    let cases = [
        ("delimiter = \";\"\n", "delimiter"),
        ("skip_rows = 2\n", "skip_rows"),
        ("date_format = \"dd-mm-yy\"\n", "date_format"),
        ("amounts = \"split\"\n", "amounts"),
        ("century_base = 2000\n", "century_base"),
        ("debit = 4\n", "debit"),
        ("credit = 5\n", "credit"),
    ];
    for (remove, needle) in cases {
        let text = SPLIT.replacen(remove, "", 1);
        assert_ne!(text, SPLIT, "case {needle} removed nothing");
        let reason = registry_reason(load("t.toml", &text).unwrap_err(), "test-split");
        assert!(reason.contains(needle), "{needle}: {reason}");
    }
    // required [columns] keys are enforced by the parser (error carries the file name as id)
    let no_date = SPLIT.replacen("date = 0\n", "", 1);
    let reason = registry_reason(load("t.toml", &no_date).unwrap_err(), "t.toml");
    assert!(reason.contains("date"), "{reason}");
    let no_columns = SPLIT.split("[columns]").next().unwrap().to_owned();
    let reason = registry_reason(load("t.toml", &no_columns).unwrap_err(), "test-split");
    assert!(reason.contains("columns"), "{reason}");
}

#[test]
fn keys_that_do_not_fit_the_convention_error() {
    // split with an amount column
    let text = SPLIT.replacen("balance = 6", "balance = 6\namount = 9", 1);
    let reason = registry_reason(load("t.toml", &text).unwrap_err(), "test-split");
    assert!(reason.contains("columns.amount"), "{reason}");
    // signed with a debit column
    let signed =
        SPLIT
            .replacen("\"split\"", "\"signed\"", 1)
            .replacen("credit = 5", "amount = 5", 1);
    let reason = registry_reason(load("t.toml", &signed).unwrap_err(), "test-split");
    assert!(reason.contains("columns.debit"), "{reason}");
    // indicator without the marker column
    let ind = SPLIT.replacen("\"split\"", "\"indicator\"", 1).replacen(
        "debit = 4\ncredit = 5",
        "amount = 4",
        1,
    );
    let reason = registry_reason(load("t.toml", &ind).unwrap_err(), "test-split");
    assert!(reason.contains("columns.indicator"), "{reason}");
}

#[test]
fn kind_specific_keys_are_checked() {
    // csv with xlsx-only keys
    let sheet = SPLIT.replacen("skip_rows = 2", "skip_rows = 2\nsheet = \"S\"", 1);
    let reason = registry_reason(load("t.toml", &sheet).unwrap_err(), "test-split");
    assert!(reason.contains("sheet"), "{reason}");
    let cells = SPLIT.replacen("skip_rows = 2", "header_cells = [\"Date\"]", 1);
    let reason = registry_reason(load("t.toml", &cells).unwrap_err(), "test-split");
    assert!(reason.contains("header_cells"), "{reason}");
    // xlsx with a delimiter, with both header keys, with an empty header list
    let xlsx = SPLIT.replacen("\"csv\"", "\"xlsx\"", 1);
    let reason = registry_reason(load("t.toml", &xlsx).unwrap_err(), "test-split");
    assert!(reason.contains("delimiter"), "{reason}");
    let both = xlsx.replacen("delimiter = \";\"\n", "", 1).replacen(
        "skip_rows = 2",
        "skip_rows = 2\nheader_cells = [\"Date\"]",
        1,
    );
    let reason = registry_reason(load("t.toml", &both).unwrap_err(), "test-split");
    assert!(reason.contains("not both"), "{reason}");
    let empty = xlsx.replacen("delimiter = \";\"\n", "", 1).replacen(
        "skip_rows = 2",
        "header_cells = []",
        1,
    );
    let reason = registry_reason(load("t.toml", &empty).unwrap_err(), "test-split");
    assert!(reason.contains("header_cells"), "{reason}");
}

#[test]
fn bad_delimiters_error() {
    for bad in ["", ",,", "\\\"", "\\n"] {
        let text = SPLIT.replacen("delimiter = \";\"", &format!("delimiter = \"{bad}\""), 1);
        let reason = registry_reason(load("t.toml", &text).unwrap_err(), "test-split");
        assert!(reason.contains("delimiter"), "{bad:?}: {reason}");
    }
    // a tab is a valid delimiter
    let tab = SPLIT.replacen("delimiter = \";\"", "delimiter = \"\\t\"", 1);
    let reg = load("t.toml", &tab).unwrap();
    assert_eq!(
        reg.get("test-split")
            .unwrap()
            .table
            .as_ref()
            .unwrap()
            .delimiter,
        Some(b'\t')
    );
}

#[test]
fn bad_date_formats_and_century_base_error() {
    for bad in [
        "dd/mm",
        "mm/dd/yyyy",
        "dd/mm-yy",
        "ddmmyyyy",
        "dd,mm,yy",
        "DD-MM-YY",
    ] {
        let text = SPLIT.replacen("\"dd-mm-yy\"", &format!("\"{bad}\""), 1);
        let reason = registry_reason(load("t.toml", &text).unwrap_err(), "test-split");
        assert!(reason.contains("date_format"), "{bad}: {reason}");
    }
    // century_base on a four-digit-year format
    let four = SPLIT.replacen("\"dd-mm-yy\"", "\"dd-mm-yyyy\"", 1);
    let reason = registry_reason(load("t.toml", &four).unwrap_err(), "test-split");
    assert!(reason.contains("century_base"), "{reason}");
    // century_base not a century
    for bad in ["2024", "1800", "2200"] {
        let text = SPLIT.replacen("century_base = 2000", &format!("century_base = {bad}"), 1);
        let reason = registry_reason(load("t.toml", &text).unwrap_err(), "test-split");
        assert!(reason.contains("century_base"), "{bad}: {reason}");
    }
}

#[test]
fn two_roles_on_one_column_error() {
    let text = SPLIT.replacen("credit = 5", "credit = 4", 1);
    let reason = registry_reason(load("t.toml", &text).unwrap_err(), "test-split");
    // derived: debit and credit are both written as column 4.
    assert!(reason.contains("column 4"), "{reason}");
    assert!(
        reason.contains("debit") && reason.contains("credit"),
        "{reason}"
    );
}

#[test]
fn bad_ids_and_names_error() {
    for bad in ["", "Has Space", "UPPER", "a/b"] {
        let text = SPLIT.replacen("test-split", bad, 1);
        let err = load("t.toml", &text).unwrap_err();
        let reason = registry_reason(err, bad);
        assert!(reason.contains("id must"), "{bad:?}: {reason}");
    }
    let text = SPLIT.replacen("\"Test bank, CSV\"", "\"  \"", 1);
    let reason = registry_reason(load("t.toml", &text).unwrap_err(), "test-split");
    assert!(reason.contains("name"), "{reason}");
}

#[test]
fn duplicate_ids_across_files_error() {
    let err = load_registry(&files(&[("a.toml", SPLIT), ("b.toml", SPLIT)])).unwrap_err();
    let reason = registry_reason(err, "test-split");
    assert!(
        reason.contains("'a.toml'") && reason.contains("'b.toml'"),
        "{reason}"
    );
}

#[test]
fn syntax_errors_and_non_toml_names_error() {
    let reason = registry_reason(load("t.toml", "id = ").unwrap_err(), "t.toml");
    assert!(reason.contains("t.toml"), "{reason}");
    let reason = registry_reason(load("README.md", SPLIT).unwrap_err(), "README.md");
    assert!(reason.contains(".toml"), "{reason}");
}

#[test]
fn error_in_a_later_file_is_reported_not_skipped() {
    let bad = "id = \"b\"\nname = \"B\"\nkind = \"csv\"\n";
    let err = load_registry(&files(&[("a.toml", SPLIT), ("b.toml", bad)])).unwrap_err();
    let reason = registry_reason(err, "b");
    assert!(reason.contains("b.toml"), "{reason}");
}

#[test]
fn registry_is_ordered_by_id_not_by_input_order() {
    let other = SPLIT.replacen("test-split", "aaa-first", 1);
    let reg = load_registry(&files(&[("t.toml", SPLIT), ("a.toml", other.as_str())])).unwrap();
    let ids: Vec<&str> = reg.iter().map(|s| s.id.as_str()).collect();
    // derived: byte order of "aaa-first" < "test-split".
    assert_eq!(ids, ["aaa-first", "test-split"]);
    let rev = load_registry(&files(&[("a.toml", other.as_str()), ("t.toml", SPLIT)])).unwrap();
    assert_eq!(reg, rev);
    assert!(reg.get("missing").is_none());
}

#[test]
fn empty_input_is_an_empty_registry() {
    let reg = load_registry(&[]).unwrap();
    assert!(reg.is_empty());
    assert_eq!(reg.len(), 0);
}

#[test]
fn csv_layout_carries_every_field() {
    let spec: FormatSpec = load("t.toml", SPLIT)
        .unwrap()
        .get("test-split")
        .unwrap()
        .clone();
    // derived: each field is the value written in SPLIT.
    assert_eq!(
        spec.csv_layout().unwrap(),
        CsvLayout {
            format_id: "test-split".to_owned(),
            delimiter: b';',
            skip_rows: 2,
            date_col: 0,
            value_date_col: Some(1),
            description_col: 2,
            reference_col: Some(3),
            amounts: AmountColumns::Split {
                debit: 4,
                credit: 5
            },
            balance_col: Some(6),
            date_format: DateFormat::DdMmYy { century_base: 2000 },
            date_separator: '-',
        }
    );
}

#[test]
fn registry_layout_drives_the_csv_parser() {
    // Fictional statement: two skipped rows, ';' delimiter, dd-mm-yy dates.
    let csv = "Fictional Bank;;\nDate;Value;Narration;Ref;Dr;Cr;Bal\n\
               05-04-24;06-04-24;RENT;C1;500.00;;1,000.00\n";
    let layout = load("t.toml", SPLIT)
        .unwrap()
        .get("test-split")
        .unwrap()
        .csv_layout()
        .unwrap();
    let st = parse_csv_with_layout(csv.as_bytes(), &layout).unwrap();
    assert_eq!(st.format_id, "test-split");
    assert_eq!(st.lines.len(), 1);
    // derived: 05-04-24 with century base 2000 is 5 April 2024; 500.00 rupees = 50_000 paise;
    // 1,000.00 rupees = 100_000 paise.
    assert_eq!(
        st.lines[0].date,
        Date::from_calendar_date(2024, Month::April, 5).unwrap()
    );
    assert_eq!(st.lines[0].debit, Paise::new(50_000));
    assert_eq!(st.lines[0].balance, Some(Paise::new(100_000)));
}

#[test]
fn shipped_formats_load() {
    let reg = builtin_registry().unwrap();
    let ids: Vec<&str> = reg.iter().map(|s| s.id.as_str()).collect();
    // derived: the id written in each formats/*.toml file, in byte order.
    assert_eq!(
        ids,
        [
            "generic-csv-indicator",
            "generic-csv-signed",
            "generic-csv-split",
            "generic-mt940",
            "generic-xlsx-split",
        ]
    );
    for spec in reg.iter() {
        match spec.kind {
            FileKind::Csv => {
                spec.csv_layout().unwrap();
            }
            FileKind::Xlsx | FileKind::Mt940 => {}
        }
    }
    let split = reg.get("generic-csv-split").unwrap().csv_layout().unwrap();
    // derived: formats/generic-csv-split.toml writes debit = 4, credit = 5, skip_rows = 1.
    assert_eq!(
        split.amounts,
        AmountColumns::Split {
            debit: 4,
            credit: 5
        }
    );
    assert_eq!(split.skip_rows, 1);
    assert_eq!(split.delimiter, b',');
}

#[test]
fn shipped_files_on_disk_match_the_compiled_in_copy() {
    // Guards the include_str! list in builtin_registry against a new file being forgotten.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("formats");
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| std::path::Path::new(n).extension() == Some(std::ffi::OsStr::new("toml")))
        .collect();
    names.sort();
    // derived: count of formats/*.toml files; ids are file stems by convention.
    let ids: Vec<String> = builtin_registry()
        .unwrap()
        .iter()
        .map(|s| format!("{}.toml", s.id))
        .collect();
    assert_eq!(names, ids);
}

#[test]
fn xlsx_balance_labels_load_from_the_toml() {
    // derived: the three label keys are copied to the table's balance labels as written.
    let text = r#"
id = "x-labels"
name = "X"
kind = "xlsx"
header_cells = ["Date", "Narration"]
date_format = "dd/mm/yyyy"
amounts = "signed"
opening_label = "Opening Balance"
closing_label = "Closing Balance"
account_label = "Account Number"
[columns]
date = 0
description = 1
amount = 2
"#;
    let reg = load("x-labels.toml", text).unwrap();
    let table = reg.get("x-labels").unwrap().table.as_ref().unwrap();
    assert_eq!(
        table.balance_labels.opening.as_deref(),
        Some("Opening Balance")
    );
    assert_eq!(
        table.balance_labels.closing.as_deref(),
        Some("Closing Balance")
    );
    assert_eq!(
        table.balance_labels.account.as_deref(),
        Some("Account Number")
    );
}

#[test]
fn balance_labels_are_xlsx_only_and_never_blank() {
    // derived: a CSV file has no labelled cells, so a balance label key is an error for it; and a
    // label that is only spaces names no cell, so it is an error for xlsx too.
    let csv = r#"
id = "c-labels"
name = "C"
kind = "csv"
delimiter = ","
skip_rows = 1
date_format = "dd/mm/yyyy"
amounts = "split"
opening_label = "Opening Balance"
[columns]
date = 0
description = 1
debit = 2
credit = 3
"#;
    let err = load("c-labels.toml", csv).unwrap_err();
    let reason = registry_reason(err, "c-labels");
    assert!(
        reason.contains("'opening_label' applies to xlsx only"),
        "{reason}"
    );

    let blank = r#"
id = "x-blank"
name = "X"
kind = "xlsx"
header_cells = ["Date", "Narration"]
date_format = "dd/mm/yyyy"
amounts = "signed"
closing_label = "   "
[columns]
date = 0
description = 1
amount = 2
"#;
    let err = load("x-blank.toml", blank).unwrap_err();
    let reason = registry_reason(err, "x-blank");
    assert!(
        reason.contains("'closing_label' must not be empty"),
        "{reason}"
    );
}
