//! Drawing of the master forms, laid out as the mock's voucher sheet (`design/mock-v2/index.html`
//! lines 233-241 and 1802-1812): fields in a grid of four columns, each with its label above it,
//! a wide field taking two columns (`.fld.span2`).
//!
//! Drawing only: every edit goes straight into the draft, and nothing is checked here. The
//! checks live in [`super::draft`], so the same rules apply to a click and to a test.

use egui::{Align, Align2, FontId, Layout, RichText, Ui, vec2};
use ledgerly_core::{DrCr, StateCode};
use ledgerly_model::{
    Masters, Nature, RegistrationType, TaxLedgerKind, Taxability, ValuationMethod,
};

use super::draft::{
    BankDraft, CostCentreDraft, GroupDraft, GstDraft, LedgerDraft, MasterDraft, PartyDraft,
    StockItemDraft, TaxDraft, TreeDraft, UnitDraft,
};
use super::view::{GstinCheck, check_gstin, gstin_hint};
use crate::theme::{TEXT_INPUT, TEXT_META};
use crate::ui::badge::{BadgeKind, badge};
use crate::ui::icon::{self, Icon};
use crate::ui::tokens;

/// Columns of the sheet (`.vhead{grid-template-columns:repeat(4,..)}`).
const COLUMNS: u8 = 4;
/// Gap between columns and between rows (`gap:12px 16px`).
const GAP_X: f32 = 16.0;
const GAP_Y: f32 = 12.0;
/// Height of a one-line field and of a read-only value (`.ro{height:32px}`).
const FIELD_HEIGHT: f32 = 32.0;
/// Gap between a label and its field (`.fld{gap:4px}`).
const LABEL_GAP: f32 = 4.0;
/// The least width of one column, so a narrow window still shows something.
const COLUMN_MIN: f32 = 60.0;

/// Draws the result of the GSTIN check under the GSTIN field (index.html:1810): nothing when the
/// field is empty; "Check character OK" with the PAN and state the GSTIN holds when every test
/// passes; the reason in the error ink when one fails.
pub(crate) fn draw_gstin_check(ui: &mut Ui, text: &str) {
    match check_gstin(text) {
        GstinCheck::Empty => {}
        GstinCheck::Valid => {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                badge(ui, BadgeKind::Ok, "Check character OK");
                if let Some(hint) = gstin_hint(text) {
                    let t = tokens(ui);
                    ui.label(RichText::new(hint).size(TEXT_META).color(t.text_3));
                }
            });
        }
        GstinCheck::Invalid(reason) => {
            let t = tokens(ui);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let (spot, _) = ui.allocate_exact_size(vec2(14.0, 16.0), egui::Sense::hover());
                let glyph = egui::Rect::from_center_size(spot.center(), egui::Vec2::splat(14.0));
                icon::paint(ui.painter(), glyph, Icon::X, t.bad);
                ui.label(RichText::new(reason).size(TEXT_META).color(t.bad));
            });
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

/// The width of a field `span` columns wide in the sheet `ui` lays out.
fn span_width(ui: &Ui, span: u8) -> f32 {
    let gaps = GAP_X * f32::from(COLUMNS - 1);
    let column = ((ui.max_rect().width() - gaps) / f32::from(COLUMNS)).max(COLUMN_MIN);
    column * f32::from(span) + GAP_X * f32::from(span.saturating_sub(1))
}

/// One field (`.fld`): `label` in the label style, then `body`, `span` columns wide.
fn fld(ui: &mut Ui, span: u8, label: &str, body: impl FnOnce(&mut Ui)) {
    let width = span_width(ui, span);
    ui.allocate_ui_with_layout(vec2(width, 0.0), Layout::top_down(Align::Min), |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing = vec2(6.0, LABEL_GAP);
        let t = tokens(ui);
        ui.label(RichText::new(label).size(TEXT_META).color(t.text_2));
        body(ui);
    });
}

/// A one-line input as wide as its field, 32 points tall.
fn input(ui: &mut Ui, value: &mut String) -> egui::Response {
    ui.add(
        egui::TextEdit::singleline(value)
            .font(FontId::proportional(TEXT_INPUT))
            .align(Align2::LEFT_CENTER)
            .min_size(vec2(0.0, FIELD_HEIGHT))
            .desired_width(ui.available_width()),
    )
}

/// A one-line text field.
fn text_row(ui: &mut Ui, span: u8, label: &str, value: &mut String) {
    fld(ui, span, label, |ui| {
        input(ui, value);
    });
}

/// A one-line field for a number, right-aligned (`input.num`).
fn number_row(ui: &mut Ui, span: u8, label: &str, value: &mut String) {
    fld(ui, span, label, |ui| {
        ui.add(
            egui::TextEdit::singleline(value)
                .font(FontId::proportional(TEXT_INPUT))
                .align(Align2::RIGHT_CENTER)
                .min_size(vec2(0.0, FIELD_HEIGHT))
                .desired_width(ui.available_width()),
        );
    });
}

