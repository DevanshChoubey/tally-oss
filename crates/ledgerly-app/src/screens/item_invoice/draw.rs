//! Drawing the screen with egui. Draws and reports events; it computes nothing.
//!
//! The layout follows the mock (`design/mock-v2/index.html`, `sales-invoice`): a page head with
//! the save action, a header card (date, party, place of supply, party GSTIN, sales ledger,
//! reference), the line table, and a footer card with the narration on the left and the engine's
//! tax box on the right. Every amount shown is one the engine returned (rule C4). The quantity,
//! rate, discount and typed amount are the user's own input, shown as typed.

use super::compute;
use super::draft::{InvoiceRow, ParticularRow, line_amount};
use super::keys::{Prompt, read_keys};
use super::{Field, GstBlock, GstStatus, InvoiceMode, ItemInvoiceScreen, Notice, UiEvent};
use crate::screen::KeyHint;
use crate::screens::voucher_entry::parse_entry_date;
use crate::shell::submit::{self, CommandReply};
use crate::theme::{SPACE_3, SPACE_4, TEXT_BODY, TEXT_H2, TEXT_META};
use crate::ui::badge::{BadgeKind, badge};
use crate::ui::button::{ButtonKind, button};
use crate::ui::card::card;
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::page_head;
use crate::ui::prov::{ProvKind, prov};
use crate::ui::{bold_font, format, table, tokens};
use crate::{AppCtx, Screen, ScreenAction};
use egui::{Align, Id, Layout, Rect, RichText, Ui};
use egui_extras::{Column, TableRow};
use ledgerly_api::Command;
use ledgerly_core::LedgerId;
use ledgerly_core::format::amount_in_words;
use ledgerly_model::Masters;

/// The one-line purpose under the page title of an item invoice. It says when the engine works
/// out tax: when the invoice changes, not on every keystroke.
const SUBTITLE: &str =
    "Item invoice · tax is worked out by the GST engine when the invoice changes";

/// The one-line purpose under the page title of an accounting invoice.
const ACCOUNTING_SUBTITLE: &str = "Accounting invoice · ledgers under Particulars; tax is worked out by the GST engine when the invoice changes";

/// Width of the header's date field, and so of its first column.
const DATE_WIDTH: f32 = 120.0;
/// Width of the party and ledger fields, and so of the header's second column.
const PARTY_WIDTH: f32 = 300.0;
/// Width of the place and reference fields, and so of the header's third column.
const PLACE_WIDTH: f32 = 200.0;

/// The particulars table's column headings, and whether each column holds numbers.
const PARTICULAR_HEADS: [(&str, bool); 4] = [
    ("#", false),
    ("Particulars", false),
    ("Amount", true),
    ("GST", true),
];

/// Where a text field sits in the focus order, how wide it is and what it says when empty.
struct FieldSpec {
    id: Id,
    field: Field,
    width: f32,
    hint: &'static str,
}

/// The line table's column headings, and whether each column holds numbers.
const LINE_HEADS: [(&str, bool); 10] = [
    ("#", false),
    ("Item", false),
    ("HSN", false),
    ("Qty", true),
    ("Unit", false),
    ("Rate", true),
    ("Disc %", true),
    ("Typed amount", true),
    ("GST", true),
    ("Amount", true),
];

/// The button pressed this frame, reported after the widgets are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Click {
    /// Save, or Yes on the save question: both ask the engine to save.
    Save,
    /// No on the save question.
    Cancel,
    /// Discard the invoice.
    Discard,
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
        | Field::Narration => None,
    }
}

/// The rows of the tax box above the invoice total, in display order, as (label, amount): the
/// engine's taxable value, each tax head it returned, and the round-off. Nothing is added here.
#[must_use]
pub fn tax_rows(block: &GstBlock) -> Vec<(String, String)> {
    let mut rows = vec![(
        "Taxable value".to_owned(),
        format::amount(block.taxable_total),
    )];
    for head in &block.heads {
        rows.push((head.head.clone(), format::amount(head.amount)));
    }
    rows.push(("Round off".to_owned(), format::amount(block.round_off)));
    rows
}

