//! Card X5 tests: Tally report exports (request builder and parsers).
//!
//! Provenance. The request shape is `derived` from research note R8 section 2.3(a)
//! (`docs/research/08-data-interop-editions.md`): `HEADER/ID` names the report and
//! `STATICVARIABLES` carries `SVCURRENTCOMPANY`, `SVFROMDATE`, `SVTODATE` (`YYYYMMDD`). The
//! response layouts below (`DSPACCNAME`/`DSPACCINFO`, `BSNAME`/`BSAMT`, `DSPSTKINFO`,
//! `BILLFIXED`/`BILLCL`) are hand-built from recall, NOT observed: they are `pending` an
//! `observed` capture (plan Prerequisite 4). Every assertion is therefore `derived` from the
//! fixture text and the arithmetic written next to it, and says nothing about what TallyPrime
//! really writes. The R8 2.3(e) sign convention (debit written negative) is `derived`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::{Date, DrCr, Month, Paise};
use ledgerly_tally_xml::TallyXmlError;
use ledgerly_tally_xml::reports::{
    TReportAmount, TReportRow, TallyReportKind, TallyReports, parse_report_xml, report_request,
    split_totals,
};

fn d(y: i32, m: Month, day: u8) -> Date {
    Date::from_calendar_date(y, m, day).unwrap()
}

const TB: &str = r"<ENVELOPE>
  <DSPACCNAME><DSPDISPNAME>Cash</DSPDISPNAME><GUID>g-cash</GUID></DSPACCNAME>
  <DSPACCINFO><DSPCLDRAMT><DSPCLDRAMTA>-50000.00</DSPCLDRAMTA></DSPCLDRAMT>
    <DSPCLCRAMT><DSPCLCRAMTA></DSPCLCRAMTA></DSPCLCRAMT></DSPACCINFO>
  <DSPACCNAME><DSPDISPNAME>Capital</DSPDISPNAME></DSPACCNAME>
  <DSPACCINFO><DSPCLDRAMT><DSPCLDRAMTA></DSPCLDRAMTA></DSPCLDRAMT>
    <DSPCLCRAMT><DSPCLCRAMTA>30000.00</DSPCLCRAMTA></DSPCLCRAMT></DSPACCINFO>
  <DSPACCNAME><DSPDISPNAME>Sales</DSPDISPNAME></DSPACCNAME>
  <DSPACCINFO><DSPCLCRAMT><DSPCLCRAMTA>20000.00</DSPCLCRAMTA></DSPCLCRAMT></DSPACCINFO>
  <DSPACCNAME><DSPDISPNAME>Grand Total</DSPDISPNAME></DSPACCNAME>
  <DSPACCINFO><DSPCLDRAMT><DSPCLDRAMTA>-50000.00</DSPCLDRAMTA></DSPCLDRAMT>
    <DSPCLCRAMT><DSPCLCRAMTA>50000.00</DSPCLCRAMTA></DSPCLCRAMT></DSPACCINFO>
</ENVELOPE>";

fn rows(xml: &str, kind: TallyReportKind) -> Vec<TReportRow> {
    let (rows, warnings) = parse_report_xml(xml, kind).unwrap();
    assert_eq!(warnings.len(), 0, "unexpected warnings: {warnings:?}");
    rows
}

#[test]
fn request_names_report_company_and_period() {
    let xml = report_request(
        "Ghosh & Sons",
        TallyReportKind::TrialBalance,
        d(2026, Month::April, 1),
        d(2027, Month::March, 31),
    );
    // derived: R8 2.3(a) request shape; company text escaped, dates YYYYMMDD.
    assert!(xml.contains("<TALLYREQUEST>Export</TALLYREQUEST>"));
    assert!(xml.contains("<ID>Trial Balance</ID>"));
    assert!(xml.contains("<SVCURRENTCOMPANY>Ghosh &amp; Sons</SVCURRENTCOMPANY>"));
    assert!(xml.contains("<SVFROMDATE>20260401</SVFROMDATE>"));
    assert!(xml.contains("<SVTODATE>20270331</SVTODATE>"));
    assert!(xml.contains("$$SysName:XML"));
}

#[test]
fn request_is_well_formed_for_every_kind() {
    for kind in [
        TallyReportKind::TrialBalance,
        TallyReportKind::BalanceSheet,
        TallyReportKind::ProfitAndLoss,
        TallyReportKind::StockSummary,
        TallyReportKind::BillsReceivable,
        TallyReportKind::BillsPayable,
    ] {
        let xml = report_request(
            "A<B",
            kind,
            d(2026, Month::April, 1),
            d(2026, Month::April, 30),
        );
        // derived: the request is itself XML, so a document parser accepts it.
        let (_, warnings) = parse_report_xml(&xml, kind).unwrap();
        // The request has no report rows: its HEADER and BODY children are reported, not dropped.
        assert_eq!(warnings.len(), 2, "{kind:?}");
    }
}

