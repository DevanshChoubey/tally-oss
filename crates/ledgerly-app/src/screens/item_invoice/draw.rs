//! Drawing the screen with egui. Draws and reports events; it computes nothing.
//!
//! The layout follows the mock (`design/mock-v2/index.html`, `sales-invoice`): a page head with
//! the save action, then one voucher sheet (see [`crate::screens::voucher_sheet`]) holding the lock
//! bar when the date is locked, the head (date, party, place of supply, party GSTIN, sales ledger,
//! reference), the line table, and the foot with the narration on the left and the engine's tax
//! box on the right. The type-ahead list floats under its field. Every amount shown is one the
//! engine returned (rule C4). The quantity, rate, discount and typed amount are the user's own
//! input, shown as typed.

use super::compute;
use super::draft::{line_amount, particular_side};
use super::extras::BalanceShown;
use super::keys::{Prompt, read_keys};
use super::with_engine_lines;
use super::{
    EInvoiceState, Field, GstBlock, GstResult, GstStatus, InvoiceKind, InvoiceMode,
    ItemInvoiceScreen, Notice, UiEvent, place_label,
};
use crate::i18n::Tr;
use crate::screen::KeyHint;
use crate::screens::entry_print::PrintKey;
use crate::screens::voucher_entry::parse_entry_date;
use crate::screens::voucher_sheet::{self as sheet, Col, InputSpec, Look, Total};
use crate::shell::submit::{self, CommandReply};
use crate::theme::{TEXT_BODY, TEXT_META};
use crate::ui::a11y;
use crate::ui::badge::{BadgeKind, badge, badge_with_icon};
use crate::ui::button::{ButtonKind, button, button_icon};
use crate::ui::icon::Icon;
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::page_head_numbered;
use crate::ui::prov::{ProvKind, prov};
use crate::ui::{format, para_text, tokens};
use crate::worker::Poll;
use crate::{AppCtx, Screen, ScreenAction};
use egui::{Frame, Id, Margin, Rect, Response, RichText, Ui};
use ledgerly_api::{Command, is_gst_tax_ledger};
use ledgerly_core::format::amount_in_words;
use ledgerly_core::{DrCr, LedgerId, Outcome, StateCode};
use ledgerly_model::{Masters, Voucher};

/// The one-line purpose under the page title of an item invoice (index.html:1282). The screen asks
/// the engine again after every edit that changes the invoice.
const SUBTITLE: &str = "Item invoice · tax is recomputed by the GST engine on every keystroke";

/// The purchase screen's line (index.html:1966, `SCREENS.purchase`): the form is typed by hand.
const PURCHASE_SUBTITLE: &str = "Typed by hand \u{b7} no bill reader \u{b7} tax is recomputed by the GST engine on every keystroke";

/// The credit note's line (index.html:2006, `noteScreen`); CGST Act s.34(1).
const CREDIT_NOTE_SUBTITLE: &str =
    "Credit note, CGST Act s.34(1): reduces the tax on an invoice already issued";

/// The debit note's line (index.html:2006): the mock names its sample company here; this says
/// "you" because the screen has no company name to hand.
const DEBIT_NOTE_SUBTITLE: &str = "Debit note, purchase return: reduces what you owe the supplier";

/// The line under a purchase's narration (index.html:2005).
const PURCHASE_META: &str =
    "Each line is checked when you leave it. Save waits until every line is complete.";

/// The one-line purpose under the page title of an accounting invoice.
const AS_VOUCHER_SUBTITLE: &str = "As voucher mode \u{b7} ledger lines \u{b7} tax is recomputed by the GST engine on every keystroke";
const ACCOUNTING_SUBTITLE: &str = "Accounting invoice · ledgers under Particulars; tax is recomputed by the GST engine on every keystroke";

/// The item table: heading, width and whether the column holds figures. The mock's columns run
/// # Item HSN Qty Unit Rate GST Amount (index.html:1283).
const LINE_COLS: [Col; 8] = [
    Col::text("#", 36.0),
    Col::rest("Item"),
    Col::text("HSN", 70.0),
    Col::num("Qty", 90.0),
    Col::text("Unit", 60.0),
    Col::num("Rate", 120.0),
    Col::num("GST", 60.0),
    Col::num("Amount", 140.0),
];

/// The item table of a purchase, credit note and debit note: the mock's # Item HSN Qty Rate GST
/// Amount, with no Unit (index.html:1999, 2020).
const LINE_COLS_NO_UNIT: [Col; 7] = [
    Col::text("#", 36.0),
    Col::rest("Item"),
    Col::text("HSN", 70.0),
    Col::num("Qty", 90.0),
    Col::num("Rate", 120.0),
    Col::num("GST", 60.0),
    Col::num("Amount", 140.0),
];

/// [`LINE_COLS_NO_UNIT`] with the discount and typed-amount columns.
const LINE_COLS_NO_UNIT_DISCOUNT: [Col; 9] = [
    Col::text("#", 36.0),
    Col::rest("Item"),
    Col::text("HSN", 70.0),
    Col::num("Qty", 90.0),
    Col::num("Rate", 120.0),
    Col::num("Disc %", 64.0),
    Col::num("Typed amount", 108.0),
    Col::num("GST", 60.0),
    Col::num("Amount", 140.0),
];

/// The item table with the discount and typed-amount columns Alt+D brings in, between Rate and
/// GST. They are live inputs the mock has no column for, so they are hidden until asked for.
const LINE_COLS_DISCOUNT: [Col; 10] = [
    Col::text("#", 36.0),
    Col::rest("Item"),
    Col::text("HSN", 70.0),
    Col::num("Qty", 90.0),
    Col::text("Unit", 60.0),
    Col::num("Rate", 120.0),
    Col::num("Disc %", 64.0),
    Col::num("Typed amount", 108.0),
    Col::num("GST", 60.0),
    Col::num("Amount", 140.0),
];

/// Where an as-voucher row comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowAt {
    /// The party: the input on line 1.
    Party,
    /// Typed particular `n` (a blank entry row included).
    Typed(usize),
    /// A line only the engine adds: the invoice round-off.
    Engine,
}

/// One line of an as-voucher entry as the table lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoucherRow {
    /// The ledger's name.
    pub ledger: String,
    /// `Dr` or `Cr`.
    pub side: &'static str,
    /// The amount with Indian grouping; empty until the engine has answered for a figure it owns.
    pub amount: String,
}

/// The particulars table of an accounting invoice.
const PARTICULAR_COLS: [Col; 4] = [
    Col::text("#", 36.0),
    Col::rest("Particulars"),
    Col::num("Amount", 140.0),
    Col::num("GST", 60.0),
];

/// The lines table of an as-voucher entry: the side column the entry mode is named for.
const VOUCHER_COLS: [Col; 4] = [
    Col::text("#", 36.0),
    Col::rest("Ledger"),
    Col::text("Dr / Cr", 70.0),
    Col::num("Amount", 170.0),
];

/// Where a text field sits in the focus order and what it says when empty.
struct FieldSpec {
    id: Id,
    field: Field,
    /// The name a screen reader gives the field: its caption, or the column heading.
    name: &'static str,
    hint: &'static str,
}

/// How a field looks: the sheet's input look, whether it holds figures, whether it is flagged.
#[derive(Clone, Copy)]
struct Style {
    look: Look,
    numeric: bool,
    invalid: bool,
}

/// A box in the head.
const BOXED: Style = Style {
    look: Look::Boxed,
    numeric: false,
    invalid: false,
};

/// A box in the head that holds figures.
const BOXED_NUM: Style = Style {
    look: Look::Boxed,
    numeric: true,
    invalid: false,
};

/// How long the toast after a save stays, in seconds.
const TOAST_SECONDS: f64 = 6.0;

/// A text cell of the line table.
const CELL: Style = Style {
    look: Look::Cell,
    numeric: false,
    invalid: false,
};

/// A cell of the line table that holds figures.
const CELL_NUM: Style = Style {
    look: Look::Cell,
    numeric: true,
    invalid: false,
};

/// What every field of a frame reports to: the field the keyboard is wanted on, the field it is on,
/// the events, and where the type-ahead list for the focused field hangs.
struct Cx<'a> {
    want: Option<Field>,
    focus: Field,
    events: &'a mut Vec<UiEvent>,
    anchor: &'a mut Option<Rect>,
}

/// The button pressed this frame, reported after the widgets are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Click {
    /// Save, or Yes on the save question: both ask the engine to save.
    Save,
    /// No on the save question.
    Cancel,
    /// Open the mode list (Ctrl+H).
    ChangeMode,
    /// Open or close the More details panel (Ctrl+I).
    More,
    /// Read a bill photo into a purchase (Alt+O): opens the bill screen.
    BillPhoto,
}

/// One line of the table as the screen shows it. Every figure is the engine's, or empty until
/// the engine has answered for this exact invoice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineShown {
    /// The line's number, counted from 1 over all rows.
    pub number: usize,
    /// The item's HSN code from the masters, or empty when no item is picked.
    pub hsn: String,
    /// The item's unit symbol from the masters, or empty when no item is picked.
    pub unit: String,
    /// The GST rate the engine applied, such as `18%`; empty until the engine has answered.
    pub gst: String,
    /// The taxable value the engine gave the line, with Indian grouping; empty until it answers.
    pub amount: String,
    /// Why the line cannot be worked out yet, for a row that has something typed in it.
    pub problem: Option<String>,
    /// Whether the keyboard is on a cell of this line.
    pub focused: bool,
}

/// The row a field belongs to, for the field kinds that live in the line table.
fn row_of(field: Field) -> Option<usize> {
    match field {
        Field::Item(i) | Field::Qty(i) | Field::Rate(i) | Field::Discount(i) | Field::Amount(i) => {
            Some(i)
        }
        Field::Date
        | Field::Number
        | Field::Party
        | Field::Place
        | Field::Account
        | Field::Reference
        | Field::ReferenceDate
        | Field::Ledger(_)
        | Field::LedgerAmount(_)
        | Field::BillKind(_)
        | Field::BillName(_)
        | Field::BillAmount(_)
        | Field::Narration => None,
    }
}

/// The rows of the tax box above the invoice total, in display order, as (label, amount): the
/// engine's taxable value, each tax head at each rate it returned ("CGST 9% on 1,30,085.00", then
/// "SGST 9%"), or the head totals when the engine gave no per-rate rows, and the round-off.
/// Nothing is added here.
#[must_use]
pub fn tax_rows(block: &GstBlock, tr: &Tr) -> Vec<(String, String)> {
    tax_rows_for(block, InvoiceKind::Sales, tr)
}

/// [`tax_rows`] as the mock draws them for `kind`. A purchase and a note write each head with its
/// rate alone ("IGST 18%", index.html:2008); a note leaves out a round-off of nothing
/// (index.html:2037 has no such row), and a round-off that is not nothing is always shown.
#[must_use]
pub(super) fn tax_rows_for(block: &GstBlock, kind: InvoiceKind, tr: &Tr) -> Vec<(String, String)> {
    let plain_heads = match kind {
        InvoiceKind::Sales => false,
        InvoiceKind::Purchase | InvoiceKind::CreditNote | InvoiceKind::DebitNote => true,
    };
    let mut rows = vec![(
        tr.t("invoice.taxable_value"),
        format::amount(block.taxable_total),
    )];
    if block.rate_rows.is_empty() {
        for head in &block.heads {
            rows.push((head.head.clone(), format::amount(head.amount)));
        }
    } else {
        let mut previous = None;
        for row in &block.rate_rows {
            let rate = row.rate_percent.normalize();
            let label = if previous == Some(rate) || plain_heads {
                format!("{} {rate}%", row.head)
            } else {
                tr.tf(
                    "invoice.tax_rate_on",
                    &[
                        ("head", &row.head),
                        ("rate", &rate.to_string()),
                        ("taxable", &format::amount(row.taxable)),
                    ],
                )
            };
            previous = Some(rate);
            rows.push((label, format::amount(row.amount)));
        }
    }
    let leave_out = match kind {
        InvoiceKind::Sales | InvoiceKind::Purchase => false,
        InvoiceKind::CreditNote | InvoiceKind::DebitNote => block.round_off.is_zero(),
    };
    if !leave_out {
        rows.push((tr.lit("Round off"), format::amount(block.round_off)));
    }
    rows
}

