//! Drawing of the master forms.
//!
//! Drawing only: every edit goes straight into the draft, and nothing is checked here. The
//! checks live in [`super::draft`], so the same rules apply to a click and to a test.

use egui::{RichText, Ui};
use ledgerly_core::{DrCr, StateCode};
use ledgerly_model::{
    Masters, Nature, RegistrationType, TaxLedgerKind, Taxability, ValuationMethod,
};

use super::draft::{
    BankDraft, CostCentreDraft, GroupDraft, GstDraft, LedgerDraft, MasterDraft, PartyDraft,
    StockItemDraft, TaxDraft, TreeDraft, UnitDraft,
};
use super::view::{GstinCheck, check_gstin};
use crate::theme::TEXT_META;
use crate::ui::badge::{BadgeKind, badge};
use crate::ui::tokens;

const FIELD_WIDTH: f32 = 320.0;

/// Draws the result of the GSTIN check under the GSTIN field: nothing when the field is empty, a
/// green check when every test passes, or a red badge with the reason when one fails. The check
/// is [`check_gstin`], the same one the detail panel uses.
pub(crate) fn draw_gstin_check(ui: &mut Ui, check: &GstinCheck) {
    match check {
        GstinCheck::Empty => {}
        GstinCheck::Valid => {
            badge(ui, BadgeKind::Ok, "Check character OK");
        }
        GstinCheck::Invalid(reason) => {
            badge(ui, BadgeKind::Bad, "Not a valid GSTIN");
            let t = tokens(ui);
            ui.label(RichText::new(reason.as_str()).size(TEXT_META).color(t.bad));
        }
    }
}

fn registration_label(r: RegistrationType) -> &'static str {
    match r {
        RegistrationType::Regular => "Regular",
        RegistrationType::Composition => "Composition",
        RegistrationType::Unregistered => "Unregistered",
        RegistrationType::Consumer => "Consumer",
        RegistrationType::Sez => "SEZ",
        RegistrationType::Overseas => "Overseas",
        RegistrationType::Uin => "UIN holder",
    }
}

const REGISTRATIONS: [RegistrationType; 7] = [
    RegistrationType::Regular,
    RegistrationType::Composition,
    RegistrationType::Unregistered,
    RegistrationType::Consumer,
    RegistrationType::Sez,
    RegistrationType::Overseas,
    RegistrationType::Uin,
];

fn nature_label(n: Nature) -> &'static str {
    match n {
        Nature::Asset => "Asset",
        Nature::Liability => "Liability",
        Nature::Income => "Income",
        Nature::Expense => "Expense",
    }
}

const NATURES: [Nature; 4] = [
    Nature::Asset,
    Nature::Liability,
    Nature::Income,
    Nature::Expense,
];

fn valuation_label(v: ValuationMethod) -> &'static str {
    match v {
        ValuationMethod::AverageCost => "Average cost",
        ValuationMethod::Fifo => "FIFO",
        ValuationMethod::LastPurchaseCost => "Last purchase cost",
        ValuationMethod::StandardCost => "Standard cost",
    }
}

const VALUATIONS: [ValuationMethod; 4] = [
    ValuationMethod::AverageCost,
    ValuationMethod::Fifo,
    ValuationMethod::LastPurchaseCost,
    ValuationMethod::StandardCost,
];

fn tax_kind_label(k: TaxLedgerKind) -> &'static str {
    match k {
        TaxLedgerKind::Cgst => "CGST",
        TaxLedgerKind::Sgst => "SGST",
        TaxLedgerKind::Utgst => "UTGST",
        TaxLedgerKind::Igst => "IGST",
        TaxLedgerKind::Cess => "Cess",
        TaxLedgerKind::Tds => "TDS",
        TaxLedgerKind::Tcs => "TCS",
        TaxLedgerKind::Other => "Other",
    }
}

const TAX_KINDS: [TaxLedgerKind; 8] = [
    TaxLedgerKind::Cgst,
    TaxLedgerKind::Sgst,
    TaxLedgerKind::Utgst,
    TaxLedgerKind::Igst,
    TaxLedgerKind::Cess,
    TaxLedgerKind::Tds,
    TaxLedgerKind::Tcs,
    TaxLedgerKind::Other,
];

