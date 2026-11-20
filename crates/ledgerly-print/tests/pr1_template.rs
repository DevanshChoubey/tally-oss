//! Card PR1: print template format. A template carries no figures, so the expected values here
//! are `derived` from the format contract in the `ledgerly_print::template` module docs (block
//! kinds, keys, slot and column names) and from the template text written in each test.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_print::template::{
    BlockBody, BlockKind, ItemColumn, PageSize, PrintError, Slot, TaxColumn, load_template,
};

/// A full tax-invoice template: header, details, item table, tax table, totals, amount in words,
/// UPI QR and a terms line.
const FULL: &str = r#"
format_version = 1
name = "Tax invoice"
page = "A4"

[[block]]
kind = "header"
slots = ["company_name", "company_address", "company_gstin", "document_title", "voucher_number", "voucher_date"]

[[block]]
kind = "details"
title = "Bill to"
slots = ["party_name", "party_address", "party_gstin", "place_of_supply"]

[[block]]
kind = "item_table"
columns = ["serial", "description", "hsn", "quantity", "unit", "rate", "taxable_value", "tax_rate", "amount"]

[[block]]
kind = "tax_table"
title = "Tax summary"
columns = ["hsn", "taxable_value", "cgst_rate", "cgst_amount", "sgst_rate", "sgst_amount", "total_tax"]

[[block]]
kind = "totals"
slots = ["taxable_total", "tax_total", "round_off", "grand_total"]

[[block]]
kind = "amount_in_words"
title = "Amount in words"

[[block]]
kind = "upi_qr"
title = "Scan to pay"

[[block]]
kind = "text"
text = "Goods once sold will not be taken back."
"#;

fn with_block(body: &str) -> String {
    format!("format_version = 1\nname = \"T\"\npage = \"A4\"\n\n[[block]]\n{body}\n")
}

fn err(toml: &str) -> PrintError {
    load_template(toml).unwrap_err()
}

#[test]
fn full_template_loads_every_block_in_order() {
    let t = load_template(FULL).unwrap();
    // provenance: derived — the template text above names 8 blocks in this order and these names.
    assert_eq!(t.name, "Tax invoice");
    assert_eq!(t.page, PageSize::A4);
    let kinds: Vec<BlockKind> = t.blocks.iter().map(|b| b.body.kind()).collect();
    assert_eq!(
        kinds,
        vec![
            BlockKind::Header,
            BlockKind::Details,
            BlockKind::ItemTable,
            BlockKind::TaxTable,
            BlockKind::Totals,
            BlockKind::AmountInWords,
            BlockKind::UpiQr,
            BlockKind::Text,
        ]
    );
}

#[test]
fn header_slots_keep_template_order() {
    let t = load_template(FULL).unwrap();
    // provenance: derived — the header block lists six slots in this order.
    assert_eq!(
        t.blocks[0].body,
        BlockBody::Header {
            slots: vec![
                Slot::CompanyName,
                Slot::CompanyAddress,
                Slot::CompanyGstin,
                Slot::DocumentTitle,
                Slot::VoucherNumber,
                Slot::VoucherDate,
            ]
        }
    );
    assert_eq!(t.blocks[0].title, None);
    assert_eq!(t.blocks[1].title.as_deref(), Some("Bill to"));
}

#[test]
fn item_table_loads_columns() {
    let t = load_template(FULL).unwrap();
    // provenance: derived — the item_table block lists nine columns in this order.
    assert_eq!(
        t.blocks[2].body,
        BlockBody::ItemTable {
            columns: vec![
                ItemColumn::Serial,
                ItemColumn::Description,
                ItemColumn::Hsn,
                ItemColumn::Quantity,
                ItemColumn::Unit,
                ItemColumn::Rate,
                ItemColumn::TaxableValue,
                ItemColumn::TaxRate,
                ItemColumn::Amount,
            ]
        }
    );
}