#[test]
fn trial_balance_rows_and_totals_are_kept_apart() {
    let all = rows(TB, TallyReportKind::TrialBalance);
    assert_eq!(all.len(), 4);
    let (details, totals) = split_totals(&all);
    assert_eq!(details.len(), 3);
    assert_eq!(totals.len(), 1);
    let labels: Vec<&str> = details.iter().map(|r| r.label.as_str()).collect();
    assert_eq!(labels, ["Cash", "Capital", "Sales"]);
    // pending: the debit-negative column convention is R8 2.3(e) for voucher import lines only
    // (tagged [vendor shape]); its use for report columns is unobserved. Cash closing column is
    // -50000.00 in the debit column; 50000.00 rupees = 5,000,000 paise (arithmetic derived).
    assert_eq!(details[0].dr_cr(), Some((DrCr::Dr, Paise::new(5_000_000))));
    assert_eq!(details[0].guid.as_deref(), Some("g-cash"));
    // derived: Capital 30000.00 in the credit column = 3,000,000 paise Cr.
    assert_eq!(details[1].dr_cr(), Some((DrCr::Cr, Paise::new(3_000_000))));
    assert_eq!(details[1].guid, None);
    assert_eq!(details[2].dr_cr(), Some((DrCr::Cr, Paise::new(2_000_000))));
    // derived: the totals row is the Grand Total fixture row: Dr 5,000,000, Cr 5,000,000 paise.
    assert!(totals[0].is_total);
    assert_eq!(
        totals[0].amount,
        TReportAmount::Columns {
            debit: Paise::new(5_000_000),
            credit: Paise::new(5_000_000)
        }
    );
    // derived arithmetic: details Cr 3,000,000 + 2,000,000 = 5,000,000 = Dr 5,000,000 = total.
    let credit_sum = details.iter().fold(0i64, |acc, r| match r.dr_cr() {
        Some((DrCr::Cr, p)) => acc + p.get(),
        Some((DrCr::Dr, _)) | None => acc,
    });
    assert_eq!(credit_sum, 5_000_000);
}

#[test]
fn trial_balance_empty_columns_are_nil() {
    let xml = "<ENVELOPE><DSPACCNAME><DSPDISPNAME>Idle</DSPDISPNAME></DSPACCNAME>\
               <DSPACCINFO><DSPCLDRAMT><DSPCLDRAMTA></DSPCLDRAMTA></DSPCLDRAMT></DSPACCINFO></ENVELOPE>";
    let all = rows(xml, TallyReportKind::TrialBalance);
    // derived: no amount text in either column means the row carries no amount.
    assert_eq!(all[0].amount, TReportAmount::Nil);
    assert_eq!(all[0].dr_cr(), None);
}

#[test]
fn trial_balance_sign_that_contradicts_its_column_is_an_error() {
    // pending: the sign rule applied here is R8 2.3(e), documented for voucher import lines only;
    // for report columns it awaits an observed capture. The refusal is what this code does.
    let dr = "<ENVELOPE><DSPACCNAME><DSPDISPNAME>X</DSPDISPNAME></DSPACCNAME><DSPACCINFO>\
              <DSPCLDRAMT><DSPCLDRAMTA>10.00</DSPCLDRAMTA></DSPCLDRAMT></DSPACCINFO></ENVELOPE>";
    assert!(matches!(
        parse_report_xml(dr, TallyReportKind::TrialBalance),
        Err(TallyXmlError::Value { .. })
    ));
    let cr = "<ENVELOPE><DSPACCNAME><DSPDISPNAME>X</DSPDISPNAME></DSPACCNAME><DSPACCINFO>\
              <DSPCLCRAMT><DSPCLCRAMTA>-10.00</DSPCLCRAMTA></DSPCLCRAMT></DSPACCINFO></ENVELOPE>";
    assert!(matches!(
        parse_report_xml(cr, TallyReportKind::TrialBalance),
        Err(TallyXmlError::Value { .. })
    ));
}

#[test]
fn trial_balance_detail_with_both_sides_is_an_error_but_total_may() {
    let xml = "<ENVELOPE><DSPACCNAME><DSPDISPNAME>X</DSPDISPNAME></DSPACCNAME><DSPACCINFO>\
               <DSPCLDRAMT><DSPCLDRAMTA>-1.00</DSPCLDRAMTA></DSPCLDRAMT>\
               <DSPCLCRAMT><DSPCLCRAMTA>2.00</DSPCLCRAMTA></DSPCLCRAMT></DSPACCINFO></ENVELOPE>";
    assert!(matches!(
        parse_report_xml(xml, TallyReportKind::TrialBalance),
        Err(TallyXmlError::Value { .. })
    ));
    // The same shape under the label "Total" is a totals row and is accepted (see TB above).
    let total = xml.replace(">X<", ">Total<");
    let all = rows(&total, TallyReportKind::TrialBalance);
    assert!(all[0].is_total);
}

