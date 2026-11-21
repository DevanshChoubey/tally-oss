//! Card PR2: PDF rendering of a tax invoice. Every expected string is `derived` by hand from the
//! books built in this file (fictional names, fictional GSTINs with valid check characters taken
//! from the `ledgerly-core` test vectors). Nothing is copied from the renderer's own output.
//!
//! The sale printed is the one in the RP2 trial-balance case: 10 pieces at 1,000.00 each is a
//! taxable value of 10,000.00; GST at 18 percent is 1,800.00; the invoice total is 11,800.00
//! (10,000.00 + 1,800.00, no round-off), posted Dr party 11,800.00.
//!
//! The PDF is written uncompressed, so the tests read the text back by scanning the content
//! streams for the `( .. )` strings the writer emits.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use ledgerly_core::{
    CompanyId, Date, Decimal, GroupId, Gstin, Hsn, LedgerId, Month, Paise, Qty, Rate, SignedAmount,
    StateCode, StockItemId, UnitId, VoucherId, VoucherTypeId,
};
use ledgerly_model::{
    Address, BaseVoucherKind, Books, Company, EntityKind, Features, GstLine, GstRegistration,
    InventoryLine, Ledger, LedgerLine, Masters, NegativeStockPolicy, NumberingConfig,
    NumberingMethod, PartyDetails, RegistrationType, RestartPeriod, StockItem, StoredGst,
    TaxLedgerKind, Unit, UnitKind, ValuationMethod, Voucher, VoucherMode, VoucherStatus,
    VoucherType,
};
use ledgerly_print::pdf::{UpiPayee, render_invoice, render_invoice_with_upi};
use ledgerly_print::template::{PrintError, Template, load_template};

const PARTY: u64 = 12;
const SALES: u64 = 13;
const IGST: u64 = 14;
const CGST: u64 = 15;
const SGST: u64 = 16;

const COMPANY_GSTIN: &str = "29ABCDE1234F1ZW";
const PARTY_GSTIN: &str = "07AAACL0000A1ZP";
const OTHER_GSTIN: &str = "24AAACB1111C1ZT";

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
columns = ["hsn", "taxable_value", "cgst_rate", "cgst_amount", "sgst_rate", "sgst_amount", "igst_rate", "igst_amount", "total_tax"]

[[block]]
kind = "totals"
slots = ["taxable_total", "tax_total", "round_off", "grand_total"]

[[block]]
kind = "amount_in_words"
title = "Amount in words"

[[block]]
kind = "text"
text = "Goods once sold will not be taken back."
"#;

fn rs(n: i64) -> Paise {
    Paise::new(n * 100)
}

fn date(y: i32, m: Month, d: u8) -> Date {
    Date::from_calendar_date(y, m, d).unwrap()
}

fn pct(n: i64) -> Rate {
    Rate(Decimal::new(n, 0))
}

fn address() -> Address {
    Address {
        lines: vec!["12 Market Road".to_owned(), "Fictional Nagar".to_owned()],
        pincode: Some("560001".to_owned()),
        state: Some(StateCode::new(29).unwrap()),
        country: String::new(),
    }
}

fn voucher_type(base: BaseVoucherKind) -> VoucherType {
    VoucherType {
        id: VoucherTypeId(1),
        name: "Sales".to_owned(),
        base,
        abbreviation: "Sal".to_owned(),
        numbering: NumberingConfig {
            method: NumberingMethod::Automatic,
            prevent_renumbering: true,
            prefix: "INV/26-27/".to_owned(),
            suffix: String::new(),
            width: 4,
            restart: RestartPeriod::Yearly,
            series: vec![],
        },
        active: true,
        tally: None,
    }
}

