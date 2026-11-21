//! Card B3 tests: staging and mapping to events.
//!
//! Provenance. Every fixture is a fictional, hand-written document in the shape of the import
//! envelope in `docs/research/08-data-interop-editions.md` section 2.3 (vendor shape); none was
//! captured from a TallyPrime release, so no expected value here is `observed`. Every expected
//! value is `derived`, and the derivation is written next to it:
//!
//! * names of the reserved groups and their identity come from the reserved-group seed (research
//!   R1, "Reserved groups (28)"); a ledger "maps to a `ReservedGroup`" when its group id is the
//!   id of the seed group carrying that `ReservedGroup` variant;
//! * the amounts are the literals typed into the XML fixture, times 100 for paise;
//! * ids come from the allocation rule in the module doc: custom groups from 29 (28 reserved),
//!   every other kind from 1, in the order the objects are met.
//!
//! The `RESERVEDNAME`, `ISCANCELLED`, `ISOPTIONAL`, `ISPOSTDATED`, `ISBILLWISEON` and
//! `OPENINGBALANCE` tag names are not in R8: they stay `pending` an `observed` capture.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::wildcard_enum_match_arm,
    clippy::match_wildcard_for_single_variants,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::{Date, DrCr, GroupId, LedgerId, Month, Paise, TallyRef, VoucherTypeId};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{
    BaseVoucherKind, BillRefKind, Group, Ledger, MasterSnapshot, ReservedGroup, Voucher,
    VoucherMode, VoucherStatus, VoucherType,
};
use ledgerly_store::Event;
use ledgerly_tally_bridge::mapping::{MapError, map_to_events, map_with_warnings};
use ledgerly_tally_xml::masters::{TallyMaster, parse_masters_text};
use ledgerly_tally_xml::vouchers::{TVoucher, parse_vouchers_xml};

fn wrap(body: &str) -> String {
    format!(
        "<ENVELOPE><HEADER><VERSION>1</VERSION></HEADER><BODY><DATA>\
         <TALLYMESSAGE xmlns:UDF=\"TallyUDF\">{body}</TALLYMESSAGE></DATA></BODY></ENVELOPE>"
    )
}

fn masters(body: &str) -> Vec<TallyMaster> {
    parse_masters_text(&wrap(body)).unwrap().masters
}

fn vouchers(body: &str) -> Vec<TVoucher> {
    parse_vouchers_xml(&wrap(body)).unwrap().0
}

fn group_xml(name: &str, guid: &str, alter: u64, parent: &str, extra: &str) -> String {
    format!(
        "<GROUP NAME=\"{name}\"><GUID>{guid}</GUID><ALTERID>{alter}</ALTERID><NAME>{name}</NAME>\
         <PARENT>{parent}</PARENT>{extra}</GROUP>"
    )
}

fn ledger_xml(name: &str, guid: &str, parent: &str, extra: &str) -> String {
    format!(
        "<LEDGER NAME=\"{name}\"><GUID>{guid}</GUID><ALTERID>1</ALTERID><NAME>{name}</NAME>\
         <PARENT>{parent}</PARENT>{extra}</LEDGER>"
    )
}

fn vtype_xml(name: &str, guid: &str, parent: &str) -> String {
    format!(
        "<VOUCHERTYPE NAME=\"{name}\"><GUID>{guid}</GUID><ALTERID>1</ALTERID><NAME>{name}</NAME>\
         <PARENT>{parent}</PARENT></VOUCHERTYPE>"
    )
}

fn seed(kind: ReservedGroup) -> Group {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(kind))
        .unwrap()
}

fn as_ledger(e: &Event) -> &Ledger {
    match e {
        Event::MasterCreated(MasterSnapshot::Ledger(l)) => l,
        other => panic!("not a ledger event: {other:?}"),
    }
}

fn as_created_group(e: &Event) -> &Group {
    match e {
        Event::MasterCreated(MasterSnapshot::Group(g)) => g,
        other => panic!("not a created group: {other:?}"),
    }
}

fn as_voucher(e: &Event) -> &Voucher {
    match e {
        Event::VoucherPosted(v) => v,
        other => panic!("not a posted voucher: {other:?}"),
    }
}