#[test]
fn tax_table_loads_columns() {
    let t = load_template(FULL).unwrap();
    // provenance: derived — the tax_table block lists seven columns in this order.
    assert_eq!(
        t.blocks[3].body,
        BlockBody::TaxTable {
            columns: vec![
                TaxColumn::Hsn,
                TaxColumn::TaxableValue,
                TaxColumn::CgstRate,
                TaxColumn::CgstAmount,
                TaxColumn::SgstRate,
                TaxColumn::SgstAmount,
                TaxColumn::TotalTax,
            ]
        }
    );
}

#[test]
fn amount_in_words_and_upi_qr_slots_load() {
    let t = load_template(FULL).unwrap();
    // provenance: derived — blocks 6 and 7 of the template text.
    assert_eq!(t.blocks[5].body, BlockBody::AmountInWords);
    assert_eq!(t.blocks[6].body, BlockBody::UpiQr);
    assert_eq!(t.blocks[6].title.as_deref(), Some("Scan to pay"));
    assert_eq!(
        t.blocks[7].body,
        BlockBody::Text {
            text: "Goods once sold will not be taken back.".to_owned()
        }
    );
}

#[test]
fn a5_page_loads() {
    let t = load_template(
        "format_version = 1\nname = \"Slip\"\npage = \"A5\"\n[[block]]\nkind = \"amount_in_words\"\n",
    )
    .unwrap();
    // provenance: derived — the template text says A5.
    assert_eq!(t.page, PageSize::A5);
}

#[test]
fn unknown_slot_errors_naming_block_and_slot() {
    let e = err(&with_block(
        "kind = \"header\"\nslots = [\"company_name\", \"company_logo\"]",
    ));
    // provenance: derived — block 1 is a header and "company_logo" is not in Slot::ALL.
    assert_eq!(
        e,
        PrintError::UnknownSlot {
            block: 1,
            kind: BlockKind::Header,
            found: "company_logo".to_owned()
        }
    );
    assert!(e.to_string().contains("company_logo"));
}

#[test]
fn unknown_slot_in_a_later_block_reports_its_position() {
    let toml = format!(
        "{}\n[[block]]\nkind = \"totals\"\nslots = [\"grand_totl\"]\n",
        with_block("kind = \"amount_in_words\"")
    );
    // provenance: derived — the totals block is the second block and "grand_totl" is misspelled.
    assert_eq!(
        err(&toml),
        PrintError::UnknownSlot {
            block: 2,
            kind: BlockKind::Totals,
            found: "grand_totl".to_owned()
        }
    );
}

#[test]
fn unknown_column_errors() {
    // provenance: derived — "cgst_rate" is a tax column, not an item column.
    assert_eq!(
        err(&with_block(
            "kind = \"item_table\"\ncolumns = [\"description\", \"cgst_rate\"]"
        )),
        PrintError::UnknownColumn {
            block: 1,
            kind: BlockKind::ItemTable,
            found: "cgst_rate".to_owned()
        }
    );
    // provenance: derived — "serial" is an item column, not a tax column.
    assert_eq!(
        err(&with_block("kind = \"tax_table\"\ncolumns = [\"serial\"]")),
        PrintError::UnknownColumn {
            block: 1,
            kind: BlockKind::TaxTable,
            found: "serial".to_owned()
        }
    );
}

#[test]
fn unknown_block_kind_errors() {
    // provenance: derived — "signature" is not in BlockKind::ALL.
    assert_eq!(
        err(&with_block("kind = \"signature\"")),
        PrintError::UnknownBlockKind {
            block: 1,
            found: "signature".to_owned()
        }
    );
}

#[test]
fn missing_required_keys_error() {
    // provenance: derived — module docs table: header needs slots, item_table needs columns,
    // text needs text.
    assert_eq!(
        err(&with_block("kind = \"header\"")),
        PrintError::MissingKey {
            block: 1,
            kind: BlockKind::Header,
            key: "slots"
        }
    );
    assert_eq!(
        err(&with_block("kind = \"item_table\"")),
        PrintError::MissingKey {
            block: 1,
            kind: BlockKind::ItemTable,
            key: "columns"
        }
    );
    assert_eq!(
        err(&with_block("kind = \"text\"")),
        PrintError::MissingKey {
            block: 1,
            kind: BlockKind::Text,
            key: "text"
        }
    );
}