fn taxability_label(t: Taxability) -> &'static str {
    match t {
        Taxability::Taxable => "Taxable",
        Taxability::Exempt => "Exempt",
        Taxability::NilRated => "Nil rated",
        Taxability::NonGst => "Non-GST",
    }
}

const TAXABILITIES: [Taxability; 4] = [
    Taxability::Taxable,
    Taxability::Exempt,
    Taxability::NilRated,
    Taxability::NonGst,
];

/// A one-line text field with a label.
fn text_row(ui: &mut Ui, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::TextEdit::singleline(value).desired_width(FIELD_WIDTH));
    });
}

/// A multi-line text field with a label.
fn area_row(ui: &mut Ui, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(
            egui::TextEdit::multiline(value)
                .desired_rows(3)
                .desired_width(FIELD_WIDTH),
        );
    });
}

/// A drop-down over `options`, with a first entry that clears the choice.
fn choose_opt<T: Copy + PartialEq>(
    ui: &mut Ui,
    salt: &str,
    label: &str,
    current: &mut Option<T>,
    none_label: &str,
    options: &[(T, String)],
) {
    let shown = (*current)
        .and_then(|c| options.iter().find(|(v, _)| *v == c))
        .map_or(none_label, |(_, name)| name.as_str());
    ui.horizontal(|ui| {
        ui.label(label);
        egui::ComboBox::from_id_salt(salt)
            .selected_text(shown)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut *current, None, none_label);
                for (value, name) in options {
                    ui.selectable_value(&mut *current, Some(*value), name.as_str());
                }
            });
    });
}

/// A drop-down over a fixed list of choices, one of which is always selected.
fn choose_one<T: Copy + PartialEq>(
    ui: &mut Ui,
    salt: &str,
    label: &str,
    current: &mut T,
    options: &[(T, &'static str)],
) {
    let shown = options
        .iter()
        .find(|(v, _)| *v == *current)
        .map_or("", |(_, name)| *name);
    ui.horizontal(|ui| {
        ui.label(label);
        egui::ComboBox::from_id_salt(salt)
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for (value, name) in options {
                    ui.selectable_value(&mut *current, *value, *name);
                }
            });
    });
}

fn labelled<T: Copy>(all: &[T], label: fn(T) -> &'static str) -> Vec<(T, &'static str)> {
    all.iter().map(|v| (*v, label(*v))).collect()
}

fn named<T: Copy>(all: &[T], label: fn(T) -> &'static str) -> Vec<(T, String)> {
    all.iter().map(|v| (*v, label(*v).to_owned())).collect()
}

/// Sorted `(id, name)` options, skipping `exclude` (the master being altered).
fn sorted_options<T: Copy>(
    items: impl Iterator<Item = (T, u64, String)>,
    exclude: Option<u64>,
) -> Vec<(T, String)> {
    let mut v: Vec<(T, u64, String)> = items.filter(|(_, id, _)| Some(*id) != exclude).collect();
    v.sort_by_cached_key(|(_, id, name)| (name.to_lowercase(), *id));
    v.into_iter().map(|(t, _, name)| (t, name)).collect()
}

fn optional_section<T>(
    ui: &mut Ui,
    label: &str,
    slot: &mut Option<T>,
    fresh: T,
    body: impl FnOnce(&mut Ui, &mut T),
) {
    let mut on = slot.is_some();
    if ui.checkbox(&mut on, label).changed() {
        *slot = on.then_some(fresh);
    }
    if let Some(value) = slot.as_mut() {
        ui.indent(label, |ui| body(ui, value));
    }
}