fn as_vtype(e: &Event) -> &VoucherType {
    match e {
        Event::MasterCreated(MasterSnapshot::VoucherType(t)) => t,
        other => panic!("not a voucher type: {other:?}"),
    }
}

fn tref(guid: &str, alter: u64) -> TallyRef {
    TallyRef {
        guid: guid.to_owned(),
        alter_id: Some(alter),
    }
}

// ---------------------------------------------------------------------------------------------
// Groups and reserved names
// ---------------------------------------------------------------------------------------------

#[test]
fn ledger_under_reserved_group_maps_to_the_reserved_group_by_name() {
    let m = masters(&format!(
        "{}{}",
        group_xml("Sundry Debtors", "g-sd", 11, "Current Assets", ""),
        ledger_xml("Sharma Hardware", "g-led", "Sundry Debtors", "")
    ));
    let events = map_to_events(&m, &[]).unwrap();
    // derived: two events, the reserved group altered (never created again), then the ledger.
    assert_eq!(events.len(), 2);
    let expected = seed(ReservedGroup::SundryDebtors);
    assert_eq!(as_ledger(&events[1]).group, expected.id);
    let Event::MasterAltered { before, after } = &events[0] else {
        panic!("first event is not an alteration: {:?}", events[0]);
    };
    assert_eq!(before, &MasterSnapshot::Group(expected.clone()));
    let MasterSnapshot::Group(after) = after else {
        panic!("not a group snapshot");
    };
    assert_eq!(after.id, expected.id);
    assert_eq!(after.reserved, Some(ReservedGroup::SundryDebtors));
    assert_eq!(after.tally, Some(tref("g-sd", 11)));
}

#[test]
fn ledger_under_reserved_group_missing_from_export_still_resolves() {
    // derived: every company has the 28 reserved groups, so an export that omits one is fine.
    let m = masters(&ledger_xml(
        "Sharma Hardware",
        "g-led",
        "Sundry Debtors",
        "",
    ));
    let events = map_to_events(&m, &[]).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        as_ledger(&events[0]).group,
        seed(ReservedGroup::SundryDebtors).id
    );
}

#[test]
fn renamed_reserved_group_is_found_by_reserved_name() {
    // The user renamed "Sundry Debtors" to "Customers"; Tally keeps its reserved name.
    let m = masters(&format!(
        "{}{}",
        group_xml(
            "Customers",
            "g-sd",
            3,
            "Current Assets",
            "<RESERVEDNAME>Sundry Debtors</RESERVEDNAME>"
        ),
        ledger_xml("Sharma Hardware", "g-led", "Customers", "")
    ));
    let events = map_to_events(&m, &[]).unwrap();
    let expected = seed(ReservedGroup::SundryDebtors);
    assert_eq!(as_ledger(&events[1]).group, expected.id);
    let Event::MasterAltered { after, .. } = &events[0] else {
        panic!("not an alteration");
    };
    let MasterSnapshot::Group(after) = after else {
        panic!("not a group snapshot");
    };
    // derived: the group keeps its reserved identity and takes the user's name.
    assert_eq!(after.name, "Customers");
    assert_eq!(after.reserved, Some(ReservedGroup::SundryDebtors));
}

#[test]
fn old_name_of_a_renamed_reserved_group_no_longer_resolves() {
    let m = masters(&format!(
        "{}{}",
        group_xml(
            "Customers",
            "g-sd",
            3,
            "Current Assets",
            "<RESERVEDNAME>Sundry Debtors</RESERVEDNAME>"
        ),
        ledger_xml("Sharma Hardware", "g-led", "Sundry Debtors", "")
    ));
    // derived: after the rename Tally has no group called "Sundry Debtors" any more.
    assert_eq!(
        map_to_events(&m, &[]).unwrap_err(),
        MapError::UnknownParent {
            kind: "ledger",
            name: "Sharma Hardware".to_owned(),
            parent: "Sundry Debtors".to_owned(),
        }
    );
}