#[test]
fn amounts_that_are_not_exact_paise_are_refused() {
    // derived: 10.005 rupees is 1000.5 paise, which cannot be held exactly.
    let xml = "<ENVELOPE><DSPACCNAME><DSPDISPNAME>X</DSPDISPNAME></DSPACCNAME><DSPACCINFO>\
               <DSPCLCRAMT><DSPCLCRAMTA>10.005</DSPCLCRAMTA></DSPCLCRAMT></DSPACCINFO></ENVELOPE>";
    assert!(matches!(
        parse_report_xml(xml, TallyReportKind::TrialBalance),
        Err(TallyXmlError::Value { .. })
    ));
}

const BS: &str = r"<ENVELOPE>
  <BSNAME><DSPACCNAME><DSPDISPNAME>Capital Account</DSPDISPNAME></DSPACCNAME></BSNAME>
  <BSAMT><BSSUBAMT></BSSUBAMT><BSMAINAMT>30000.00</BSMAINAMT></BSAMT>
  <BSNAME><DSPACCNAME><DSPDISPNAME>Cash</DSPDISPNAME></DSPACCNAME></BSNAME>
  <BSAMT><BSSUBAMT>-50000.00</BSSUBAMT><BSMAINAMT></BSMAINAMT></BSAMT>
  <BSNAME><DSPACCNAME><DSPDISPNAME>Total</DSPDISPNAME></DSPACCNAME></BSNAME>
  <BSAMT><BSMAINAMT>50000.00</BSMAINAMT></BSAMT>
</ENVELOPE>";

#[test]
fn balance_sheet_keeps_sign_as_written_and_flags_total() {
    let all = rows(BS, TallyReportKind::BalanceSheet);
    assert_eq!(all.len(), 3);
    // derived: 30000.00 = 3,000,000 paise, sign as written (positive); no side is inferred.
    assert_eq!(all[0].amount, TReportAmount::Signed(Paise::new(3_000_000)));
    assert_eq!(all[1].amount, TReportAmount::Signed(Paise::new(-5_000_000)));
    assert_eq!(all[0].dr_cr(), None);
    let (details, totals) = split_totals(&all);
    assert_eq!(details.len(), 2);
    assert_eq!(totals.len(), 1);
    assert_eq!(totals[0].label, "Total");
}

#[test]
fn profit_and_loss_accepts_its_own_amount_tags() {
    let xml = "<ENVELOPE><BSNAME><DSPACCNAME><DSPDISPNAME>Sales</DSPDISPNAME></DSPACCNAME></BSNAME>\
               <PLAMT><PLSUBAMT>20000.00</PLSUBAMT></PLAMT></ENVELOPE>";
    let all = rows(xml, TallyReportKind::ProfitAndLoss);
    // derived: 20000.00 rupees = 2,000,000 paise as written.
    assert_eq!(all[0].amount, TReportAmount::Signed(Paise::new(2_000_000)));
}

#[test]
fn balance_sheet_row_with_main_and_sub_amount_is_an_error() {
    let xml = "<ENVELOPE><BSNAME><DSPACCNAME><DSPDISPNAME>G</DSPDISPNAME></DSPACCNAME></BSNAME>\
               <BSAMT><BSSUBAMT>1.00</BSSUBAMT><BSMAINAMT>2.00</BSMAINAMT></BSAMT></ENVELOPE>";
    assert!(matches!(
        parse_report_xml(xml, TallyReportKind::BalanceSheet),
        Err(TallyXmlError::Value { .. })
    ));
}

#[test]
fn stock_summary_closing_values() {
    let xml = "<ENVELOPE>\
        <DSPACCNAME><DSPDISPNAME>Widget</DSPDISPNAME></DSPACCNAME>\
        <DSPSTKINFO><DSPSTKCL><DSPCLAMTA>1200.50</DSPCLAMTA></DSPSTKCL></DSPSTKINFO>\
        <DSPACCNAME><DSPDISPNAME>Grand Total</DSPDISPNAME></DSPACCNAME>\
        <DSPSTKINFO><DSPSTKCL><DSPCLAMTA>1200.50</DSPCLAMTA></DSPSTKCL></DSPSTKINFO>\
        </ENVELOPE>";
    let all = rows(xml, TallyReportKind::StockSummary);
    // derived: 1200.50 rupees = 120,050 paise.
    assert_eq!(all[0].amount, TReportAmount::Signed(Paise::new(120_050)));
    assert!(!all[0].is_total);
    assert!(all[1].is_total);
}

