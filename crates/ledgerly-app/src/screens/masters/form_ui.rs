//! Drawing of the master forms, laid out as the mock's voucher sheet (`design/mock-v2/index.html`
//! lines 233-241 and 1802-1812): fields in a grid of four columns, each with its label above it,
//! a wide field taking two columns (`.fld.span2`).
//!
//! Drawing only: every edit goes straight into the draft, and nothing is checked here. The
//! checks live in [`super::draft`], so the same rules apply to a click and to a test.
//!
//! Enter in a one-line field moves to the next field and Shift+Enter to the previous one
//! (index.html:1831 "Enter Next field"; 1344 "Shift+Enter Previous"), as Tab and Shift+Tab do. A
//! multi-line field keeps Enter for a new line.

use crate::i18n::Tr;
use egui::{Align, Align2, FocusDirection, FontId, Id, Key, Layout, Modifiers, RichText, Ui, vec2};
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

/// Where the form keeps the ids of the one-line fields it drew, in order.
fn lines_key() -> Id {
    Id::unique("masters-form-lines")
}

/// The ids of the form's one-line fields, in the order the last frame drew them. A test gives
/// one the keyboard with these.
#[must_use]
pub fn line_ids(ctx: &egui::Context) -> Vec<Id> {
    ctx.data(|d| d.get_temp::<Vec<Id>>(lines_key()))
        .unwrap_or_default()
}

/// The id of the party's address, the form's multi-line field.
#[must_use]
pub fn address_id() -> Id {
    Id::unique("masters-form-address")
}

/// Turns Enter in a one-line field into a move to the next field, and Shift+Enter into a move to
/// the previous one. Runs before the fields draw: egui moves the keyboard when the focused field
/// says it wants focus, and forgets the direction at the start of each frame. The fields are the
/// ones the last frame drew; the list is drawn afresh after this.
fn enter_moves_focus(ui: &Ui) {
    let ctx = ui.ctx();
    let last = ctx
        .data_mut(|d| d.remove_temp::<Vec<Id>>(lines_key()))
        .unwrap_or_default();
    let Some(focused) = ctx.memory(egui::Memory::focused) else {
        return;
    };
    if !last.contains(&focused) {
        return;
    }
    let direction = ctx.input_mut(|i| {
        if i.consume_key(Modifiers::SHIFT, Key::Enter) {
            FocusDirection::Previous
        } else if i.consume_key(Modifiers::NONE, Key::Enter) {
            FocusDirection::Next
        } else {
            FocusDirection::None
        }
    });
    if direction != FocusDirection::None {
        ctx.memory_mut(|m| m.move_focus(direction));
    }
}

/// Adds a one-line field and notes its id for [`enter_moves_focus`].
fn one_line(ui: &mut Ui, edit: egui::TextEdit<'_>) -> egui::Response {
    let response = ui.add(edit);
    ui.data_mut(|d| {
        d.get_temp_mut_or_default::<Vec<Id>>(lines_key())
            .push(response.id);
    });
    response
}