/// A note with a small "i" in a ring and a bold run, as the mock's place-of-supply note
/// (`.note` with an info icon, index.html:1373).
fn info_note(ui: &mut Ui, lead: &str, bold: &str, rest: &str) {
    let t = tokens(ui);
    let width = ui.available_width();
    Frame::new()
        .fill(t.surface_2)
        .stroke(egui::Stroke::new(
            crate::theme::border_width(crate::ui::kind(ui)),
            t.border,
        ))
        .corner_radius(crate::theme::RADIUS_S)
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(width - 24.0 - 2.0);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let (slot, _) =
                    ui.allocate_exact_size(egui::vec2(16.0, 18.0), egui::Sense::hover());
                let c = egui::pos2(slot.center().x, slot.top() + 9.0);
                ui.painter()
                    .circle_stroke(c, 7.0, egui::Stroke::new(1.3, t.text_2));
                ui.painter().text(
                    c,
                    egui::Align2::CENTER_CENTER,
                    "i",
                    egui::FontId::proportional(10.0),
                    t.text_2,
                );
                let mut job = egui::text::LayoutJob::default();
                let line = crate::ui::line_height(crate::theme::TEXT_META);
                let plain = |color| egui::TextFormat {
                    font_id: egui::FontId::proportional(crate::theme::TEXT_META),
                    color,
                    line_height: Some(line),
                    ..egui::TextFormat::default()
                };
                job.append(lead, 0.0, plain(t.text_2));
                job.append(
                    bold,
                    0.0,
                    egui::TextFormat {
                        font_id: crate::ui::bold_font(ui, crate::theme::TEXT_META),
                        color: t.text,
                        line_height: Some(line),
                        ..egui::TextFormat::default()
                    },
                );
                job.append(rest, 0.0, plain(t.text_2));
                ui.add(egui::Label::new(job).wrap());
            });
        });
}

/// "₹1,93,500.00 Dr" for a balance the engine gave.
fn balance_text(b: ledgerly_core::SignedAmount) -> String {
    let side = match b.side {
        ledgerly_core::DrCr::Dr => "Dr",
        ledgerly_core::DrCr::Cr => "Cr",
    };
    format!("\u{20b9}{} {side}", format::amount(b.amount))
}

/// A text field: reports focus, typing and Enter as events, and notes where it sits so the list
/// can hang under it.
fn text_field(
    ui: &mut Ui,
    cx: &mut Cx<'_>,
    spec: &FieldSpec,
    text: &mut String,
    style: Style,
) -> Response {
    let response = sheet::input(
        ui,
        text,
        &InputSpec {
            id: spec.id,
            hint: spec.hint,
            look: style.look,
            numeric: style.numeric,
            invalid: style.invalid,
            bad_ink: false,
            width: None,
        },
    );
    a11y::name(
        ui,
        &response,
        egui::Role::TextInput,
        &crate::i18n::lit(spec.name),
    );
    if cx.want == Some(spec.field) {
        response.request_focus();
    }
    if response.has_focus() {
        cx.events.push(UiEvent::Focus(spec.field));
    }
    if cx.focus == spec.field {
        *cx.anchor = Some(response.rect);
    }
    if response.changed() {
        cx.events.push(UiEvent::Typed(spec.field));
    }
    if response.lost_focus() && ui.ctx().input(|i| i.key_pressed(egui::Key::Enter)) {
        let back = ui.ctx().input(|i| i.modifiers.shift);
        cx.events.push(UiEvent::Submit {
            field: spec.field,
            back,
        });
    }
    response
}

/// How far the plain date's text starts from the left edge of its field.
const DATE_TEXT_INSET: f32 = 10.0;

/// A text field that shows `shown` in place of its text while the keyboard is not on it (the mock
/// shows `21 Nov 2026` and `3,850.00`; typing still works on the raw text). With no `shown`, or
/// while the field has the keyboard, it is [`text_field`].
fn text_field_shown(
    ui: &mut Ui,
    cx: &mut Cx<'_>,
    spec: &FieldSpec,
    text: &mut String,
    style: Style,
    shown: Option<String>,
) -> Response {
    match shown {
        Some(mut copy) if cx.focus != spec.field => text_field(ui, cx, spec, &mut copy, style),
        Some(_) | None => text_field(ui, cx, spec, text, style),
    }
}

/// The party as the mock's read-only value (index.html:1993): the name, then "· GSTIN" and the
/// check badge, until the keyboard is on the field, when it is the box. An unregistered party
/// shows the name alone.
fn party_inline(
    ui: &mut Ui,
    cx: &mut Cx<'_>,
    spec: &FieldSpec,
    text: &mut String,
    masters: &Masters,
    party: Option<LedgerId>,
) {
    let Some(party) = party.filter(|_| cx.focus != spec.field) else {
        text_field(ui, cx, spec, text, BOXED);
        return;
    };
    let t = tokens(ui);
    let gstin = masters
        .ledgers
        .get(&party)
        .and_then(|l| l.party.as_ref())
        .and_then(|p| p.gstin.as_ref())
        .map(|g| g.as_str().to_owned());
    let name_width = ui
        .painter()
        .layout_no_wrap(
            text.clone(),
            egui::FontId::proportional(crate::theme::TEXT_INPUT),
            t.text,
        )
        .size()
        .x;
    sheet::read_only(ui, |ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let mut copy = text.clone();
        let room = (name_width + 2.0 * 10.0).min(ui.available_width());
        let inline = FieldSpec {
            id: spec.id,
            field: spec.field,
            name: spec.name,
            hint: spec.hint,
        };
        inline_cell(ui, cx, &inline, &mut copy, room);
        if let Some(gstin) = gstin {
            ui.label(para_text(&format!("\u{b7} {gstin}"), TEXT_BODY, t.text_2));
            // A `Gstin` exists only after `Gstin::parse`, which checks the check character.
            badge(ui, BadgeKind::Ok, &crate::i18n::t("masters.check_ok"));
        }
    });
}

/// A bare field `width` wide, for [`party_inline`].
fn inline_cell(ui: &mut Ui, cx: &mut Cx<'_>, spec: &FieldSpec, text: &mut String, width: f32) {
    let response = sheet::input(
        ui,
        text,
        &InputSpec {
            id: spec.id,
            hint: spec.hint,
            look: Look::Cell,
            numeric: false,
            invalid: false,
            bad_ink: false,
            width: Some(width),
        },
    );
    a11y::name(
        ui,
        &response,
        egui::Role::TextInput,
        &crate::i18n::lit(spec.name),
    );
    if cx.want == Some(spec.field) {
        response.request_focus();
    }
    if response.has_focus() {
        cx.events.push(UiEvent::Focus(spec.field));
    }
}

/// A cell of plain text in the line table, in the secondary ink unless `strong`.
fn cell_text(ui: &mut Ui, text: &str, strong: bool) {
    let t = tokens(ui);
    let ink = if strong { t.text } else { t.text_2 };
    ui.label(para_text(text, TEXT_BODY, ink));
}

/// The party's GSTIN with its check badge, or the B2C words for an unregistered party.
fn gstin_line(ui: &mut Ui, masters: &Masters, party: Option<LedgerId>) {
    let tr = &crate::i18n::current();
    let t = tokens(ui);
    let Some(party) = party else {
        ui.label(
            RichText::new(tr.t("invoice.choose_party"))
                .size(TEXT_BODY)
                .color(t.text_3),
        );
        return;
    };
    let gstin = masters
        .ledgers
        .get(&party)
        .and_then(|l| l.party.as_ref())
        .and_then(|p| p.gstin.as_ref());
    match gstin {
        Some(gstin) => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(gstin.as_str()).size(TEXT_BODY).color(t.text));
                // A `Gstin` exists only after `Gstin::parse`, which checks the check character.
                badge(ui, BadgeKind::Ok, &tr.t("invoice.check_ok"));
            });
        }
        None => {
            ui.label(
                RichText::new(tr.t("invoice.unregistered"))
                    .size(TEXT_BODY)
                    .color(t.text_2),
            );
        }
    }
}
/// The e-Invoice cell (index.html: "JSON ready (INV-01)", "Not ready" with the reason, "Not
/// applicable: B2C"). It says "JSON ready" only when the API mapped this invoice to the schema;
/// until it has answered the cell shows a dash, as the tax box does.
fn einvoice_cell(ui: &mut Ui, masters: &Masters, party: Option<LedgerId>, state: &EInvoiceState) {
    let t = tokens(ui);
    let Some(party) = party else {
        return;
    };
    let registered = masters
        .ledgers
        .get(&party)
        .and_then(|l| l.party.as_ref())
        .is_some_and(|p| p.gstin.is_some());
    if !registered {
        ui.label(
            RichText::new(crate::i18n::lit("Not applicable: B2C"))
                .size(TEXT_BODY)
                .color(t.text_2),
        );
        return;
    }
    match state {
        EInvoiceState::Unknown | EInvoiceState::Checking(_) => {
            ui.label(RichText::new("\u{2013}").size(TEXT_BODY).color(t.text_3));
        }
        EInvoiceState::Ready => {
            ui.horizontal(|ui| {
                badge_with_icon(
                    ui,
                    BadgeKind::Accent,
                    &crate::i18n::lit("JSON ready (INV-01)"),
                    Some(Icon::File),
                );
                ui.label(
                    RichText::new(crate::i18n::lit("upload: v1.1"))
                        .size(TEXT_META)
                        .color(t.text_3),
                );
            });
        }
        EInvoiceState::NotReady(reason) => {
            ui.horizontal_wrapped(|ui| {
                badge_with_icon(
                    ui,
                    BadgeKind::Warn,
                    &crate::i18n::lit("Not ready"),
                    Some(Icon::Alert),
                );
                ui.label(RichText::new(reason).size(TEXT_META).color(t.text_3));
            });
        }
    }
}

/// The tax box for an answer the engine computed: its rows, then the invoice total with its
/// engine tag, and the total in words.
fn tax_box(ui: &mut Ui, block: &GstBlock, kind: InvoiceKind) {
    let tr = &crate::i18n::current();
    let rows = tax_rows_for(block, kind, tr);
    let total = format::amount(block.invoice_total);
    sheet::taxbox(
        ui,
        &rows,
        Some(&Total {
            label: &total_label(kind, tr),
            value: &total,
            tag: Some(ProvKind::Engine),
            bad: false,
        }),
    );
    // The purchase's box has the eligible input credit under it (index.html, purchase `.meta`).
    if let Some((label, value)) = super::itc_line_text(block) {
        let t = tokens(ui);
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.label(para_text(&label, crate::theme::TEXT_META, t.text_2));
            prov(ui, ProvKind::Engine);
            ui.label(
                RichText::new(value)
                    .size(crate::theme::TEXT_META)
                    .font(crate::ui::bold_font(ui, crate::theme::TEXT_META))
                    .color(t.text),
            );
        });
    }
    // Only the sales invoice's box has the total in words under it (index.html:1321).
    match kind {
        InvoiceKind::Sales => sheet::words(ui, &amount_in_words(block.invoice_total)),
        InvoiceKind::Purchase | InvoiceKind::CreditNote | InvoiceKind::DebitNote => {}
    }
}

/// The label of the total row: "Note total" on a note (index.html:2038), else "Invoice total".
fn total_label(kind: InvoiceKind, tr: &Tr) -> String {
    tr.lit(match kind {
        InvoiceKind::CreditNote | InvoiceKind::DebitNote => "Note total",
        InvoiceKind::Sales | InvoiceKind::Purchase => "Invoice total",
    })
}

/// The title of the type-ahead list for the open picker.
fn list_caption(focus: Field, tr: &Tr) -> String {
    tr.lit(match focus {
        Field::Party => "Parties",
        Field::Item(_) => "Stock items",
        Field::Account | Field::Ledger(_) => "Ledgers",
        Field::Date
        | Field::Number
        | Field::Place
        | Field::Reference
        | Field::ReferenceDate
        | Field::Qty(_)
        | Field::Rate(_)
        | Field::Discount(_)
        | Field::Amount(_)
        | Field::LedgerAmount(_)
        | Field::BillKind(_)
        | Field::BillName(_)
        | Field::BillAmount(_)
        | Field::Narration => "Choices",
    })
}