/// Draws the form for `draft`. `editing` is the id of the master being altered, so it is left
/// out of the lists of possible parents.
pub(crate) fn draw_form(
    ui: &mut Ui,
    masters: &Masters,
    draft: &mut MasterDraft,
    editing: Option<u64>,
) {
    match draft {
        MasterDraft::Group(d) => draw_group(ui, masters, d, editing),
        MasterDraft::Ledger(d) => draw_ledger(ui, masters, d),
        MasterDraft::Unit(d) => draw_unit(ui, d),
        MasterDraft::StockGroup(d) => {
            text_row(ui, "Name", &mut d.name);
            let options = sorted_options(
                masters
                    .stock_groups
                    .values()
                    .map(|g| (g.id.0, g.id.0, g.name.clone())),
                editing,
            );
            choose_opt(
                ui,
                "sg-parent",
                "Under",
                &mut d.parent,
                "(Primary)",
                &options,
            );
        }
        MasterDraft::Godown(d) => draw_godown(ui, masters, d, editing),
        MasterDraft::CostCategory(d) => text_row(ui, "Name", &mut d.name),
        MasterDraft::CostCentre(d) => draw_cost_centre(ui, masters, d, editing),
        MasterDraft::StockItem(d) => draw_stock_item(ui, masters, d),
    }
}

fn draw_godown(ui: &mut Ui, masters: &Masters, d: &mut TreeDraft, editing: Option<u64>) {
    text_row(ui, "Name", &mut d.name);
    let options = sorted_options(
        masters
            .godowns
            .values()
            .map(|g| (g.id.0, g.id.0, g.name.clone())),
        editing,
    );
    choose_opt(
        ui,
        "gd-parent",
        "Under",
        &mut d.parent,
        "(Primary)",
        &options,
    );
}

fn draw_cost_centre(ui: &mut Ui, masters: &Masters, d: &mut CostCentreDraft, editing: Option<u64>) {
    text_row(ui, "Name", &mut d.name);
    let categories = sorted_options(
        masters
            .cost_categories
            .values()
            .map(|c| (c.id, c.id.0, c.name.clone())),
        None,
    );
    choose_opt(
        ui,
        "cc-category",
        "Category",
        &mut d.category,
        "(choose)",
        &categories,
    );
    let centres = sorted_options(
        masters
            .cost_centres
            .values()
            .map(|c| (c.id, c.id.0, c.name.clone())),
        editing,
    );
    choose_opt(
        ui,
        "cc-parent",
        "Under",
        &mut d.parent,
        "(Primary)",
        &centres,
    );
}

fn draw_group(ui: &mut Ui, masters: &Masters, d: &mut GroupDraft, editing: Option<u64>) {
    text_row(ui, "Name", &mut d.name);
    text_row(ui, "Aliases", &mut d.aliases);
    let groups = sorted_options(
        masters
            .groups
            .values()
            .map(|g| (g.id, g.id.0, g.name.clone())),
        editing,
    );
    choose_opt(ui, "g-parent", "Under", &mut d.parent, "(Primary)", &groups);
    if d.parent.is_none() {
        choose_opt(
            ui,
            "g-nature",
            "Nature",
            &mut d.nature,
            "(choose)",
            &named(&NATURES, nature_label),
        );
        ui.checkbox(&mut d.affects_gross_profit, "Affects gross profit");
    } else {
        ui.weak("Nature and gross-profit treatment come from the primary group.");
    }
}

fn draw_ledger(ui: &mut Ui, masters: &Masters, d: &mut LedgerDraft) {
    text_row(ui, "Name", &mut d.name);
    text_row(ui, "Aliases", &mut d.aliases);
    let groups = sorted_options(
        masters
            .groups
            .values()
            .map(|g| (g.id, g.id.0, g.name.clone())),
        None,
    );
    choose_opt(ui, "l-group", "Under", &mut d.group, "(choose)", &groups);
    ui.horizontal(|ui| {
        ui.label("Opening balance");
        ui.add(egui::TextEdit::singleline(&mut d.opening).desired_width(160.0));
        let sides = [(DrCr::Dr, "Dr"), (DrCr::Cr, "Cr")];
        choose_one(ui, "l-side", "", &mut d.opening_side, &sides);
    });
    ui.checkbox(&mut d.bill_wise, "Bill-wise details");
    if d.bill_wise {
        text_row(ui, "Default credit days", &mut d.credit_days);
    }
    ui.checkbox(&mut d.cost_centres, "Cost centres apply");
    optional_section(
        ui,
        "Party details",
        &mut d.party,
        PartyDraft::default(),
        draw_party,
    );
    optional_section(
        ui,
        "Bank details",
        &mut d.bank,
        BankDraft::default(),
        draw_bank,
    );
    let tax = TaxDraft {
        kind: TaxLedgerKind::Cgst,
        rate_percent: String::new(),
    };
    optional_section(ui, "Tax details", &mut d.tax, tax, draw_tax);
    let gst = GstDraft {
        hsn: String::new(),
        taxability: Taxability::Taxable,
        rate_percent: String::new(),
    };
    optional_section(ui, "GST classification", &mut d.gst, gst, draw_gst);
}

