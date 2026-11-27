//! Card S7 (accessibility): table rows on the masters list read their cells, as the report rows do.
//! Expected names are the cells the list draws (derived from `screens/masters/list.rs`
//! `ledger_entries`: the ledger's name, then "under <group>", then the opening balance when it is not
//! zero), never from this crate's output.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    reason = "test code: rule B2 lets tests unwrap and panic; fixtures are long"
)]

use std::collections::BTreeMap;

use egui::accesskit::{Node, Role, TreeUpdate};
use egui::{Context, Pos2, RawInput, Rect, Ui, Vec2};
use ledgerly_app::screens::masters::MastersScreen;
use ledgerly_app::theme::{self, ThemeKind};
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{GroupId, Gstin, LedgerId, Paise, SignedAmount, StateCode, VoucherTypeId};
use ledgerly_model::{
    Address, BaseVoucherKind, BillWiseConfig, Books, Company, EntityKind, Features, Group, Ledger,
    Masters, NegativeStockPolicy, NumberingConfig, NumberingMethod, PartyDetails, RegistrationType,
    RestartPeriod, VoucherType,
};
use time::macros::date;

// ---- fixture (copied from tests/u4_voucher_entry.rs) -----------------------------------------

const CASH: LedgerId = LedgerId(1);
const HDFC: LedgerId = LedgerId(2);
const SHARMA: LedgerId = LedgerId(3);
const SHREE: LedgerId = LedgerId(4);
const RENT: LedgerId = LedgerId(5);
const BILLWISE: LedgerId = LedgerId(6);

const T_JOURNAL: VoucherTypeId = VoucherTypeId(1);
const T_PAYMENT: VoucherTypeId = VoucherTypeId(2);
const T_SALES: VoucherTypeId = VoucherTypeId(3);
const T_OFF: VoucherTypeId = VoucherTypeId(4);
const T_STOCK: VoucherTypeId = VoucherTypeId(5);
const GROUP: GroupId = GroupId(1);

fn ledger(id: LedgerId, name: &str, aliases: &[&str]) -> Ledger {
    Ledger {
        ineligible_itc: None,
        id,
        name: name.to_owned(),
        aliases: aliases.iter().map(|a| (*a).to_owned()).collect(),
        group: Some(GROUP),
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

fn voucher_type(id: VoucherTypeId, name: &str, base: BaseVoucherKind, active: bool) -> VoucherType {
    VoucherType {
        id,
        name: name.to_owned(),
        base,
        abbreviation: name.chars().take(3).collect(),
        numbering: NumberingConfig {
            method: NumberingMethod::Automatic,
            prevent_renumbering: false,
            prefix: String::new(),
            suffix: String::new(),
            width: 4,
            restart: RestartPeriod::Yearly,
            series: Vec::new(),
        },
        active,
        tally: None,
    }
}

fn books() -> Books {
    let mut masters = Masters::default();
    masters.groups.insert(
        GROUP,
        Group {
            ineligible_itc: None,
            id: GROUP,
            name: "Current Assets".to_owned(),
            aliases: Vec::new(),
            parent: None,
            reserved: None,
            nature: None,
            affects_gross_profit: false,
            tally: None,
        },
    );
    let mut sharma = ledger(SHARMA, "Sharma Traders", &["ST"]);
    sharma.party = Some(PartyDetails {
        registration: RegistrationType::Regular,
        // A valid GSTIN from ledgerly-core's own tested vectors (gstin.rs `VALID`).
        gstin: Some(Gstin::parse("29ABCDE1234F1ZW").unwrap()),
        pan: None,
        state: None,
        address: Address::default(),
    });
    let mut billwise = ledger(BILLWISE, "Kaveri Packaging", &[]);
    billwise.bill_wise = Some(BillWiseConfig {
        default_credit_days: Some(30),
    });
    for l in [
        ledger(CASH, "Cash", &[]),
        ledger(HDFC, "HDFC Bank", &[]),
        sharma,
        ledger(SHREE, "Shree Stationers", &[]),
        ledger(RENT, "Rent", &[]),
        billwise,
    ] {
        masters.ledgers.insert(l.id, l);
    }
    for t in [
        voucher_type(T_JOURNAL, "Journal", BaseVoucherKind::Journal, true),
        voucher_type(T_PAYMENT, "Payment", BaseVoucherKind::Payment, true),
        voucher_type(T_SALES, "Sales", BaseVoucherKind::Sales, true),
        voucher_type(T_OFF, "Contra", BaseVoucherKind::Contra, false),
        voucher_type(
            T_STOCK,
            "Stock Journal",
            BaseVoucherKind::StockJournal,
            true,
        ),
    ] {
        masters.voucher_types.insert(t.id, t);
    }
    Books {
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
            features: Features::default(),
            negative_stock: NegativeStockPolicy::Block,
        },
        masters,
        vouchers: BTreeMap::new(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

fn context() -> Context {
    let ctx = Context::default();
    ctx.enable_accesskit();
    theme::apply(&ctx, ThemeKind::Light);
    ctx
}

fn tree(ctx: &Context, frames: usize, mut body: impl FnMut(&mut Ui)) -> TreeUpdate {
    let mut last = None;
    for _ in 0..frames {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            ..RawInput::default()
        };
        let mut out = ctx.run_ui(input, &mut body);
        last = out.platform_output.accesskit_update.take();
        out.drop_without_applying_deltas();
    }
    last.expect("AccessKit is enabled")
}

fn rows(update: &TreeUpdate) -> Vec<String> {
    update
        .nodes
        .iter()
        .filter(|(_, node): &&(_, Node)| node.role() == Role::Row)
        .filter_map(|(_, node)| node.label().map(str::to_owned))
        .collect()
}

#[test]
fn a_masters_ledger_row_reads_its_cells_in_column_order() {
    // derived: the Cash ledger is in the group "Current Assets" (fixture GROUP) with no opening
    // balance, so its cells as the list draws them are the name, the group and a dash for the
    // opening figure (masters/list.rs `ledger_entries`, masters/look.rs `paint_cells`). A row's
    // name is those cells in column order. Before the row was named, no row had a name at all.
    let ctx = context();
    let b = books();
    let mut app = AppCtx {
        tr: ledgerly_app::i18n::Tr::default(),
        books: Some(b.clone()),
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    let mut screen = MastersScreen::new();
    let update = tree(&ctx, 3, |ui| {
        let _ = screen.ui(ui, &mut app);
    });
    assert!(
        rows(&update).iter().any(|r| r == "Cash, Current Assets, —"),
        "the Cash row reads its cells; rows were {:?}",
        rows(&update)
    );
}