/// The save question at the foot of the sheet (index.html `savebar`): a green bar with the
/// question in bold, then Yes and No.
fn save_bar(ui: &mut Ui, clicks: &mut Option<Click>, title: &str) {
    let t = tokens(ui);
    let width = ui.available_width() - 32.0;
    let drawn = Frame::new()
        .outer_margin(Margin {
            left: 16,
            right: 16,
            top: 0,
            bottom: 16,
        })
        .fill(t.ok_soft)
        .corner_radius(crate::theme::RADIUS_S)
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(width - 24.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                crate::ui::icon::icon(ui, crate::ui::icon::Icon::Save, t.ok);
                let mut job = egui::text::LayoutJob::default();
                let line = crate::ui::line_height(crate::theme::TEXT_META);
                job.append(
                    title,
                    0.0,
                    egui::TextFormat {
                        font_id: crate::ui::bold_font(ui, crate::theme::TEXT_META),
                        color: t.ok,
                        line_height: Some(line),
                        ..egui::TextFormat::default()
                    },
                );
                job.append(
                    &crate::i18n::t("invoice.save_bar_tail"),
                    0.0,
                    egui::TextFormat {
                        font_id: egui::FontId::proportional(crate::theme::TEXT_META),
                        color: t.ok,
                        line_height: Some(line),
                        ..egui::TextFormat::default()
                    },
                );
                ui.label(job);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if button(
                        ui,
                        &crate::i18n::t("invoice.no"),
                        Some("Esc"),
                        ButtonKind::Secondary,
                    )
                    .clicked()
                    {
                        *clicks = Some(Click::Cancel);
                    }
                    if button(
                        ui,
                        &crate::i18n::t("invoice.yes"),
                        Some("Enter"),
                        ButtonKind::Primary,
                    )
                    .clicked()
                    {
                        *clicks = Some(Click::Save);
                    }
                });
            });
        });
    // The bar is the question the keyboard is on: scroll to it when the sheet is taller than the
    // window, so it is never cut off at the foot.
    ui.scroll_to_rect(drawn.response.rect, Some(egui::Align::BOTTOM));
}

impl ItemInvoiceScreen {
    /// The lines of the table as the screen shows them, for the invoice as it stands. The
    /// engine's answer fills the GST and amount cells only when it is for exactly this invoice.
    #[must_use]
    pub fn lines_shown(&self, masters: &Masters) -> Vec<LineShown> {
        let computed: Option<&GstBlock> = self.shown_block(masters);
        let focused_row = row_of(self.focus);
        let mut engine_line = 0_usize;
        let mut shown = Vec::with_capacity(self.draft.rows.len());
        for (i, row) in self.draft.rows.iter().enumerate() {
            let focused = focused_row == Some(i);
            let number = i.saturating_add(1);
            if row.is_blank() {
                shown.push(LineShown {
                    number,
                    hsn: String::new(),
                    unit: String::new(),
                    gst: String::new(),
                    amount: String::new(),
                    problem: None,
                    focused,
                });
                continue;
            }
            // The engine's lines are the valid non-blank rows, in order
            // (`InvoiceDraft::build_partial`).
            let problem = line_amount(masters, row, number).err();
            let engine = if problem.is_none() {
                let at = engine_line;
                engine_line = engine_line.saturating_add(1);
                computed.and_then(|block| block.lines.get(at))
            } else {
                None
            };
            let item = row.item.and_then(|id| masters.stock_items.get(&id));
            let hsn = item
                .and_then(|item| item.hsn.as_ref())
                .map_or_else(String::new, |h| h.as_str().to_owned());
            let unit = item
                .and_then(|item| masters.units.get(&item.unit))
                .map_or_else(String::new, |u| u.symbol.clone());
            shown.push(LineShown {
                number,
                hsn,
                unit,
                gst: engine.map_or_else(String::new, |line| {
                    format!("{}%", line.rate_percent.normalize())
                }),
                amount: engine.map_or_else(String::new, |line| format::amount(line.taxable)),
                problem: problem.map(|e| e.to_string()),
                focused,
            });
        }
        shown
    }