#[test]
fn keys_a_kind_does_not_take_error() {
    // provenance: derived — module docs table: upi_qr takes only title; header takes no columns.
    assert_eq!(
        err(&with_block("kind = \"upi_qr\"\nslots = [\"grand_total\"]")),
        PrintError::UnexpectedKey {
            block: 1,
            kind: BlockKind::UpiQr,
            key: "slots"
        }
    );
    assert_eq!(
        err(&with_block(
            "kind = \"header\"\nslots = [\"company_name\"]\ncolumns = [\"hsn\"]"
        )),
        PrintError::UnexpectedKey {
            block: 1,
            kind: BlockKind::Header,
            key: "columns"
        }
    );
}

#[test]
fn empty_and_repeated_lists_error() {
    // provenance: derived — an empty list prints nothing, a repeated entry prints twice; both
    // are refused rather than guessed.
    assert_eq!(
        err(&with_block("kind = \"header\"\nslots = []")),
        PrintError::EmptyList {
            block: 1,
            kind: BlockKind::Header,
            key: "slots"
        }
    );
    assert_eq!(
        err(&with_block("kind = \"tax_table\"\ncolumns = []")),
        PrintError::EmptyList {
            block: 1,
            kind: BlockKind::TaxTable,
            key: "columns"
        }
    );
    assert_eq!(
        err(&with_block(
            "kind = \"totals\"\nslots = [\"grand_total\", \"grand_total\"]"
        )),
        PrintError::DuplicateEntry {
            block: 1,
            kind: BlockKind::Totals,
            found: "grand_total".to_owned()
        }
    );
}

#[test]
fn single_use_blocks_may_not_repeat() {
    let toml = format!(
        "{}\n[[block]]\nkind = \"upi_qr\"\n",
        with_block("kind = \"upi_qr\"")
    );
    // provenance: derived — upi_qr is at-most-once; the second one is block 2, the first block 1.
    assert_eq!(
        err(&toml),
        PrintError::DuplicateBlock {
            block: 2,
            first: 1,
            kind: BlockKind::UpiQr
        }
    );
}

#[test]
fn repeatable_blocks_may_repeat() {
    let toml = format!(
        "{}\n[[block]]\nkind = \"text\"\ntext = \"b\"\n",
        with_block("kind = \"text\"\ntext = \"a\"")
    );
    // provenance: derived — text blocks are not at-most-once; two blocks load.
    assert_eq!(load_template(&toml).unwrap().blocks.len(), 2);
}

#[test]
fn header_level_errors() {
    // provenance: derived — module docs: only format_version 1, pages A4/A5, a name, one block.
    assert_eq!(
        err("format_version = 2\nname = \"T\"\npage = \"A4\"\n"),
        PrintError::UnsupportedVersion { found: 2 }
    );
    assert_eq!(
        err("format_version = 1\nname = \"  \"\npage = \"A4\"\n[[block]]\nkind = \"upi_qr\"\n"),
        PrintError::EmptyName
    );
    assert_eq!(
        err("format_version = 1\nname = \"T\"\npage = \"Legal\"\n[[block]]\nkind = \"upi_qr\"\n"),
        PrintError::UnknownPage {
            found: "Legal".to_owned()
        }
    );
    assert_eq!(
        err("format_version = 1\nname = \"T\"\npage = \"A4\"\n"),
        PrintError::NoBlocks
    );
}

#[test]
fn malformed_toml_and_unknown_keys_are_parse_errors() {
    // provenance: derived — not TOML; a missing required key; a key the format does not define.
    assert!(matches!(err("this is not toml ["), PrintError::Parse(_)));
    assert!(matches!(
        err("name = \"T\"\npage = \"A4\"\n"),
        PrintError::Parse(_)
    ));
    assert!(matches!(
        err(&with_block("kind = \"upi_qr\"\nsize_mm = 30")),
        PrintError::Parse(_)
    ));
    assert!(matches!(
        err("format_version = 1\nname = \"T\"\npage = \"A4\"\ncolour = \"red\"\n"),
        PrintError::Parse(_)
    ));
}
