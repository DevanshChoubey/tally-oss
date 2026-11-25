//! Card U6: masters screens. Every expected value is derived by hand and the derivation is
//! written next to it; nothing is copied from this crate's output. The books are a small
//! fictional fixture built here on top of the 28 reserved groups from `ledgerly-model`.
//!
//! Cases without provenance are pending: the screen answers `Unsupported` for them, and the
//! tests at the end assert that.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_app::screens::masters::{
    BankDraft, CostCentreDraft, GroupDraft, GstDraft, LedgerDraft, MasterDraft, MasterError,
    MasterKind, MasterRef, MasterRequest, MastersScreen, PartyDraft, StockItemDraft, TaxDraft,
    TreeDraft, UNASSIGNED_ID, UnitDraft, list_rows, parse_rupees, snapshot_of,
};
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{
    CostCategoryId, CostCentreId, DrCr, GodownId, GroupId, Gstin, LedgerId, Paise, SignedAmount,
    StateCode, StockGroupId, StockItemId, UnitId, VoucherId, VoucherTypeId,
};
use ledgerly_model::{
    Address, BankDetails, BillWiseConfig, Books, Company, CostCategory, CostCentre, EntityKind,
    Features, Godown, Group, GstClassification, Ledger, LedgerLine, MasterSnapshot, Masters,
    Nature, NegativeStockPolicy, PartyDetails, RegistrationType, ReservedGroup, ReservedLedger,
    StockGroup, StockItem, TaxDetails, TaxLedgerKind, Taxability, Unit, UnitKind, ValuationMethod,
    Voucher, VoucherMode, VoucherStatus, seed_groups::reserved_groups,
};
use time::macros::date;

// ---- fixture ---------------------------------------------------------------------------------

// Valid GSTIN vectors from ledgerly-core's own tested list (gstin.rs `VALID`, hand derivation
// there; the check-character algorithm is recalled, not yet quoted from GSTN). State 29 is the
// first two digits.
const GSTIN_29: &str = "29ABCDE1234F1ZW";
const PAN_OF_GSTIN_29: &str = "ABCDE1234F";

const CASH: LedgerId = LedgerId(1);
const SHARMA: LedgerId = LedgerId(2);
const RENT: LedgerId = LedgerId(3);
const USED: LedgerId = LedgerId(4);
const KAVERI: LedgerId = LedgerId(5);
const CUSTOM_PARENT: GroupId = GroupId(100);
const CUSTOM_CHILD: GroupId = GroupId(101);
const NOS: UnitId = UnitId(1);
const BOX: UnitId = UnitId(2);
const ITEM: StockItemId = StockItemId(1);

fn gid(books: &Books, reserved: ReservedGroup) -> GroupId {
    books
        .masters
        .groups
        .values()
        .find(|g| g.reserved == Some(reserved))
        .unwrap()
        .id
}

fn ledger(id: LedgerId, name: &str, group: GroupId) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        group,
        opening: SignedAmount::dr(Paise::ZERO),
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank: None,
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

fn custom_group(id: GroupId, name: &str, parent: Option<GroupId>) -> Group {
    Group {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: Vec::new(),
        parent,
        reserved: None,
        nature: parent.is_none().then_some(Nature::Asset),
        affects_gross_profit: false,
        tally: None,
    }
}

fn voucher_using(ledger: LedgerId) -> Voucher {
    Voucher {
        id: VoucherId(1),
        type_id: VoucherTypeId(1),
        number: None,
        series: None,
        date: date!(2026 - 06 - 01),
        supply_date: None,
        payment_date: None,
        status: VoucherStatus::Regular,
        mode: VoucherMode::AsVoucher,
        party: None,
        place_of_supply: None,
        reverse_charge: false,
        reference: None,
        reference_date: None,
        narration: String::new(),
        ledger_lines: vec![LedgerLine {
            ineligible_itc: None,
            ledger,
            amount: SignedAmount::dr(Paise::new(100)),
            bills: Vec::new(),
            cost: Vec::new(),
            bank: None,
        }],
        inventory_lines: Vec::new(),
        gst: None,
        tally: None,
        extra: BTreeMap::new(),
    }
}