/// A multi-line text field.
fn area_row(ui: &mut Ui, span: u8, label: &str, value: &mut String) {
    fld(ui, span, label, |ui| {
        ui.add(
            egui::TextEdit::multiline(value)
                .font(FontId::proportional(TEXT_INPUT))
                .desired_rows(3)
                .desired_width(ui.available_width()),
        );
    });
}

/// A switch: the label above, then a check box reading "On".
fn switch_row(ui: &mut Ui, span: u8, label: &str, value: &mut bool) {
    fld(ui, span, label, |ui| {
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), FIELD_HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.checkbox(value, "On");
            },
        );
    });
}

/// A value shown, not edited (`.ro`): the text in the input size, 32 points tall.
fn read_only_row(ui: &mut Ui, span: u8, label: &str, text: &str) {
    fld(ui, span, label, |ui| {
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), FIELD_HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| {
                let t = tokens(ui);
                ui.label(RichText::new(text).size(TEXT_INPUT).color(t.text));
            },
        );
    });
}

/// A note across the sheet, in the hint style.
fn sheet_note(ui: &mut Ui, text: &str) {
    fld(ui, COLUMNS, "", |ui| {
        let t = tokens(ui);
        ui.label(RichText::new(text).size(TEXT_META).color(t.text_3));
    });
}