#[test]
fn unknown_reserved_name_is_an_error() {
    let m = masters(&group_xml(
        "Odd",
        "g-1",
        1,
        "Current Assets",
        "<RESERVEDNAME>No Such Group</RESERVEDNAME>",
    ));
    assert_eq!(
        map_to_events(&m, &[]).unwrap_err(),
        MapError::UnknownReservedName {
            group: "Odd".to_owned(),
            reserved_name: "No Such Group".to_owned(),
        }
    );
}

#[test]
fn reserved_group_under_another_parent_is_an_error() {
    // derived: R1 puts Sundry Debtors under Current Assets, not Current Liabilities.
    let m = masters(&group_xml(
        "Sundry Debtors",
        "g-1",
        1,
        "Current Liabilities",
        "",
    ));
    assert_eq!(
        map_to_events(&m, &[]).unwrap_err(),
        MapError::ReservedGroupMoved {
            group: "Sundry Debtors".to_owned(),
        }
    );
}

#[test]
fn ledger_with_unknown_parent_is_an_error() {
    let m = masters(&ledger_xml("Sharma Hardware", "g-led", "No Such Group", ""));
    assert_eq!(
        map_to_events(&m, &[]).unwrap_err(),
        MapError::UnknownParent {
            kind: "ledger",
            name: "Sharma Hardware".to_owned(),
            parent: "No Such Group".to_owned(),
        }
    );
}

#[test]
fn group_with_unknown_parent_is_an_error() {
    let m = masters(&group_xml("Local Debtors", "g-1", 1, "Nowhere", ""));
    assert_eq!(
        map_to_events(&m, &[]).unwrap_err(),
        MapError::UnknownParent {
            kind: "group",
            name: "Local Debtors".to_owned(),
            parent: "Nowhere".to_owned(),
        }
    );
}

#[test]
fn groups_that_name_each_other_are_a_cycle() {
    let m = masters(&format!(
        "{}{}",
        group_xml("A", "g-a", 1, "B", ""),
        group_xml("B", "g-b", 1, "A", "")
    ));
    assert_eq!(
        map_to_events(&m, &[]).unwrap_err(),
        MapError::GroupCycle {
            name: "A".to_owned()
        }
    );
}

#[test]
fn custom_groups_are_created_parents_first_whatever_the_file_order() {
    // The child is listed before its parent.
    let m = masters(&format!(
        "{}{}{}",
        group_xml("Retail Debtors", "g-r", 2, "Local Debtors", ""),
        group_xml("Local Debtors", "g-l", 1, "Sundry Debtors", ""),
        ledger_xml("Sharma Hardware", "g-led", "Retail Debtors", "")
    ));
    let events = map_to_events(&m, &[]).unwrap();
    assert_eq!(events.len(), 3);
    // derived: ids from 29 (28 reserved). Pass 1 creates Local Debtors (its parent is reserved)
    // = 29; Retail Debtors waits for it and is created in pass 2 = 30.
    let local = as_created_group(&events[0]);
    let retail = as_created_group(&events[1]);
    assert_eq!(
        (local.name.as_str(), local.id),
        ("Local Debtors", GroupId(29))
    );
    assert_eq!(
        (retail.name.as_str(), retail.id),
        ("Retail Debtors", GroupId(30))
    );
    assert_eq!(local.parent, Some(seed(ReservedGroup::SundryDebtors).id));
    assert_eq!(retail.parent, Some(GroupId(29)));
    assert_eq!(as_ledger(&events[2]).group, GroupId(30));
}

#[test]
fn custom_primary_group_is_unsupported_not_guessed() {
    let m = masters(&group_xml("Odd Primary", "g-1", 1, "", ""));
    assert!(matches!(
        map_to_events(&m, &[]).unwrap_err(),
        MapError::Unsupported { .. }
    ));
}

