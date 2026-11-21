//! Drawing the screen with egui. Draws and reports events; it computes nothing.
//!
//! The layout follows the mock (`design/mock-v2/index.html`, `sales-invoice`): a page head with
//! the save action, a header card (date, party, place of supply, party GSTIN, sales ledger,
//! reference), the line table, and a footer card with the narration on the left and the engine's
//! tax box on the right. Every amount shown is one the engine returned (rule C4). The quantity,
//! rate, discount and typed amount are the user's own input, shown as typed.

use super::compute;
use super::draft::line_amount;
use super::keys::read_keys;
use super::{Field, GstBlock, GstStatus, ItemInvoiceScreen, Notice, UiEvent};
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
use egui::{Align, Id, Layout, RichText, Ui};
use egui_extras::Column;
use ledgerly_api::Command;
use ledgerly_core::LedgerId;
use ledgerly_core::format::amount_in_words;
use ledgerly_model::Masters;

/// The one-line purpose under the page title. It says when the engine works out tax: when the
/// invoice changes, not on every keystroke.
const SUBTITLE: &str = "Item invoice · tax is worked out by the GST engine when the invoice changes";

/// Where a text field sits in the focus order, how wide it is and what it says when empty.
struct FieldSpec {
    id: Id,
    field: Field,
    width: f32,
    hint: &'static str,
}

/// The buttons pressed this frame, reported after the widgets are drawn.
#[derive(Default)]
struct Clicks {
    save: bool,
    confirm: bool,
    cancel: bool,
    discard: bool,
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
        Field::Item(i)
        | Field::Qty(i)
        | Field::Rate(i)
        | Field::Discount(i)
        | Field::Amount(i) => Some(i),
        Field::Date
        | Field::Number
        | Field::Party
        | Field::Place
        | Field::Account
        | Field::Reference
        | Field::Narration => None,
    }
}