    /// The head: the fields in four equal columns, the party across two (`.vhead`). Date and the
    /// sales ledger read as plain values until the keyboard is on them (index.html:1287, `.ro`);
    /// the e-Invoice cell says what this build can say about it.
    fn draw_head(&mut self, ui: &mut Ui, base: Id, masters: &Masters, cx: &mut Cx<'_>) {
        match self.draft.kind {
            InvoiceKind::Sales => self.draw_head_sales(ui, base, masters, cx),
            InvoiceKind::Purchase | InvoiceKind::CreditNote | InvoiceKind::DebitNote => {
                self.draw_head_bill(ui, base, masters, cx);
            }
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one block of header fields, each named by its caption"
    )]
    fn draw_head_sales(&mut self, ui: &mut Ui, base: Id, masters: &Masters, cx: &mut Cx<'_>) {
        let kind = self.draft.kind;
        let mode = self.draft.mode;
        let required = self.draft.number_required;
        let party = self.draft.party;
        let fy = self.fy;
        let date_value = parse_entry_date(&self.draft.date_text, fy).ok();
        let date_plain = cx.focus != Field::Date && date_value.is_some();
        let locked = self.locked_on().is_some();
        let account_plain = cx.focus != Field::Account && self.draft.account.is_some();
        let spec = |id: &str, name: &'static str, field: Field, hint: &'static str| FieldSpec {
            id: base.with(id),
            field,
            name,
            hint,
        };
        let mut fields: Vec<(&'static str, Rect)> = Vec::new();
        let mut note_field = |name: &'static str, rect: Rect| fields.push((name, rect));
        sheet::head(ui, |ui, total| {
            sheet::head_row(ui, |ui| {
                let shown = sheet::fld(
                    ui,
                    total,
                    1,
                    |ui| sheet::caption(ui, &crate::i18n::lit("Date"), Some("F2")),
                    |ui| {
                        // Bare like the mock's read-only date until the keyboard is on it; it
                        // stays a field, so Tab and a screen reader still reach it.
                        let s = spec("date", "Date", Field::Date, "dd-mm-yyyy");
                        let style = if date_plain { CELL } else { BOXED };
                        let shown = date_value
                            .filter(|_| date_plain)
                            .map(crate::ui::format::date);
                        let width = shown.as_ref().map(|text| {
                            ui.painter()
                                .layout_no_wrap(
                                    text.clone(),
                                    egui::FontId::proportional(crate::theme::TEXT_INPUT),
                                    egui::Color32::PLACEHOLDER,
                                )
                                .size()
                                .x
                        });
                        let field =
                            text_field_shown(ui, cx, &s, &mut self.draft.date_text, style, shown);
                        // index.html:1359: a date on or before the lock date wears a "Locked"
                        // badge right after the value.
                        if let (true, Some(width)) = (locked, width) {
                            crate::ui::badge::paint_badge_with_icon(
                                ui,
                                field.rect.left() + DATE_TEXT_INSET + width + 8.0,
                                field.rect.center().y,
                                crate::ui::badge::BadgeKind::Bad,
                                &crate::i18n::lit("Locked"),
                                Some(crate::ui::icon::Icon::Lock),
                            );
                        }
                    },
                );
                note_field("Date", shown.response.rect);
                // As voucher: the party is typed on line 1 of the table (the product's Dr line),
                // so the head has no Party box.
                if mode != InvoiceMode::AsVoucher {
                    let label = kind.party_label();
                    let shown = sheet::fld(
                        ui,
                        total,
                        2,
                        |ui| sheet::caption(ui, label, None),
                        |ui| {
                            let s = spec("party", label, Field::Party, "type to search");
                            text_field(ui, cx, &s, &mut self.draft.party_text, BOXED);
                        },
                    );
                    note_field(label, shown.response.rect);
                }
                let shown = sheet::fld(
                    ui,
                    total,
                    1,
                    |ui| sheet::caption(ui, &crate::i18n::lit("Place of supply"), None),
                    |ui| {
                        let s = spec("place", "Place of supply", Field::Place, "code or name");
                        text_field(ui, cx, &s, &mut self.draft.place_text, BOXED);
                    },
                );
                note_field("Place of supply", shown.response.rect);
            });
            sheet::head_row(ui, |ui| {
                let shown = sheet::fld(
                    ui,
                    total,
                    1,
                    |ui| sheet::caption(ui, &crate::i18n::lit("Party GSTIN"), None),
                    |ui| sheet::read_only(ui, |ui| gstin_line(ui, masters, party)),
                );
                note_field("Party GSTIN", shown.response.rect);
                match mode {
                    InvoiceMode::Item => {
                        let label = kind.account_label();
                        let shown = sheet::fld(
                            ui,
                            total,
                            1,
                            |ui| sheet::caption(ui, label, None),
                            |ui| {
                                let s = spec("account", label, Field::Account, "type to search");
                                let style = if account_plain { CELL } else { BOXED };
                                text_field(ui, cx, &s, &mut self.draft.account_text, style);
                            },
                        );
                        note_field(label, shown.response.rect);
                    }
                    // The sales FAQ: no Sales ledger field in accounting invoice mode.
                    InvoiceMode::Accounting | InvoiceMode::AsVoucher => {
                        ui.allocate_space(egui::vec2(sheet::span_width(total, 1), 0.0));
                    }
                }
                let label = kind.reference_label();
                let shown = sheet::fld(
                    ui,
                    total,
                    1,
                    |ui| sheet::caption(ui, label, None),
                    |ui| {
                        let s = spec("ref", label, Field::Reference, "");
                        text_field(ui, cx, &s, &mut self.draft.reference_text, BOXED);
                    },
                );
                note_field(label, shown.response.rect);
                if let Some(label) = kind.reference_date_label() {
                    let shown = sheet::fld(
                        ui,
                        total,
                        1,
                        |ui| sheet::caption(ui, label, None),
                        |ui| {
                            let s = spec("ref-date", label, Field::ReferenceDate, "dd-mm-yyyy");
                            text_field(ui, cx, &s, &mut self.draft.reference_date_text, BOXED);
                        },
                    );
                    note_field(label, shown.response.rect);
                } else {
                    let shown = sheet::fld(
                        ui,
                        total,
                        1,
                        |ui| sheet::caption(ui, &crate::i18n::lit("e-Invoice"), None),
                        |ui| {
                            sheet::read_only(ui, |ui| {
                                let state =
                                    if matches!(self.gst_status(masters), GstStatus::Computed(_)) {
                                        &self.einvoice
                                    } else {
                                        &EInvoiceState::Unknown
                                    };
                                einvoice_cell(ui, masters, party, state);
                            });
                        },
                    );
                    note_field("e-Invoice", shown.response.rect);
                }
            });
            if required {
                sheet::head_row(ui, |ui| {
                    let shown = sheet::fld(
                        ui,
                        total,
                        1,
                        |ui| sheet::caption(ui, &crate::i18n::lit("Voucher no."), None),
                        |ui| {
                            let s = spec("number", "Voucher no.", Field::Number, "Required");
                            text_field(ui, cx, &s, &mut self.draft.number_text, BOXED);
                        },
                    );
                    note_field("Voucher no.", shown.response.rect);
                });
            }
        });
        self.shown.fields = fields;
    }

    /// The head of a purchase (index.html:1992-1998) and of a credit or debit note
    /// (index.html:2029-2033): every box reads as plain text until the keyboard is on it, the
    /// party is followed by its GSTIN and badge, and there is no "Party GSTIN" or e-Invoice cell.
    ///
    /// A purchase: row one Supplier (two columns), Supplier bill no., Bill date; row two Purchase
    /// ledger, Place of supply, Date. The mock has no Date; the voucher still has one, so it sits
    /// in the free cell. A note: row one Party (two columns), Date, Place of supply; row two the
    /// invoice it is against, its date and the sales or purchase ledger. The mock's Reason box is
    /// not drawn: the voucher has nowhere to keep it (see `design/PARITY-GAPS.md`).
    #[expect(
        clippy::too_many_lines,
        reason = "one block of header fields, each named by its caption"
    )]
    fn draw_head_bill(&mut self, ui: &mut Ui, base: Id, masters: &Masters, cx: &mut Cx<'_>) {
        let kind = self.draft.kind;
        let purchase = match kind {
            InvoiceKind::Purchase => true,
            InvoiceKind::Sales | InvoiceKind::CreditNote | InvoiceKind::DebitNote => false,
        };
        let mode = self.draft.mode;
        let required = self.draft.number_required;
        let party = self.draft.party;
        let fy = self.fy;
        let date_value = parse_entry_date(&self.draft.date_text, fy).ok();
        let date_shown = date_value
            .filter(|_| cx.focus != Field::Date)
            .map(crate::ui::format::date);
        let ref_date_shown = parse_entry_date(&self.draft.reference_date_text, fy)
            .ok()
            .filter(|_| cx.focus != Field::ReferenceDate)
            .map(crate::ui::format::date);
        let place_shown = super::parse_place(&self.draft.place_text)
            .ok()
            .filter(|_| cx.focus != Field::Place)
            .map(place_label);
        let style_of = |plain: bool| if plain { CELL } else { BOXED };
        let account_plain = cx.focus != Field::Account && self.draft.account.is_some();
        let reference_plain =
            cx.focus != Field::Reference && !self.draft.reference_text.trim().is_empty();
        let spec = |id: &str, name: &'static str, field: Field, hint: &'static str| FieldSpec {
            id: base.with(id),
            field,
            name,
            hint,
        };
        let mut fields: Vec<(&'static str, Rect)> = Vec::new();
        let mut note_field = |name: &'static str, rect: Rect| fields.push((name, rect));
        sheet::head(ui, |ui, total| {
            sheet::head_row(ui, |ui| {
                // As voucher: the party is line 1 of the table, not a head box.
                if mode != InvoiceMode::AsVoucher {
                    let label = kind.party_label();
                    let shown = sheet::fld(
                        ui,
                        total,
                        2,
                        |ui| sheet::caption(ui, label, None),
                        |ui| {
                            let s = spec("party", label, Field::Party, "type to search");
                            party_inline(ui, cx, &s, &mut self.draft.party_text, masters, party);
                        },
                    );
                    note_field(label, shown.response.rect);
                }
                if purchase {
                    let label = kind.reference_label();
                    let shown = sheet::fld(
                        ui,
                        total,
                        1,
                        |ui| sheet::caption(ui, label, None),
                        |ui| {
                            let s = spec("ref", label, Field::Reference, "");
                            text_field(
                                ui,
                                cx,
                                &s,
                                &mut self.draft.reference_text,
                                style_of(reference_plain),
                            );
                        },
                    );
                    note_field(label, shown.response.rect);
                    if let Some(label) = kind.reference_date_label() {
                        let shown = sheet::fld(
                            ui,
                            total,
                            1,
                            |ui| sheet::caption(ui, label, None),
                            |ui| {
                                let s = spec("ref-date", label, Field::ReferenceDate, "dd-mm-yyyy");
                                let style = style_of(ref_date_shown.is_some());
                                text_field_shown(
                                    ui,
                                    cx,
                                    &s,
                                    &mut self.draft.reference_date_text,
                                    style,
                                    ref_date_shown.clone(),
                                );
                            },
                        );
                        note_field(label, shown.response.rect);
                    }
                } else {
                    let shown = sheet::fld(
                        ui,
                        total,
                        1,
                        |ui| sheet::caption(ui, &crate::i18n::lit("Date"), None),
                        |ui| {
                            let s = spec("date", "Date", Field::Date, "dd-mm-yyyy");
                            let style = style_of(date_shown.is_some());
                            text_field_shown(
                                ui,
                                cx,
                                &s,
                                &mut self.draft.date_text,
                                style,
                                date_shown.clone(),
                            );
                        },
                    );
                    note_field("Date", shown.response.rect);
                    let shown = sheet::fld(
                        ui,
                        total,
                        1,
                        |ui| sheet::caption(ui, &crate::i18n::lit("Place of supply"), None),
                        |ui| {
                            let s = spec("place", "Place of supply", Field::Place, "code or name");
                            let style = style_of(place_shown.is_some());
                            text_field_shown(
                                ui,
                                cx,
                                &s,
                                &mut self.draft.place_text,
                                style,
                                place_shown.clone(),
                            );
                        },
                    );
                    note_field("Place of supply", shown.response.rect);
                }
            });
            sheet::head_row(ui, |ui| {
                if purchase {
                    let label = kind.account_label();
                    if mode == InvoiceMode::Item {
                        let shown = sheet::fld(
                            ui,
                            total,
                            1,
                            |ui| sheet::caption(ui, label, None),
                            |ui| {
                                let s = spec("account", label, Field::Account, "type to search");
                                let style = style_of(account_plain);
                                text_field(ui, cx, &s, &mut self.draft.account_text, style);
                            },
                        );
                        note_field(label, shown.response.rect);
                    } else {
                        ui.allocate_space(egui::vec2(sheet::span_width(total, 1), 0.0));
                    }
                    let shown = sheet::fld(
                        ui,
                        total,
                        1,
                        |ui| sheet::caption(ui, &crate::i18n::lit("Place of supply"), None),
                        |ui| {
                            let s = spec("place", "Place of supply", Field::Place, "code or name");
                            let style = style_of(place_shown.is_some());
                            text_field_shown(
                                ui,
                                cx,
                                &s,
                                &mut self.draft.place_text,
                                style,
                                place_shown.clone(),
                            );
                        },
                    );
                    note_field("Place of supply", shown.response.rect);
                    let shown = sheet::fld(
                        ui,
                        total,
                        1,
                        |ui| sheet::caption(ui, &crate::i18n::lit("Date"), Some("F2")),
                        |ui| {
                            let s = spec("date", "Date", Field::Date, "dd-mm-yyyy");
                            let style = style_of(date_shown.is_some());
                            text_field_shown(
                                ui,
                                cx,
                                &s,
                                &mut self.draft.date_text,
                                style,
                                date_shown.clone(),
                            );
                        },
                    );
                    note_field("Date", shown.response.rect);
                } else {
                    let label = kind.reference_label();
                    let shown = sheet::fld(
                        ui,
                        total,
                        1,
                        |ui| sheet::caption(ui, label, None),
                        |ui| {
                            let s = spec("ref", label, Field::Reference, "");
                            text_field(
                                ui,
                                cx,
                                &s,
                                &mut self.draft.reference_text,
                                style_of(reference_plain),
                            );
                        },
                    );
                    note_field(label, shown.response.rect);
                    if let Some(label) = kind.reference_date_label() {
                        let shown = sheet::fld(
                            ui,
                            total,
                            1,
                            |ui| sheet::caption(ui, label, None),
                            |ui| {
                                let s = spec("ref-date", label, Field::ReferenceDate, "dd-mm-yyyy");
                                let style = style_of(ref_date_shown.is_some());
                                text_field_shown(
                                    ui,
                                    cx,
                                    &s,
                                    &mut self.draft.reference_date_text,
                                    style,
                                    ref_date_shown.clone(),
                                );
                            },
                        );
                        note_field(label, shown.response.rect);
                    }
                    if mode == InvoiceMode::Item {
                        let label = kind.account_label();
                        let shown = sheet::fld(
                            ui,
                            total,
                            1,
                            |ui| sheet::caption(ui, label, None),
                            |ui| {
                                let s = spec("account", label, Field::Account, "type to search");
                                let style = style_of(account_plain);
                                text_field(ui, cx, &s, &mut self.draft.account_text, style);
                            },
                        );
                        note_field(label, shown.response.rect);
                    }
                }
            });
            if required {
                sheet::head_row(ui, |ui| {
                    let shown = sheet::fld(
                        ui,
                        total,
                        1,
                        |ui| sheet::caption(ui, &crate::i18n::lit("Voucher no."), None),
                        |ui| {
                            let s = spec("number", "Voucher no.", Field::Number, "Required");
                            text_field(ui, cx, &s, &mut self.draft.number_text, BOXED);
                        },
                    );
                    note_field("Voucher no.", shown.response.rect);
                });
            }
        });
        self.shown.fields = fields;
    }

    /// The GST rate the engine applied to each particular, as shown (`18%`), or empty for a
    /// blank particular and until the engine has answered for exactly this invoice. The engine's
    /// lines are the non-blank particulars, in order (`InvoiceDraft::build_voucher`).
    #[must_use]
    pub fn particular_rates(&self, masters: &Masters) -> Vec<String> {
        let computed: Option<&GstBlock> = self.shown_block(masters);
        let mut engine_line = 0_usize;
        let mut rates = Vec::with_capacity(self.draft.particulars.len());
        for (i, row) in self.draft.particulars.iter().enumerate() {
            if row.is_blank() || self.draft.particular_problem(masters, i, row).is_some() {
                rates.push(String::new());
                continue;
            }
            // As voucher: a GST ledger line is not a taxable line, the engine has none for it.
            if self.draft.mode == InvoiceMode::AsVoucher
                && row.ledger.is_some_and(|l| is_gst_tax_ledger(masters, l))
            {
                rates.push(String::new());
                continue;
            }
            let engine = computed.and_then(|block| block.lines.get(engine_line));
            engine_line = engine_line.saturating_add(1);
            rates.push(engine.map_or_else(String::new, |line| {
                format!("{}%", line.rate_percent.normalize())
            }));
        }
        rates
    }

    /// The lines of an as-voucher entry as the table lists them: the party first on the side
    /// opposite the typed lines, each typed ledger, and the engine's round-off line after the
    /// last typed one. Every figure the engine owns (the party, a GST ledger, the round-off) is
    /// the engine's, formatted, and empty until it has answered for exactly this invoice.
    #[must_use]
    pub fn voucher_rows(&self, masters: &Masters) -> Vec<VoucherRow> {
        self.voucher_display(masters)
            .into_iter()
            .filter(|(at, row)| !matches!(at, RowAt::Typed(_)) || !row.ledger.is_empty())
            .map(|(_, row)| row)
            .collect()
    }

    /// The as-voucher rows with where each comes from, the blank entry rows included.
    fn voucher_display(&self, masters: &Masters) -> Vec<(RowAt, VoucherRow)> {
        let side_text = |side: DrCr| match side {
            DrCr::Dr => "Dr",
            DrCr::Cr => "Cr",
        };
        let typed_side = side_text(particular_side(self.draft.kind));
        let party_side = side_text(match particular_side(self.draft.kind) {
            DrCr::Dr => DrCr::Cr,
            DrCr::Cr => DrCr::Dr,
        });
        let block = self.shown_block(masters);
        let engine_amount = |ledger: LedgerId| {
            block
                .and_then(|b| b.ledger_lines.iter().find(|l| l.ledger == ledger))
                .map(|l| format::amount(l.amount.amount))
        };
        let name = |id: Option<LedgerId>, text: &str| {
            id.and_then(|id| masters.ledgers.get(&id))
                .map_or_else(|| text.trim().to_owned(), |l| l.name.clone())
        };
        let mut out = vec![(
            RowAt::Party,
            VoucherRow {
                ledger: name(self.draft.party, &self.draft.party_text),
                side: party_side,
                amount: self.draft.party.and_then(engine_amount).unwrap_or_default(),
            },
        )];
        let last_typed = self
            .draft
            .particulars
            .iter()
            .rposition(|r| !r.is_blank())
            .unwrap_or(0);
        for (i, row) in self.draft.particulars.iter().enumerate() {
            let engine_owns = row.ledger.is_some_and(|l| is_gst_tax_ledger(masters, l));
            let amount = match row.ledger {
                Some(l) if engine_owns => engine_amount(l).unwrap_or_default(),
                _ => format::grouped_text(&row.amount_text)
                    .unwrap_or_else(|| row.amount_text.trim().to_owned()),
            };
            out.push((
                RowAt::Typed(i),
                VoucherRow {
                    ledger: name(row.ledger, &row.ledger_text),
                    side: typed_side,
                    amount,
                },
            ));
            if i == last_typed {
                let known = |l: LedgerId| {
                    Some(l) == self.draft.party
                        || self.draft.particulars.iter().any(|r| r.ledger == Some(l))
                };
                let round = block.and_then(|b| b.ledger_lines.iter().find(|l| !known(l.ledger)));
                if let Some(line) = round {
                    out.push((
                        RowAt::Engine,
                        VoucherRow {
                            ledger: name(Some(line.ledger), ""),
                            side: side_text(line.amount.side),
                            amount: format::amount(line.amount.amount),
                        },
                    ));
                }
            }
        }
        out
    }

    /// The item table, or the particulars table of an accounting invoice.
    #[expect(
        clippy::too_many_lines,
        reason = "one table drawn column by column, each named by its heading"
    )]
    fn draw_lines(&mut self, ui: &mut Ui, base: Id, masters: &Masters, cx: &mut Cx<'_>) {
        if matches!(
            self.draft.mode,
            InvoiceMode::Accounting | InvoiceMode::AsVoucher
        ) {
            return self.draw_particulars(ui, base, masters, cx);
        }
        let shown = self.lines_shown(masters);
        let with_discount = self.discount_columns();
        let unit = match self.draft.kind {
            InvoiceKind::Sales => true,
            InvoiceKind::Purchase | InvoiceKind::CreditNote | InvoiceKind::DebitNote => false,
        };
        let blank_hint = match self.draft.kind {
            InvoiceKind::Purchase => "Item, or Enter on empty to finish",
            InvoiceKind::Sales | InvoiceKind::CreditNote | InvoiceKind::DebitNote => "",
        };
        let last_row = shown.len().saturating_sub(1);
        let cols: &[Col] = match (unit, with_discount) {
            (true, true) => &LINE_COLS_DISCOUNT,
            (true, false) => &LINE_COLS,
            (false, true) => &LINE_COLS_NO_UNIT_DISCOUNT,
            (false, false) => &LINE_COLS_NO_UNIT,
        };
        let rows = &mut self.draft.rows;
        let rects = sheet::lines(
            ui,
            cols,
            shown.len(),
            |r| shown.get(r).is_some_and(|l| l.focused),
            |ui, r, c| {
                let (Some(line), Some(cells)) = (shown.get(r), rows.get_mut(r)) else {
                    return;
                };
                let item_flag =
                    !line.focused && cells.item.is_none() && !cells.item_text.trim().is_empty();
                let qty_flag = cells.item.is_some() && cells.qty_text.trim().is_empty();
                // Cell numbers below are those of the widest table (with Unit and the two extra
                // columns): shift past a missing Unit, then past the two extra columns.
                let c = if unit || c < 4 {
                    c
                } else {
                    c.saturating_add(1)
                };
                // The cells of the table without the two extra columns are the first six and the
                // last two of the table with them.
                let c = if with_discount || c < 6 {
                    c
                } else {
                    c.saturating_add(2)
                };
                // No hints in the cells: the mock's empty row is its number and nothing else.
                let spec = |id: &str, name: &'static str, field: Field| FieldSpec {
                    id: base.with((id, r)),
                    field,
                    name,
                    hint: "",
                };
                // The purchase's empty last row says how to finish (index.html:2001).
                let item_hint = if r == last_row && cells.is_blank() {
                    blank_hint
                } else {
                    ""
                };
                match c {
                    0 => cell_text(ui, &line.number.to_string(), false),
                    1 => {
                        let s = FieldSpec {
                            hint: item_hint,
                            ..spec("item", "Item", Field::Item(r))
                        };
                        let style = Style {
                            invalid: item_flag,
                            ..CELL
                        };
                        text_field(ui, cx, &s, &mut cells.item_text, style);
                    }
                    2 => cell_text(ui, &line.hsn, false),
                    3 => {
                        let s = spec("qty", "Qty", Field::Qty(r));
                        let style = Style {
                            invalid: qty_flag,
                            ..CELL_NUM
                        };
                        text_field(ui, cx, &s, &mut cells.qty_text, style);
                    }
                    4 => cell_text(ui, &line.unit, false),
                    5 => {
                        let s = spec("rate", "Rate", Field::Rate(r));
                        let shown = crate::ui::format::grouped_text(&cells.rate_text);
                        text_field_shown(ui, cx, &s, &mut cells.rate_text, CELL_NUM, shown);
                    }
                    6 => {
                        let s = spec("disc", "Discount", Field::Discount(r));
                        text_field(ui, cx, &s, &mut cells.discount_text, CELL_NUM);
                    }
                    7 => {
                        let s = spec("amt", "Amount", Field::Amount(r));
                        text_field(ui, cx, &s, &mut cells.amount_text, CELL_NUM);
                    }
                    8 => cell_text(ui, &line.gst, false),
                    9 => cell_text(ui, &line.amount, true),
                    _ => {}
                }
            },
        );
        self.shown.heads = cols.iter().map(|c| c.label).zip(rects).collect();
    }

    /// The as-voucher lines: the party first (an input: the product's Dr line), each typed ledger,
    /// the engine's round-off. The engine's round-off is read-only; a GST ledger's amount is the
    /// engine's.
    #[expect(
        clippy::too_many_lines,
        reason = "one table drawn column by column, each named by its heading"
    )]
    fn draw_voucher_lines(&mut self, ui: &mut Ui, base: Id, masters: &Masters, cx: &mut Cx<'_>) {
        let display = self.voucher_display(masters);
        let focused_row = match self.focus {
            Field::Ledger(i) | Field::LedgerAmount(i) => Some(i),
            Field::Date
            | Field::Number
            | Field::Party
            | Field::Place
            | Field::Account
            | Field::Reference
            | Field::ReferenceDate
            | Field::Item(_)
            | Field::Qty(_)
            | Field::Rate(_)
            | Field::Discount(_)
            | Field::Amount(_)
            | Field::BillKind(_)
            | Field::BillName(_)
            | Field::BillAmount(_)
            | Field::Narration => None,
        };
        let party_focused = self.focus == Field::Party;
        let party_label = self.draft.kind.party_label();
        let mut party_rect: Option<Rect> = None;
        let party_text = &mut self.draft.party_text;
        let parts = &mut self.draft.particulars;
        let rects = sheet::lines(
            ui,
            &VOUCHER_COLS,
            display.len(),
            |r| {
                display.get(r).is_some_and(|(at, _)| match at {
                    RowAt::Typed(i) => focused_row == Some(*i),
                    RowAt::Party => party_focused,
                    RowAt::Engine => false,
                })
            },
            |ui, r, c| {
                let Some((at, shown)) = display.get(r) else {
                    return;
                };
                match (c, at) {
                    (0, _) => cell_text(ui, &r.saturating_add(1).to_string(), false),
                    (1, RowAt::Typed(i)) => {
                        let Some(cells) = parts.get_mut(*i) else {
                            return;
                        };
                        let s = FieldSpec {
                            id: base.with(("ledger", *i)),
                            field: Field::Ledger(*i),
                            name: "Ledger",
                            hint: "Next ledger, or Enter on empty to finish",
                        };
                        text_field(ui, cx, &s, &mut cells.ledger_text, CELL);
                    }
                    (1, RowAt::Party) => {
                        let s = FieldSpec {
                            id: base.with("party"),
                            field: Field::Party,
                            name: party_label,
                            hint: "Party: type to search",
                        };
                        party_rect = Some(text_field(ui, cx, &s, party_text, CELL).rect);
                    }
                    (1, RowAt::Engine) => cell_text(ui, &shown.ledger, true),
                    (2, _) => {
                        if !shown.ledger.is_empty() {
                            cell_text(ui, shown.side, false);
                        }
                    }
                    (3, RowAt::Typed(i)) => {
                        let Some(cells) = parts.get_mut(*i) else {
                            return;
                        };
                        // Only a line the user is on says what to type; a GST ledger's amount is
                        // the engine's, so it shows that.
                        let on_it = focused_row == Some(*i);
                        let s = FieldSpec {
                            id: base.with(("ledger-amt", *i)),
                            field: Field::LedgerAmount(*i),
                            name: "Amount",
                            hint: if on_it { "Amount" } else { "" },
                        };
                        let engine_owns =
                            cells.ledger.is_some_and(|l| is_gst_tax_ledger(masters, l));
                        let text = Some(shown.amount.clone())
                            .filter(|a| !a.is_empty() && (engine_owns || !on_it));
                        text_field_shown(ui, cx, &s, &mut cells.amount_text, CELL_NUM, text);
                    }
                    (3, RowAt::Party | RowAt::Engine) => cell_text(ui, &shown.amount, true),
                    _ => {}
                }
            },
        );
        if let Some(rect) = party_rect {
            self.shown.fields.push((party_label, rect));
        }
        self.shown.heads = VOUCHER_COLS.iter().map(|c| c.label).zip(rects).collect();
    }

    fn draw_particulars(&mut self, ui: &mut Ui, base: Id, masters: &Masters, cx: &mut Cx<'_>) {
        if self.draft.mode == InvoiceMode::AsVoucher {
            return self.draw_voucher_lines(ui, base, masters, cx);
        }
        let rates = self.particular_rates(masters);
        let focused_row = match self.focus {
            Field::Ledger(i) | Field::LedgerAmount(i) => Some(i),
            Field::Date
            | Field::Number
            | Field::Party
            | Field::Place
            | Field::Account
            | Field::Reference
            | Field::ReferenceDate
            | Field::Item(_)
            | Field::Qty(_)
            | Field::Rate(_)
            | Field::Discount(_)
            | Field::Amount(_)
            | Field::BillKind(_)
            | Field::BillName(_)
            | Field::BillAmount(_)
            | Field::Narration => None,
        };
        let parts = &mut self.draft.particulars;
        let rects = sheet::lines(
            ui,
            &PARTICULAR_COLS,
            rates.len(),
            |r| focused_row == Some(r),
            |ui, r, c| {
                let (Some(gst), Some(cells)) = (rates.get(r), parts.get_mut(r)) else {
                    return;
                };
                match c {
                    0 => cell_text(ui, &r.saturating_add(1).to_string(), false),
                    1 => {
                        let s = FieldSpec {
                            id: base.with(("ledger", r)),
                            field: Field::Ledger(r),
                            name: "Ledger",
                            hint: "Ledger: type to search",
                        };
                        text_field(ui, cx, &s, &mut cells.ledger_text, CELL);
                    }
                    2 => {
                        let s = FieldSpec {
                            id: base.with(("ledger-amt", r)),
                            field: Field::LedgerAmount(r),
                            name: "Amount",
                            hint: "Amount",
                        };
                        text_field(ui, cx, &s, &mut cells.amount_text, CELL_NUM);
                    }
                    3 => cell_text(ui, gst, false),
                    _ => {}
                }
            },
        );
        self.shown.heads = PARTICULAR_COLS.iter().map(|c| c.label).zip(rects).collect();
    }

    /// The tax box, with the note that says why it has no figures yet when it has none. Until the
    /// engine has answered for this exact invoice every figure is a dash, never a zero.
    fn draw_tax(&self, ui: &mut Ui, masters: &Masters) {
        let tr = &crate::i18n::current();
        let status = self.gst_status(masters);
        if let GstStatus::Computed(block) = status {
            return tax_box(ui, block, self.draft.kind);
        }
        let dash = "\u{2014}".to_owned();
        sheet::taxbox(
            ui,
            &[
                (tr.lit("Taxable value"), dash.clone()),
                (tr.lit("Round off"), dash.clone()),
            ],
            Some(&Total {
                label: &total_label(self.draft.kind, tr),
                value: &dash,
                tag: None,
                bad: false,
            }),
        );
        ui.add_space(8.0);
        match status {
            GstStatus::Incomplete(why) => {
                if !self.draft.is_blank() {
                    note(
                        ui,
                        NoteKind::Info,
                        &tr.tf("invoice.tax_after_complete", &[("why", &why.clone())]),
                    );
                }
            }
            GstStatus::NotComputedYet => {
                note(ui, NoteKind::Info, &tr.t("invoice.tax_not_computed"));
            }
            GstStatus::Unsupported(u) => {
                note(
                    ui,
                    NoteKind::Warn,
                    &tr.tf(
                        "invoice.not_supported",
                        &[("what", &u.what), ("reason", &u.reason)],
                    ),
                );
            }
            GstStatus::Failed(m) => {
                note(
                    ui,
                    NoteKind::Bad,
                    &tr.tf("invoice.engine_failed", &[("message", m)]),
                );
            }
            GstStatus::Computed(_) => {}
        }
    }

    /// The note under the narration that says which tax applies and where the rates come from,
    /// built from the engine's own heads and the states on screen.
    fn place_note(
        &self,
        masters: &Masters,
        block: &GstBlock,
    ) -> Option<(String, &'static str, String)> {
        let place = super::parse_place(&self.draft.place_text).ok()?;
        let supplier = self
            .draft
            .party
            .and_then(|p| masters.ledgers.get(&p))
            .and_then(|l| l.party.as_ref())
            .and_then(|d| d.state);
        let heads: Vec<&str> = block.heads.iter().map(|h| h.head.as_str()).collect();
        place_note_parts(
            self.draft.kind,
            &heads,
            place,
            self.home_state,
            supplier,
            &block.pack_version,
        )
    }

    /// The voucher a computed answer belongs to, with the engine's accounting lines on it: what
    /// the balance line asks the engine about.
    pub(super) fn computed_voucher(&self, masters: &Masters) -> Option<Voucher> {
        let voucher = self.draft.build_voucher(masters, self.fy).ok()?;
        let bills = self.draft.party_bills().ok()?;
        match self.answer_for(&voucher, &bills) {
            Some(GstResult::Computed(block)) => Some(with_engine_lines(voucher, block)),
            Some(GstResult::Unsupported(_) | GstResult::Failed(_)) | None => None,
        }
    }

    /// The line under the narration: the party's balance now and after saving, the engine's.
    fn balance_line(&self, ui: &mut Ui, masters: &Masters) {
        let t = tokens(ui);
        let (Some(party), Some(voucher)) = (self.draft.party, self.computed_voucher(masters))
        else {
            return;
        };
        let name = masters.ledgers.get(&party).map_or("", |l| l.name.as_str());
        match self.extras.balance_for(party, &voucher) {
            Some(BalanceShown::Change(change)) => {
                ui.label(para_text(
                    &crate::i18n::tf(
                        "invoice.balance_now_after",
                        &[
                            ("name", name),
                            ("now", &balance_text(change.now)),
                            ("after", &balance_text(change.after)),
                        ],
                    ),
                    crate::theme::TEXT_META,
                    t.text_2,
                ));
            }
            Some(BalanceShown::Missing(why)) => {
                ui.label(para_text(
                    &crate::i18n::tf(
                        "invoice.balance_not_shown",
                        &[("name", name), ("why", &why.clone())],
                    ),
                    crate::theme::TEXT_META,
                    t.text_2,
                ));
            }
            None => {}
        }
    }

    /// The red note over the foot of a locked invoice (index.html:1449): "1 thing to fix before
    /// saving" and the thing.
    fn lock_note(&self, left: &mut Ui) {
        if let Some((date, lock)) = self.locked_on() {
            crate::ui::note::note_with_icon(left, NoteKind::Bad, Icon::Alert, |ui, ink| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(
                    egui::RichText::new(crate::i18n::t("invoice.one_thing_to_fix"))
                        .font(crate::ui::bold_font(ui, crate::theme::TEXT_META))
                        .color(ink),
                );
                ui.label(para_text(
                    &crate::i18n::tf(
                        "invoice.lock_fix",
                        &[("date", &format::date(date)), ("lock", &format::date(lock))],
                    ),
                    crate::theme::TEXT_META,
                    ink,
                ));
            });
        }
    }

    /// The foot: narration, the notices and line problems on the left; the tax box on the right.
    #[allow(
        clippy::too_many_lines,
        reason = "one drawing pass of the screen, a line for each part of it"
    )]
    fn draw_foot(
        &mut self,
        ui: &mut Ui,
        base: Id,
        masters: &Masters,
        cx: &mut Cx<'_>,
        clicks: &mut Option<Click>,
    ) {
        let tr = &crate::i18n::current();
        let lines = self.lines_shown(masters);
        let kind = self.draft.kind;
        // Accounting and as voucher have no per-line checks to show; item mode does.
        let accounting = self.draft.mode != InvoiceMode::Item;
        let recall = self
            .last_narration
            .as_ref()
            .is_some_and(|(p, n)| *p == self.draft.party && n.is_some());
        let mut narration = None;
        sheet::foot(ui, |left, right| {
            sheet::fld(
                left,
                left.available_width(),
                4,
                |ui| match kind {
                    // The mock's purchase and notes caption it with the bare word
                    // (index.html:2003); Alt+R still recalls on them.
                    InvoiceKind::Purchase | InvoiceKind::CreditNote | InvoiceKind::DebitNote => {
                        sheet::caption(ui, &crate::i18n::lit("Narration"), None);
                    }
                    InvoiceKind::Sales => {
                        let t = tokens(ui);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            ui.label(para_text(
                                &crate::i18n::lit("Narration"),
                                crate::theme::TEXT_META,
                                t.text_2,
                            ));
                            crate::ui::keycap::keycap(ui, "Alt+R");
                            let hint = if recall {
                                &crate::i18n::lit("recalls the last one for this party")
                            } else {
                                &crate::i18n::lit("recalls the last one for this party (none yet)")
                            };
                            ui.label(para_text(
                                &crate::i18n::lit(hint),
                                crate::theme::TEXT_META,
                                t.text_3,
                            ));
                        });
                    }
                },
                |ui| {
                    let s = FieldSpec {
                        id: base.with("narration"),
                        field: Field::Narration,
                        name: "Narration",
                        hint: "",
                    };
                    narration = Some(text_field(ui, cx, &s, &mut self.draft.narration, BOXED).rect);
                },
            );
            self.lock_note(left);
            match &self.notice {
                Some(Notice::Error(s)) => error_box(left, s),
                Some(Notice::Info(s)) if !(*s == tr.t("invoice.saved") && self.toast.is_some()) => {
                    note(left, NoteKind::Info, s);
                }
                Some(Notice::Info(_)) | None => {}
            }
            if !accounting {
                for line in lines.iter().filter(|line| !line.focused) {
                    if let Some(problem) = &line.problem {
                        note(left, NoteKind::Warn, problem);
                    }
                }
            }
            match kind {
                InvoiceKind::Sales => {
                    if let GstStatus::Computed(block) = self.gst_status(masters)
                        && let Some((lead, bold, rest)) = self.place_note(masters, block)
                    {
                        info_note(left, &lead, bold, &rest);
                    }
                    self.balance_line(left, masters);
                }
                // The mock's purchase says how lines are checked (index.html:2005, with "Save"
                // for its "Accept": that word belongs to the bill reader); a note has no line.
                InvoiceKind::Purchase => sheet::hint(left, &tr.lit(PURCHASE_META)),
                InvoiceKind::CreditNote | InvoiceKind::DebitNote => {
                    self.draw_invoiced_line(left, masters);
                }
            }
            self.draw_tax(right, masters);
        });
        self.shown.narration = narration;
        if self.confirming_save {
            save_bar(ui, clicks, &self.save_title());
        }
    }

    /// The mode list (Ctrl+H): a card under the mode button (or under the head's right edge when
    /// the voucher has no mode button), one row per mode with the current highlighted row.
    /// Returns the mode a click picked.
    fn draw_mode_list(&mut self, ui: &Ui, base: Id, button: Option<Rect>) -> Option<InvoiceMode> {
        let tr = &crate::i18n::current();
        let at = self.mode_list?;
        let choices = self.mode_choices();
        let anchor = button.or_else(|| {
            self.shown.sheet.map(|sheet| {
                Rect::from_min_size(
                    egui::pos2(sheet.right() - 360.0, sheet.top() - 8.0),
                    egui::vec2(360.0, 0.0),
                )
            })
        })?;
        let labels: Vec<String> = choices
            .iter()
            .map(|m| format!("{}   {}", mode_name(*m), mode_blurb(*m)))
            .collect();
        let (rect, clicked) = sheet::type_ahead(
            ui,
            base.with("modes"),
            anchor,
            &tr.lit("Change mode"),
            &labels,
            at,
        );
        self.shown.picker = Some(rect);
        clicked.and_then(|j| choices.get(j).copied())
    }

    /// The note's "Invoiced on ..." line with the engine tag, when the engine has found the
    /// invoice.
    fn draw_invoiced_line(&self, ui: &mut Ui, masters: &Masters) {
        let Some(text) = self.invoiced_line(masters) else {
            return;
        };
        let t = tokens(ui);
        let label = para_text(&text, crate::theme::TEXT_META, t.text_2);
        // A long line (several items on the invoice) wraps; the tag then goes under it, not into
        // the sliver of width the wrapped text leaves.
        if text.chars().count() > 72 {
            ui.vertical(|ui| {
                ui.label(label);
                prov(ui, ProvKind::Engine);
            });
        } else {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(label);
                prov(ui, ProvKind::Engine);
            });
        }
    }

    /// The question the save bar asks: "Save SI/0418?" with the number the engine will give, or
    /// "Save this invoice?" when it has none to give.
    fn save_title(&self) -> String {
        match &self.extras.number {
            Some(n) => crate::i18n::tf("invoice.save_number", &[("n", n)]),
            None => crate::i18n::t("invoice.save_this"),
        }
    }

    /// The More details panel: the bill references of the party line (TallyPrime's Bill-wise
    /// Details, <https://help.tallysolutions.com/advances-received/>, read 2026-11-25). A party
    /// that does not track bills has none to enter.
    fn draw_more(&mut self, ui: &mut Ui, base: Id, masters: &Masters, cx: &mut Cx<'_>) {
        let bill_wise = self
            .draft
            .party
            .and_then(|p| masters.ledgers.get(&p))
            .is_some_and(|l| l.bill_wise.is_some());
        let rows = self.draft.bills.len();
        sheet::head(ui, |ui, total| {
            sheet::head_row(ui, |ui| {
                let t = tokens(ui);
                let text = if bill_wise {
                    "invoice.bill_refs_help"
                } else if self.draft.party.is_some() {
                    "invoice.bill_refs_none"
                } else {
                    "invoice.bill_refs_choose_party"
                };
                ui.label(para_text(&crate::i18n::t(text), TEXT_BODY, t.text_2));
            });
            if !bill_wise {
                return;
            }
            for i in 0..rows {
                let Some(row) = self.draft.bills.get_mut(i) else {
                    continue;
                };
                sheet::head_row(ui, |ui| {
                    let kind_spec = FieldSpec {
                        id: base.with(("bill-kind", i)),
                        field: Field::BillKind(i),
                        name: "Bill type",
                        hint: "New Ref",
                    };
                    sheet::fld(
                        ui,
                        total,
                        1,
                        |ui| sheet::caption(ui, &crate::i18n::lit("Type"), None),
                        |ui| {
                            text_field(ui, cx, &kind_spec, &mut row.kind_text, BOXED);
                        },
                    );
                    let name_spec = FieldSpec {
                        id: base.with(("bill-name", i)),
                        field: Field::BillName(i),
                        name: "Bill name",
                        hint: "voucher number",
                    };
                    sheet::fld(
                        ui,
                        total,
                        2,
                        |ui| sheet::caption(ui, &crate::i18n::lit("Name"), None),
                        |ui| {
                            text_field(ui, cx, &name_spec, &mut row.name, BOXED);
                        },
                    );
                    let amount_spec = FieldSpec {
                        id: base.with(("bill-amount", i)),
                        field: Field::BillAmount(i),
                        name: "Bill amount",
                        hint: "the rest",
                    };
                    sheet::fld(
                        ui,
                        total,
                        1,
                        |ui| sheet::caption(ui, &crate::i18n::lit("Amount"), None),
                        |ui| {
                            text_field(ui, cx, &amount_spec, &mut row.amount_text, BOXED_NUM);
                        },
                    );
                });
            }
        });
    }

    /// The toast after a save, at the foot of the screen: dark pill, a check, the saved number,
    /// the party and the total. It stays for a few seconds.
    fn draw_toast(&mut self, ui: &mut Ui) {
        let Some(toast) = self.toast.as_mut() else {
            return;
        };
        let now = ui.input(|i| i.time);
        let started = *toast.shown_at.get_or_insert(now);
        if now - started > TOAST_SECONDS {
            self.toast = None;
            return;
        }
        let t = tokens(ui);
        let text = toast.text.clone();
        let screen = ui.ctx().content_rect();
        egui::Area::new(egui::Id::unique("invoice-toast"))
            .order(egui::Order::Foreground)
            .fade_in(false)
            .fixed_pos(egui::pos2(
                screen.center().x - 240.0,
                screen.bottom() - 84.0,
            ))
            .show(ui.ctx(), |ui| {
                Frame::new()
                    .fill(t.text)
                    .corner_radius(crate::theme::RADIUS_S)
                    .inner_margin(Margin::symmetric(16, 10))
                    .show(ui, |ui| {
                        ui.set_width(448.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            crate::ui::icon::icon(ui, crate::ui::icon::Icon::Check, t.surface);
                            ui.label(para_text(&text, TEXT_BODY, t.surface));
                        });
                    });
            });
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(500));
    }

    /// Reads this frame's keys and runs them. Returns whether any key came in, and whether one
    /// asked to close the screen.
    fn run_keys(&mut self, ui: &Ui, masters: &Masters, labels: &[String]) -> (bool, bool) {
        let escape_wanted = self.confirm_discard
            || self.mode_list.is_some()
            || self.create.is_some()
            || self.confirming_save
            || self.searching()
            || !self.draft.is_blank();
        let prompt = Prompt {
            discard: self.confirm_discard,
            save: self.confirming_save,
            modes: self.mode_list.is_some(),
        };
        let keys = read_keys(ui, !labels.is_empty(), escape_wanted, prompt);
        let mut pop = false;
        for cmd in &keys {
            if matches!(
                self.handle_key(masters, *cmd, labels.len()),
                ScreenAction::Pop
            ) {
                pop = true;
            }
        }
        (!keys.is_empty(), pop)
    }

    /// The discard question above the invoice. The period lock is the sheet's own bar.
    fn draw_notes(&self, ui: &mut Ui) {
        let tr = &crate::i18n::current();
        if self.confirm_discard {
            note(ui, NoteKind::Warn, &tr.t("invoice.confirm_discard"));
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one sheet drawn top to bottom; splitting would only pass the same borrows around"
    )]
    fn draw(&mut self, ui: &mut Ui, masters: &Masters) -> ScreenAction {
        let tr = &crate::i18n::current();
        self.shown = Shown::default();
        let base = ui.scope_id().with("item-invoice");
        // While a save is out the form stays on screen but takes no keys.
        let busy = self.posting.is_some();
        let mut labels = self.pick_labels(masters);
        let (any_key, pop) = if busy {
            (false, false)
        } else {
            self.run_keys(ui, masters, &labels)
        };
        let action = if pop {
            ScreenAction::Pop
        } else {
            ScreenAction::Stay
        };
        if any_key {
            labels = self.pick_labels(masters);
        }

        let mut clicks: Option<Click> = None;
        let mut mode_anchor: Option<Rect> = None;
        let mut bill_photo = !busy
            && self.draft.kind == InvoiceKind::Purchase
            && !self.confirm_discard
            && !self.confirming_save
            && self.mode_list.is_none()
            && self.create.is_none()
            && super::keys::consume_alt_o(ui);
        let locked = self.locked_on();
        let subtitle = self.subtitle(tr).unwrap_or_default();
        let is_note = matches!(
            self.draft.kind,
            InvoiceKind::CreditNote | InvoiceKind::DebitNote
        );
        let mode_label = match (is_note, self.draft.mode) {
            (false, InvoiceMode::Item) => tr.lit("Item invoice"),
            (false, InvoiceMode::Accounting) => tr.lit("Accounting invoice"),
            (true, InvoiceMode::Item) => tr.lit("Item mode"),
            (true, InvoiceMode::Accounting) => tr.lit("Accounting mode"),
            (_, InvoiceMode::AsVoucher) => tr.lit("As voucher"),
        };
        let number = self.extras.number.clone();
        let save_ok = self.save_enabled(masters);
        page_head_numbered(
            ui,
            &self.page_title(tr),
            number.as_deref(),
            Some(subtitle.as_str()),
            |ui| {
                let saved = ui
                    .add_enabled_ui(locked.is_none() && !busy && save_ok, |ui| {
                        button_icon(
                            ui,
                            Some(Icon::Save),
                            &tr.lit("Save"),
                            Some("Ctrl+A"),
                            ButtonKind::Primary,
                        )
                        .clicked()
                    })
                    .inner;
                if saved {
                    clicks = Some(Click::Save);
                }
                // The head lays buttons out right to left. A sale: More details sits left of
                // Save and the mode button left of that (index.html:1308). A purchase has "Bill
                // photo Alt+O" there (index.html:1990); a note has Save alone (index.html:2027).
                match self.draft.kind {
                    InvoiceKind::Sales => {
                        if button(
                            ui,
                            &tr.lit("More details"),
                            Some("Ctrl+I"),
                            ButtonKind::Secondary,
                        )
                        .clicked()
                        {
                            clicks = Some(Click::More);
                        }
                        let mode_button =
                            button(ui, &mode_label, Some("Ctrl+H"), ButtonKind::Secondary);
                        mode_anchor = Some(mode_button.rect);
                        if mode_button.clicked() {
                            clicks = Some(Click::ChangeMode);
                        }
                    }
                    InvoiceKind::Purchase => {
                        if button(
                            ui,
                            &tr.lit("Bill photo"),
                            Some("Alt+O"),
                            ButtonKind::Secondary,
                        )
                        .clicked()
                        {
                            clicks = Some(Click::BillPhoto);
                        }
                    }
                    InvoiceKind::CreditNote | InvoiceKind::DebitNote => {}
                }
            },
        );
        self.draw_notes(ui);
        self.draw_create_form(ui, masters);

        let mut events = Vec::new();
        let mut anchor = None;
        let enabled =
            !self.confirm_discard && self.mode_list.is_none() && self.create.is_none() && !busy;
        let (focus, pick) = (self.focus, self.pick_index);
        let want = self.focus_pending.then_some(self.focus);
        ui.add_enabled_ui(enabled, |ui| {
            let mut cx = Cx {
                want,
                focus,
                events: &mut events,
                anchor: &mut anchor,
            };
            // The sheet scrolls when it is taller than the window (the mock scrolls its page), so
            // the save bar at its foot is never cut off.
            let drawn = egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    sheet::sheet(ui, |ui| {
                        if let Some((date, lock)) = locked {
                            let (left, top, width) =
                                (ui.cursor().min.x, ui.cursor().min.y, ui.available_width());
                            sheet::lock_bar(
                                ui,
                                &tr.t("invoice.lock_bar_title"),
                                &tr.tf(
                                    "invoice.locked_before",
                                    &[("date", &format::date(date)), ("lock", &format::date(lock))],
                                ),
                                "F2",
                                &tr.t("invoice.locked_change_date"),
                            );
                            let bottom = ui.cursor().min.y;
                            self.shown.lock_bar = Some(Rect::from_min_max(
                                egui::pos2(left, top),
                                egui::pos2(left + width, bottom),
                            ));
                        }
                        self.draw_head(ui, base, masters, &mut cx);
                        self.draw_lines(ui, base, masters, &mut cx);
                        if self.more_open {
                            self.draw_more(ui, base, masters, &mut cx);
                        }
                        self.draw_foot(ui, base, masters, &mut cx, &mut clicks);
                    })
                })
                .inner;
            self.shown.sheet = Some(drawn.response.rect);
        });
        if let Some(anchor) = anchor
            && !labels.is_empty()
        {
            let (rect, clicked) = sheet::type_ahead(
                ui,
                base.with("list"),
                anchor,
                &list_caption(focus, tr),
                &labels,
                pick,
            );
            self.shown.picker = Some(rect);
            if let Some(j) = clicked {
                events.push(UiEvent::Pick(j));
            }
        }
        self.focus_pending = false;
        let listed = self.draw_mode_list(ui, base, mode_anchor);
        if let Some(mode) = listed {
            self.choose_mode(mode);
        }

        self.apply_events(masters, events);
        match clicks {
            Some(Click::Cancel) => self.confirming_save = false,
            Some(Click::Save) => self.save(masters),
            Some(Click::ChangeMode) => self.open_mode_list(),
            Some(Click::More) => self.toggle_more(masters),
            Some(Click::BillPhoto) => bill_photo = true,
            None => {}
        }
        self.draw_toast(ui);
        if any_key || self.focus_pending {
            ui.ctx().request_repaint();
        }
        if bill_photo {
            return ScreenAction::Push(Box::new(
                crate::screens::purchase_bill::PurchaseBillScreen::default(),
            ));
        }
        action
    }
}