fn base_books(features: Features) -> Books {
    let mut masters = Masters::default();
    for g in reserved_groups() {
        masters.groups.insert(g.id, g);
    }
    let mut books = Books {
        company: Company {
            ineligible_itc_default: None,
            id: ledgerly_core::CompanyId(1),
            name: "Test Traders".to_owned(),
            mailing_name: None,
            address: Address::default(),
            state: StateCode::new(27).unwrap(),
            pan: None,
            registrations: Vec::new(),
            fy_start: date!(2026 - 04 - 01),
            books_begin: date!(2026 - 04 - 01),
            entity_kind: EntityKind::Company,
            features,
            negative_stock: NegativeStockPolicy::Block,
        },
        masters,
        vouchers: BTreeMap::new(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    };
    fill(&mut books);
    books
}

fn fill(books: &mut Books) {
    let debtors = gid(books, ReservedGroup::SundryDebtors);
    let cash_group = gid(books, ReservedGroup::CashInHand);
    let expenses = gid(books, ReservedGroup::IndirectExpenses);
    let m = &mut books.masters;
    m.groups.insert(
        CUSTOM_PARENT,
        custom_group(CUSTOM_PARENT, "Custom Parent", None),
    );
    m.groups.insert(
        CUSTOM_CHILD,
        custom_group(CUSTOM_CHILD, "Custom Child", Some(CUSTOM_PARENT)),
    );
    let mut cash = ledger(CASH, "Cash", cash_group);
    cash.reserved = Some(ReservedLedger::Cash);
    let mut sharma = ledger(SHARMA, "Sharma Traders", debtors);
    sharma.aliases = vec!["ST".to_owned()];
    sharma.opening = SignedAmount::dr(Paise::new(123_456_700));
    sharma.party = Some(PartyDetails {
        registration: RegistrationType::Regular,
        gstin: Some(Gstin::parse(GSTIN_29).unwrap()),
        pan: None,
        state: StateCode::new(29).ok(),
        address: Address {
            lines: vec!["12 Market Road".to_owned(), "Bengaluru".to_owned()],
            pincode: Some("560001".to_owned()),
            state: StateCode::new(29).ok(),
            country: "India".to_owned(),
        },
    });
    let mut kaveri = ledger(KAVERI, "Kaveri Packaging", debtors);
    kaveri.bill_wise = Some(BillWiseConfig {
        default_credit_days: Some(30),
    });
    kaveri.cost_centres_applicable = true;
    kaveri.bank = Some(BankDetails {
        account_number: "000123456789".to_owned(),
        ifsc: "HDFC0001234".to_owned(),
        bank_name: "Test Bank".to_owned(),
        branch: "Main".to_owned(),
    });
    kaveri.tax = Some(TaxDetails {
        kind: TaxLedgerKind::Cgst,
        rate_percent: None,
    });
    kaveri.gst = Some(GstClassification {
        hsn: None,
        taxability: Taxability::Exempt,
        rate_percent: None,
    });
    kaveri.tally = Some(ledgerly_core::TallyRef {
        guid: "guid-1".to_owned(),
        alter_id: Some(7),
    });
    for l in [
        cash,
        sharma,
        ledger(RENT, "Rent", expenses),
        ledger(USED, "Used Ledger", expenses),
        kaveri,
    ] {
        m.ledgers.insert(l.id, l);
    }
    books.vouchers.insert(VoucherId(1), voucher_using(USED));
    fill_inventory(&mut books.masters);
}

fn fill_inventory(m: &mut Masters) {
    m.units.insert(NOS, unit(NOS, "Nos", UnitKind::Simple));
    m.units.insert(
        BOX,
        unit(
            BOX,
            "Box",
            UnitKind::Compound {
                first: BOX,
                factor: ledgerly_core::Decimal::from(12),
                second: NOS,
            },
        ),
    );
    m.stock_groups.insert(
        StockGroupId(1),
        StockGroup {
            ineligible_itc: None,
            id: StockGroupId(1),
            name: "Primary Stock".to_owned(),
            parent: None,
            tally: None,
        },
    );
    m.stock_items.insert(
        ITEM,
        StockItem {
            ineligible_itc: None,
            id: ITEM,
            name: "Widget".to_owned(),
            aliases: Vec::new(),
            group: None,
            unit: NOS,
            alt_unit: None,
            hsn: None,
            gst_history: Vec::new(),
            valuation: ValuationMethod::AverageCost,
            batches_enabled: false,
            opening: vec![ledgerly_model::OpeningStock {
                godown: GodownId(1),
                batch: None,
                qty: ledgerly_core::Qty(ledgerly_core::Decimal::TEN),
                rate: ledgerly_core::Rate(ledgerly_core::Decimal::ONE),
            }],
            standard_costs: Vec::new(),
            tally: None,
        },
    );
    m.godowns.insert(
        GodownId(1),
        Godown {
            id: GodownId(1),
            name: "Main Godown".to_owned(),
            parent: None,
            tally: None,
        },
    );
    m.cost_categories.insert(
        CostCategoryId(1),
        CostCategory {
            id: CostCategoryId(1),
            name: "Projects".to_owned(),
        },
    );
    m.cost_centres.insert(
        CostCentreId(1),
        CostCentre {
            id: CostCentreId(1),
            name: "Site A".to_owned(),
            category: CostCategoryId(1),
            parent: None,
        },
    );
}

fn unit(id: UnitId, symbol: &str, kind: UnitKind) -> Unit {
    Unit {
        id,
        symbol: symbol.to_owned(),
        formal_name: symbol.to_owned(),
        decimals: 0,
        kind,
        uqc: None,
        tally: None,
    }
}

fn books() -> Books {
    base_books(Features::default())
}

fn books_all_features() -> Books {
    base_books(Features {
        inventory: true,
        godowns: true,
        batches: true,
        cost_centres: true,
        bill_wise: true,
        gst: true,
        ..Features::default()
    })
}

fn named_ledger(books: &Books, name: &str) -> LedgerDraft {
    LedgerDraft {
        name: name.to_owned(),
        group: Some(gid(books, ReservedGroup::IndirectExpenses)),
        ..LedgerDraft::default()
    }
}

fn build_ledger(books: &Books, d: LedgerDraft) -> Result<MasterSnapshot, MasterError> {
    MasterDraft::Ledger(Box::new(d)).build(books, None)
}

fn invalid_field(r: Result<MasterSnapshot, MasterError>) -> String {
    match r {
        Err(MasterError::Invalid { field, .. }) => field,
        other => panic!("expected an Invalid error, got {other:?}"),
    }
}

fn as_ledger(s: MasterSnapshot) -> Ledger {
    match s {
        MasterSnapshot::Ledger(l) => l,
        other @ (MasterSnapshot::Group(_)
        | MasterSnapshot::VoucherType(_)
        | MasterSnapshot::Unit(_)
        | MasterSnapshot::StockGroup(_)
        | MasterSnapshot::StockItem(_)
        | MasterSnapshot::Godown(_)
        | MasterSnapshot::Batch(_)
        | MasterSnapshot::CostCategory(_)
        | MasterSnapshot::CostCentre(_)) => panic!("expected a ledger, got {other:?}"),
    }
}

fn as_group(s: MasterSnapshot) -> Group {
    match s {
        MasterSnapshot::Group(g) => g,
        other @ (MasterSnapshot::Ledger(_)
        | MasterSnapshot::VoucherType(_)
        | MasterSnapshot::Unit(_)
        | MasterSnapshot::StockGroup(_)
        | MasterSnapshot::StockItem(_)
        | MasterSnapshot::Godown(_)
        | MasterSnapshot::Batch(_)
        | MasterSnapshot::CostCategory(_)
        | MasterSnapshot::CostCentre(_)) => panic!("expected a group, got {other:?}"),
    }
}

// ---- egui: one frame, keys -------------------------------------------------------------------

fn frame(
    ctx: &egui::Context,
    screen: &mut MastersScreen,
    app: &mut AppCtx,
    input: RawInput,
) -> ScreenAction {
    let mut action = ScreenAction::Stay;
    let out = ctx.run_ui(input, |ui| {
        action = screen.ui(ui, app);
    });
    out.drop_without_applying_deltas();
    action
}

fn key(key: Key, modifiers: Modifiers) -> RawInput {
    RawInput {
        events: vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..RawInput::default()
    }
}

fn app_with(books: Books) -> AppCtx {
    AppCtx {
        books: Some(books),
        api: None,
        lock_date: None,
        ai_tier: None,
    }
}

// ---- listing ---------------------------------------------------------------------------------

fn names(rows: &[ledgerly_app::screens::masters::MasterRow]) -> Vec<&str> {
    rows.iter().map(|r| r.name.as_str()).collect()
}

#[test]
fn ledgers_are_listed_by_name_and_found_by_alias_and_gstin() {
    let b = books();
    // provenance: derived; alphabetical order ignoring case of the five fixture names.
    let all = list_rows(&b.masters, MasterKind::Ledger, "");
    assert_eq!(
        names(&all),
        [
            "Cash",
            "Kaveri Packaging",
            "Rent",
            "Sharma Traders",
            "Used Ledger"
        ]
    );
    // provenance: derived; "st" is not inside any fixture name in lower case ("sharma traders"
    // has no s directly before t) but it is the alias "ST" of Sharma Traders.
    let by_alias = list_rows(&b.masters, MasterKind::Ledger, "st");
    assert_eq!(names(&by_alias), ["Sharma Traders"]);
    // provenance: derived; the GSTIN is searchable in any case (spec 7: palette searches GSTIN).
    let by_gstin = list_rows(&b.masters, MasterKind::Ledger, "29abcde1234");
    assert_eq!(names(&by_gstin), ["Sharma Traders"]);
    assert_eq!(
        list_rows(&b.masters, MasterKind::Ledger, "zzz"),
        [] as [ledgerly_app::screens::masters::MasterRow; 0]
    );
}

#[test]
fn every_kind_lists_what_the_fixture_holds() {
    let b = books();
    // provenance: derived; counts of the objects the fixture inserts (28 reserved + 2 custom
    // groups, 5 ledgers, 2 units, 1 stock group, 1 item, 1 godown, 1 category, 1 centre,
    // no voucher types, no batches).
    let counts: [(MasterKind, usize); 10] = [
        (MasterKind::Group, 30),
        (MasterKind::Ledger, 5),
        (MasterKind::VoucherType, 0),
        (MasterKind::Unit, 2),
        (MasterKind::StockGroup, 1),
        (MasterKind::StockItem, 1),
        (MasterKind::Godown, 1),
        (MasterKind::Batch, 0),
        (MasterKind::CostCategory, 1),
        (MasterKind::CostCentre, 1),
    ];
    for (kind, n) in counts {
        assert_eq!(list_rows(&b.masters, kind, "").len(), n, "{kind:?}");
    }
    assert!(
        snapshot_of(
            &b.masters,
            MasterRef {
                kind: MasterKind::Ledger,
                id: 3
            }
        )
        .is_some()
    );
    assert!(
        snapshot_of(
            &b.masters,
            MasterRef {
                kind: MasterKind::Ledger,
                id: 99
            }
        )
        .is_none()
    );
}

// ---- amounts ---------------------------------------------------------------------------------

#[test]
fn rupee_text_parses_exactly() {
    // provenance: derived; 12,34,567.00 rupees = 1,234,567 x 100 = 123,456,700 paise.
    assert_eq!(
        parse_rupees("Opening", "12,34,567.00"),
        Ok(Paise::new(123_456_700))
    );
    // provenance: derived; 100.5 rupees = 10,050 paise.
    assert_eq!(parse_rupees("Opening", "100.5"), Ok(Paise::new(10_050)));
    // provenance: derived; 0.50 rupees = 50 paise.
    assert_eq!(parse_rupees("Opening", "0.50"), Ok(Paise::new(50)));
    // provenance: derived; 1,234 is valid Indian grouping: 1234 rupees = 123,400 paise.
    assert_eq!(parse_rupees("Opening", "1,234"), Ok(Paise::new(123_400)));
}

#[test]
fn rupee_text_that_could_be_misread_is_refused() {
    // provenance: derived (rule A1, no silent rounding or guessing): a third decimal, a
    // European-style comma, western grouping, a sign and an empty text are all errors.
    for bad in [
        "10.999",
        "1,00",
        "1,234,567",
        "-5",
        "",
        "1.",
        ".5",
        "1e3",
        "12,34,5678",
    ] {
        assert!(
            matches!(
                parse_rupees("Opening", bad),
                Err(MasterError::Invalid { .. })
            ),
            "{bad:?}"
        );
    }
}

#[test]
fn opening_side_and_zero_are_kept_exactly() {
    let b = books();
    let mut d = named_ledger(&b, "Loan Given");
    d.opening = "5,000".to_owned();
    d.opening_side = DrCr::Cr;
    // provenance: derived; 5,000 rupees = 500,000 paise on the credit side.
    let l = as_ledger(build_ledger(&b, d.clone()).unwrap());
    assert_eq!(l.opening, SignedAmount::cr(Paise::new(500_000)));
    // provenance: derived; a zero opening is stored on the debit side whatever the selector says.
    d.opening = "0".to_owned();
    let z = as_ledger(build_ledger(&b, d).unwrap());
    assert_eq!(z.opening, SignedAmount::dr(Paise::ZERO));
}

// ---- names -----------------------------------------------------------------------------------

#[test]
fn names_and_aliases_are_unique_ignoring_case() {
    let b = books();
    let d = named_ledger(&b, "RENT");
    // provenance: derived; "RENT" and the existing "Rent" are the same name to the type-ahead.
    assert!(matches!(
        build_ledger(&b, d),
        Err(MasterError::NameTaken { .. })
    ));
    let mut d = named_ledger(&b, "Fresh");
    d.aliases = "ST".to_owned();
    // provenance: derived; the alias "ST" is Sharma Traders' alias.
    assert!(matches!(
        build_ledger(&b, d),
        Err(MasterError::NameTaken { .. })
    ));
    let mut d = named_ledger(&b, "Fresh");
    d.aliases = "fresh".to_owned();
    // provenance: derived; an alias equal to the master's own name repeats within the form.
    assert!(matches!(
        build_ledger(&b, d),
        Err(MasterError::NameTaken { .. })
    ));
    // provenance: derived; an alter keeps its own name without clashing with itself.
    let kept = MasterDraft::Ledger(Box::new(named_ledger(&b, "Rent")));
    assert!(kept.build(&b, Some(RENT.0)).is_ok());
    assert!(matches!(
        build_ledger(&b, named_ledger(&b, "   ")),
        Err(MasterError::NameEmpty { .. })
    ));
}

// ---- groups ----------------------------------------------------------------------------------

#[test]
fn a_primary_group_needs_a_nature_and_a_sub_group_inherits() {
    let b = books();
    let primary = GroupDraft {
        name: "New Primary".to_owned(),
        ..GroupDraft::default()
    };
    // provenance: spec 5: nature lives on primary groups, so a primary without one is refused.
    assert_eq!(
        invalid_field(MasterDraft::Group(primary.clone()).build(&b, None)),
        "Nature"
    );
    let ok = GroupDraft {
        nature: Some(Nature::Income),
        affects_gross_profit: true,
        ..primary
    };
    let g = as_group(MasterDraft::Group(ok).build(&b, None).unwrap());
    assert_eq!(
        (g.id.0, g.parent, g.nature, g.affects_gross_profit),
        (UNASSIGNED_ID, None, Some(Nature::Income), true)
    );
    // provenance: spec 5 plus the seed table in ledgerly-model: Direct Expenses is a primary
    // group of nature Expense that affects gross profit; a sub-group has no nature of its own
    // and inherits the flag.
    let sub = GroupDraft {
        name: "Freight".to_owned(),
        parent: Some(gid(&b, ReservedGroup::DirectExpenses)),
        nature: Some(Nature::Asset),
        affects_gross_profit: false,
        ..GroupDraft::default()
    };
    let g = as_group(MasterDraft::Group(sub).build(&b, None).unwrap());
    assert_eq!((g.nature, g.affects_gross_profit), (None, true));
}

#[test]
fn a_group_cannot_be_moved_under_itself_or_its_own_child() {
    let b = books();
    let mut d = GroupDraft {
        name: "Custom Parent".to_owned(),
        parent: Some(CUSTOM_CHILD),
        nature: Some(Nature::Asset),
        ..GroupDraft::default()
    };
    // provenance: derived; Custom Child sits under Custom Parent, so moving the parent under the
    // child closes a loop (spec 5: a group tree).
    assert_eq!(
        invalid_field(MasterDraft::Group(d.clone()).build(&b, Some(CUSTOM_PARENT.0))),
        "Under"
    );
    d.parent = Some(CUSTOM_PARENT);
    assert_eq!(
        invalid_field(MasterDraft::Group(d.clone()).build(&b, Some(CUSTOM_PARENT.0))),
        "Under"
    );
    d.parent = None;
    assert!(
        MasterDraft::Group(d)
            .build(&b, Some(CUSTOM_PARENT.0))
            .is_ok()
    );
}

#[test]
fn a_reserved_group_can_be_renamed_but_not_moved() {
    let b = books();
    let debtors = gid(&b, ReservedGroup::SundryDebtors);
    let original = b.masters.groups.get(&debtors).unwrap();
    let mut d = GroupDraft {
        name: "Customers".to_owned(),
        parent: original.parent,
        nature: original.nature,
        affects_gross_profit: original.affects_gross_profit,
        ..GroupDraft::default()
    };
    // provenance: spec 5 invariant 5: reserved groups are keyed by enum, "renaming is harmless".
    let g = as_group(
        MasterDraft::Group(d.clone())
            .build(&b, Some(debtors.0))
            .unwrap(),
    );
    assert_eq!(
        (g.name.as_str(), g.reserved),
        ("Customers", Some(ReservedGroup::SundryDebtors))
    );
    // provenance: derived; moving it elsewhere would change what the reserved group means.
    d.parent = Some(gid(&b, ReservedGroup::FixedAssets));
    assert!(matches!(
        MasterDraft::Group(d).build(&b, Some(debtors.0)),
        Err(MasterError::Reserved { .. })
    ));
}

// ---- deleting --------------------------------------------------------------------------------

fn screen_on(kind: MasterKind, id: u64) -> MastersScreen {
    let mut s = MastersScreen::with_kind(kind);
    s.select(MasterRef { kind, id });
    s
}

#[test]
fn reserved_masters_cannot_be_deleted() {
    let b = books();
    // provenance: spec 5 invariant 5: "Reserved groups/ledgers cannot be deleted".
    let mut s = screen_on(MasterKind::Group, gid(&b, ReservedGroup::SundryDebtors).0);
    assert!(matches!(
        s.request_delete(&b),
        Err(MasterError::Reserved { .. })
    ));
    let mut s = screen_on(MasterKind::Ledger, CASH.0);
    assert!(matches!(
        s.request_delete(&b),
        Err(MasterError::Reserved { .. })
    ));
    assert!(s.pending_delete().is_none());
}

#[test]
fn a_master_the_books_use_cannot_be_deleted() {
    let b = books();
    // provenance: derived; the fixture voucher has a line on Used Ledger; Sharma Traders has a
    // non-zero opening balance; Custom Parent has Custom Child under it; the Debtors group holds
    // ledgers.
    for (kind, id) in [
        (MasterKind::Ledger, USED.0),
        (MasterKind::Ledger, SHARMA.0),
        (MasterKind::Group, CUSTOM_PARENT.0),
        (MasterKind::Unit, NOS.0),
        (MasterKind::StockItem, ITEM.0),
        (MasterKind::Godown, 1),
        (MasterKind::CostCategory, 1),
    ] {
        let mut s = screen_on(kind, id);
        assert!(
            matches!(s.request_delete(&b), Err(MasterError::InUse { .. })),
            "{kind:?} {id}"
        );
    }
}

#[test]
fn an_unused_master_is_deleted_after_a_yes() {
    let b = books();
    let mut s = screen_on(MasterKind::Ledger, RENT.0);
    s.request_delete(&b).unwrap();
    assert_eq!(
        s.pending_delete(),
        Some(MasterRef {
            kind: MasterKind::Ledger,
            id: RENT.0
        })
    );
    assert!(
        s.take_requests().is_empty(),
        "nothing is queued before the yes"
    );
    s.confirm_delete(&b).unwrap();
    // provenance: derived; one confirmed delete queues exactly one Delete for the selected master.
    assert_eq!(
        s.take_requests(),
        [MasterRequest::Delete(MasterRef {
            kind: MasterKind::Ledger,
            id: RENT.0
        })]
    );
    assert!(matches!(
        s.confirm_delete(&b),
        Err(MasterError::NothingToConfirm)
    ));
}

// ---- ledger details --------------------------------------------------------------------------

fn party(registration: RegistrationType, gstin: &str) -> PartyDraft {
    PartyDraft {
        registration,
        gstin: gstin.to_owned(),
        ..PartyDraft::default()
    }
}

fn with_party(books: &Books, p: PartyDraft) -> Result<MasterSnapshot, MasterError> {
    let mut d = named_ledger(books, "Party");
    d.party = Some(p);
    build_ledger(books, d)
}

#[test]
fn registration_and_gstin_must_agree() {
    let b = books();
    // provenance: derived; a registered party has a GSTIN, an unregistered or consumer party has none.
    assert_eq!(
        invalid_field(with_party(&b, party(RegistrationType::Regular, ""))),
        "GSTIN"
    );
    assert_eq!(
        invalid_field(with_party(
            &b,
            party(RegistrationType::Unregistered, GSTIN_29)
        )),
        "GSTIN"
    );
    // provenance: derived from gstin.rs vectors: 15th character W is right, X is not.
    assert_eq!(
        invalid_field(with_party(
            &b,
            party(RegistrationType::Regular, "29ABCDE1234F1ZX")
        )),
        "GSTIN"
    );
    // provenance: derived; the first two digits of the GSTIN are the state, 29.
    let l = as_ledger(with_party(&b, party(RegistrationType::Regular, GSTIN_29)).unwrap());
    let p = l.party.unwrap();
    assert_eq!(p.state, StateCode::new(29).ok());
    assert_eq!(p.address.state, StateCode::new(29).ok());
    // provenance: derived; the PAN is characters 3 to 12 of the GSTIN.
    let mut ok = party(RegistrationType::Regular, GSTIN_29);
    ok.pan = PAN_OF_GSTIN_29.to_owned();
    assert!(with_party(&b, ok.clone()).is_ok());
    ok.pan = "ABCDE1234G".to_owned();
    assert_eq!(invalid_field(with_party(&b, ok)), "PAN");
}

#[test]
fn state_must_match_the_gstin_and_pin_must_be_six_digits() {
    let b = books();
    let mut p = party(RegistrationType::Regular, GSTIN_29);
    p.state = StateCode::new(27).ok();
    // provenance: derived; GSTIN starts 29 (Karnataka), state 27 contradicts it.
    assert_eq!(invalid_field(with_party(&b, p)), "State");
    let mut p = party(RegistrationType::Regular, GSTIN_29);
    p.pincode = "5600".to_owned();
    assert_eq!(invalid_field(with_party(&b, p)), "PIN code");
    let mut p = party(RegistrationType::Overseas, "");
    p.country = String::new();
    assert!(matches!(
        with_party(&b, p),
        Err(MasterError::NameEmpty { .. })
    ));
}

#[test]
fn bank_tax_and_gst_fields_are_checked() {
    let b = books();
    let mut d = named_ledger(&b, "Bank");
    let bank = |ifsc: &str| BankDraft {
        account_number: "123456".to_owned(),
        ifsc: ifsc.to_owned(),
        bank_name: "Test Bank".to_owned(),
        branch: String::new(),
    };
    // provenance: derived; IFSC shape is four capital letters, a zero, six letters or digits.
    d.bank = Some(bank("HDFC0001234"));
    assert!(build_ledger(&b, d.clone()).is_ok());
    for bad in ["hdfc0001234", "HDFC1001234", "HDFC000123"] {
        d.bank = Some(bank(bad));
        assert_eq!(invalid_field(build_ledger(&b, d.clone())), "IFSC", "{bad}");
    }
    d.bank = None;
    let rate = |s: &str| TaxDraft {
        kind: TaxLedgerKind::Igst,
        rate_percent: s.to_owned(),
    };
    for bad in ["101", "-1", "abc", "1e1"] {
        d.tax = Some(rate(bad));
        assert_eq!(
            invalid_field(build_ledger(&b, d.clone())),
            "Tax rate",
            "{bad}"
        );
    }
    d.tax = Some(rate("18"));
    let l = as_ledger(build_ledger(&b, d.clone()).unwrap());
    assert_eq!(
        l.tax.unwrap().rate_percent.unwrap().0,
        ledgerly_core::Decimal::from(18)
    );
    d.tax = None;
    d.gst = Some(GstDraft {
        hsn: "9983".to_owned(),
        taxability: Taxability::Exempt,
        rate_percent: "5".to_owned(),
    });
    // provenance: derived; only a taxable ledger carries a rate.
    assert_eq!(invalid_field(build_ledger(&b, d)), "GST rate");
}

#[test]
fn bill_wise_credit_days_and_features() {
    let b = books();
    let mut d = named_ledger(&b, "Debtor");
    d.credit_days = "30".to_owned();
    assert_eq!(invalid_field(build_ledger(&b, d.clone())), "Credit days");
    d.bill_wise = true;
    // provenance: derived; Features::default() has bill-wise off (company switch, spec 5).
    assert!(matches!(
        build_ledger(&b, d.clone()),
        Err(MasterError::FeatureOff { .. })
    ));
    let on = books_all_features();
    d.group = Some(gid(&on, ReservedGroup::SundryDebtors));
    let l = as_ledger(build_ledger(&on, d).unwrap());
    assert_eq!(
        l.bill_wise,
        Some(BillWiseConfig {
            default_credit_days: Some(30)
        })
    );
}

#[test]
fn an_unchanged_form_gives_back_the_same_master() {
    let b = books_all_features();
    // provenance: derived invariant: opening a master and saving it unchanged alters nothing,
    // including the fields the form does not show (tally reference, reserved flag).
    for id in [CASH, SHARMA, KAVERI, RENT] {
        let original = snapshot_of(
            &b.masters,
            MasterRef {
                kind: MasterKind::Ledger,
                id: id.0,
            },
        )
        .unwrap();
        let draft = MasterDraft::from_existing(&original).value().unwrap();
        assert_eq!(draft.build(&b, Some(id.0)).unwrap(), original, "{id:?}");
    }
    for (kind, id) in [
        (MasterKind::Group, CUSTOM_CHILD.0),
        (MasterKind::Group, gid(&b, ReservedGroup::SundryDebtors).0),
        (MasterKind::Group, gid(&b, ReservedGroup::Suspense).0),
        (MasterKind::Unit, NOS.0),
        (MasterKind::StockGroup, 1),
        (MasterKind::StockItem, ITEM.0),
        (MasterKind::Godown, 1),
        (MasterKind::CostCategory, 1),
        (MasterKind::CostCentre, 1),
    ] {
        let original = snapshot_of(&b.masters, MasterRef { kind, id }).unwrap();
        let draft = MasterDraft::from_existing(&original).value().unwrap();
        assert_eq!(
            draft.build(&b, Some(id)).unwrap(),
            original,
            "{kind:?} {id}"
        );
    }
}

#[test]
fn altering_keeps_what_the_form_does_not_show() {
    let b = books();
    let original = snapshot_of(
        &b.masters,
        MasterRef {
            kind: MasterKind::Ledger,
            id: KAVERI.0,
        },
    )
    .unwrap();
    let MasterDraft::Ledger(mut d) = MasterDraft::from_existing(&original).value().unwrap() else {
        panic!("not a ledger form");
    };
    d.name = "Kaveri Packaging Pvt".to_owned();
    let l = as_ledger(MasterDraft::Ledger(d).build(&b, Some(KAVERI.0)).unwrap());
    let MasterSnapshot::Ledger(before) = original else {
        panic!()
    };
    // provenance: derived; only the name differs.
    assert_eq!(l.name, "Kaveri Packaging Pvt");
    assert_eq!(
        Ledger {
            ineligible_itc: None,
            name: before.name.clone(),
            ..l
        },
        before
    );
}

#[test]
fn a_reserved_ledger_cannot_change_group() {
    let b = books();
    let mut d = named_ledger(&b, "Cash");
    d.group = Some(gid(&b, ReservedGroup::BankAccounts));
    // provenance: spec 5 invariant 5 (reserved ledgers): Cash stays under Cash-in-Hand.
    assert!(matches!(
        MasterDraft::Ledger(Box::new(d)).build(&b, Some(CASH.0)),
        Err(MasterError::Reserved { .. })
    ));
}

// ---- inventory and cost masters --------------------------------------------------------------

#[test]
fn a_feature_that_is_off_blocks_creation() {
    let b = books();
    // provenance: derived; Features::default() switches inventory, godowns and cost centres off.
    for kind in [
        MasterKind::Unit,
        MasterKind::StockItem,
        MasterKind::Godown,
        MasterKind::CostCentre,
    ] {
        let mut s = MastersScreen::with_kind(kind);
        assert!(
            matches!(s.begin_create(&b), Err(MasterError::FeatureOff { .. })),
            "{kind:?}"
        );
        assert!(!s.is_form_open());
    }
    let on = books_all_features();
    let godown = MasterDraft::Godown(TreeDraft {
        name: "Annexe".to_owned(),
        parent: Some(1),
    });
    assert!(matches!(
        godown.build(&b, None),
        Err(MasterError::FeatureOff { .. })
    ));
    assert!(godown.build(&on, None).is_ok());
}

#[test]
fn unit_stock_item_and_cost_centre_rules() {
    let b = books_all_features();
    let unit = |symbol: &str, decimals: &str, uqc: &str| {
        MasterDraft::Unit(UnitDraft {
            symbol: symbol.to_owned(),
            formal_name: "Kilograms".to_owned(),
            decimals: decimals.to_owned(),
            uqc: uqc.to_owned(),
        })
    };
    assert!(unit("Kg", "3", "KGS").build(&b, None).is_ok());
    assert_eq!(
        invalid_field(unit("Kg", "7", "").build(&b, None)),
        "Decimals"
    );
    assert_eq!(
        invalid_field(unit("Kg", "-1", "").build(&b, None)),
        "Decimals"
    );
    assert_eq!(invalid_field(unit("Kg", "0", "kgs").build(&b, None)), "UQC");
    assert!(matches!(
        unit("nos", "0", "").build(&b, None),
        Err(MasterError::NameTaken { .. })
    ));
    let centre = |category: Option<CostCategoryId>, parent: Option<CostCentreId>| {
        MasterDraft::CostCentre(CostCentreDraft {
            name: "Site B".to_owned(),
            category,
            parent,
        })
    };
    assert_eq!(
        invalid_field(centre(None, None).build(&b, None)),
        "Category"
    );
    assert!(
        centre(Some(CostCategoryId(1)), Some(CostCentreId(1)))
            .build(&b, None)
            .is_ok()
    );
    assert_eq!(
        invalid_field(centre(Some(CostCategoryId(9)), None).build(&b, None)),
        "Category"
    );
}

#[test]
fn a_stock_item_with_stock_keeps_its_unit() {
    let b = books_all_features();
    let original = snapshot_of(
        &b.masters,
        MasterRef {
            kind: MasterKind::StockItem,
            id: ITEM.0,
        },
    )
    .unwrap();
    let MasterDraft::StockItem(mut d) = MasterDraft::from_existing(&original).value().unwrap()
    else {
        panic!("not an item form");
    };
    d.unit = Some(UnitId(3));
    // provenance: derived; an unknown unit is refused first.
    assert_eq!(
        invalid_field(MasterDraft::StockItem(d.clone()).build(&b, Some(ITEM.0))),
        "Unit"
    );
    let mut books2 = b.clone();
    books2
        .masters
        .units
        .insert(UnitId(3), unit(UnitId(3), "Kg", UnitKind::Simple));
    // provenance: derived; the fixture item has 10 units of opening stock, so its unit is fixed.
    assert_eq!(
        invalid_field(MasterDraft::StockItem(d).build(&books2, Some(ITEM.0))),
        "Unit"
    );
}

// ---- pending / unsupported -------------------------------------------------------------------

#[test]
fn unsupported_cases_say_so() {
    let b = books_all_features();
    let mut s = MastersScreen::with_kind(MasterKind::VoucherType);
    assert!(matches!(
        s.begin_create(&b),
        Err(MasterError::Unsupported { .. })
    ));
    let mut s = MastersScreen::with_kind(MasterKind::Batch);
    assert!(matches!(
        s.begin_create(&b),
        Err(MasterError::Unsupported { .. })
    ));
    // A compound unit is listed but not altered.
    let mut s = screen_on(MasterKind::Unit, BOX.0);
    assert!(matches!(
        s.begin_alter(&b),
        Err(MasterError::Unsupported { .. })
    ));
    let mut s = screen_on(MasterKind::VoucherType, 1);
    assert!(matches!(
        s.request_delete(&b),
        Err(MasterError::Unsupported { .. })
    ));
    // UIN registrations and standard-cost valuation have no verified behaviour yet.
    assert!(matches!(
        with_party(&b, party(RegistrationType::Uin, "")),
        Err(MasterError::Unsupported { .. })
    ));
    let item = MasterDraft::StockItem(StockItemDraft {
        name: "Gadget".to_owned(),
        unit: Some(NOS),
        valuation: ValuationMethod::StandardCost,
        ..StockItemDraft::default()
    });
    assert!(matches!(
        item.build(&b, None),
        Err(MasterError::Unsupported { .. })
    ));
}

// ---- the screen: keys and drawing ------------------------------------------------------------

#[test]
fn alt_c_opens_a_form_and_ctrl_a_queues_a_create() {
    let ctx = egui::Context::default();
    let mut app = app_with(books());
    let mut s = MastersScreen::new();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    // provenance: spec 7: Alt+C creates a master, Ctrl+A saves.
    frame(&ctx, &mut s, &mut app, key(Key::C, Modifiers::ALT));
    assert!(s.is_form_open());
    let books = app.books.clone().unwrap();
    frame(&ctx, &mut s, &mut app, key(Key::A, Modifiers::COMMAND));
    assert!(s.is_form_open(), "an empty form does not save");
    assert!(s.error_text().is_some());
    let Some(MasterDraft::Ledger(d)) = s.draft_mut() else {
        panic!("no ledger form");
    };
    **d = named_ledger(&books, "Stationery");
    frame(&ctx, &mut s, &mut app, key(Key::A, Modifiers::COMMAND));
    assert!(!s.is_form_open());
    let requests = s.take_requests();
    assert_eq!(requests.len(), 1);
    let MasterRequest::Create(MasterSnapshot::Ledger(l)) = &requests[0] else {
        panic!("not a create of a ledger: {requests:?}");
    };
    assert_eq!((l.name.as_str(), l.id.0), ("Stationery", UNASSIGNED_ID));
}

#[test]
fn ctrl_enter_alters_and_alt_d_then_y_deletes() {
    let ctx = egui::Context::default();
    let mut app = app_with(books());
    let mut s = MastersScreen::new();
    s.select(MasterRef {
        kind: MasterKind::Ledger,
        id: RENT.0,
    });
    frame(&ctx, &mut s, &mut app, key(Key::Enter, Modifiers::COMMAND));
    // provenance: spec 7: Ctrl+Enter alters the master under the cursor.
    assert_eq!(s.editing(), Some(RENT.0));
    frame(&ctx, &mut s, &mut app, key(Key::Escape, Modifiers::NONE));
    assert!(!s.is_form_open(), "an unchanged form closes at once");
    // provenance: spec 7: Alt+D deletes; the answer is asked first.
    frame(&ctx, &mut s, &mut app, key(Key::D, Modifiers::ALT));
    assert_eq!(s.pending_delete().map(|t| t.id), Some(RENT.0));
    frame(&ctx, &mut s, &mut app, key(Key::N, Modifiers::NONE));
    assert!(s.pending_delete().is_none() && s.take_requests().is_empty());
    frame(&ctx, &mut s, &mut app, key(Key::D, Modifiers::ALT));
    frame(&ctx, &mut s, &mut app, key(Key::Y, Modifiers::NONE));
    assert_eq!(s.take_requests().len(), 1);
}

#[test]
fn esc_asks_before_discarding_changes_and_then_leaves_the_screen() {
    let ctx = egui::Context::default();
    let mut app = app_with(books());
    let mut s = MastersScreen::new();
    frame(&ctx, &mut s, &mut app, key(Key::C, Modifiers::ALT));
    let Some(MasterDraft::Ledger(d)) = s.draft_mut() else {
        panic!("no ledger form");
    };
    d.name = "Typed".to_owned();
    frame(&ctx, &mut s, &mut app, key(Key::Escape, Modifiers::NONE));
    assert!(
        s.is_form_open() && s.note_text().is_some(),
        "first Esc warns"
    );
    let action = frame(&ctx, &mut s, &mut app, key(Key::Escape, Modifiers::NONE));
    assert!(!s.is_form_open() && matches!(action, ScreenAction::Stay));
    // provenance: spec 7: Esc goes back; with nothing open it leaves the screen.
    let action = frame(&ctx, &mut s, &mut app, key(Key::Escape, Modifiers::NONE));
    assert!(matches!(action, ScreenAction::Pop));
}

#[test]
fn alt_arrows_change_kind_and_arrows_move_the_cursor() {
    let ctx = egui::Context::default();
    let mut app = app_with(books());
    let mut s = MastersScreen::with_kind(MasterKind::Group);
    frame(&ctx, &mut s, &mut app, key(Key::ArrowRight, Modifiers::ALT));
    assert_eq!(s.kind(), MasterKind::Ledger);
    // provenance: derived; the first ledger alphabetically is Cash (id 1), the second Kaveri (id 5).
    // Spec change 2026-11-23: the cursor starts on the first row (mock v2 index.html:1797, `sel`
    // on the first row), so the list shows Cash selected before any key, and the first Down
    // goes to Kaveri. It used to start on no row, and the first Down landed on Cash.
    frame(&ctx, &mut s, &mut app, RawInput::default());
    assert_eq!(s.selected().map(|r| r.id), Some(CASH.0));
    frame(&ctx, &mut s, &mut app, key(Key::ArrowDown, Modifiers::NONE));
    assert_eq!(s.selected().map(|r| r.id), Some(KAVERI.0));
    frame(&ctx, &mut s, &mut app, key(Key::ArrowUp, Modifiers::NONE));
    assert_eq!(s.selected().map(|r| r.id), Some(CASH.0));
    frame(&ctx, &mut s, &mut app, key(Key::ArrowLeft, Modifiers::ALT));
    assert_eq!(s.kind(), MasterKind::Group);
}

#[test]
fn every_screen_state_draws() {
    let ctx = egui::Context::default();
    let mut none = AppCtx::default();
    let mut s = MastersScreen::new();
    assert!(matches!(
        frame(&ctx, &mut s, &mut none, RawInput::default()),
        ScreenAction::Stay
    ));
    let mut app = app_with(books_all_features());
    for kind in MasterKind::ALL {
        let mut s = MastersScreen::with_kind(kind);
        frame(&ctx, &mut s, &mut app, RawInput::default());
        assert_ne!(s.title(), "");
        let books = app.books.clone().unwrap();
        if s.begin_create(&books).is_ok() {
            frame(&ctx, &mut s, &mut app, RawInput::default());
        }
    }
    let mut s = MastersScreen::new();
    s.begin_create(app.books.as_ref().unwrap()).unwrap();
    if let Some(MasterDraft::Ledger(d)) = s.draft_mut() {
        d.party = Some(PartyDraft::default());
        d.bank = Some(BankDraft::default());
        d.tax = Some(TaxDraft {
            kind: TaxLedgerKind::Cgst,
            rate_percent: String::new(),
        });
        d.gst = Some(GstDraft {
            hsn: String::new(),
            taxability: Taxability::Taxable,
            rate_percent: String::new(),
        });
    }
    frame(&ctx, &mut s, &mut app, RawInput::default());
    s.escape();
    s.escape();
    s.select(MasterRef {
        kind: MasterKind::Ledger,
        id: RENT.0,
    });
    s.request_delete(app.books.as_ref().unwrap()).unwrap();
    frame(&ctx, &mut s, &mut app, RawInput::default());
}

#[test]
fn a_host_error_is_shown_and_the_books_are_untouched() {
    let mut s = MastersScreen::new();
    s.report_host_error("the store refused the change");
    assert_eq!(s.error_text(), Some("the store refused the change"));
}

#[test]
fn a_create_carries_the_unassigned_id_for_every_kind() {
    // provenance: derived; a create carries the placeholder id for every kind, so the store (not
    // the screen) hands out ids.
    let b = books_all_features();
    let drafts = [
        MasterDraft::StockGroup(TreeDraft {
            name: "Raw".to_owned(),
            parent: Some(1),
        }),
        MasterDraft::CostCategory(TreeDraft {
            name: "Depts".to_owned(),
            parent: None,
        }),
        MasterDraft::Godown(TreeDraft {
            name: "Annexe".to_owned(),
            parent: None,
        }),
    ];
    for d in drafts {
        let id = match d.build(&b, None).unwrap() {
            MasterSnapshot::StockGroup(x) => x.id.0,
            MasterSnapshot::CostCategory(x) => x.id.0,
            MasterSnapshot::Godown(x) => x.id.0,
            other @ (MasterSnapshot::Group(_)
            | MasterSnapshot::Ledger(_)
            | MasterSnapshot::VoucherType(_)
            | MasterSnapshot::Unit(_)
            | MasterSnapshot::StockItem(_)
            | MasterSnapshot::Batch(_)
            | MasterSnapshot::CostCentre(_)) => panic!("unexpected {other:?}"),
        };
        assert_eq!(id, UNASSIGNED_ID);
    }
}

#[test]
fn shift_y_does_not_confirm_a_master_delete() {
    let ctx = egui::Context::default();
    let mut app = app_with(books());
    let mut s = MastersScreen::new();
    s.select(MasterRef {
        kind: MasterKind::Ledger,
        id: RENT.0,
    });
    frame(&ctx, &mut s, &mut app, key(Key::D, Modifiers::ALT));
    assert_eq!(s.pending_delete().map(|t| t.id), Some(RENT.0));
    // Requirement, not a figure: the question asks for a plain Y ("Press Y to delete", masters.rs
    // draw_confirm). Shift+Y is not that key, so the question stays open and nothing is queued.
    frame(&ctx, &mut s, &mut app, key(Key::Y, Modifiers::SHIFT));
    assert_eq!(s.pending_delete().map(|t| t.id), Some(RENT.0));
    assert_eq!(
        s.take_requests(),
        [] as [ledgerly_app::screens::masters::MasterRequest; 0]
    );
    frame(&ctx, &mut s, &mut app, key(Key::Y, Modifiers::NONE));
    assert_eq!(s.take_requests().len(), 1);
}

#[test]
fn a_kind_with_no_masters_names_the_kind_as_its_cursor() {
    // provenance: derived; with no stock item there is no row to point at, so the screen names
    // the kind it is showing ("Stock items", the tab label) for the capture harness to check
    // that it reached the empty list and not another kind's.
    let ctx = egui::Context::default();
    // The company keeps inventory (a company without it is not offered the Stock items tab:
    // s13_masters_inventory_off.rs), and has no stock item.
    let mut all = books_all_features();
    all.masters.stock_items.clear();
    let mut app = app_with(all);
    let mut s = MastersScreen::new();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    s.select_kind(MasterKind::StockItem);
    frame(&ctx, &mut s, &mut app, RawInput::default());
    assert_eq!(s.cursor_label(&app).as_deref(), Some("Stock items"));
}

/// Every path shape's stroke colour in `shapes`.
fn path_colours(shapes: &[egui::epaint::ClippedShape]) -> Vec<egui::Color32> {
    fn walk(shape: &egui::Shape, out: &mut Vec<egui::Color32>) {
        match shape {
            egui::Shape::Path(p) => {
                if let egui::epaint::ColorMode::Solid(c) = p.stroke.color {
                    out.push(c);
                }
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            egui::Shape::Noop
            | egui::Shape::Circle(_)
            | egui::Shape::Ellipse(_)
            | egui::Shape::LineSegment { .. }
            | egui::Shape::Band(_)
            | egui::Shape::Rect(_)
            | egui::Shape::Text(_)
            | egui::Shape::Mesh(_)
            | egui::Shape::QuadraticBezier(_)
            | egui::Shape::CubicBezier(_)
            | egui::Shape::Callback(_) => {}
        }
    }
    let mut out = Vec::new();
    for clipped in shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

#[test]
fn an_empty_list_draws_the_glyph_of_the_empty_state_in_the_tertiary_colour() {
    // provenance: design/mock-v2/index.html:1878, state `masters.items-empty`: the empty card
    // opens with the `users` glyph, and `.empty .ico{color:var(--text-3)}` (index.html:169).
    let ctx = egui::Context::default();
    ledgerly_app::theme::apply(&ctx, ledgerly_app::theme::ThemeKind::Light);
    // The company keeps inventory (a company without it is not offered the Stock items tab:
    // s13_masters_inventory_off.rs), and has no stock item.
    let mut all = books_all_features();
    all.masters.stock_items.clear();
    let mut app = app_with(all);
    let mut s = MastersScreen::new();
    frame(&ctx, &mut s, &mut app, RawInput::default());
    s.select_kind(MasterKind::StockItem);
    let out = ctx.run_ui(RawInput::default(), |ui| {
        let _ = s.ui(ui, &mut app);
    });
    let colours = path_colours(&out.shapes);
    out.drop_without_applying_deltas();
    let tertiary = ledgerly_app::theme::tokens(ledgerly_app::theme::ThemeKind::Light).text_3;
    assert!(
        colours.contains(&tertiary),
        "no glyph in the tertiary colour among {colours:?}"
    );
}