/// One line of the item table: its number, the item and number fields, and the engine's figures.
fn line_row(
    row: &mut TableRow<'_, '_>,
    line: &LineShown,
    cells: &mut InvoiceRow,
    base: Id,
    i: usize,
    want: Option<Field>,
    ev: &mut Vec<UiEvent>,
) {
    row.set_selected(line.focused);
    row.col(|ui| table::text(ui, &line.number.to_string()));
    row.col(|ui| {
        let s = FieldSpec {
            id: base.with(("item", i)),
            field: Field::Item(i),
            width: ui.available_width(),
            hint: "Item: type to search",
        };
        text_field(ui, &s, &mut cells.item_text, want, ev);
    });
    row.col(|ui| table::text(ui, &line.hsn));
    row.col(|ui| {
        let id = base.with(("qty", i));
        numeric_field(ui, id, Field::Qty(i), "Qty", &mut cells.qty_text, want, ev);
    });
    row.col(|ui| table::text(ui, &line.unit));
    row.col(|ui| {
        let id = base.with(("rate", i));
        numeric_field(
            ui,
            id,
            Field::Rate(i),
            "Rate",
            &mut cells.rate_text,
            want,
            ev,
        );
    });
    row.col(|ui| {
        let id = base.with(("disc", i));
        numeric_field(
            ui,
            id,
            Field::Discount(i),
            "%",
            &mut cells.discount_text,
            want,
            ev,
        );
    });
    row.col(|ui| {
        let id = base.with(("amt", i));
        numeric_field(
            ui,
            id,
            Field::Amount(i),
            "optional",
            &mut cells.amount_text,
            want,
            ev,
        );
    });
    row.col(|ui| table::numeric(ui, &line.gst));
    row.col(|ui| table::numeric(ui, &line.amount));
}

/// One particular of an accounting invoice: its number, the ledger and amount fields, and the
/// GST rate the engine applied (`gst`, empty until the engine has answered).
#[allow(
    clippy::too_many_arguments,
    reason = "one table row's widgets and where it reports to"
)]
fn particular_row(
    row: &mut TableRow<'_, '_>,
    cells: &mut ParticularRow,
    gst: &str,
    base: Id,
    i: usize,
    focused: bool,
    want: Option<Field>,
    ev: &mut Vec<UiEvent>,
) {
    row.set_selected(focused);
    row.col(|ui| table::text(ui, &i.saturating_add(1).to_string()));
    row.col(|ui| {
        let s = FieldSpec {
            id: base.with(("ledger", i)),
            field: Field::Ledger(i),
            width: ui.available_width(),
            hint: "Ledger: type to search",
        };
        text_field(ui, &s, &mut cells.ledger_text, want, ev);
    });
    row.col(|ui| {
        let id = base.with(("ledger-amt", i));
        numeric_field(
            ui,
            id,
            Field::LedgerAmount(i),
            "Amount",
            &mut cells.amount_text,
            want,
            ev,
        );
    });
    row.col(|ui| table::numeric(ui, gst));
}

/// A right-aligned text field for a number in a table cell, as wide as the cell.
fn numeric_field(
    ui: &mut Ui,
    id: Id,
    field: Field,
    hint: &'static str,
    text: &mut String,
    want: Option<Field>,
    events: &mut Vec<UiEvent>,
) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let spec = FieldSpec {
            id,
            field,
            width: ui.available_width(),
            hint,
        };
        text_field(ui, &spec, text, want, events);
    });
}

/// A text field: reports focus, typing and Enter as events.
fn text_field(
    ui: &mut Ui,
    spec: &FieldSpec,
    text: &mut String,
    want: Option<Field>,
    events: &mut Vec<UiEvent>,
) {
    let response = ui.add(
        egui::TextEdit::singleline(text)
            .id(spec.id)
            .desired_width(spec.width)
            .hint_text(spec.hint),
    );
    if want == Some(spec.field) {
        response.request_focus();
    }
    if response.has_focus() {
        events.push(UiEvent::Focus(spec.field));
    }
    if response.changed() {
        events.push(UiEvent::Typed(spec.field));
    }
    if response.lost_focus() && ui.ctx().input(|i| i.key_pressed(egui::Key::Enter)) {
        let back = ui.ctx().input(|i| i.modifiers.shift);
        events.push(UiEvent::Submit {
            field: spec.field,
            back,
        });
    }
}

