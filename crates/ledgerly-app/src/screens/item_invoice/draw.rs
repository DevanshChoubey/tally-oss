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
use super::draft::line_amount;
use super::extras::BalanceShown;
use super::keys::{Prompt, read_keys};
use super::{
    Field, GstBlock, GstResult, GstStatus, InvoiceMode, ItemInvoiceScreen, Notice, UiEvent,
};
use crate::screen::KeyHint;
use crate::screens::voucher_entry::parse_entry_date;
use crate::screens::voucher_sheet::{self as sheet, Col, InputSpec, Look, Total};
use crate::shell::submit::{self, CommandReply};
use crate::theme::TEXT_BODY;
use crate::ui::a11y;
use crate::ui::badge::{BadgeKind, badge};
use crate::ui::button::{ButtonKind, button};
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::page_head_numbered;
use crate::ui::prov::ProvKind;
use crate::ui::{format, para_text, tokens};
use crate::{AppCtx, Screen, ScreenAction};
use egui::{Frame, Id, Margin, Rect, Response, RichText, Ui};
use ledgerly_api::Command;
use ledgerly_core::LedgerId;
use ledgerly_core::format::amount_in_words;
use ledgerly_model::{Masters, Voucher};

/// The one-line purpose under the page title of an item invoice (index.html:1282). The screen asks
/// the engine again after every edit that changes the invoice.
const SUBTITLE: &str = "Item invoice · tax is recomputed by the GST engine on every keystroke";

/// The one-line purpose under the page title of an accounting invoice.
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

/// The particulars table of an accounting invoice.
const PARTICULAR_COLS: [Col; 4] = [
    Col::text("#", 36.0),
    Col::rest("Particulars"),
    Col::num("Amount", 140.0),
    Col::num("GST", 60.0),
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
    /// Change the mode (Ctrl+H).
    ChangeMode,
    /// Open or close the More details panel (Ctrl+I).
    More,
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
pub fn tax_rows(block: &GstBlock) -> Vec<(String, String)> {
    let mut rows = vec![(
        "Taxable value".to_owned(),
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
            let label = if previous == Some(rate) {
                format!("{} {rate}%", row.head)
            } else {
                format!("{} {rate}% on {}", row.head, format::amount(row.taxable))
            };
            previous = Some(rate);
            rows.push((label, format::amount(row.amount)));
        }
    }
    rows.push(("Round off".to_owned(), format::amount(block.round_off)));
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
            width: None,
        },
    );
    a11y::name(ui, &response, egui::Role::TextInput, spec.name);
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

/// A cell of plain text in the line table, in the secondary ink unless `strong`.
fn cell_text(ui: &mut Ui, text: &str, strong: bool) {
    let t = tokens(ui);
    let ink = if strong { t.text } else { t.text_2 };
    ui.label(para_text(text, TEXT_BODY, ink));
}

/// The party's GSTIN with its check badge, or the B2C words for an unregistered party.
fn gstin_line(ui: &mut Ui, masters: &Masters, party: Option<LedgerId>) {
    let t = tokens(ui);
    let Some(party) = party else {
        ui.label(
            RichText::new("Choose a party first")
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
                badge(ui, BadgeKind::Ok, "Check OK");
            });
        }
        None => {
            ui.label(
                RichText::new("Unregistered (B2C)")
                    .size(TEXT_BODY)
                    .color(t.text_2),
            );
        }
    }
}
/// The e-Invoice cell. This build writes no e-Invoice JSON from a voucher yet (the INV-01 writer
/// in `ledgerly-gst-formats` is not wired to the invoice screen), so a registered buyer's cell says
/// so rather than claiming a file is ready; an unregistered buyer is B2C, which has none.
fn einvoice_cell(ui: &mut Ui, masters: &Masters, party: Option<LedgerId>) {
    let t = tokens(ui);
    let Some(party) = party else {
        return;
    };
    let registered = masters
        .ledgers
        .get(&party)
        .and_then(|l| l.party.as_ref())
        .is_some_and(|p| p.gstin.is_some());
    let (text, ink) = if registered {
        ("Not built yet", t.text_3)
    } else {
        ("Not applicable: B2C", t.text_2)
    };
    ui.label(RichText::new(text).size(TEXT_BODY).color(ink));
}