#[test]
fn duplicate_names_and_guids_are_errors() {
    let dup_name = masters(&format!(
        "{}{}",
        ledger_xml("Cash Box", "g-1", "Sundry Debtors", ""),
        ledger_xml("Cash Box", "g-2", "Sundry Debtors", "")
    ));
    assert_eq!(
        map_to_events(&dup_name, &[]).unwrap_err(),
        MapError::DuplicateName {
            kind: "ledger",
            name: "Cash Box".to_owned(),
        }
    );
    let dup_guid = masters(&format!(
        "{}{}",
        ledger_xml("One", "g-same", "Sundry Debtors", ""),
        ledger_xml("Two", "g-same", "Sundry Debtors", "")
    ));
    assert_eq!(
        map_to_events(&dup_guid, &[]).unwrap_err(),
        MapError::DuplicateGuid {
            guid: "g-same".to_owned(),
        }
    );
}

// ---------------------------------------------------------------------------------------------
// Ledgers
// ---------------------------------------------------------------------------------------------

#[test]
fn guid_and_alter_id_are_kept_in_tally_ref() {
    let m = masters(&format!(
        "{}{}{}",
        group_xml("Local Debtors", "guid-grp", 7, "Sundry Debtors", ""),
        ledger_xml("Sharma Hardware", "guid-led", "Local Debtors", ""),
        vtype_xml("Sales", "guid-vt", "Sales")
    ));
    let events = map_to_events(&m, &[]).unwrap();
    // derived: the literals typed in the fixtures (ledger and voucher type ALTERID is 1).
    assert_eq!(
        as_created_group(&events[0]).tally,
        Some(tref("guid-grp", 7))
    );
    assert_eq!(as_ledger(&events[1]).tally, Some(tref("guid-led", 1)));
    assert_eq!(as_vtype(&events[2]).tally, Some(tref("guid-vt", 1)));
}

#[test]
fn ledger_flags_and_zero_opening_are_read() {
    let m = masters(&ledger_xml(
        "Sharma Hardware",
        "g-1",
        "Sundry Debtors",
        "<OPENINGBALANCE>0.00</OPENINGBALANCE><ISBILLWISEON>Yes</ISBILLWISEON>\
         <ISCOSTCENTRESON>No</ISCOSTCENTRESON>",
    ));
    let mapped = map_with_warnings(&m, &[]).unwrap();
    let l = as_ledger(&mapped.events[0]);
    assert!(l.bill_wise.is_some());
    assert!(!l.cost_centres_applicable);
    assert_eq!(l.opening.amount, Paise::ZERO);
    // derived: all three tags were read, so nothing is reported as dropped.
    assert_eq!(
        mapped.warnings,
        [] as [ledgerly_tally_bridge::mapping::MapWarning; 0]
    );
}

#[test]
fn nonzero_opening_balance_is_unsupported() {
    let m = masters(&ledger_xml(
        "Sharma Hardware",
        "g-1",
        "Sundry Debtors",
        "<OPENINGBALANCE>-5000.00</OPENINGBALANCE>",
    ));
    assert!(matches!(
        map_to_events(&m, &[]).unwrap_err(),
        MapError::Unsupported { .. }
    ));
}

#[test]
fn unmodelled_ledger_field_is_a_warning_not_silent() {
    let m = masters(&ledger_xml(
        "Sharma Hardware",
        "g-1",
        "Sundry Debtors",
        "<LEDGERPHONE>000</LEDGERPHONE>",
    ));
    let mapped = map_with_warnings(&m, &[]).unwrap();
    assert_eq!(mapped.warnings.len(), 1);
    assert_eq!(mapped.warnings[0].object, "ledger 'Sharma Hardware'");
    assert!(mapped.warnings[0].message.contains("LEDGERPHONE"));
}

#[test]
fn ledger_without_group_is_an_error_except_the_reserved_profit_and_loss() {
    let m = masters(&ledger_xml("Stray", "g-1", "", ""));
    assert_eq!(
        map_to_events(&m, &[]).unwrap_err(),
        MapError::LedgerWithoutGroup {
            ledger: "Stray".to_owned()
        }
    );
    let pl = masters(&ledger_xml("Profit & Loss A/c", "g-2", "", ""));
    let mapped = map_with_warnings(&pl, &[]).unwrap();
    assert_eq!(mapped.events, [] as [ledgerly_store::Event; 0]);
    assert_eq!(mapped.warnings.len(), 1);
}