fn party_ledger(registration: RegistrationType, gstin: Option<&str>, name: &str) -> Ledger {
    Ledger {
        id: LedgerId(PARTY),
        name: name.to_owned(),
        aliases: vec![],
        group: GroupId(4),
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: Some(PartyDetails {
            registration,
            gstin: gstin.map(|g| Gstin::parse(g).unwrap()),
            pan: None,
            state: Some(StateCode::new(29).unwrap()),
            address: Address {
                lines: vec!["7 Fictional Lane".to_owned()],
                pincode: Some("110001".to_owned()),
                state: Some(StateCode::new(7).unwrap()),
                country: String::new(),
            },
        }),
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn registration(gstin: &str, composition: bool) -> GstRegistration {
    GstRegistration {
        gstin: Gstin::parse(gstin).unwrap(),
        state: StateCode::new(29).unwrap(),
        effective_from: date(2026, Month::April, 1),
        composition,
    }
}

fn item(hsn: &str) -> StockItem {
    StockItem {
        id: StockItemId(1),
        name: "Widget".to_owned(),
        aliases: vec![],
        group: None,
        unit: UnitId(1),
        alt_unit: None,
        hsn: Some(Hsn::parse(hsn).unwrap()),
        gst_history: vec![],
        valuation: ValuationMethod::AverageCost,
        batches_enabled: false,
        opening: vec![],
        standard_costs: vec![],
        tally: None,
    }
}

fn books(base: BaseVoucherKind, party: Ledger) -> Books {
    let mut masters = Masters::default();
    masters.ledgers.insert(party.id, party);
    masters.stock_items.insert(StockItemId(1), item("8479"));
    masters.units.insert(
        UnitId(1),
        Unit {
            id: UnitId(1),
            symbol: "pcs".to_owned(),
            formal_name: "Pieces".to_owned(),
            decimals: 0,
            kind: UnitKind::Simple,
            uqc: None,
            tally: None,
        },
    );
    masters
        .voucher_types
        .insert(VoucherTypeId(1), voucher_type(base));
    Books {
        company: Company {
            id: CompanyId(1),
            name: "Test Traders".to_owned(),
            mailing_name: None,
            address: address(),
            state: StateCode::new(29).unwrap(),
            pan: None,
            registrations: vec![registration(COMPANY_GSTIN, false)],
            fy_start: date(2026, Month::April, 1),
            books_begin: date(2026, Month::April, 1),
            entity_kind: EntityKind::Proprietorship,
            features: Features::default(),
            negative_stock: NegativeStockPolicy::Block,
        },
        masters,
        vouchers: BTreeMap::new(),
        postings: vec![],
        stock_moves: vec![],
    }
}

fn line(n: i64) -> InventoryLine {
    InventoryLine {
        item: StockItemId(1),
        qty: Qty(Decimal::new(n, 0)),
        unit: UnitId(1),
        rate: Rate(Decimal::new(1000, 0)),
        discount_percent: None,
        amount: rs(n * 1000),
        ledger: Some(LedgerId(SALES)),
        godowns: vec![],
    }
}

fn gst_line(ledger: u64, kind: TaxLedgerKind, taxable: Paise, rate: i64, tax: Paise) -> GstLine {
    GstLine {
        ledger: LedgerId(ledger),
        kind,
        taxable,
        rate_percent: pct(rate),
        tax,
    }
}

/// An inter-state sale: IGST 18 percent on 10,000.00.
fn igst_sale() -> (Voucher, StoredGst) {
    let gst = StoredGst {
        pack_version: "test".to_owned(),
        lines: vec![gst_line(
            IGST,
            TaxLedgerKind::Igst,
            rs(10_000),
            18,
            rs(1_800),
        )],
        round_off: Paise::ZERO,
    };
    (voucher(rs(11_800), vec![line(10)], &gst, 7), gst)
}

fn voucher(posted: Paise, items: Vec<InventoryLine>, gst: &StoredGst, pos: u8) -> Voucher {
    Voucher {
        id: VoucherId(1),
        type_id: VoucherTypeId(1),
        number: Some("INV/26-27/0001".to_owned()),
        series: None,
        date: date(2026, Month::April, 10),
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::ItemInvoice,
        party: Some(LedgerId(PARTY)),
        place_of_supply: Some(StateCode::new(pos).unwrap()),
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: String::new(),
        ledger_lines: vec![LedgerLine {
            ledger: LedgerId(PARTY),
            amount: SignedAmount::dr(posted),
            bills: vec![],
            cost: vec![],
            bank: None,
        }],
        inventory_lines: items,
        gst: Some(gst.clone()),
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn template(toml: &str) -> Template {
    load_template(toml).unwrap()
}

fn default_books() -> Books {
    books(
        BaseVoucherKind::Sales,
        party_ledger(RegistrationType::Regular, Some(PARTY_GSTIN), "Customer One"),
    )
}

/// The text strings drawn on the pages, in drawing order.
fn texts(pdf: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < pdf.len() {
        if pdf[i] != b'(' {
            i += 1;
            continue;
        }
        let mut depth = 1;
        let mut s = Vec::new();
        i += 1;
        while i < pdf.len() && depth > 0 {
            match pdf[i] {
                b'\\' => {
                    i += 1;
                    s.push(pdf[i]);
                }
                b'(' => {
                    depth += 1;
                    s.push(b'(');
                }
                b')' => {
                    depth -= 1;
                    if depth > 0 {
                        s.push(b')');
                    }
                }
                other => s.push(other),
            }
            i += 1;
        }
        out.push(String::from_utf8(s).unwrap());
    }
    out
}

fn has(strings: &[String], wanted: &str) -> bool {
    strings.iter().any(|s| s == wanted)
}

fn err(r: Result<Vec<u8>, PrintError>) -> PrintError {
    r.unwrap_err()
}

#[test]
fn tax_invoice_for_the_rp2_sale_renders() {
    let b = default_books();
    let (v, gst) = igst_sale();
    let pdf = render_invoice(&template(FULL), &v, &b, &gst).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    assert!(String::from_utf8_lossy(&pdf).contains("%%EOF"));
    let t = texts(&pdf);
    // derived: document title for a sales voucher; the company GSTIN is the registration given.
    assert!(has(&t, "Tax Invoice"));
    assert!(has(&t, "GSTIN: 29ABCDE1234F1ZW"));
    assert!(has(&t, "Test Traders"));
    assert!(has(&t, "Invoice No: INV/26-27/0001"));
    // derived: voucher date 10 April 2026 as dd-mm-yyyy.
    assert!(has(&t, "Date: 10-04-2026"));
    // derived: 10 x 1,000.00 = 10,000.00 taxable; 18% of 10,000.00 = 1,800.00; total 11,800.00.
    assert!(has(&t, "10,000.00"));
    assert!(has(&t, "1,800.00"));
    assert!(has(&t, "11,800.00"));
    assert!(has(&t, "0.00"));
    assert!(has(&t, "18%"));
    // derived: party details come from the party ledger.
    assert!(has(&t, "Name: Customer One"));
    assert!(has(&t, "GSTIN: 07AAACL0000A1ZP"));
    assert!(t.iter().any(|s| s.starts_with("Place of supply: 07-")));
    // derived: item line from the inventory line.
    assert!(has(&t, "Widget"));
    assert!(has(&t, "8479"));
    assert!(has(&t, "pcs"));
    // derived: 11,800 is eleven thousand eight hundred rupees.
    assert!(
        t.iter()
            .any(|s| s.contains("Eleven Thousand Eight Hundred"))
    );
    assert!(has(&t, "Goods once sold will not be taken back."));
    assert!(has(&t, "Page 1 of 1"));
}

#[test]
fn same_input_gives_the_same_bytes() {
    let b = default_books();
    let (v, gst) = igst_sale();
    let t = template(FULL);
    assert_eq!(
        render_invoice(&t, &v, &b, &gst).unwrap(),
        render_invoice(&t, &v, &b, &gst).unwrap()
    );
}

#[test]
fn intra_state_sale_shows_cgst_and_sgst_halves() {
    let b = books(
        BaseVoucherKind::Sales,
        party_ledger(RegistrationType::Consumer, None, "Walk-in Customer"),
    );
    // derived: 9% of 10,000.00 = 900.00 each for CGST and SGST; total tax 1,800.00.
    let gst = StoredGst {
        pack_version: "test".to_owned(),
        lines: vec![
            gst_line(CGST, TaxLedgerKind::Cgst, rs(10_000), 9, rs(900)),
            gst_line(SGST, TaxLedgerKind::Sgst, rs(10_000), 9, rs(900)),
        ],
        round_off: Paise::ZERO,
    };
    let v = voucher(rs(11_800), vec![line(10)], &gst, 29);
    let pdf = render_invoice(&template(FULL), &v, &b, &gst).unwrap();
    let t = texts(&pdf);
    assert_eq!(t.iter().filter(|s| *s == "900.00").count(), 2);
    assert!(has(&t, "9%"));
    assert!(has(&t, "1,800.00"));
    assert!(has(&t, "11,800.00"));
    // derived: a consumer has no GSTIN, so the invoice says so.
    assert!(has(&t, "GSTIN: Unregistered"));
}

#[test]
fn round_off_is_part_of_the_total() {
    let b = default_books();
    // derived: taxable 1,000.10 + 18% (180.018 -> 180.02) = 1,180.12; round-off -0.12 gives
    // 1,180.00 posted to the party.
    let gst = StoredGst {
        pack_version: "test".to_owned(),
        lines: vec![GstLine {
            ledger: LedgerId(IGST),
            kind: TaxLedgerKind::Igst,
            taxable: Paise::new(100_010),
            rate_percent: pct(18),
            tax: Paise::new(18_002),
        }],
        round_off: Paise::new(-12),
    };
    let mut l = line(1);
    l.rate = Rate(Decimal::new(100_010, 2));
    l.amount = Paise::new(100_010);
    let v = voucher(Paise::new(118_000), vec![l], &gst, 7);
    let pdf = render_invoice(&template(FULL), &v, &b, &gst).unwrap();
    let t = texts(&pdf);
    assert!(has(&t, "-0.12"));
    assert!(has(&t, "1,180.00"));
    assert!(has(&t, "1,000.10"));
}

#[test]
fn credit_note_uses_its_own_title() {
    let b = books(
        BaseVoucherKind::CreditNote,
        party_ledger(RegistrationType::Regular, Some(PARTY_GSTIN), "Customer One"),
    );
    let (v, gst) = igst_sale();
    let t = texts(&render_invoice(&template(FULL), &v, &b, &gst).unwrap());
    assert!(has(&t, "Credit Note"));
    assert!(has(&t, "Credit Note No: INV/26-27/0001"));
    assert!(!has(&t, "Tax Invoice"));
}

#[test]
fn totals_that_disagree_with_the_party_posting_are_an_error() {
    let b = default_books();
    let (mut v, gst) = igst_sale();
    // derived: the invoice is 11,800.00 but only 11,000.00 is posted to the party.
    v.ledger_lines[0].amount = SignedAmount::dr(rs(11_000));
    let PrintError::Render { reason } = err(render_invoice(&template(FULL), &v, &b, &gst)) else {
        panic!("expected a render error");
    };
    assert!(reason.contains("posted to the party ledger"));
}

#[test]
fn item_lines_that_differ_from_the_taxable_value_are_unsupported() {
    let b = default_books();
    let (mut v, gst) = igst_sale();
    // derived: 9 x 1,000.00 = 9,000.00 against a stored taxable value of 10,000.00.
    v.inventory_lines = vec![line(9)];
    assert!(matches!(
        err(render_invoice(&template(FULL), &v, &b, &gst)),
        PrintError::Unsupported { .. }
    ));
}

#[test]
fn gst_that_differs_from_the_stored_gst_is_an_error() {
    let b = default_books();
    let (v, mut gst) = igst_sale();
    gst.round_off = Paise::new(1);
    assert!(matches!(
        err(render_invoice(&template(FULL), &v, &b, &gst)),
        PrintError::Render { .. }
    ));
}

#[test]
fn vouchers_that_are_not_tax_documents_are_unsupported() {
    let (v, gst) = igst_sale();
    let t = template(FULL);
    let purchase = books(
        BaseVoucherKind::Purchase,
        party_ledger(RegistrationType::Regular, Some(PARTY_GSTIN), "Customer One"),
    );
    assert!(matches!(
        err(render_invoice(&t, &v, &purchase, &gst)),
        PrintError::Unsupported { .. }
    ));
    let b = default_books();
    for status in [VoucherStatus::Cancelled, VoucherStatus::Optional] {
        let mut v = v.clone();
        v.status = status;
        assert!(matches!(
            err(render_invoice(&t, &v, &b, &gst)),
            PrintError::Unsupported { .. }
        ));
    }
}

#[test]
fn composition_or_several_gstins_are_unsupported() {
    let (v, gst) = igst_sale();
    let t = template(FULL);
    let mut composition = default_books();
    composition.company.registrations = vec![registration(COMPANY_GSTIN, true)];
    assert!(matches!(
        err(render_invoice(&t, &v, &composition, &gst)),
        PrintError::Unsupported { .. }
    ));
    let mut two = default_books();
    two.company.registrations = vec![
        registration(COMPANY_GSTIN, false),
        registration(OTHER_GSTIN, false),
    ];
    assert!(matches!(
        err(render_invoice(&t, &v, &two, &gst)),
        PrintError::Unsupported { .. }
    ));
}

#[test]
fn text_outside_ascii_is_refused_not_replaced() {
    let mut b = default_books();
    b.company.name = "Caf\u{e9} Traders".to_owned();
    let (v, gst) = igst_sale();
    assert!(matches!(
        err(render_invoice(&template(FULL), &v, &b, &gst)),
        PrintError::Unsupported { .. }
    ));
}

#[test]
fn hsn_column_with_two_codes_is_unsupported() {
    let mut b = default_books();
    let mut second = item("8480");
    second.id = StockItemId(2);
    second.name = "Gadget".to_owned();
    b.masters.stock_items.insert(StockItemId(2), second);
    let (mut v, gst) = igst_sale();
    let mut l = line(5);
    l.item = StockItemId(2);
    v.inventory_lines = vec![line(5), l];
    // the item table alone is fine: each line has its own code
    let items_only = template(
        "format_version = 1\nname = \"x\"\npage = \"A4\"\n[[block]]\nkind = \"item_table\"\ncolumns = [\"description\", \"hsn\", \"amount\"]\n",
    );
    let t = texts(&render_invoice(&items_only, &v, &b, &gst).unwrap());
    assert!(has(&t, "8479"));
    assert!(has(&t, "8480"));
    // the tax table cannot say which code goes with which stored tax line
    let tax_only = template(
        "format_version = 1\nname = \"x\"\npage = \"A4\"\n[[block]]\nkind = \"tax_table\"\ncolumns = [\"hsn\", \"total_tax\"]\n",
    );
    assert!(matches!(
        err(render_invoice(&tax_only, &v, &b, &gst)),
        PrintError::Unsupported { .. }
    ));
}

#[test]
fn upi_qr_needs_a_payee_and_then_draws_the_code() {
    let b = default_books();
    let (v, gst) = igst_sale();
    let t = template(
        "format_version = 1\nname = \"x\"\npage = \"A4\"\n[[block]]\nkind = \"upi_qr\"\ntitle = \"Scan to pay\"\n",
    );
    assert!(matches!(
        err(render_invoice(&t, &v, &b, &gst)),
        PrintError::Unsupported { .. }
    ));
    let payee = UpiPayee {
        vpa: "shop@examplebank",
        name: "Test Traders",
    };
    let pdf = render_invoice_with_upi(&t, &v, &b, &gst, Some(&payee)).unwrap();
    assert!(has(&texts(&pdf), "Pay to: shop@examplebank"));
    // derived (plan card K5 shape): amount 11,800.00 is "11800.00"; the name and the voucher
    // number are percent-encoded. One filled square is drawn per dark module of that code.
    let payload =
        "upi://pay?pa=shop@examplebank&pn=Test%20Traders&am=11800.00&cu=INR&tn=INV%2F26-27%2F0001";
    let code = qrcode::QrCode::new(payload.as_bytes()).unwrap();
    let dark = code
        .into_colors()
        .into_iter()
        .filter(|c| matches!(c, qrcode::Color::Dark))
        .count();
    let text = String::from_utf8_lossy(&pdf);
    let squares = text.lines().filter(|l| l.ends_with(" re")).count();
    assert_eq!(squares, dark);
}

#[test]
fn a_long_invoice_continues_on_new_pages_with_the_heading_repeated() {
    let b = default_books();
    // derived: 120 lines of 1 piece at 1,000.00 = 120,000.00; 18% = 21,600.00; total 141,600.00.
    let gst = StoredGst {
        pack_version: "test".to_owned(),
        lines: vec![gst_line(
            IGST,
            TaxLedgerKind::Igst,
            rs(120_000),
            18,
            rs(21_600),
        )],
        round_off: Paise::ZERO,
    };
    let v = voucher(rs(141_600), (0..120).map(|_| line(1)).collect(), &gst, 7);
    let t = texts(&render_invoice(&template(FULL), &v, &b, &gst).unwrap());
    let pages = (1_usize..=50)
        .find(|n| has(&t, &format!("Page {n} of {n}")))
        .unwrap();
    assert!(pages >= 2);
    for n in 1..=pages {
        assert!(has(&t, &format!("Page {n} of {pages}")));
    }
    // derived: the item table heading is printed once per page that holds table rows.
    let headings = t.iter().filter(|s| *s == "Description").count();
    assert!((2..=pages).contains(&headings));
    assert_eq!(t.iter().filter(|s| *s == "Widget").count(), 120);
    assert!(has(&t, "1,41,600.00"));
}

#[test]
fn long_text_wraps_without_losing_words() {
    let b = default_books();
    let (v, gst) = igst_sale();
    let words: Vec<String> = (0..80).map(|n| format!("term{n}")).collect();
    let text = words.join(" ");
    let toml = format!(
        "format_version = 1\nname = \"x\"\npage = \"A5\"\n[[block]]\nkind = \"text\"\ntext = \"{text}\"\n"
    );
    let t = texts(&render_invoice(&template(&toml), &v, &b, &gst).unwrap());
    let body: Vec<&str> = t
        .iter()
        .filter(|s| !s.starts_with("Page "))
        .map(String::as_str)
        .collect();
    assert!(body.len() > 1, "80 words do not fit one A5 line");
    assert_eq!(body.join(" "), text);
}

/// Two lines of one tax row each, with the party posting split into a debit and a credit.
fn party_lines(debit: Paise, credit: Option<Paise>) -> Vec<LedgerLine> {
    let mut lines = vec![LedgerLine {
        ledger: LedgerId(PARTY),
        amount: SignedAmount::dr(debit),
        bills: vec![],
        cost: vec![],
        bank: None,
    }];
    if let Some(c) = credit {
        lines.push(LedgerLine {
            ledger: LedgerId(PARTY),
            amount: SignedAmount::cr(c),
            bills: vec![],
            cost: vec![],
            bank: None,
        });
    }
    lines
}

#[test]
fn balanced_debit_and_credit_to_the_party_are_not_the_invoice_total() {
    // derived: the invoice is 11,800.00. A debit of 5,900.00 and a credit of 5,900.00 to the party
    // net to 0.00, not 11,800.00. Before the fix the sides were ignored and the two summed to
    // 11,800.00, which printed as the total.
    let b = default_books();
    let (mut v, gst) = igst_sale();
    v.ledger_lines = party_lines(rs(5_900), Some(rs(5_900)));
    let PrintError::Render { reason } = err(render_invoice(&template(FULL), &v, &b, &gst)) else {
        panic!("expected a render error");
    };
    assert!(reason.contains("posted to the party ledger"), "{reason}");
}

#[test]
fn a_credit_posting_to_the_party_is_not_the_debit_the_total_needs() {
    // derived: the invoice is 11,800.00 and the party line is a credit of 11,800.00, which is
    // minus 11,800.00 once sides are counted. It is not the debit the total needs.
    let b = default_books();
    let (mut v, gst) = igst_sale();
    v.ledger_lines = party_lines(rs(11_800), None);
    v.ledger_lines[0].amount = SignedAmount::cr(rs(11_800));
    let PrintError::Render { reason } = err(render_invoice(&template(FULL), &v, &b, &gst)) else {
        panic!("expected a render error");
    };
    assert!(reason.contains("a net credit"), "{reason}");
}

#[test]
fn two_tax_rows_with_the_same_taxable_value_leave_a_cess_line_unsupported() {
    // derived: two IGST rows of 10,000.00 (at 18% and 12%) and a cess line of 10,000.00. The
    // stored lines do not say which row the cess belongs to, so it is refused, not attached to the
    // first row. Before the fix it was attached to the 18% row.
    let b = default_books();
    let gst = StoredGst {
        pack_version: "test".to_owned(),
        lines: vec![
            gst_line(IGST, TaxLedgerKind::Igst, rs(10_000), 18, rs(1_800)),
            gst_line(IGST, TaxLedgerKind::Igst, rs(10_000), 12, rs(1_200)),
            gst_line(17, TaxLedgerKind::Cess, rs(10_000), 1, rs(100)),
        ],
        round_off: Paise::ZERO,
    };
    let v = voucher(rs(13_100), vec![], &gst, 7);
    assert!(matches!(
        err(render_invoice(&template(FULL), &v, &b, &gst)),
        PrintError::Unsupported { .. }
    ));
}

#[test]
fn every_item_heading_is_followed_by_a_row_before_its_page_footer() {
    // derived: 120 lines of 1 piece each. Each heading ("Description") must be followed, before the
    // footer of its page ("Page n of N"), by at least one item row ("Widget"). A heading left at
    // the foot of a page would have no row between it and that footer.
    let b = default_books();
    let gst = StoredGst {
        pack_version: "test".to_owned(),
        lines: vec![gst_line(
            IGST,
            TaxLedgerKind::Igst,
            rs(120_000),
            18,
            rs(21_600),
        )],
        round_off: Paise::ZERO,
    };
    let v = voucher(rs(141_600), (0..120).map(|_| line(1)).collect(), &gst, 7);
    let t = texts(&render_invoice(&template(FULL), &v, &b, &gst).unwrap());
    let mut checked = 0_usize;
    for (i, s) in t.iter().enumerate() {
        if s != "Description" {
            continue;
        }
        let footer = t
            .iter()
            .enumerate()
            .skip(i)
            .find(|(_, x)| x.starts_with("Page "))
            .map(|(j, _)| j)
            .unwrap();
        assert!(
            t[i..footer].iter().any(|x| x == "Widget"),
            "the heading at text {i} has no item row on its page"
        );
        checked = checked.saturating_add(1);
    }
    assert!(checked >= 2, "{checked}");
}