#[test]
fn bills_outstanding_rows_carry_party_and_date_text() {
    let xml = "<ENVELOPE>\
        <BILLFIXED><BILLDATE>1-Apr-26</BILLDATE><BILLREF>INV-7</BILLREF>\
        <BILLPARTY>Sharma Hardware</BILLPARTY></BILLFIXED><BILLCL>-11800.00</BILLCL>\
        </ENVELOPE>";
    let all = rows(xml, TallyReportKind::BillsReceivable);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].label, "INV-7");
    assert_eq!(all[0].party.as_deref(), Some("Sharma Hardware"));
    assert_eq!(all[0].date_text.as_deref(), Some("1-Apr-26"));
    // derived: 11800.00 rupees = 1,180,000 paise, sign as written (negative).
    assert_eq!(all[0].amount, TReportAmount::Signed(Paise::new(-1_180_000)));
    assert!(!all[0].is_total);
}

#[test]
fn unmodelled_elements_are_reported_not_dropped() {
    // derived: rule A9, a dropped field is a reported warning.
    let xml = "<ENVELOPE><SOMETHING/><DSPACCNAME><DSPDISPNAME>Cash</DSPDISPNAME></DSPACCNAME>\
               <DSPACCINFO><DSPOPDRAMT><DSPOPDRAMTA>-1.00</DSPOPDRAMTA></DSPOPDRAMT>\
               <DSPCLDRAMT><DSPCLDRAMTA>-2.00</DSPCLDRAMTA></DSPCLDRAMT></DSPACCINFO></ENVELOPE>";
    let (all, warnings) = parse_report_xml(xml, TallyReportKind::TrialBalance).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].dr_cr(), Some((DrCr::Dr, Paise::new(200))));
    assert_eq!(warnings.len(), 2);
}

#[test]
fn structural_problems_are_errors() {
    // A name without its amount element.
    let no_info = "<ENVELOPE><DSPACCNAME><DSPDISPNAME>X</DSPDISPNAME></DSPACCNAME></ENVELOPE>";
    assert!(matches!(
        parse_report_xml(no_info, TallyReportKind::TrialBalance),
        Err(TallyXmlError::Missing { .. })
    ));
    // An amount element with no name before it.
    let orphan = "<ENVELOPE><DSPACCINFO/></ENVELOPE>";
    assert!(matches!(
        parse_report_xml(orphan, TallyReportKind::TrialBalance),
        Err(TallyXmlError::Missing { .. })
    ));
    // A row name with no text.
    let blank = "<ENVELOPE><DSPACCNAME><DSPDISPNAME/></DSPACCNAME><DSPACCINFO/></ENVELOPE>";
    assert!(matches!(
        parse_report_xml(blank, TallyReportKind::TrialBalance),
        Err(TallyXmlError::Missing { .. })
    ));
    // Not an envelope (for example Tally's error response) and an empty document.
    assert!(matches!(
        parse_report_xml("<RESPONSE>x</RESPONSE>", TallyReportKind::TrialBalance),
        Err(TallyXmlError::Missing { .. })
    ));
    assert!(matches!(
        parse_report_xml("", TallyReportKind::TrialBalance),
        Err(TallyXmlError::Missing { .. })
    ));
}

#[test]
fn truncated_and_malformed_documents_are_errors() {
    let cut = &TB[..TB.len() - 40];
    assert!(matches!(
        parse_report_xml(cut, TallyReportKind::TrialBalance),
        Err(TallyXmlError::Truncated { .. } | TallyXmlError::Xml { .. })
    ));
    assert!(matches!(
        parse_report_xml("<ENVELOPE></WRONG>", TallyReportKind::TrialBalance),
        Err(TallyXmlError::Xml { .. })
    ));
}

#[test]
fn empty_report_is_an_empty_list() {
    let (all, warnings) =
        parse_report_xml("<ENVELOPE></ENVELOPE>", TallyReportKind::StockSummary).unwrap();
    assert_eq!(all.len(), 0);
    assert_eq!(warnings.len(), 0);
}

#[test]
fn tally_reports_holds_the_five_lists() {
    let reports = TallyReports {
        tb: rows(TB, TallyReportKind::TrialBalance),
        ..TallyReports::default()
    };
    assert_eq!(reports.tb.len(), 4);
    assert_eq!(reports.bs.len(), 0);
    assert_eq!(reports.pl.len(), 0);
    assert_eq!(reports.stock.len(), 0);
    assert_eq!(reports.outstanding.len(), 0);
}