#[test]
fn root_marker_in_parent_means_primary() {
    // Tally writes the root as a control character then "Primary"; here a plain "Primary".
    let m = masters(&group_xml("Sundry Debtors", "g-sd", 1, "Primary", ""));
    // derived: a reserved group that is not primary in Tally is a moved group (Sundry Debtors
    // has the parent Current Assets in R1), so this is refused rather than mapped.
    assert!(matches!(
        map_to_events(&m, &[]).unwrap_err(),
        MapError::ReservedGroupMoved { .. }
    ));
    let primary = masters(&group_xml("Current Assets", "g-ca", 1, "Primary", ""));
    assert_eq!(map_to_events(&primary, &[]).unwrap().len(), 1);
}

// ---------------------------------------------------------------------------------------------
// Voucher types and other masters
// ---------------------------------------------------------------------------------------------

#[test]
fn voucher_type_base_comes_from_parent_or_name() {
    let m = masters(&format!(
        "{}{}{}",
        vtype_xml("Sales", "g-1", "Sales"),
        vtype_xml("Sales - Mumbai", "g-2", "Sales"),
        vtype_xml("Contra", "g-3", "")
    ));
    let events = map_to_events(&m, &[]).unwrap();
    assert_eq!(as_vtype(&events[0]).base, BaseVoucherKind::Sales);
    assert_eq!(as_vtype(&events[1]).base, BaseVoucherKind::Sales);
    assert_eq!(as_vtype(&events[1]).name, "Sales - Mumbai");
    assert_eq!(as_vtype(&events[2]).base, BaseVoucherKind::Contra);
    // derived: ids from 1 in the order met.
    assert_eq!(as_vtype(&events[1]).id, VoucherTypeId(2));
}

#[test]
fn voucher_type_with_unknown_base_is_unsupported() {
    let m = masters(&vtype_xml("Payroll", "g-1", "Payroll"));
    assert!(matches!(
        map_to_events(&m, &[]).unwrap_err(),
        MapError::Unsupported { .. }
    ));
}

#[test]
fn unsupported_master_kinds_are_reported() {
    let m =
        masters("<UNIT NAME=\"Nos\"><GUID>g-u</GUID><ALTERID>1</ALTERID><NAME>Nos</NAME></UNIT>");
    let mapped = map_with_warnings(&m, &[]).unwrap();
    assert_eq!(mapped.events, [] as [ledgerly_store::Event; 0]);
    assert_eq!(mapped.warnings.len(), 1);
    assert_eq!(mapped.warnings[0].object, "unit 'Nos'");
}

// ---------------------------------------------------------------------------------------------
// Vouchers
// ---------------------------------------------------------------------------------------------

const BASE_MASTERS: &str = "\
<LEDGER NAME=\"Sharma Hardware\"><GUID>g-l1</GUID><ALTERID>1</ALTERID><NAME>Sharma Hardware</NAME>\
<PARENT>Sundry Debtors</PARENT><ISBILLWISEON>Yes</ISBILLWISEON></LEDGER>\
<LEDGER NAME=\"Sales @18%\"><GUID>g-l2</GUID><ALTERID>1</ALTERID><NAME>Sales @18%</NAME>\
<PARENT>Sales Accounts</PARENT></LEDGER>\
<LEDGER NAME=\"Output IGST\"><GUID>g-l3</GUID><ALTERID>1</ALTERID><NAME>Output IGST</NAME>\
<PARENT>Duties &amp; Taxes</PARENT></LEDGER>\
<VOUCHERTYPE NAME=\"Sales\"><GUID>g-vt</GUID><ALTERID>1</ALTERID><NAME>Sales</NAME>\
<PARENT>Sales</PARENT></VOUCHERTYPE>";