/// The red box for an error (the mock's `errNote`): an alert icon, then the message. A message of
/// several lines is a heading ("2 things to fix before saving", in bold) and one line each.
fn error_box(ui: &mut Ui, message: &str) {
    crate::ui::note::note_with_icon(ui, NoteKind::Bad, Icon::Alert, |ui, ink| {
        ui.spacing_mut().item_spacing.y = 2.0;
        let many = message.lines().count() > 1;
        for (i, line) in message.lines().enumerate() {
            if many && i == 0 {
                ui.label(
                    egui::RichText::new(line)
                        .font(crate::ui::bold_font(ui, crate::theme::TEXT_META))
                        .color(ink),
                );
            } else {
                ui.label(para_text(line, crate::theme::TEXT_META, ink));
            }
        }
    });
}

/// Where parts of the screen were drawn in the last frame, for layout tests.
#[derive(Debug, Clone, Default)]
pub(super) struct Shown {
    /// The narration field.
    pub(super) narration: Option<Rect>,
    /// The open type-ahead list.
    pub(super) picker: Option<Rect>,
    /// The header fields by caption.
    pub(super) fields: Vec<(&'static str, Rect)>,
    /// The line table's headings in drawing order.
    pub(super) heads: Vec<(&'static str, Rect)>,
    /// The period-lock bar at the top of the sheet.
    pub(super) lock_bar: Option<Rect>,
    /// The voucher sheet card.
    pub(super) sheet: Option<Rect>,
}

impl ItemInvoiceScreen {
    /// Where the narration field was drawn in the last frame; `None` before the first frame.
    #[must_use]
    pub fn narration_rect(&self) -> Option<Rect> {
        self.shown.narration
    }

    /// Where the open type-ahead list was drawn in the last frame; `None` when none was open.
    #[must_use]
    pub fn picker_rect(&self) -> Option<Rect> {
        self.shown.picker
    }

    /// Where the header field captioned `name` was drawn in the last frame.
    #[must_use]
    pub fn field_rect(&self, name: &str) -> Option<Rect> {
        self.shown
            .fields
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, r)| *r)
    }