/// A small caption above a field, in the secondary text colour.
fn caption(ui: &mut Ui, label: &str) {
    let t = tokens(ui);
    ui.label(RichText::new(label).size(TEXT_META).color(t.text_2));
}

/// How tall the type-ahead list grows before it scrolls, so a long list (every ledger, on an
/// empty query) never pushes the rest of the invoice off the screen.
const PICKER_MAX_HEIGHT: f32 = 220.0;

/// The type-ahead list under a field: one selectable row per entry; a click picks it. Returns
/// where the list was drawn, or `None` when there is nothing to list.
fn draw_picker(
    ui: &mut Ui,
    labels: &[String],
    pick: usize,
    events: &mut Vec<UiEvent>,
) -> Option<Rect> {
    if labels.is_empty() {
        return None;
    }
    let shown = ui.group(|ui| {
        // A grid cell offers little height, and a scroll area then shrinks to its default
        // minimum (about three rows); a long list keeps its full height instead.
        egui::ScrollArea::vertical()
            .max_height(PICKER_MAX_HEIGHT)
            .min_scrolled_height(PICKER_MAX_HEIGHT)
            .show(ui, |ui| {
                for (j, label) in labels.iter().enumerate() {
                    let row = ui.selectable_label(j == pick, label.as_str());
                    if j == pick {
                        row.scroll_to_me(None);
                    }
                    if row.clicked() {
                        events.push(UiEvent::Pick(j));
                    }
                }
            });
    });
    Some(shown.response.rect)
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

/// One tax-box line: the label on the left, the amount right-aligned.
fn tax_line(ui: &mut Ui, label: &str, value: &str) {
    let t = tokens(ui);
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(TEXT_BODY).color(t.text_2));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            table::numeric(ui, value);
        });
    });
}