/// R8 2.3(e) sales voucher: 11,800.00 Dr party, 10,000.00 Cr sales, 1,800.00 Cr IGST.
const SALES: &str = "\
<VOUCHER VCHTYPE=\"Sales\" ACTION=\"Create\"><GUID>guid-v1</GUID><ALTERID>42</ALTERID>\
<DATE>20261115</DATE><VOUCHERTYPENAME>Sales</VOUCHERTYPENAME>\
<VOUCHERNUMBER>S-0042</VOUCHERNUMBER><PARTYLEDGERNAME>Sharma Hardware</PARTYLEDGERNAME>\
<NARRATION>Bolts &amp; nuts</NARRATION><PERSISTEDVIEW>Accounting Voucher View</PERSISTEDVIEW>\
<ALLLEDGERENTRIES.LIST><LEDGERNAME>Sharma Hardware</LEDGERNAME>\
<ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE><AMOUNT>-11800.00</AMOUNT>\
<BILLALLOCATIONS.LIST><NAME>S-0042</NAME><BILLTYPE>New Ref</BILLTYPE>\
<AMOUNT>-11800.00</AMOUNT></BILLALLOCATIONS.LIST></ALLLEDGERENTRIES.LIST>\
<ALLLEDGERENTRIES.LIST><LEDGERNAME>Sales @18%</LEDGERNAME>\
<ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>10000.00</AMOUNT></ALLLEDGERENTRIES.LIST>\
<ALLLEDGERENTRIES.LIST><LEDGERNAME>Output IGST</LEDGERNAME>\
<ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>1800.00</AMOUNT></ALLLEDGERENTRIES.LIST>\
</VOUCHER>";

fn map_sales(voucher_xml: &str) -> Result<Vec<Event>, MapError> {
    map_to_events(&masters(BASE_MASTERS), &vouchers(voucher_xml))
}

fn plain_voucher(guid: &str, flags: &str, lines: &str) -> String {
    format!(
        "<VOUCHER><GUID>{guid}</GUID><ALTERID>5</ALTERID><DATE>20261116</DATE>\
         <VOUCHERTYPENAME>Sales</VOUCHERTYPENAME><VOUCHERNUMBER>S-1</VOUCHERNUMBER>{flags}{lines}\
         </VOUCHER>"
    )
}

fn line(ledger: &str, deemed: &str, amount: &str, inner: &str) -> String {
    format!(
        "<ALLLEDGERENTRIES.LIST><LEDGERNAME>{ledger}</LEDGERNAME>\
         <ISDEEMEDPOSITIVE>{deemed}</ISDEEMEDPOSITIVE><AMOUNT>{amount}</AMOUNT>{inner}\
         </ALLLEDGERENTRIES.LIST>"
    )
}

#[test]
fn sales_voucher_maps_with_lines_bills_and_identity() {
    let events = map_sales(SALES).unwrap();
    // derived: 3 ledgers, 1 voucher type, 1 voucher, in dependency order.
    assert_eq!(events.len(), 5);
    let v = as_voucher(&events[4]);
    assert_eq!(v.id.0, 1);
    assert_eq!(v.type_id, VoucherTypeId(1));
    assert_eq!(v.number.as_deref(), Some("S-0042"));
    assert_eq!(
        v.date,
        Date::from_calendar_date(2026, Month::November, 15).unwrap()
    );
    assert_eq!(v.narration, "Bolts & nuts");
    assert_eq!(v.status, VoucherStatus::Regular);
    assert_eq!(v.mode, VoucherMode::AsVoucher);
    assert_eq!(v.tally, Some(tref("guid-v1", 42)));
    // derived: ledger ids 1, 2, 3 in document order; the party is the first.
    assert_eq!(v.party, Some(LedgerId(1)));
    assert_eq!(v.ledger_lines.len(), 3);
    // derived (R8 2.3(e)): -11800.00 with ISDEEMEDPOSITIVE Yes is a debit of 11,800.00
    // = 1,180,000 paise; 10,000.00 = 1,000,000 paise credit; 1,800.00 = 180,000 paise credit.
    let l = &v.ledger_lines;
    assert_eq!(
        (l[0].ledger, l[0].amount.side, l[0].amount.amount),
        (LedgerId(1), DrCr::Dr, Paise::new(1_180_000))
    );
    assert_eq!(
        (l[1].ledger, l[1].amount.side, l[1].amount.amount),
        (LedgerId(2), DrCr::Cr, Paise::new(1_000_000))
    );
    assert_eq!(
        (l[2].ledger, l[2].amount.side, l[2].amount.amount),
        (LedgerId(3), DrCr::Cr, Paise::new(180_000))
    );
    // derived: the one bill totals the Dr line (-1,180,000 as written), so it is a debit of
    // 1,180,000 paise, kind New Ref.
    assert_eq!(l[0].bills.len(), 1);
    assert_eq!(l[0].bills[0].kind, BillRefKind::New);
    assert_eq!(l[0].bills[0].name, "S-0042");
    assert_eq!(l[0].bills[0].amount.side, DrCr::Dr);
    assert_eq!(l[0].bills[0].amount.amount, Paise::new(1_180_000));
}