    /// The line table's headings and where each was drawn in the last frame, left to right.
    #[must_use]
    pub fn heading_rects(&self) -> Vec<(&'static str, Rect)> {
        self.shown.heads.clone()
    }

    /// Where the period-lock bar was drawn in the last frame; `None` when the date is not locked.
    #[must_use]
    pub fn lock_bar_rect(&self) -> Option<Rect> {
        self.shown.lock_bar
    }

    /// Where the voucher sheet card was drawn in the last frame.
    #[must_use]
    pub fn sheet_rect(&self) -> Option<Rect> {
        self.shown.sheet
    }
}

impl Screen for ItemInvoiceScreen {
    fn title(&self, tr: &Tr) -> String {
        match self.draft.kind {
            InvoiceKind::Sales => {
                tr.tf("invoice.title_sales", &[("name", &tr.lit(&self.type_name))])
            }
            InvoiceKind::Purchase | InvoiceKind::CreditNote | InvoiceKind::DebitNote => {
                tr.lit(&self.type_name)
            }
        }
    }

    fn cursor_label(&self, _ctx: &AppCtx) -> Option<String> {
        let at = self.mode_list?;
        self.mode_choices().get(at).map(|m| mode_name(*m))
    }

    fn subtitle(&self, tr: &Tr) -> Option<String> {
        if let Some(saved) = self.entry_print.subtitle() {
            return Some(saved.to_owned());
        }
        let note = match self.draft.kind {
            InvoiceKind::Sales | InvoiceKind::Purchase => None,
            InvoiceKind::CreditNote => Some(tr.lit("Credit note")),
            InvoiceKind::DebitNote => Some(tr.lit("Debit note")),
        };
        let text = match (note, self.draft.mode) {
            (None, InvoiceMode::Item) if self.draft.kind == InvoiceKind::Purchase => {
                tr.lit(PURCHASE_SUBTITLE)
            }
            (None, InvoiceMode::Item) => tr.lit(SUBTITLE),
            (None, InvoiceMode::Accounting) => tr.lit(ACCOUNTING_SUBTITLE),
            (_, InvoiceMode::AsVoucher) => tr.lit(AS_VOUCHER_SUBTITLE),
            (Some(_), InvoiceMode::Item) => match self.draft.kind {
                InvoiceKind::DebitNote => tr.lit(DEBIT_NOTE_SUBTITLE),
                InvoiceKind::CreditNote | InvoiceKind::Sales | InvoiceKind::Purchase => {
                    tr.lit(CREDIT_NOTE_SUBTITLE)
                }
            },
            (Some(name), InvoiceMode::Accounting) => {
                tr.tf("invoice.subtitle_note_accounting", &[("name", &name)])
            }
        };
        Some(text)
    }