/// The tax box for an answer the engine computed: its rows, then the invoice total with its
/// engine tag and the total in words.
fn tax_box(ui: &mut Ui, block: &GstBlock) {
    let t = tokens(ui);
    for (label, value) in tax_rows(block) {
        tax_line(ui, &label, &value);
    }
    ui.separator();
    ui.horizontal(|ui| {
        let h2 = bold_font(ui, TEXT_H2);
        ui.label(
            RichText::new("Invoice total")
                .font(h2.clone())
                .color(t.text),
        );
        prov(ui, ProvKind::Engine);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(format::amount(block.invoice_total))
                    .font(h2)
                    .color(t.text),
            );
        });
    });
    ui.label(
        RichText::new(amount_in_words(block.invoice_total))
            .size(TEXT_META)
            .color(t.text_2),
    );
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

    fn draw_header(
        &mut self,
        ui: &mut Ui,
        base: Id,
        masters: &Masters,
        labels: &[String],
        ev: &mut Vec<UiEvent>,
    ) {
        let want = self.focus_pending.then_some(self.focus);
        let (focus, pick, fy) = (self.focus, self.pick_index, self.fy);
        let kind = self.draft.kind;
        let t = tokens(ui);
        let spec = |name: &str, field: Field, width: f32, hint: &'static str| FieldSpec {
            id: base.with(name),
            field,
            width,
            hint,
        };
        egui::Grid::new(base.with("header"))
            .num_columns(3)
            .spacing([SPACE_4, SPACE_3])
            .show(ui, |ui| {
                // A grid column has no width until a frame has measured it, and egui then lays it
                // out at its minimum, which wraps every label in it. The first row's cells say
                // how wide their columns are.
                ui.vertical(|ui| {
                    ui.set_min_width(DATE_WIDTH);
                    caption(ui, "Date");
                    let s = spec("date", Field::Date, DATE_WIDTH, "dd-mm-yyyy");
                    text_field(ui, &s, &mut self.draft.date_text, want, ev);
                    if let Ok(date) = parse_entry_date(&self.draft.date_text, fy) {
                        ui.label(
                            RichText::new(format::date(date))
                                .size(TEXT_META)
                                .color(t.text_3),
                        );
                    }
                });
                ui.vertical(|ui| {
                    ui.set_min_width(PARTY_WIDTH);
                    caption(ui, kind.party_label());
                    let s = spec("party", Field::Party, PARTY_WIDTH, "type to search");
                    text_field(ui, &s, &mut self.draft.party_text, want, ev);
                    if focus == Field::Party {
                        self.shown.picker = draw_picker(ui, labels, pick, ev);
                    }
                });
                ui.vertical(|ui| {
                    ui.set_min_width(PLACE_WIDTH);
                    caption(ui, "Place of supply");
                    let s = spec("place", Field::Place, PLACE_WIDTH, "code or name");
                    text_field(ui, &s, &mut self.draft.place_text, want, ev);
                });
                ui.end_row();

                ui.vertical(|ui| {
                    caption(ui, "Party GSTIN");
                    gstin_line(ui, masters, self.draft.party);
                });
                match self.draft.mode {
                    InvoiceMode::Item => {
                        ui.vertical(|ui| {
                            caption(ui, kind.account_label());
                            let s = spec("account", Field::Account, PARTY_WIDTH, "type to search");
                            text_field(ui, &s, &mut self.draft.account_text, want, ev);
                            if focus == Field::Account {
                                self.shown.picker = draw_picker(ui, labels, pick, ev);
                            }
                        });
                    }
                    // The sales FAQ: no Sales ledger field in accounting invoice mode.
                    InvoiceMode::Accounting => {
                        ui.label("");
                    }
                }
                ui.vertical(|ui| {
                    caption(ui, kind.reference_label());
                    let s = spec("ref", Field::Reference, PLACE_WIDTH, "Optional");
                    text_field(ui, &s, &mut self.draft.reference_text, want, ev);
                });
                ui.end_row();

                if self.draft.number_required {
                    ui.vertical(|ui| {
                        caption(ui, "Voucher no.");
                        let s = spec("number", Field::Number, 160.0, "Required");
                        text_field(ui, &s, &mut self.draft.number_text, want, ev);
                    });
                    ui.end_row();
                }
            });
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

    fn draw_particulars(
        &mut self,
        ui: &mut Ui,
        base: Id,
        masters: &Masters,
        labels: &[String],
        ev: &mut Vec<UiEvent>,
    ) {
        let want = self.focus_pending.then_some(self.focus);
        let (focus, pick) = (self.focus, self.pick_index);
        let rates = self.particular_rates(masters);
        let focused_row = match focus {
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
            | Field::Narration => None,
        };
        let count = self.draft.particulars.len();
        table::builder(ui)
            .id_salt(base.with("particulars"))
            .vscroll(false)
            .column(Column::exact(28.0))
            .column(Column::remainder().at_least(240.0))
            .column(Column::exact(140.0))
            .column(Column::exact(64.0))
            .header(table::HEADER_HEIGHT, |mut row| {
                for (label, numeric) in PARTICULAR_HEADS {
                    table::header(&mut row, label, numeric);
                }
            })
            .body(|body| {
                body.rows(table::ROW_HEIGHT, count, |mut row| {
                    let i = row.index();
                    let (Some(gst), Some(cells)) =
                        (rates.get(i), self.draft.particulars.get_mut(i))
                    else {
                        return;
                    };
                    let focused = focused_row == Some(i);
                    particular_row(&mut row, cells, gst, base, i, focused, want, ev);
                });
            });
        if matches!(focus, Field::Ledger(_)) {
            self.shown.picker = draw_picker(ui, labels, pick, ev);
        }
    }

    fn draw_lines(
        &mut self,
        ui: &mut Ui,
        base: Id,
        masters: &Masters,
        labels: &[String],
        ev: &mut Vec<UiEvent>,
    ) {
        if self.draft.mode == InvoiceMode::Accounting {
            return self.draw_particulars(ui, base, masters, labels, ev);
        }
        let want = self.focus_pending.then_some(self.focus);
        let (focus, pick) = (self.focus, self.pick_index);
        let shown = self.lines_shown(masters);
        let count = shown.len();
        table::builder(ui)
            .id_salt(base.with("lines"))
            .vscroll(false)
            .column(Column::exact(28.0))
            .column(Column::remainder().at_least(180.0))
            .column(Column::exact(84.0))
            .column(Column::exact(72.0))
            .column(Column::exact(56.0))
            .column(Column::exact(100.0))
            .column(Column::exact(64.0))
            .column(Column::exact(108.0))
            .column(Column::exact(56.0))
            .column(Column::exact(112.0))
            .header(table::HEADER_HEIGHT, |mut row| {
                for (label, numeric) in LINE_HEADS {
                    table::header(&mut row, label, numeric);
                }
            })
            .body(|body| {
                body.rows(table::ROW_HEIGHT, count, |mut row| {
                    let i = row.index();
                    let (Some(line), Some(cells)) = (shown.get(i), self.draft.rows.get_mut(i))
                    else {
                        return;
                    };
                    line_row(&mut row, line, cells, base, i, want, ev);
                });
            });
        if matches!(focus, Field::Item(_)) {
            self.shown.picker = draw_picker(ui, labels, pick, ev);
        }
        for line in shown.iter().filter(|line| !line.focused) {
            if let Some(problem) = &line.problem {
                note(ui, NoteKind::Warn, problem);
            }
        }
    }

    fn draw_tax(&self, ui: &mut Ui, masters: &Masters) {
        match self.gst_status(masters) {
            GstStatus::Incomplete(why) => {
                note(
                    ui,
                    NoteKind::Info,
                    &format!("Tax appears once the invoice is complete: {why}."),
                );
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
            GstStatus::Computed(block) => tax_box(ui, block),
        }
    }

    fn draw_footer(
        &mut self,
        ui: &mut Ui,
        base: Id,
        masters: &Masters,
        ev: &mut Vec<UiEvent>,
        clicks: &mut Option<Click>,
    ) {
        let want = self.focus_pending.then_some(self.focus);
        ui.columns(2, |cols| {
            if let [left, right] = cols {
                left.vertical(|ui| {
                    caption(ui, "Narration");
                    let s = FieldSpec {
                        id: base.with("narration"),
                        field: Field::Narration,
                        width: ui.available_width(),
                        hint: "Optional",
                    };
                    let field = ui.scope(|ui| text_field(ui, &s, &mut self.draft.narration, want, ev));
                    self.shown.narration = Some(field.response.rect);
                    match &self.notice {
                        Some(Notice::Error(s)) => {
                            note(ui, NoteKind::Bad, s);
                        }
                        Some(Notice::Info(s)) => {
                            note(ui, NoteKind::Info, s);
                        }
                        None => {}
                    }
                    if self.confirming_save {
                        ui.horizontal(|ui| {
                            note(
                                ui,
                                NoteKind::Ok,
                                "Save this invoice? The engine checks it again before it is posted.",
                            );
                            if button(ui, "Yes", Some("Enter"), ButtonKind::Primary).clicked() {
                                *clicks = Some(Click::Save);
                            }
                            if button(ui, "No", Some("Esc"), ButtonKind::Secondary).clicked() {
                                *clicks = Some(Click::Cancel);
                            }
                        });
                    } else {
                        ui.horizontal(|ui| {
                            if button(ui, "Discard", Some("Esc"), ButtonKind::Danger).clicked() {
                                *clicks = Some(Click::Discard);
                            }
                        });
                    }
                });
                right.vertical(|ui| {
                    card(ui, |ui| self.draw_tax(ui, masters));
                });
            }
        });
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

    /// The discard question and the period-lock note above the invoice.
    fn draw_notes(&self, ui: &mut Ui) {
        if self.confirm_discard {
            note(
                ui,
                NoteKind::Warn,
                "Discard this invoice? Press Y to discard it, or N to keep it.",
            );
        }
        if let Some((date, lock)) = self.locked_on() {
            note(
                ui,
                NoteKind::Bad,
                &format!(
                    "Period locked. {} is on or before the lock date {}. You can look, but saving is blocked. Choose a later date to save.",
                    format::date(date),
                    format::date(lock)
                ),
            );
        }
    }

    fn draw(&mut self, ui: &mut Ui, masters: &Masters) -> ScreenAction {
        self.shown = Shown::default();
        let base = ui.scope_id().with("item-invoice");
        let mut labels = self.pick_labels(masters);
        let (any_key, pop) = self.run_keys(ui, masters, &labels);
        let mut action = if pop {
            ScreenAction::Pop
        } else {
            ScreenAction::Stay
        };
        if any_key {
            labels = self.pick_labels(masters);
        }

        let mut clicks: Option<Click> = None;
        let locked = self.locked_on().is_some();
        let subtitle = self.subtitle().unwrap_or_default();
        page_head(ui, &self.page_title(), Some(subtitle.as_str()), |ui| {
            let saved = ui
                .add_enabled_ui(!locked, |ui| {
                    button(ui, "Save", Some("Ctrl+A"), ButtonKind::Primary).clicked()
                })
                .inner;
            if saved {
                clicks = Some(Click::Save);
            }
        });
        self.draw_notes(ui);
        self.draw_create_form(ui, masters);

        let mut events = Vec::new();
        let enabled = !self.confirm_discard && self.create.is_none();
        ui.add_enabled_ui(enabled, |ui| {
            card(ui, |ui| {
                self.draw_header(ui, base, masters, &labels, &mut events);
                ui.add_space(SPACE_3);
                self.draw_lines(ui, base, masters, &labels, &mut events);
            });
            ui.add_space(SPACE_3);
            self.draw_footer(ui, base, masters, &mut events, &mut clicks);
        });
        self.focus_pending = false;

        if clicks == Some(Click::Discard) {
            events.push(UiEvent::Leave);
        }
        if matches!(self.apply_events(masters, events), ScreenAction::Pop) {
            action = ScreenAction::Pop;
        }
        match clicks {
            Some(Click::Cancel) => self.confirming_save = false,
            Some(Click::Save) => self.save(masters),
            Some(Click::Discard) | None => {}
        }
        if any_key || self.focus_pending {
            ui.ctx().request_repaint();
        }
        action
    }
}

/// Where parts of the screen were drawn in the last frame, for layout tests.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Shown {
    /// The narration field.
    pub(super) narration: Option<Rect>,
    /// The open type-ahead list.
    pub(super) picker: Option<Rect>,
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
            KeyHint::new("Shift+Enter", "Previous field"),
            KeyHint::new("Up / Down", "List"),
            KeyHint::new("Alt+C", "Create master"),
            KeyHint::new("Ctrl+A", "Save"),
        ]
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        if self.posting.is_some() {
            self.wait_for_post(ui);
            return ScreenAction::Stay;
        }
        self.lock_date = ctx.lock_date;
        self.drive_compute(ui, ctx);
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
            let ticket = compute::send(worker, voucher.clone());
            self.computing = Some((voucher, ticket));
        }
        if self.computing.is_some() {
            ui.ctx().request_repaint();
        }
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

    /// Polls the post in progress and shows "Saving..." until the worker answers.
    fn wait_for_post(&mut self, ui: &mut Ui) {
        let Some(ticket) = self.posting.as_mut() else {
            return;
        };
        if let Some(reply) = submit::settle(ticket) {
            self.posting = None;
            self.finish_post(reply);
        } else {
            ui.label("Saving...");
            ui.ctx().request_repaint();
        }
    }

    /// Shows the engine's answer. A booked invoice starts a fresh one; anything else keeps the
    /// draft and says why nothing was saved.
    fn finish_post(&mut self, reply: CommandReply) {
        match reply {
            CommandReply::Booked(_) => self.confirm_saved(),
            CommandReply::Unsupported(u) => {
                self.save_failed(&format!("not supported yet: {} ({})", u.what, u.reason));
            }
            CommandReply::Failed(reason) => self.save_failed(&reason),
        }
    }
}