/// The tax box for an answer the engine computed: its rows, then the invoice total with its
/// engine tag, and the total in words.
fn tax_box(ui: &mut Ui, block: &GstBlock) {
    let rows = tax_rows(block);
    let total = format::amount(block.invoice_total);
    sheet::taxbox(
        ui,
        &rows,
        Some(&Total {
            label: "Invoice total",
            value: &total,
            tag: Some(ProvKind::Engine),
        }),
    );
    sheet::words(ui, &amount_in_words(block.invoice_total));
}

/// The title of the type-ahead list for the open picker.
fn list_caption(focus: Field) -> &'static str {
    match focus {
        Field::Party => "Parties",
        Field::Item(_) => "Stock items",
        Field::Account | Field::Ledger(_) => "Ledgers",
        Field::Date
        | Field::Number
        | Field::Place
        | Field::Reference
        | Field::Qty(_)
        | Field::Rate(_)
        | Field::Discount(_)
        | Field::Amount(_)
        | Field::LedgerAmount(_)
        | Field::BillKind(_)
        | Field::BillName(_)
        | Field::BillAmount(_)
        | Field::Narration => "Choices",
    }
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
                    " The engine re-validates, posts, and writes the audit event in one step.",
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
                    if button(ui, "No", Some("Esc"), ButtonKind::Secondary).clicked() {
                        *clicks = Some(Click::Cancel);
                    }
                    if button(ui, "Yes", Some("Enter"), ButtonKind::Primary).clicked() {
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
        let computed: Option<&GstBlock> = match self.gst_status(masters) {
            GstStatus::Computed(block) => Some(block),
            GstStatus::Incomplete(_)
            | GstStatus::NotComputedYet
            | GstStatus::Unsupported(_)
            | GstStatus::Failed(_) => None,
        };
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
            // The engine's lines are the non-blank rows, in order (`InvoiceDraft::build_voucher`).
            let engine = computed.and_then(|block| block.lines.get(engine_line));
            engine_line = engine_line.saturating_add(1);
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
                problem: line_amount(masters, row, number)
                    .err()
                    .map(|e| e.to_string()),
                focused,
            });
        }
        shown
    }

    /// The head: the fields in four equal columns, the party across two (`.vhead`). Date and the
    /// sales ledger read as plain values until the keyboard is on them (index.html:1287, `.ro`);
    /// the e-Invoice cell says what this build can say about it.
    #[expect(
        clippy::too_many_lines,
        reason = "one block of header fields, each named by its caption"
    )]
    fn draw_head(&mut self, ui: &mut Ui, base: Id, masters: &Masters, cx: &mut Cx<'_>) {
        let kind = self.draft.kind;
        let mode = self.draft.mode;
        let required = self.draft.number_required;
        let party = self.draft.party;
        let fy = self.fy;
        let date_value = parse_entry_date(&self.draft.date_text, fy).ok();
        let date_plain = cx.focus != Field::Date && date_value.is_some();
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
                    |ui| sheet::caption(ui, "Date", Some("F2")),
                    |ui| {
                        // Bare like the mock's read-only date until the keyboard is on it; it
                        // stays a field, so Tab and a screen reader still reach it.
                        let s = spec("date", "Date", Field::Date, "dd-mm-yyyy");
                        let style = if date_plain { CELL } else { BOXED };
                        let shown = date_value
                            .filter(|_| date_plain)
                            .map(crate::ui::format::date);
                        text_field_shown(ui, cx, &s, &mut self.draft.date_text, style, shown);
                    },
                );
                note_field("Date", shown.response.rect);
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
                let shown = sheet::fld(
                    ui,
                    total,
                    1,
                    |ui| sheet::caption(ui, "Place of supply", None),
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
                    |ui| sheet::caption(ui, "Party GSTIN", None),
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
                    InvoiceMode::Accounting => {
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
                let shown = sheet::fld(
                    ui,
                    total,
                    1,
                    |ui| sheet::caption(ui, "e-Invoice", None),
                    |ui| sheet::read_only(ui, |ui| einvoice_cell(ui, masters, party)),
                );
                note_field("e-Invoice", shown.response.rect);
            });
            if required {
                sheet::head_row(ui, |ui| {
                    let shown = sheet::fld(
                        ui,
                        total,
                        1,
                        |ui| sheet::caption(ui, "Voucher no.", None),
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
        let computed: Option<&GstBlock> = match self.gst_status(masters) {
            GstStatus::Computed(block) => Some(block),
            GstStatus::Incomplete(_)
            | GstStatus::NotComputedYet
            | GstStatus::Unsupported(_)
            | GstStatus::Failed(_) => None,
        };
        let mut engine_line = 0_usize;
        let mut rates = Vec::with_capacity(self.draft.particulars.len());
        for row in &self.draft.particulars {
            if row.is_blank() {
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

    /// The item table, or the particulars table of an accounting invoice.
    fn draw_lines(&mut self, ui: &mut Ui, base: Id, masters: &Masters, cx: &mut Cx<'_>) {
        if self.draft.mode == InvoiceMode::Accounting {
            return self.draw_particulars(ui, base, masters, cx);
        }
        let shown = self.lines_shown(masters);
        let with_discount = self.discount_columns();
        let cols: &[Col] = if with_discount {
            &LINE_COLS_DISCOUNT
        } else {
            &LINE_COLS
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
                match c {
                    0 => cell_text(ui, &line.number.to_string(), false),
                    1 => {
                        let s = spec("item", "Item", Field::Item(r));
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

    fn draw_particulars(&mut self, ui: &mut Ui, base: Id, masters: &Masters, cx: &mut Cx<'_>) {
        let rates = self.particular_rates(masters);
        let focused_row = match self.focus {
            Field::Ledger(i) | Field::LedgerAmount(i) => Some(i),
            Field::Date
            | Field::Number
            | Field::Party
            | Field::Place
            | Field::Account
            | Field::Reference
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
        let status = self.gst_status(masters);
        if let GstStatus::Computed(block) = status {
            return tax_box(ui, block);
        }
        let dash = "\u{2014}".to_owned();
        sheet::taxbox(
            ui,
            &[
                ("Taxable value".to_owned(), dash.clone()),
                ("Round off".to_owned(), dash.clone()),
            ],
            Some(&Total {
                label: "Invoice total",
                value: &dash,
                tag: None,
            }),
        );
        ui.add_space(8.0);
        match status {
            GstStatus::Incomplete(why) => {
                if !self.draft.is_blank() {
                    note(
                        ui,
                        NoteKind::Info,
                        &format!("Tax appears once the invoice is complete: {why}."),
                    );
                }
            }
            GstStatus::NotComputedYet => {
                note(
                    ui,
                    NoteKind::Info,
                    "Tax is not computed yet for this invoice.",
                );
            }
            GstStatus::Unsupported(u) => {
                note(
                    ui,
                    NoteKind::Warn,
                    &format!("Not supported yet: {} ({})", u.what, u.reason),
                );
            }
            GstStatus::Failed(m) => {
                note(ui, NoteKind::Bad, &format!("The tax engine failed: {m}"));
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
        let _ = masters;
        let heads: Vec<&str> = block.heads.iter().map(|h| h.head.as_str()).collect();
        let rates = format!(
            " Rates come from the item masters and rule pack {}.",
            block.pack_version
        );
        if heads.contains(&"IGST") {
            Some((
                format!(
                    "Place of supply {} ({}), you are in {} ({}). ",
                    place.name(),
                    place.get(),
                    self.home_state.name(),
                    self.home_state.get()
                ),
                "IGST applies.",
                rates,
            ))
        } else if heads.contains(&"CGST") && (heads.contains(&"SGST") || heads.contains(&"UTGST")) {
            let second = if heads.contains(&"SGST") {
                "SGST"
            } else {
                "UTGST"
            };
            let bold: &'static str = if second == "SGST" {
                "Tax splits into CGST and SGST."
            } else {
                "Tax splits into CGST and UTGST."
            };
            Some((
                format!(
                    "Place of supply and your state are both {} ({}). ",
                    place.name(),
                    place.get()
                ),
                bold,
                rates,
            ))
        } else {
            None
        }
    }

    /// The voucher a computed answer belongs to, with the engine's accounting lines on it: what
    /// the balance line asks the engine about.
    pub(super) fn computed_voucher(&self, masters: &Masters) -> Option<Voucher> {
        let voucher = self.draft.build_voucher(masters, self.fy).ok()?;
        let bills = self.draft.party_bills().ok()?;
        match self.answer_for(&voucher, &bills) {
            Some(GstResult::Computed(block)) => {
                let mut v = voucher;
                v.ledger_lines.clone_from(&block.ledger_lines);
                v.gst = Some(block.stored.clone());
                Some(v)
            }
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
                    &format!(
                        "{name}: balance now {}, after saving {} (engine).",
                        balance_text(change.now),
                        balance_text(change.after)
                    ),
                    crate::theme::TEXT_META,
                    t.text_2,
                ));
            }
            Some(BalanceShown::Missing(why)) => {
                ui.label(para_text(
                    &format!("{name}: balance not shown ({why})."),
                    crate::theme::TEXT_META,
                    t.text_2,
                ));
            }
            None => {}
        }
    }

    /// The foot: narration, the notices and line problems on the left; the tax box on the right.
    fn draw_foot(
        &mut self,
        ui: &mut Ui,
        base: Id,
        masters: &Masters,
        cx: &mut Cx<'_>,
        clicks: &mut Option<Click>,
    ) {
        let lines = self.lines_shown(masters);
        let accounting = self.draft.mode == InvoiceMode::Accounting;
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
                |ui| {
                    let t = tokens(ui);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui.label(para_text("Narration", crate::theme::TEXT_META, t.text_2));
                        crate::ui::keycap::keycap(ui, "Alt+R");
                        let hint = if recall {
                            "recalls the last one for this party"
                        } else {
                            "recalls the last one for this party (none yet)"
                        };
                        ui.label(para_text(hint, crate::theme::TEXT_META, t.text_3));
                    });
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
            match &self.notice {
                Some(Notice::Error(s)) => {
                    note(left, NoteKind::Bad, s);
                }
                Some(Notice::Info(s)) if !(s == "Saved." && self.toast.is_some()) => {
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
            if let GstStatus::Computed(block) = self.gst_status(masters)
                && let Some((lead, bold, rest)) = self.place_note(masters, block)
            {
                info_note(left, &lead, bold, &rest);
            }
            self.balance_line(left, masters);
            self.draw_tax(right, masters);
        });
        self.shown.narration = narration;
        if self.confirming_save {
            save_bar(ui, clicks, &self.save_title());
        }
    }

    /// The question the save bar asks: "Save SI/0418?" with the number the engine will give, or
    /// "Save this invoice?" when it has none to give.
    fn save_title(&self) -> String {
        match &self.extras.number {
            Some(n) => format!("Save {n}?"),
            None => "Save this invoice?".to_owned(),
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
                    "Bill references. Leave empty for one New Ref named by the voucher number. \
                     Types: New Ref, Agst Ref, Advance, On Account."
                } else if self.draft.party.is_some() {
                    "This party does not track bills, so there are no bill references to enter."
                } else {
                    "Choose a party first: bill references depend on whether it tracks bills."
                };
                ui.label(para_text(text, TEXT_BODY, t.text_2));
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
                        |ui| sheet::caption(ui, "Type", None),
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
                        |ui| sheet::caption(ui, "Name", None),
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
                        |ui| sheet::caption(ui, "Amount", None),
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
            || self.create.is_some()
            || self.confirming_save
            || self.searching()
            || !self.draft.is_blank();
        let prompt = Prompt {
            discard: self.confirm_discard,
            save: self.confirming_save,
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
        if self.confirm_discard {
            note(
                ui,
                NoteKind::Warn,
                "Discard this invoice? Press Y to discard it, or N to keep it.",
            );
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one sheet drawn top to bottom; splitting would only pass the same borrows around"
    )]
    fn draw(&mut self, ui: &mut Ui, masters: &Masters) -> ScreenAction {
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
        let locked = self.locked_on();
        let subtitle = self.subtitle().unwrap_or_default();
        let mode_label = match self.draft.mode {
            InvoiceMode::Item => "Item invoice",
            InvoiceMode::Accounting => "Accounting invoice",
        };
        let number = self.extras.number.clone();
        page_head_numbered(
            ui,
            &self.page_title(),
            number.as_deref(),
            Some(subtitle.as_str()),
            |ui| {
                let saved = ui
                    .add_enabled_ui(locked.is_none() && !busy, |ui| {
                        button(ui, "Save", Some("Ctrl+A"), ButtonKind::Primary).clicked()
                    })
                    .inner;
                if saved {
                    clicks = Some(Click::Save);
                }
                // The head lays buttons out right to left: More details sits left of Save and the
                // mode button left of that, as in the mock.
                if button(ui, "More details", Some("Ctrl+I"), ButtonKind::Secondary).clicked() {
                    clicks = Some(Click::More);
                }
                if button(ui, mode_label, Some("Ctrl+H"), ButtonKind::Secondary).clicked() {
                    clicks = Some(Click::ChangeMode);
                }
            },
        );
        self.draw_notes(ui);
        self.draw_create_form(ui, masters);

        let mut events = Vec::new();
        let mut anchor = None;
        let enabled = !self.confirm_discard && self.create.is_none() && !busy;
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
            let drawn = egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
              sheet::sheet(ui, |ui| {
                if let Some((date, lock)) = locked {
                    let (left, top, width) = (ui.cursor().min.x, ui.cursor().min.y, ui.available_width());
                    sheet::lock_bar(
                        ui,
                        "Period locked.",
                        &format!(
                            "{} is on or before the lock date {}. You can look, but saving is blocked. Choose a later date to save.",
                            format::date(date),
                            format::date(lock)
                        ),
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
            }).inner;
            self.shown.sheet = Some(drawn.response.rect);
        });
        if let Some(anchor) = anchor
            && !labels.is_empty()
        {
            let (rect, clicked) = sheet::type_ahead(
                ui,
                base.with("list"),
                anchor,
                list_caption(focus),
                &labels,
                pick,
            );
            self.shown.picker = Some(rect);
            if let Some(j) = clicked {
                events.push(UiEvent::Pick(j));
            }
        }
        self.focus_pending = false;

        self.apply_events(masters, events);
        match clicks {
            Some(Click::Cancel) => self.confirming_save = false,
            Some(Click::Save) => self.save(masters),
            Some(Click::ChangeMode) => self.change_mode(),
            Some(Click::More) => self.toggle_more(masters),
            None => {}
        }
        self.draw_toast(ui);
        if any_key || self.focus_pending {
            ui.ctx().request_repaint();
        }
        action
    }
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
    fn title(&self) -> String {
        format!("{} invoice", self.type_name)
    }

    fn subtitle(&self) -> Option<String> {
        let text = match self.draft.mode {
            InvoiceMode::Item => SUBTITLE,
            InvoiceMode::Accounting => ACCOUNTING_SUBTITLE,
        };
        Some(text.to_owned())
    }

    fn keys(&self) -> Vec<KeyHint> {
        if self.confirm_discard {
            return vec![KeyHint::new("Y", "Discard"), KeyHint::new("N", "Keep it")];
        }
        if self.confirming_save {
            return vec![
                KeyHint::new("Enter", "Save"),
                KeyHint::new("Esc", "Keep editing"),
            ];
        }
        vec![
            KeyHint::new("Enter", "Next field"),
            KeyHint::new("Shift+Enter", "Previous"),
            KeyHint::new("\u{2191} \u{2193}", "List"),
            KeyHint::new("Alt+C", "Create master"),
            KeyHint::new("Ctrl+H", "Change mode"),
            KeyHint::new("Ctrl+A", "Save"),
            KeyHint::new("Esc", "Back"),
        ]
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        // A save that is out is polled first; the form is drawn either way, taking no input while
        // it waits.
        self.wait_for_post(ui);
        self.lock_date = ctx.lock_date;
        self.drive_compute(ui, ctx);
        self.drive_extras(ui, ctx);
        let action = if let Some(books) = ctx.books.as_ref() {
            self.draw(ui, &books.masters)
        } else {
            ui.label(no_books_text(ctx));
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
fn no_books_text(ctx: &AppCtx) -> &'static str {
    if ctx.api.is_some() {
        "The company is open, but this screen cannot read its masters yet."
    } else {
        "No company is open."
    }
}

impl ItemInvoiceScreen {
    /// Hands in the tax answer when the worker has it, and asks for the tax of the invoice as it
    /// now stands when it is complete and no answer or request exists for it (card C2). Without a
    /// worker nothing is asked, and the screen keeps saying tax is not computed.
    fn drive_compute(&mut self, ui: &Ui, ctx: &AppCtx) {
        let (Some(worker), Some(books)) = (ctx.api.as_ref(), ctx.books.as_ref()) else {
            return;
        };
        if let Some((voucher, mut ticket)) = self.computing.take() {
            match compute::settle(&mut ticket) {
                Some(reply) => self.set_gst(voucher, compute::to_result(reply)),
                None => self.computing = Some((voucher, ticket)),
            }
        }
        if self.computing.is_none()
            && let Some(voucher) = self.take_compute_request(&books.masters)
        {
            let ticket = compute::send(worker, voucher.clone(), self.requested_bills().to_vec());
            self.computing = Some((voucher, ticket));
        }
        if self.computing.is_some() {
            ui.ctx().request_repaint();
        }
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
        if number_busy || balance_busy {
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
            self.notice = Some(Notice::Info("Saving...".to_owned()));
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
                self.confirm_saved_as(number.as_deref());
            }
            CommandReply::Unsupported(u) => {
                self.save_failed(&format!("not supported yet: {} ({})", u.what, u.reason));
            }
            CommandReply::Failed(reason) => self.save_failed(&reason),
        }
    }
}