    fn keys(&self, tr: &Tr) -> Vec<KeyHint> {
        if self.entry_print.preview().is_some() {
            // index.html:2272: the preview's key bar.
            return vec![
                KeyHint::new("Ctrl+P", tr.lit("Save as PDF")),
                KeyHint::new("Esc", tr.lit("Close")),
            ];
        }
        if self.confirm_discard {
            return vec![
                KeyHint::new("Y", tr.t("invoice.discard")),
                KeyHint::new("N", tr.t("invoice.keep_it")),
            ];
        }
        if self.confirming_save {
            return vec![
                KeyHint::new("Enter", tr.t("invoice.save")),
                KeyHint::new("Esc", tr.t("invoice.keep_editing")),
            ];
        }
        if self.mode_list.is_some() {
            return vec![
                KeyHint::new("\u{2191} \u{2193}", tr.lit("Choose")),
                KeyHint::new("Enter", tr.lit("Pick")),
                KeyHint::new("Esc", tr.lit("Close")),
            ];
        }
        match self.draft.kind {
            InvoiceKind::Sales => vec![
                KeyHint::new("Enter", tr.lit("Next field")),
                KeyHint::new("Shift+Enter", tr.lit("Previous")),
                KeyHint::new("\u{2191} \u{2193}", tr.lit("List")),
                KeyHint::new("Alt+C", tr.lit("Create master")),
                KeyHint::new("Ctrl+H", tr.lit("Change mode")),
                KeyHint::new("Ctrl+A", tr.lit("Save")),
                KeyHint::new("Esc", tr.lit("Back")),
            ],
            // index.html:2013 and :2041.
            InvoiceKind::Purchase => vec![
                KeyHint::new("Enter", tr.lit("Next field")),
                KeyHint::new("Alt+C", tr.lit("Create master")),
                KeyHint::new("Alt+O", tr.lit("Read a bill")),
                KeyHint::new("Ctrl+A", tr.lit("Save")),
                KeyHint::new("Esc", tr.lit("Back")),
            ],
            InvoiceKind::CreditNote | InvoiceKind::DebitNote => vec![
                KeyHint::new("Enter", tr.lit("Next field")),
                KeyHint::new("Alt+C", tr.lit("Create master")),
                KeyHint::new("Ctrl+A", tr.lit("Save")),
                KeyHint::new("Esc", tr.lit("Back")),
            ],
        }
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        // A save that is out is polled first; the form is drawn either way, taking no input while
        // it waits.
        self.wait_for_post(ui);
        if self.draw_print(ui, ctx) {
            return ScreenAction::Stay;
        }
        self.lock_date = ctx.lock_date;
        self.drive_compute(ui, ctx);
        self.drive_extras(ui, ctx);
        let action = if let Some(books) = ctx.books.as_ref() {
            self.draw(ui, &books.masters)
        } else {
            ui.label(no_books_text(ctx, &ctx.tr));
            ScreenAction::Stay
        };
        if ctx.api.is_some() {
            self.send_saves(ctx);
            self.poll_ledger_create(ui);
            self.send_ledger_creates(ctx);
        }
        action
    }
}