fn draw_party(ui: &mut Ui, p: &mut PartyDraft) {
    choose_one(
        ui,
        "p-reg",
        "Registration",
        &mut p.registration,
        &labelled(&REGISTRATIONS, registration_label),
    );
    text_row(ui, "GSTIN", &mut p.gstin);
    draw_gstin_check(ui, &check_gstin(&p.gstin));
    text_row(ui, "PAN", &mut p.pan);
    let states: Vec<(StateCode, String)> = StateCode::all()
        .map(|s| (s, format!("{:02} {}", s.get(), s.name())))
        .collect();
    choose_opt(
        ui,
        "p-state",
        "State",
        &mut p.state,
        "(from GSTIN)",
        &states,
    );
    area_row(ui, "Address", &mut p.address_lines);
    text_row(ui, "PIN code", &mut p.pincode);
    text_row(ui, "Country", &mut p.country);
}

fn draw_bank(ui: &mut Ui, b: &mut BankDraft) {
    text_row(ui, "Account number", &mut b.account_number);
    text_row(ui, "IFSC", &mut b.ifsc);
    text_row(ui, "Bank name", &mut b.bank_name);
    text_row(ui, "Branch", &mut b.branch);
}

fn draw_tax(ui: &mut Ui, t: &mut TaxDraft) {
    choose_one(
        ui,
        "t-kind",
        "Tax",
        &mut t.kind,
        &labelled(&TAX_KINDS, tax_kind_label),
    );
    text_row(ui, "Rate %", &mut t.rate_percent);
}

fn draw_gst(ui: &mut Ui, g: &mut GstDraft) {
    text_row(ui, "HSN/SAC", &mut g.hsn);
    choose_one(
        ui,
        "g-tax",
        "Taxability",
        &mut g.taxability,
        &labelled(&TAXABILITIES, taxability_label),
    );
    text_row(ui, "Rate %", &mut g.rate_percent);
}

fn draw_unit(ui: &mut Ui, d: &mut UnitDraft) {
    text_row(ui, "Symbol", &mut d.symbol);
    text_row(ui, "Formal name", &mut d.formal_name);
    text_row(ui, "Decimal places", &mut d.decimals);
    text_row(ui, "GST unit code (UQC)", &mut d.uqc);
}

fn draw_stock_item(ui: &mut Ui, masters: &Masters, d: &mut StockItemDraft) {
    text_row(ui, "Name", &mut d.name);
    text_row(ui, "Aliases", &mut d.aliases);
    let groups = sorted_options(
        masters
            .stock_groups
            .values()
            .map(|g| (g.id, g.id.0, g.name.clone())),
        None,
    );
    choose_opt(ui, "i-group", "Under", &mut d.group, "(none)", &groups);
    let units = sorted_options(
        masters
            .units
            .values()
            .map(|u| (u.id, u.id.0, u.symbol.clone())),
        None,
    );
    choose_opt(ui, "i-unit", "Unit", &mut d.unit, "(choose)", &units);
    choose_one(
        ui,
        "i-valuation",
        "Valuation",
        &mut d.valuation,
        &labelled(&VALUATIONS, valuation_label),
    );
    text_row(ui, "HSN/SAC", &mut d.hsn);
    ui.checkbox(&mut d.batches, "Track batches");
}