/// A drop-down over `options` as wide as its field, with a first entry that clears the choice.
fn choose_opt<T: Copy + PartialEq>(
    ui: &mut Ui,
    span: u8,
    salt: &str,
    label: &str,
    current: &mut Option<T>,
    none_label: &str,
    options: &[(T, String)],
) {
    let shown = (*current)
        .and_then(|c| options.iter().find(|(v, _)| *v == c))
        .map_or(none_label, |(_, name)| name.as_str());
    fld(ui, span, label, |ui| {
        egui::ComboBox::from_id_salt(salt)
            .selected_text(shown)
            .width(ui.available_width())
            .height(FIELD_HEIGHT * 8.0)
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
    span: u8,
    salt: &str,
    label: &str,
    current: &mut T,
    options: &[(T, &'static str)],
) {
    let shown = options
        .iter()
        .find(|(v, _)| *v == *current)
        .map_or("", |(_, name)| *name);
    fld(ui, span, label, |ui| {
        egui::ComboBox::from_id_salt(salt)
            .selected_text(shown)
            .width(ui.available_width())
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

/// A switch that adds or drops an optional section of the draft.
fn section_switch<T>(ui: &mut Ui, label: &str, slot: &mut Option<T>, fresh: impl FnOnce() -> T) {
    let mut on = slot.is_some();
    switch_row(ui, 1, label, &mut on);
    if on != slot.is_some() {
        *slot = on.then(fresh);
    }
}

/// Draws the form for `draft` as the sheet. `editing` is the id of the master being altered, so
/// it is left out of the lists of possible parents.
pub(crate) fn draw_form(
    ui: &mut Ui,
    masters: &Masters,
    draft: &mut MasterDraft,
    editing: Option<u64>,
    balance: Option<&str>,
) {
    let width = ui.available_width();
    ui.allocate_ui_with_layout(
        vec2(width, 0.0),
        Layout::left_to_right(Align::Min).with_main_wrap(true),
        |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing = vec2(GAP_X, GAP_Y);
            draw_fields(ui, masters, draft, editing, balance);
        },
    );
}

/// The fields of each kind, in sheet order.
fn draw_fields(
    ui: &mut Ui,
    masters: &Masters,
    draft: &mut MasterDraft,
    editing: Option<u64>,
    balance: Option<&str>,
) {
    match draft {
        MasterDraft::Group(d) => draw_group(ui, masters, d, editing),
        MasterDraft::Ledger(d) => draw_ledger(ui, masters, d, balance),
        MasterDraft::Unit(d) => draw_unit(ui, d),
        MasterDraft::StockGroup(d) => {
            text_row(ui, 2, "Name", &mut d.name);
            let options = sorted_options(
                masters
                    .stock_groups
                    .values()
                    .map(|g| (g.id.0, g.id.0, g.name.clone())),
                editing,
            );
            choose_opt(
                ui,
                1,
                "sg-parent",
                "Under",
                &mut d.parent,
                "(Primary)",
                &options,
            );
        }
        MasterDraft::Godown(d) => draw_godown(ui, masters, d, editing),
        MasterDraft::CostCategory(d) => text_row(ui, 2, "Name", &mut d.name),
        MasterDraft::CostCentre(d) => draw_cost_centre(ui, masters, d, editing),
        MasterDraft::StockItem(d) => draw_stock_item(ui, masters, d),
    }
}

fn draw_godown(ui: &mut Ui, masters: &Masters, d: &mut TreeDraft, editing: Option<u64>) {
    text_row(ui, 2, "Name", &mut d.name);
    let options = sorted_options(
        masters
            .godowns
            .values()
            .map(|g| (g.id.0, g.id.0, g.name.clone())),
        editing,
    );
    choose_opt(
        ui,
        1,
        "gd-parent",
        "Under",
        &mut d.parent,
        "(Primary)",
        &options,
    );
}

fn draw_cost_centre(ui: &mut Ui, masters: &Masters, d: &mut CostCentreDraft, editing: Option<u64>) {
    text_row(ui, 2, "Name", &mut d.name);
    let categories = sorted_options(
        masters
            .cost_categories
            .values()
            .map(|c| (c.id, c.id.0, c.name.clone())),
        None,
    );
    choose_opt(
        ui,
        1,
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
        1,
        "cc-parent",
        "Under",
        &mut d.parent,
        "(Primary)",
        &centres,
    );
}

fn draw_group(ui: &mut Ui, masters: &Masters, d: &mut GroupDraft, editing: Option<u64>) {
    text_row(ui, 2, "Name", &mut d.name);
    text_row(ui, 1, "Alias", &mut d.aliases);
    let groups = sorted_options(
        masters
            .groups
            .values()
            .map(|g| (g.id, g.id.0, g.name.clone())),
        editing,
    );
    choose_opt(
        ui,
        1,
        "g-parent",
        "Under",
        &mut d.parent,
        "(Primary)",
        &groups,
    );
    ui.end_row();
    if d.parent.is_none() {
        choose_opt(
            ui,
            1,
            "g-nature",
            "Nature",
            &mut d.nature,
            "(choose)",
            &named(&NATURES, nature_label),
        );
        switch_row(ui, 1, "Affects gross profit", &mut d.affects_gross_profit);
    } else {
        sheet_note(
            ui,
            "Nature and gross-profit treatment come from the primary group.",
        );
    }
}

/// The ledger: the mock's fields first, in its order (index.html:1803-1805: Name, Alias, Under;
/// GSTIN, State, Credit days; Bill-wise, Balance), then the fields the mock does not show.
/// `balance` is the altered ledger's closing balance as the engine gave it, shown read-only.
fn draw_ledger(ui: &mut Ui, masters: &Masters, d: &mut LedgerDraft, balance: Option<&str>) {
    text_row(ui, 2, "Name", &mut d.name);
    text_row(ui, 1, "Alias", &mut d.aliases);
    let groups = sorted_options(
        masters
            .groups
            .values()
            .map(|g| (g.id, g.id.0, g.name.clone())),
        None,
    );
    choose_opt(ui, 1, "l-group", "Under", &mut d.group, "(choose)", &groups);
    ui.end_row();
    if let Some(p) = d.party.as_mut() {
        draw_party_head(ui, p);
    }
    switch_row(ui, 1, "Bill-wise", &mut d.bill_wise);
    if d.bill_wise {
        number_row(ui, 1, "Credit days", &mut d.credit_days);
    }
    if let Some(text) = balance {
        read_only_row(ui, 1, "Balance", text);
    }
    fld(ui, 2, "Opening balance", |ui| {
        ui.horizontal(|ui| {
            let side_w = 72.0;
            let text_w = (ui.available_width() - side_w - 8.0).max(40.0);
            ui.add(
                egui::TextEdit::singleline(&mut d.opening)
                    .font(FontId::proportional(TEXT_INPUT))
                    .align(Align2::RIGHT_CENTER)
                    .min_size(vec2(0.0, FIELD_HEIGHT))
                    .desired_width(text_w),
            );
            let sides = [(DrCr::Dr, "Dr"), (DrCr::Cr, "Cr")];
            let shown = if d.opening_side == DrCr::Dr {
                "Dr"
            } else {
                "Cr"
            };
            egui::ComboBox::from_id_salt("l-side")
                .selected_text(shown)
                .width(side_w)
                .show_ui(ui, |ui| {
                    for (value, name) in sides {
                        ui.selectable_value(&mut d.opening_side, value, name);
                    }
                });
        });
    });
    ui.end_row();
    switch_row(ui, 1, "Cost centres", &mut d.cost_centres);
    section_switch(ui, "Party details", &mut d.party, PartyDraft::default);
    section_switch(ui, "Bank details", &mut d.bank, BankDraft::default);
    section_switch(ui, "Tax details", &mut d.tax, || TaxDraft {
        kind: TaxLedgerKind::Cgst,
        rate_percent: String::new(),
    });
    ui.end_row();
    section_switch(ui, "GST classification", &mut d.gst, || GstDraft {
        hsn: String::new(),
        taxability: Taxability::Taxable,
        rate_percent: String::new(),
    });
    ui.end_row();
    if let Some(p) = d.party.as_mut() {
        draw_party_rest(ui, p);
    }
    if let Some(b) = d.bank.as_mut() {
        draw_bank(ui, b);
    }
    if let Some(t) = d.tax.as_mut() {
        draw_tax(ui, t);
    }
    if let Some(g) = d.gst.as_mut() {
        draw_gst(ui, g);
    }
}

/// The party fields the mock shows on the ledger's second line: the GSTIN with its check, two
/// columns wide, then the state (index.html:1804).
fn draw_party_head(ui: &mut Ui, p: &mut PartyDraft) {
    fld(ui, 2, "GSTIN", |ui| {
        input(ui, &mut p.gstin);
        draw_gstin_check(ui, &p.gstin);
    });
    let states: Vec<(StateCode, String)> = StateCode::all()
        .map(|s| (s, format!("{:02} {}", s.get(), s.name())))
        .collect();
    // With no state chosen the GSTIN's own state is used; say which one it is.
    let from_gstin = ledgerly_core::Gstin::parse(p.gstin.trim())
        .ok()
        .and_then(|g| g.state())
        .map_or_else(
            || "From the GSTIN".to_owned(),
            |s| format!("{:02} {} (from GSTIN)", s.get(), s.name()),
        );
    choose_opt(
        ui,
        1,
        "p-state",
        "State",
        &mut p.state,
        &from_gstin,
        &states,
    );
}

/// The party fields the mock does not show, on lines of their own.
fn draw_party_rest(ui: &mut Ui, p: &mut PartyDraft) {
    choose_one(
        ui,
        1,
        "p-reg",
        "Registration",
        &mut p.registration,
        &labelled(&REGISTRATIONS, registration_label),
    );
    text_row(ui, 1, "PAN", &mut p.pan);
    text_row(ui, 1, "PIN code", &mut p.pincode);
    text_row(ui, 1, "Country", &mut p.country);
    ui.end_row();
    area_row(ui, 2, "Address", &mut p.address_lines);
    ui.end_row();
}

fn draw_bank(ui: &mut Ui, b: &mut BankDraft) {
    text_row(ui, 1, "Account number", &mut b.account_number);
    text_row(ui, 1, "IFSC", &mut b.ifsc);
    text_row(ui, 1, "Bank name", &mut b.bank_name);
    text_row(ui, 1, "Branch", &mut b.branch);
    ui.end_row();
}

fn draw_tax(ui: &mut Ui, t: &mut TaxDraft) {
    choose_one(
        ui,
        1,
        "t-kind",
        "Tax",
        &mut t.kind,
        &labelled(&TAX_KINDS, tax_kind_label),
    );
    number_row(ui, 1, "Rate %", &mut t.rate_percent);
    ui.end_row();
}

fn draw_gst(ui: &mut Ui, g: &mut GstDraft) {
    text_row(ui, 1, "HSN/SAC", &mut g.hsn);
    choose_one(
        ui,
        1,
        "g-tax",
        "Taxability",
        &mut g.taxability,
        &labelled(&TAXABILITIES, taxability_label),
    );
    number_row(ui, 1, "Rate %", &mut g.rate_percent);
    ui.end_row();
}

fn draw_unit(ui: &mut Ui, d: &mut UnitDraft) {
    text_row(ui, 1, "Symbol", &mut d.symbol);
    text_row(ui, 2, "Formal name", &mut d.formal_name);
    number_row(ui, 1, "Decimal places", &mut d.decimals);
    ui.end_row();
    text_row(ui, 1, "GST unit code (UQC)", &mut d.uqc);
}

fn draw_stock_item(ui: &mut Ui, masters: &Masters, d: &mut StockItemDraft) {
    text_row(ui, 2, "Name", &mut d.name);
    text_row(ui, 1, "Alias", &mut d.aliases);
    let groups = sorted_options(
        masters
            .stock_groups
            .values()
            .map(|g| (g.id, g.id.0, g.name.clone())),
        None,
    );
    choose_opt(ui, 1, "i-group", "Under", &mut d.group, "(none)", &groups);
    ui.end_row();
    let units = sorted_options(
        masters
            .units
            .values()
            .map(|u| (u.id, u.id.0, u.symbol.clone())),
        None,
    );
    choose_opt(ui, 1, "i-unit", "Unit", &mut d.unit, "(choose)", &units);
    choose_one(
        ui,
        1,
        "i-valuation",
        "Valuation",
        &mut d.valuation,
        &labelled(&VALUATIONS, valuation_label),
    );
    text_row(ui, 1, "HSN/SAC", &mut d.hsn);
    switch_row(ui, 1, "Track batches", &mut d.batches);
}