/// What the screen says when it has no books to draw from.
fn no_books_text(ctx: &AppCtx, tr: &Tr) -> String {
    if ctx.api.is_some() {
        tr.t("invoice.no_masters")
    } else {
        tr.t("invoice.no_company")
    }
}

impl ItemInvoiceScreen {
    /// Ctrl+P and the print preview. `true` while the preview (or its busy note) takes the
    /// screen, so the sheet is not drawn and takes no key.
    fn draw_print(&mut self, ui: &mut Ui, ctx: &AppCtx) -> bool {
        if let Some(note) = self.entry_print.tick(ctx) {
            self.notice = Some(match note {
                crate::screens::report_view::Notice::Info(s) => Notice::Info(s),
                crate::screens::report_view::Notice::Error(s) => Notice::Error(s),
            });
        }
        let busy = self.posting.is_some()
            || self.creating.is_some()
            || self.create.is_some()
            || self.confirming_save
            || self.confirm_discard;
        if !busy {
            let title = self.title(&ctx.tr);
            match self
                .entry_print
                .on_ctrl_p(ui, ctx, &title, self.has_unsaved_lines())
            {
                PrintKey::None | PrintKey::Started => {}
                PrintKey::Said(text) => self.notice = Some(Notice::Info(text)),
            }
        }
        self.entry_print.show(ui)
    }

    /// Hands in the tax answer when the worker has it, and asks for the tax of the invoice as it
    /// now stands when it is complete and no answer or request exists for it (card C2). Without a
    /// worker nothing is asked, and the screen keeps saying tax is not computed.
    fn drive_compute(&mut self, ui: &Ui, ctx: &AppCtx) {
        let (Some(worker), Some(books)) = (ctx.api.as_ref(), ctx.books.as_ref()) else {
            return;
        };
        if let Some((voucher, mut ticket)) = self.computing.take() {
            match compute::settle(&mut ticket) {
                Some(reply) => {
                    let computed = matches!(reply, Ok(Outcome::Value(_)));
                    if computed && self.draft.kind == InvoiceKind::Sales {
                        let draft = voucher.clone();
                        self.einvoice =
                            EInvoiceState::Checking(worker.run(move |api| {
                                api.einvoice_check(&draft).map_err(|e| e.to_string())
                            }));
                    }
                    self.set_gst(voucher, compute::to_result(reply));
                }
                None => self.computing = Some((voucher, ticket)),
            }
        }
        self.poll_einvoice();
        if self.computing.is_none()
            && let Some(voucher) = self.take_compute_request(&books.masters)
        {
            self.einvoice = EInvoiceState::Unknown;
            let ticket = compute::send(worker, voucher.clone(), self.requested_bills().to_vec());
            self.computing = Some((voucher, ticket));
        }
        if self.computing.is_some() {
            ui.ctx().request_repaint();
        }
    }

    /// Takes the e-invoice check's answer when the worker has it.
    fn poll_einvoice(&mut self) {
        let EInvoiceState::Checking(ticket) = &mut self.einvoice else {
            return;
        };
        let answer = match ticket.poll() {
            Poll::Pending | Poll::Taken => return,
            Poll::Ready(reply) => reply,
            Poll::Lost => Err("the engine stopped before it answered".to_owned()),
        };
        self.einvoice = match answer {
            Ok(Outcome::Value(())) => EInvoiceState::Ready,
            Ok(Outcome::Unsupported(u)) => EInvoiceState::NotReady(u.reason),
            Err(message) => EInvoiceState::NotReady(message),
        };
    }

    /// Asks the engine for the number the voucher will get and the party's balance after it, and
    /// finds the party's last narration for Alt+R. All three are the engine's or the books' own
    /// words; the screen adds nothing to them.
    fn drive_extras(&mut self, ui: &Ui, ctx: &AppCtx) {
        if let Some(books) = ctx.books.as_ref() {
            self.find_last_narration(books);
        }
        let (Some(worker), Some(books)) = (ctx.api.as_ref(), ctx.books.as_ref()) else {
            return;
        };
        let date = parse_entry_date(&self.draft.date_text, self.fy).ok();
        let want = date.map(|d| (self.draft.type_id, d));
        let number_busy = self.extras.drive_number(worker, want);
        let balance_want = self.draft.party.zip(self.computed_voucher(&books.masters));
        let balance_busy = self.extras.drive_balance(worker, balance_want);
        let invoiced_busy = self.extras.drive_invoiced(worker, self.invoiced_key());
        if number_busy || balance_busy || invoiced_busy {
            ui.ctx().request_repaint();
        }
    }

    /// Finds the narration of the party's latest voucher of this type, for Alt+R.
    fn find_last_narration(&mut self, books: &ledgerly_model::Books) {
        let party = self.draft.party;
        if self
            .last_narration
            .as_ref()
            .is_some_and(|(p, _)| *p == party)
        {
            return;
        }
        let text = party.and_then(|p| {
            books
                .vouchers
                .values()
                .filter(|v| {
                    v.type_id == self.draft.type_id
                        && v.party == Some(p)
                        && !v.narration.trim().is_empty()
                })
                .max_by_key(|v| (v.date, v.id))
                .map(|v| v.narration.clone())
        });
        self.last_narration = Some((party, text));
    }

    /// Sends the voucher the user asked to save to the worker. Without a worker nothing can be
    /// posted, and the screen says so.
    fn send_saves(&mut self, ctx: &AppCtx) {
        // Without a worker the host takes the request itself, so it stays in the outbox.
        let Some(worker) = ctx.api.as_ref() else {
            return;
        };
        if let Some(voucher) = self.take_save_request() {
            self.posting = Some(submit::send(worker, Command::PostVoucher(voucher)));
            self.notice = Some(Notice::Info(ctx.tr.t("invoice.saving")));
        }
    }

    /// Polls the post in progress; the notice "Saving..." stays up until the worker answers.
    fn wait_for_post(&mut self, ui: &Ui) {
        let Some(ticket) = self.posting.as_mut() else {
            return;
        };
        if let Some(reply) = submit::settle(ticket) {
            self.posting = None;
            self.finish_post(reply);
        } else {
            ui.ctx().request_repaint();
        }
    }

    /// Shows the engine's answer. A booked invoice starts a fresh one; anything else keeps the
    /// draft and says why nothing was saved.
    fn finish_post(&mut self, reply: CommandReply) {
        match reply {
            CommandReply::Booked(result) => {
                let number = result.posted.as_ref().and_then(|v| v.number.clone());
                self.entry_print.remember(result.posted);
                self.confirm_saved_as(number.as_deref());
            }
            CommandReply::Unsupported(u) => {
                self.save_failed(&format!("not supported yet: {} ({})", u.what, u.reason));
            }
            CommandReply::Failed(reason) => self.save_failed(&reason),
        }
    }
}

/// The three parts of the note under the narration (lead, bold sentence, the rest): which tax
/// applies, from the engine's own `heads`, and the states that decide it.
///
/// A sale (and a credit note) is supplied by the company, so the company's own state is the
/// supplier's. A purchase (and a debit note) is supplied by the party, whose state is
/// `supplier`; without it there is nothing honest to say, so no note is made.
#[must_use]
pub fn place_note_parts(
    kind: InvoiceKind,
    heads: &[&str],
    place: StateCode,
    home: StateCode,
    supplier: Option<StateCode>,
    pack_version: &str,
) -> Option<(String, &'static str, String)> {
    let rates = format!(" Rates come from the item masters and rule pack {pack_version}.");
    let (inter, intra) = match kind {
        InvoiceKind::Sales | InvoiceKind::CreditNote => (
            format!(
                "Place of supply {} ({}), you are in {} ({}). ",
                place.name(),
                place.get(),
                home.name(),
                home.get()
            ),
            format!(
                "Place of supply and your state are both {} ({}). ",
                place.name(),
                place.get()
            ),
        ),
        InvoiceKind::Purchase | InvoiceKind::DebitNote => {
            let supplier = supplier?;
            (
                format!(
                    "Supplier in {} ({}), place of supply {} ({}). ",
                    supplier.name(),
                    supplier.get(),
                    place.name(),
                    place.get()
                ),
                format!(
                    "Supplier and place of supply are both {} ({}). ",
                    place.name(),
                    place.get()
                ),
            )
        }
    };
    if heads.contains(&"IGST") {
        Some((inter, "IGST applies.", rates))
    } else if heads.contains(&"CGST") && (heads.contains(&"SGST") || heads.contains(&"UTGST")) {
        let bold: &'static str = if heads.contains(&"SGST") {
            "Tax splits into CGST and SGST."
        } else {
            "Tax splits into CGST and UTGST."
        };
        Some((intra, bold, rates))
    } else {
        None
    }
}

/// A mode as the mode list names it.
fn mode_name(mode: InvoiceMode) -> String {
    crate::i18n::lit(match mode {
        InvoiceMode::Item => "Item invoice",
        InvoiceMode::Accounting => "Accounting invoice",
        InvoiceMode::AsVoucher => "As voucher",
    })
}

/// The few words under a mode's name in the mode list.
fn mode_blurb(mode: InvoiceMode) -> String {
    crate::i18n::lit(match mode {
        InvoiceMode::Item => "Stock items, quantity and rate",
        InvoiceMode::Accounting => "Ledgers and amounts",
        InvoiceMode::AsVoucher => "Dr and Cr ledger lines",
    })
}