#[test]
fn unmodelled_voucher_fields_are_kept_in_extra() {
    let events = map_sales(SALES).unwrap();
    let v = as_voucher(&events[4]);
    // derived: PERSISTEDVIEW and the VOUCHER attributes are in the fixture and not modelled.
    assert_eq!(
        v.extra.get("PERSISTEDVIEW").map(String::as_str),
        Some("Accounting Voucher View")
    );
    assert_eq!(v.extra.get("@VCHTYPE").map(String::as_str), Some("Sales"));
    assert_eq!(v.extra.get("@ACTION").map(String::as_str), Some("Create"));
}

#[test]
fn credit_line_bill_keeps_its_side_and_kind() {
    let xml = plain_voucher(
        "guid-v2",
        "",
        &format!(
            "{}{}",
            line(
                "Sharma Hardware",
                "No",
                "100.00",
                "<BILLALLOCATIONS.LIST><NAME>S-0042</NAME><BILLTYPE>Agst Ref</BILLTYPE>\
                 <AMOUNT>100.00</AMOUNT></BILLALLOCATIONS.LIST>"
            ),
            line("Sales @18%", "Yes", "-100.00", "")
        ),
    );
    let events = map_sales(&xml).unwrap();
    let v = as_voucher(&events[4]);
    // derived: 100.00 credit as written is positive; the bill's 100.00 equals the line, so it is
    // a credit of 10,000 paise against a bill.
    let bill = &v.ledger_lines[0].bills[0];
    assert_eq!(bill.kind, BillRefKind::Against);
    assert_eq!(bill.amount.side, DrCr::Cr);
    assert_eq!(bill.amount.amount, Paise::new(10_000));
}

#[test]
fn bills_that_do_not_total_the_line_are_an_error() {
    let xml = plain_voucher(
        "guid-v3",
        "",
        &line(
            "Sharma Hardware",
            "Yes",
            "-100.00",
            "<BILLALLOCATIONS.LIST><NAME>B1</NAME><BILLTYPE>New Ref</BILLTYPE>\
             <AMOUNT>-50.00</AMOUNT></BILLALLOCATIONS.LIST>",
        ),
    );
    // derived: line -100.00 = -10,000 paise as written; bills -50.00 = -5,000 paise.
    let err = map_sales(&xml).unwrap_err();
    assert!(matches!(
        err,
        MapError::BillSumMismatch {
            allocated: -5_000,
            line: -10_000,
            ..
        }
    ));
}

#[test]
fn unknown_bill_type_is_unsupported() {
    let xml = plain_voucher(
        "guid-v4",
        "",
        &line(
            "Sharma Hardware",
            "Yes",
            "-100.00",
            "<BILLALLOCATIONS.LIST><NAME>B1</NAME><BILLTYPE>Weird</BILLTYPE>\
             <AMOUNT>-100.00</AMOUNT></BILLALLOCATIONS.LIST>",
        ),
    );
    assert!(matches!(
        map_sales(&xml).unwrap_err(),
        MapError::Unsupported { .. }
    ));
}

#[test]
fn voucher_with_unknown_ledger_or_type_or_no_guid_is_an_error() {
    let unknown_ledger = plain_voucher("g-1", "", &line("Nobody", "Yes", "-1.00", ""));
    assert!(matches!(
        map_sales(&unknown_ledger).unwrap_err(),
        MapError::UnknownLedger { ledger, .. } if ledger == "Nobody"
    ));
    let unknown_type =
        plain_voucher("g-2", "", "").replace("Sales</VOUCHERTYPENAME>", "Gone</VOUCHERTYPENAME>");
    assert!(matches!(
        map_sales(&unknown_type).unwrap_err(),
        MapError::UnknownVoucherType { voucher_type, .. } if voucher_type == "Gone"
    ));
    let no_guid =
        "<VOUCHER><DATE>20261116</DATE><VOUCHERTYPENAME>Sales</VOUCHERTYPENAME></VOUCHER>";
    assert!(matches!(
        map_sales(no_guid).unwrap_err(),
        MapError::MissingGuid { .. }
    ));
}