/// Draws the result of the GSTIN check under the GSTIN field (index.html:1810): nothing when the
/// field is empty; "Check character OK" with the PAN and state the GSTIN holds when every test
/// passes; the reason in the error ink when one fails.
pub(crate) fn draw_gstin_check(ui: &mut Ui, text: &str, tr: &Tr) {
    match check_gstin(text) {
        GstinCheck::Empty => {}
        GstinCheck::Valid => {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                badge(ui, BadgeKind::Ok, &tr.t("masters.check_ok"));
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

fn registration_label(r: RegistrationType, tr: &Tr) -> String {
    match r {
        RegistrationType::Regular => tr.t("masters.reg.regular"),
        RegistrationType::Composition => tr.t("masters.reg.composition"),
        RegistrationType::Unregistered => tr.t("masters.reg.unregistered"),
        RegistrationType::Consumer => tr.t("masters.reg.consumer"),
        RegistrationType::Sez => tr.t("masters.reg.sez"),
        RegistrationType::Overseas => tr.t("masters.reg.overseas"),
        RegistrationType::Uin => tr.t("masters.reg.uin"),
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

fn nature_label(n: Nature, tr: &Tr) -> String {
    match n {
        Nature::Asset => tr.t("masters.nature.asset"),
        Nature::Liability => tr.t("masters.nature.liability"),
        Nature::Income => tr.t("masters.nature.income"),
        Nature::Expense => tr.t("masters.nature.expense"),
    }
}

const NATURES: [Nature; 4] = [
    Nature::Asset,
    Nature::Liability,
    Nature::Income,
    Nature::Expense,
];

fn valuation_label(v: ValuationMethod, tr: &Tr) -> String {
    match v {
        ValuationMethod::AverageCost => tr.t("masters.valuation.average_cost"),
        ValuationMethod::Fifo => tr.t("masters.valuation.fifo"),
        ValuationMethod::LastPurchaseCost => tr.t("masters.valuation.last_purchase_cost"),
        ValuationMethod::StandardCost => tr.t("masters.valuation.standard_cost"),
    }
}

const VALUATIONS: [ValuationMethod; 4] = [
    ValuationMethod::AverageCost,
    ValuationMethod::Fifo,
    ValuationMethod::LastPurchaseCost,
    ValuationMethod::StandardCost,
];

fn tax_kind_label(k: TaxLedgerKind, tr: &Tr) -> String {
    match k {
        TaxLedgerKind::Cgst => "CGST".to_owned(),
        TaxLedgerKind::Sgst => "SGST".to_owned(),
        TaxLedgerKind::Utgst => "UTGST".to_owned(),
        TaxLedgerKind::Igst => "IGST".to_owned(),
        TaxLedgerKind::Cess => "Cess".to_owned(),
        TaxLedgerKind::Tds => "TDS".to_owned(),
        TaxLedgerKind::Tcs => "TCS".to_owned(),
        TaxLedgerKind::Other => tr.t("masters.tax_kind.other"),
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

fn taxability_label(t: Taxability, tr: &Tr) -> String {
    match t {
        Taxability::Taxable => tr.t("masters.taxability.taxable"),
        Taxability::Exempt => tr.t("masters.taxability.exempt"),
        Taxability::NilRated => tr.t("masters.taxability.nil_rated"),
        Taxability::NonGst => tr.t("masters.taxability.non_gst"),
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
    one_line(
        ui,
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
        one_line(
            ui,
            egui::TextEdit::singleline(value)
                .font(FontId::proportional(TEXT_INPUT))
                .align(Align2::RIGHT_CENTER)
                .min_size(vec2(0.0, FIELD_HEIGHT))
                .desired_width(ui.available_width()),
        );
    });
}

/// A multi-line text field.
fn area_row(ui: &mut Ui, span: u8, id: Id, label: &str, value: &mut String) {
    fld(ui, span, label, |ui| {
        ui.add(
            egui::TextEdit::multiline(value)
                .id(id)
                .font(FontId::proportional(TEXT_INPUT))
                .desired_rows(3)
                .desired_width(ui.available_width()),
        );
    });
}

/// A switch: the label above, then a check box reading "On".
fn switch_row(ui: &mut Ui, tr: &Tr, span: u8, label: &str, value: &mut bool) {
    fld(ui, span, label, |ui| {
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), FIELD_HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.checkbox(value, tr.t("masters.field.on"));
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
    options: &[(T, String)],
) {
    let shown = options
        .iter()
        .find(|(v, _)| *v == *current)
        .map_or("", |(_, name)| name.as_str());
    fld(ui, span, label, |ui| {
        egui::ComboBox::from_id_salt(salt)
            .selected_text(shown)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (value, name) in options {
                    ui.selectable_value(&mut *current, *value, name.as_str());
                }
            });
    });
}

fn labelled<T: Copy>(all: &[T], label: impl Fn(T) -> String) -> Vec<(T, String)> {
    all.iter().map(|v| (*v, label(*v))).collect()
}

fn named<T: Copy>(all: &[T], label: impl Fn(T) -> String) -> Vec<(T, String)> {
    all.iter().map(|v| (*v, label(*v))).collect()
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
fn section_switch<T>(
    ui: &mut Ui,
    tr: &Tr,
    label: &str,
    slot: &mut Option<T>,
    fresh: impl FnOnce() -> T,
) {
    let mut on = slot.is_some();
    switch_row(ui, tr, 1, label, &mut on);
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
    tr: &Tr,
) {
    enter_moves_focus(ui);
    let width = ui.available_width();
    ui.allocate_ui_with_layout(
        vec2(width, 0.0),
        Layout::left_to_right(Align::Min).with_main_wrap(true),
        |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing = vec2(GAP_X, GAP_Y);
            draw_fields(ui, masters, draft, editing, balance, tr);
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
    tr: &Tr,
) {
    match draft {
        MasterDraft::Group(d) => draw_group(ui, masters, d, editing, tr),
        MasterDraft::Ledger(d) => draw_ledger(ui, masters, d, balance, tr),
        MasterDraft::Unit(d) => draw_unit(ui, d, tr),
        MasterDraft::StockGroup(d) => {
            text_row(ui, 2, &tr.t("masters.field.name"), &mut d.name);
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
                &tr.t("masters.field.under"),
                &mut d.parent,
                &tr.t("masters.field.primary"),
                &options,
            );
        }
        MasterDraft::Godown(d) => draw_godown(ui, masters, d, editing, tr),
        MasterDraft::CostCategory(d) => text_row(ui, 2, &tr.t("masters.field.name"), &mut d.name),
        MasterDraft::CostCentre(d) => draw_cost_centre(ui, masters, d, editing, tr),
        MasterDraft::StockItem(d) => draw_stock_item(ui, masters, d, tr),
    }
}

fn draw_godown(ui: &mut Ui, masters: &Masters, d: &mut TreeDraft, editing: Option<u64>, tr: &Tr) {
    text_row(ui, 2, &tr.t("masters.field.name"), &mut d.name);
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
        &tr.t("masters.field.under"),
        &mut d.parent,
        &tr.t("masters.field.primary"),
        &options,
    );
}

fn draw_cost_centre(
    ui: &mut Ui,
    masters: &Masters,
    d: &mut CostCentreDraft,
    editing: Option<u64>,
    tr: &Tr,
) {
    text_row(ui, 2, &tr.t("masters.field.name"), &mut d.name);
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
        &tr.t("masters.field.category"),
        &mut d.category,
        &tr.t("masters.field.choose"),
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
        &tr.t("masters.field.under"),
        &mut d.parent,
        &tr.t("masters.field.primary"),
        &centres,
    );
}

fn draw_group(ui: &mut Ui, masters: &Masters, d: &mut GroupDraft, editing: Option<u64>, tr: &Tr) {
    text_row(ui, 2, &tr.t("masters.field.name"), &mut d.name);
    text_row(ui, 1, &tr.t("masters.field.alias"), &mut d.aliases);
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
        &tr.t("masters.field.under"),
        &mut d.parent,
        &tr.t("masters.field.primary"),
        &groups,
    );
    ui.end_row();
    if d.parent.is_none() {
        choose_opt(
            ui,
            1,
            "g-nature",
            &tr.t("masters.field.nature"),
            &mut d.nature,
            &tr.t("masters.field.choose"),
            &named(&NATURES, |n| nature_label(n, tr)),
        );
        switch_row(
            ui,
            tr,
            1,
            &tr.t("masters.field.affects_gross_profit"),
            &mut d.affects_gross_profit,
        );
    } else {
        sheet_note(ui, &tr.t("masters.field.nature_note"));
    }
}

/// The ledger: the mock's fields first, in its order (index.html:1803-1805: Name, Alias, Under;
/// GSTIN, State, Credit days; Bill-wise, Balance), then the fields the mock does not show.
/// `balance` is the altered ledger's closing balance as the engine gave it, shown read-only.
fn draw_ledger(
    ui: &mut Ui,
    masters: &Masters,
    d: &mut LedgerDraft,
    balance: Option<&str>,
    tr: &Tr,
) {
    text_row(ui, 2, &tr.t("masters.field.name"), &mut d.name);
    text_row(ui, 1, &tr.t("masters.field.alias"), &mut d.aliases);
    let groups = sorted_options(
        masters
            .groups
            .values()
            .map(|g| (g.id, g.id.0, g.name.clone())),
        None,
    );
    choose_opt(
        ui,
        1,
        "l-group",
        &tr.t("masters.field.under"),
        &mut d.group,
        &tr.t("masters.field.choose"),
        &groups,
    );
    ui.end_row();
    if let Some(p) = d.party.as_mut() {
        draw_party_head(ui, p, tr);
    }
    switch_row(
        ui,
        tr,
        1,
        &tr.t("masters.field.bill_wise"),
        &mut d.bill_wise,
    );
    if d.bill_wise {
        number_row(
            ui,
            1,
            &tr.t("masters.field.credit_days"),
            &mut d.credit_days,
        );
    }
    if let Some(text) = balance {
        read_only_row(ui, 1, &tr.t("masters.field.balance"), text);
    }
    fld(ui, 2, &tr.t("masters.field.opening_balance"), |ui| {
        ui.horizontal(|ui| {
            let side_w = 72.0;
            let text_w = (ui.available_width() - side_w - 8.0).max(40.0);
            one_line(
                ui,
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
    draw_ledger_sections(ui, d, tr);
}

/// The ledger's switches and the sections they open, below the mock's fields.
fn draw_ledger_sections(ui: &mut Ui, d: &mut LedgerDraft, tr: &Tr) {
    switch_row(
        ui,
        tr,
        1,
        &tr.t("masters.kind_plural.cost_centre"),
        &mut d.cost_centres,
    );
    section_switch(
        ui,
        tr,
        &tr.t("masters.field.party_details"),
        &mut d.party,
        PartyDraft::default,
    );
    section_switch(
        ui,
        tr,
        &tr.t("masters.field.bank_details"),
        &mut d.bank,
        BankDraft::default,
    );
    section_switch(
        ui,
        tr,
        &tr.t("masters.field.tax_details"),
        &mut d.tax,
        || TaxDraft {
            kind: TaxLedgerKind::Cgst,
            rate_percent: String::new(),
        },
    );
    ui.end_row();
    section_switch(
        ui,
        tr,
        &tr.t("masters.field.gst_classification"),
        &mut d.gst,
        || GstDraft {
            hsn: String::new(),
            taxability: Taxability::Taxable,
            rate_percent: String::new(),
        },
    );
    ui.end_row();
    if let Some(p) = d.party.as_mut() {
        draw_party_rest(ui, p, tr);
    }
    if let Some(b) = d.bank.as_mut() {
        draw_bank(ui, b, tr);
    }
    if let Some(t) = d.tax.as_mut() {
        draw_tax(ui, t, tr);
    }
    if let Some(g) = d.gst.as_mut() {
        draw_gst(ui, g, tr);
    }
}

/// The party fields the mock shows on the ledger's second line: the GSTIN with its check, two
/// columns wide, then the state (index.html:1804).
fn draw_party_head(ui: &mut Ui, p: &mut PartyDraft, tr: &Tr) {
    fld(ui, 2, "GSTIN", |ui| {
        input(ui, &mut p.gstin);
        draw_gstin_check(ui, &p.gstin, tr);
    });
    let states: Vec<(StateCode, String)> = StateCode::all()
        .map(|s| (s, format!("{:02} {}", s.get(), s.name())))
        .collect();
    // With no state chosen the GSTIN's own state is used; say which one it is.
    let from_gstin = ledgerly_core::Gstin::parse(p.gstin.trim())
        .ok()
        .and_then(|g| g.state())
        .map_or_else(
            || tr.t("masters.field.from_gstin"),
            |s| {
                tr.tf(
                    "masters.field.state_from_gstin",
                    &[("state", &format!("{:02} {}", s.get(), s.name()))],
                )
            },
        );
    choose_opt(
        ui,
        1,
        "p-state",
        &tr.t("masters.field.state"),
        &mut p.state,
        &from_gstin,
        &states,
    );
}

/// The party fields the mock does not show, on lines of their own.
fn draw_party_rest(ui: &mut Ui, p: &mut PartyDraft, tr: &Tr) {
    choose_one(
        ui,
        1,
        "p-reg",
        &tr.t("masters.field.registration"),
        &mut p.registration,
        &labelled(&REGISTRATIONS, |r| registration_label(r, tr)),
    );
    text_row(ui, 1, "PAN", &mut p.pan);
    text_row(ui, 1, &tr.t("masters.field.pin_code"), &mut p.pincode);
    text_row(ui, 1, &tr.t("masters.field.country"), &mut p.country);
    ui.end_row();
    area_row(
        ui,
        2,
        address_id(),
        &tr.t("masters.field.address"),
        &mut p.address_lines,
    );
    ui.end_row();
}

fn draw_bank(ui: &mut Ui, b: &mut BankDraft, tr: &Tr) {
    text_row(
        ui,
        1,
        &tr.t("masters.field.account_number"),
        &mut b.account_number,
    );
    text_row(ui, 1, "IFSC", &mut b.ifsc);
    text_row(ui, 1, &tr.t("masters.field.bank_name"), &mut b.bank_name);
    text_row(ui, 1, &tr.t("masters.field.branch"), &mut b.branch);
    ui.end_row();
}

fn draw_tax(ui: &mut Ui, t: &mut TaxDraft, tr: &Tr) {
    choose_one(
        ui,
        1,
        "t-kind",
        &tr.t("masters.field.tax"),
        &mut t.kind,
        &labelled(&TAX_KINDS, |k| tax_kind_label(k, tr)),
    );
    number_row(
        ui,
        1,
        &tr.t("masters.field.rate_percent"),
        &mut t.rate_percent,
    );
    ui.end_row();
}

fn draw_gst(ui: &mut Ui, g: &mut GstDraft, tr: &Tr) {
    text_row(ui, 1, "HSN/SAC", &mut g.hsn);
    choose_one(
        ui,
        1,
        "g-tax",
        &tr.t("masters.field.taxability"),
        &mut g.taxability,
        &labelled(&TAXABILITIES, |t| taxability_label(t, tr)),
    );
    number_row(
        ui,
        1,
        &tr.t("masters.field.rate_percent"),
        &mut g.rate_percent,
    );
    ui.end_row();
}

fn draw_unit(ui: &mut Ui, d: &mut UnitDraft, tr: &Tr) {
    text_row(ui, 1, &tr.t("masters.field.symbol"), &mut d.symbol);
    text_row(
        ui,
        2,
        &tr.t("masters.field.formal_name"),
        &mut d.formal_name,
    );
    number_row(
        ui,
        1,
        &tr.t("masters.field.decimal_places"),
        &mut d.decimals,
    );
    ui.end_row();
    text_row(ui, 1, &tr.t("masters.field.uqc"), &mut d.uqc);
}

fn draw_stock_item(ui: &mut Ui, masters: &Masters, d: &mut StockItemDraft, tr: &Tr) {
    text_row(ui, 2, &tr.t("masters.field.name"), &mut d.name);
    text_row(ui, 1, &tr.t("masters.field.alias"), &mut d.aliases);
    let groups = sorted_options(
        masters
            .stock_groups
            .values()
            .map(|g| (g.id, g.id.0, g.name.clone())),
        None,
    );
    choose_opt(
        ui,
        1,
        "i-group",
        &tr.t("masters.field.under"),
        &mut d.group,
        &tr.t("masters.field.none"),
        &groups,
    );
    ui.end_row();
    let units = sorted_options(
        masters
            .units
            .values()
            .map(|u| (u.id, u.id.0, u.symbol.clone())),
        None,
    );
    choose_opt(
        ui,
        1,
        "i-unit",
        &tr.t("masters.field.unit"),
        &mut d.unit,
        &tr.t("masters.field.choose"),
        &units,
    );
    choose_one(
        ui,
        1,
        "i-valuation",
        &tr.t("masters.field.valuation"),
        &mut d.valuation,
        &labelled(&VALUATIONS, |v| valuation_label(v, tr)),
    );
    text_row(ui, 1, "HSN/SAC", &mut d.hsn);
    switch_row(
        ui,
        tr,
        1,
        &tr.t("masters.field.track_batches"),
        &mut d.batches,
    );
}