/// The rows of the tax box above the invoice total, in display order, as (label, amount): the
/// engine's taxable value, each tax head it returned, and the round-off. Nothing is added here.
#[must_use]
pub fn tax_rows(block: &GstBlock) -> Vec<(String, String)> {
    let mut rows = vec![("Taxable value".to_owned(), format::amount(block.taxable_total))];
    for head in &block.heads {
        rows.push((head.head.clone(), format::amount(head.amount)));
    }
    rows.push(("Round off".to_owned(), format::amount(block.round_off)));
    rows
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

/// The type-ahead list under a field: one selectable row per entry; a click picks it.
fn draw_picker(ui: &mut Ui, labels: &[String], pick: usize, events: &mut Vec<UiEvent>) {
    if labels.is_empty() {
        return;
    }
    ui.group(|ui| {
        for (j, label) in labels.iter().enumerate() {
            if ui.selectable_label(j == pick, label.as_str()).clicked() {
                events.push(UiEvent::Pick(j));
            }
        }
    });
}

/// The party's GSTIN with its check badge, or the B2C words for an unregistered party.
fn gstin_line(ui: &mut Ui, masters: &Masters, party: Option<LedgerId>) {
    let t = tokens(ui);
    let Some(party) = party else {
        ui.label(RichText::new("Choose a party first").size(TEXT_BODY).color(t.text_3));
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
            ui.label(RichText::new("Unregistered (B2C)").size(TEXT_BODY).color(t.text_2));
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
        ui.label(RichText::new("Invoice total").font(h2.clone()).color(t.text));
        prov(ui, ProvKind::Engine);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(format::amount(block.invoice_total)).font(h2).color(t.text));
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
                ui.vertical(|ui| {
                    caption(ui, "Date");
                    let s = spec("date", Field::Date, 120.0, "dd-mm-yyyy");
                    text_field(ui, &s, &mut self.draft.date_text, want, ev);
                    if let Ok(date) = parse_entry_date(&self.draft.date_text, fy) {
                        ui.label(RichText::new(format::date(date)).size(TEXT_META).color(t.text_3));
                    }
                });
                ui.vertical(|ui| {
                    caption(ui, kind.party_label());
                    let s = spec("party", Field::Party, 300.0, "type to search");
                    text_field(ui, &s, &mut self.draft.party_text, want, ev);
                    if focus == Field::Party {
                        draw_picker(ui, labels, pick, ev);
                    }
                });
                ui.vertical(|ui| {
                    caption(ui, "Place of supply");
                    let s = spec("place", Field::Place, 200.0, "code or name");
                    text_field(ui, &s, &mut self.draft.place_text, want, ev);
                });
                ui.end_row();

                ui.vertical(|ui| {
                    caption(ui, "Party GSTIN");
                    gstin_line(ui, masters, self.draft.party);
                });
                ui.vertical(|ui| {
                    caption(ui, kind.account_label());
                    let s = spec("account", Field::Account, 300.0, "type to search");
                    text_field(ui, &s, &mut self.draft.account_text, want, ev);
                    if focus == Field::Account {
                        draw_picker(ui, labels, pick, ev);
                    }
                });
                ui.vertical(|ui| {
                    caption(ui, kind.reference_label());
                    let s = spec("ref", Field::Reference, 200.0, "Optional");
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

    fn draw_lines(
        &mut self,
        ui: &mut Ui,
        base: Id,
        masters: &Masters,
        labels: &[String],
        ev: &mut Vec<UiEvent>,
    ) {
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
                let heads = [
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
                for (label, numeric) in heads {
                    table::header(&mut row, label, numeric);
                }
            })
            .body(|mut body| {
                body.rows(table::ROW_HEIGHT, count, |mut row| {
                    let i = row.index();
                    let (Some(line), Some(cells)) = (shown.get(i), self.draft.rows.get_mut(i))
                    else {
                        return;
                    };
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
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let s = FieldSpec {
                                id: base.with(("qty", i)),
                                field: Field::Qty(i),
                                width: ui.available_width(),
                                hint: "Qty",
                            };
                            text_field(ui, &s, &mut cells.qty_text, want, ev);
                        });
                    });
                    row.col(|ui| table::text(ui, &line.unit));
                    row.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let s = FieldSpec {
                                id: base.with(("rate", i)),
                                field: Field::Rate(i),
                                width: ui.available_width(),
                                hint: "Rate",
                            };
                            text_field(ui, &s, &mut cells.rate_text, want, ev);
                        });
                    });
                    row.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let s = FieldSpec {
                                id: base.with(("disc", i)),
                                field: Field::Discount(i),
                                width: ui.available_width(),
                                hint: "%",
                            };
                            text_field(ui, &s, &mut cells.discount_text, want, ev);
                        });
                    });
                    row.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let s = FieldSpec {
                                id: base.with(("amt", i)),
                                field: Field::Amount(i),
                                width: ui.available_width(),
                                hint: "optional",
                            };
                            text_field(ui, &s, &mut cells.amount_text, want, ev);
                        });
                    });
                    row.col(|ui| table::numeric(ui, &line.gst));
                    row.col(|ui| table::numeric(ui, &line.amount));
                });
            });
        if matches!(focus, Field::Item(_)) {
            draw_picker(ui, labels, pick, ev);
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
                note(ui, NoteKind::Info, "Tax is not computed yet for this invoice.");
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
        clicks: &mut Clicks,
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
                    text_field(ui, &s, &mut self.draft.narration, want, ev);
                    match &self.notice {
                        Some(Notice::Error(s)) => note(ui, NoteKind::Bad, s),
                        Some(Notice::Info(s)) => note(ui, NoteKind::Info, s),
                        None => {}
                    }
                    if self.confirming_save {
                        ui.horizontal(|ui| {
                            note(
                                ui,
                                NoteKind::Ok,
                                "Save this invoice? The engine checks it again before it is posted.",
                            );
                            clicks.confirm |=
                                button(ui, "Yes", Some("Enter"), ButtonKind::Primary).clicked();
                            clicks.cancel |=
                                button(ui, "No", Some("Esc"), ButtonKind::Secondary).clicked();
                        });
                    } else {
                        ui.horizontal(|ui| {
                            clicks.discard |=
                                button(ui, "Discard", Some("Esc"), ButtonKind::Danger).clicked();
                        });
                    }
                });
                right.vertical(|ui| {
                    card(ui, |ui| self.draw_tax(ui, masters));
                });
            }
        });
    }

    fn draw(&mut self, ui: &mut Ui, masters: &Masters) -> ScreenAction {
        let base = ui.scope_id().with("item-invoice");
        let escape_wanted = self.confirm_discard
            || self.confirming_save
            || self.searching()
            || !self.draft.is_blank();
        let mut labels = self.pick_labels(masters);
        let keys = read_keys(
            ui,
            !labels.is_empty(),
            escape_wanted,
            self.confirm_discard,
            self.confirming_save,
        );
        let mut action = ScreenAction::Stay;
        for cmd in &keys {
            if matches!(
                self.handle_key(masters, *cmd, labels.len()),
                ScreenAction::Pop
            ) {
                action = ScreenAction::Pop;
            }
        }
        if !keys.is_empty() {
            labels = self.pick_labels(masters);
        }

        let mut clicks = Clicks::default();
        let locked = self.locked_on().is_some();
        let subtitle = self.subtitle().unwrap_or_default();
        page_head(ui, &self.page_title(), Some(subtitle.as_str()), |ui| {
            clicks.save = ui
                .add_enabled_ui(!locked, |ui| {
                    button(ui, "Save", Some("Ctrl+A"), ButtonKind::Primary).clicked()
                })
                .inner;
        });

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

        let mut events = Vec::new();
        let enabled = !self.confirm_discard;
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

        if clicks.discard {
            events.push(UiEvent::Leave);
        }
        if matches!(self.apply_events(masters, events), ScreenAction::Pop) {
            action = ScreenAction::Pop;
        }
        if clicks.cancel {
            self.confirming_save = false;
        }
        if clicks.save || clicks.confirm {
            self.save(masters);
        }
        if !keys.is_empty() || self.focus_pending {
            ui.ctx().request_repaint();
        }
        action
    }
}

impl Screen for ItemInvoiceScreen {
    fn title(&self) -> String {
        format!("{} invoice", self.type_name)
    }

    fn subtitle(&self) -> Option<String> {
        Some(SUBTITLE.to_owned())
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