#[test]
fn duplicate_voucher_guid_is_an_error() {
    let both = format!(
        "{}{}",
        plain_voucher("same", "", ""),
        plain_voucher("same", "", "")
    );
    assert_eq!(
        map_sales(&both).unwrap_err(),
        MapError::DuplicateGuid {
            guid: "same".to_owned()
        }
    );
}

#[test]
fn inventory_lines_are_unsupported() {
    let xml = plain_voucher(
        "g-inv",
        "",
        "<ALLINVENTORYENTRIES.LIST><STOCKITEMNAME>Steel Bolt</STOCKITEMNAME>\
         <AMOUNT>-100.00</AMOUNT></ALLINVENTORYENTRIES.LIST>",
    );
    assert!(matches!(
        map_sales(&xml).unwrap_err(),
        MapError::Unsupported { .. }
    ));
}

#[test]
fn status_flags_map_and_conflicts_are_refused() {
    let cancelled = plain_voucher("g-c", "<ISCANCELLED>Yes</ISCANCELLED>", "");
    let events = map_sales(&cancelled).unwrap();
    assert_eq!(as_voucher(&events[4]).status, VoucherStatus::Cancelled);
    let optional = plain_voucher(
        "g-o",
        "<ISOPTIONAL>Yes</ISOPTIONAL><ISCANCELLED>No</ISCANCELLED>",
        "",
    );
    assert_eq!(
        as_voucher(&map_sales(&optional).unwrap()[4]).status,
        VoucherStatus::Optional
    );
    let both = plain_voucher(
        "g-b",
        "<ISCANCELLED>Yes</ISCANCELLED><ISOPTIONAL>Yes</ISOPTIONAL>",
        "",
    );
    assert!(matches!(
        map_sales(&both).unwrap_err(),
        MapError::Unsupported { .. }
    ));
    let bad = plain_voucher("g-x", "<ISCANCELLED>Maybe</ISCANCELLED>", "");
    assert!(matches!(
        map_sales(&bad).unwrap_err(),
        MapError::BadValue { .. }
    ));
}

#[test]
fn invoice_flag_selects_accounting_invoice_mode() {
    let xml = plain_voucher("g-i", "<ISINVOICE>Yes</ISINVOICE>", "");
    let events = map_sales(&xml).unwrap();
    assert_eq!(as_voucher(&events[4]).mode, VoucherMode::AccountingInvoice);
}

#[test]
fn events_are_in_dependency_order_and_deterministic() {
    let m = masters(&format!(
        "{}{}",
        group_xml("Local Debtors", "g-g", 1, "Sundry Debtors", ""),
        BASE_MASTERS
    ));
    let v = vouchers(SALES);
    let first = map_to_events(&m, &v).unwrap();
    let second = map_to_events(&m, &v).unwrap();
    // derived: determinism, rule A7: same input, same output.
    assert_eq!(first, second);
    // derived: groups, then ledgers, then voucher types, then vouchers.
    let rank = |e: &Event| match e {
        Event::MasterCreated(MasterSnapshot::Group(_)) | Event::MasterAltered { .. } => 0,
        Event::MasterCreated(MasterSnapshot::Ledger(_)) => 1,
        Event::MasterCreated(MasterSnapshot::VoucherType(_)) => 2,
        Event::VoucherPosted(_) => 3,
        other => panic!("unexpected event {other:?}"),
    };
    let ranks: Vec<i32> = first.iter().map(rank).collect();
    let mut sorted = ranks.clone();
    sorted.sort_unstable();
    assert_eq!(ranks, sorted);
    assert_eq!(first.len(), 6);
}
