//! Drawing the screen with egui. Draws and reports events; it computes nothing.

use super::keys::read_keys;
use super::{Field, GstBlock, GstStatus, ItemInvoiceScreen, Notice, UiEvent};
use crate::shell::submit::{self, CommandReply};
use crate::{AppCtx, Screen, ScreenAction};
use egui::{Id, Key, Ui};
use ledgerly_api::Command;
use ledgerly_core::format::{amount_in_words, format_inr};
use ledgerly_model::Masters;

struct FieldSpec {
    id: Id,
    field: Field,
    label: &'static str,
    width: f32,
    hint: &'static str,
}

/// A single-line field that can take the keyboard on request. Reports focus, typing and Enter as
/// events.
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
    if response.lost_focus() && ui.ctx().input(|i| i.key_pressed(Key::Enter)) {
        let back = ui.ctx().input(|i| i.modifiers.shift);
        events.push(UiEvent::Submit {
            field: spec.field,
            back,
        });
    }
}

fn labelled(
    ui: &mut Ui,
    spec: &FieldSpec,
    text: &mut String,
    want: Option<Field>,
    events: &mut Vec<UiEvent>,
) {
    ui.horizontal(|ui| {
        ui.label(spec.label);
        text_field(ui, spec, text, want, events);
    });
}

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

/// Shows the engine's figures exactly as handed in, and the total in words.
fn draw_block(ui: &mut Ui, block: &GstBlock) {
    ui.weak(format!("Rule pack {}", block.pack_version));
    for (i, line) in block.lines.iter().enumerate() {
        ui.label(format!(
            "Line {}: taxable {} at {}%",
            i.saturating_add(1),
            format_inr(line.taxable),
            line.rate_percent.normalize()
        ));
    }
    ui.label(format!("Taxable value {}", format_inr(block.taxable_total)));
    for head in &block.heads {
        ui.label(format!("{} {}", head.head, format_inr(head.amount)));
    }
    ui.label(format!("Round off {}", format_inr(block.round_off)));
    ui.strong(format!("Invoice total {}", format_inr(block.invoice_total)));
    ui.label(amount_in_words(block.invoice_total));
}

impl ItemInvoiceScreen {
    fn draw_header(&mut self, ui: &mut Ui, base: Id, labels: &[String], ev: &mut Vec<UiEvent>) {
        let want = self.focus_pending.then_some(self.focus);
        let kind = self.draft.kind;
        let spec =
            |name: &str, field: Field, label: &'static str, width: f32, hint: &'static str| {
                FieldSpec {
                    id: base.with(name),
                    field,
                    label,
                    width,
                    hint,
                }
            };
        let date = spec("date", Field::Date, "Date", 110.0, "dd-mm-yyyy");
        labelled(ui, &date, &mut self.draft.date_text, want, ev);
        if self.draft.number_required {
            let s = spec("number", Field::Number, "Voucher no.", 140.0, "Required");
            labelled(ui, &s, &mut self.draft.number_text, want, ev);
        }
        let party = kind.party_label();
        let s = spec("party", Field::Party, party, 320.0, "type to search");
        labelled(ui, &s, &mut self.draft.party_text, want, ev);
        if self.focus == Field::Party {
            draw_picker(ui, labels, self.pick_index, ev);
        }
        let s = spec(
            "place",
            Field::Place,
            "Place of supply",
            220.0,
            "code or name",
        );
        labelled(ui, &s, &mut self.draft.place_text, want, ev);
        let account = kind.account_label();
        let s = spec("account", Field::Account, account, 320.0, "type to search");
        labelled(ui, &s, &mut self.draft.account_text, want, ev);
        if self.focus == Field::Account {
            draw_picker(ui, labels, self.pick_index, ev);
        }
        let reference = kind.reference_label();
        let s = spec("ref", Field::Reference, reference, 220.0, "Optional");
        labelled(ui, &s, &mut self.draft.reference_text, want, ev);
    }

    fn draw_rows(
        &mut self,
        ui: &mut Ui,
        base: Id,
        masters: &Masters,
        labels: &[String],
        ev: &mut Vec<UiEvent>,
    ) {
        let want = self.focus_pending.then_some(self.focus);
        let (focus, pick) = (self.focus, self.pick_index);
        let summaries: Vec<String> = (0..self.draft.rows.len())
            .map(|i| self.row_summary(masters, i))
            .collect();
        for (i, row) in self.draft.rows.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                let cells: [(&str, Field, f32, &'static str); 5] = [
                    ("item", Field::Item(i), 260.0, "Item: type to search"),
                    ("qty", Field::Qty(i), 70.0, "Qty"),
                    ("rate", Field::Rate(i), 90.0, "Rate"),
                    ("disc", Field::Discount(i), 60.0, "Disc %"),
                    ("amt", Field::Amount(i), 110.0, "Amount (optional)"),
                ];
                for (name, field, width, hint) in cells {
                    let text = match field {
                        Field::Item(_) => &mut row.item_text,
                        Field::Qty(_) => &mut row.qty_text,
                        Field::Rate(_) => &mut row.rate_text,
                        Field::Discount(_) => &mut row.discount_text,
                        Field::Amount(_) => &mut row.amount_text,
                        Field::Date
                        | Field::Number
                        | Field::Party
                        | Field::Place
                        | Field::Account
                        | Field::Reference
                        | Field::Narration => continue,
                    };
                    let spec = FieldSpec {
                        id: base.with((name, i)),
                        field,
                        label: "",
                        width,
                        hint,
                    };
                    text_field(ui, &spec, text, want, ev);
                }
                ui.weak(summaries.get(i).map_or("", String::as_str));
            });
            if focus == Field::Item(i) {
                draw_picker(ui, labels, pick, ev);
            }
        }
    }

    fn draw_gst(&self, ui: &mut Ui, masters: &Masters) {
        ui.strong("Tax (computed by the engine)");
        match self.gst_status(masters) {
            GstStatus::Incomplete(why) => {
                ui.weak(format!("Complete the invoice to see tax ({why})."));
            }
            GstStatus::NotComputedYet => {
                ui.weak("Tax is not computed yet for this invoice.");
            }
            GstStatus::Unsupported(u) => {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    format!("Warning: not supported yet: {} ({})", u.what, u.reason),
                );
            }
            GstStatus::Failed(m) => {
                ui.colored_label(ui.visuals().error_fg_color, format!("Error: {m}"));
            }
            GstStatus::Computed(block) => draw_block(ui, block),
        }
    }

    fn draw_footer(&mut self, ui: &mut Ui, base: Id, masters: &Masters, ev: &mut Vec<UiEvent>) {
        let want = self.focus_pending.then_some(self.focus);
        let spec = FieldSpec {
            id: base.with("narration"),
            field: Field::Narration,
            label: "Narration",
            width: 480.0,
            hint: "Optional",
        };
        labelled(ui, &spec, &mut self.draft.narration, want, ev);
        match &self.notice {
            Some(Notice::Error(s)) => {
                ui.colored_label(ui.visuals().error_fg_color, format!("Error: {s}"));
            }
            Some(Notice::Info(s)) => {
                ui.label(format!("Note: {s}"));
            }
            None => {}
        }
        ui.horizontal(|ui| {
            if ui.button("Save (Ctrl+A)").clicked() {
                self.save(masters);
            }
            if ui.button("Discard (Esc)").clicked() {
                ev.push(UiEvent::Leave);
            }
        });
    }

    fn draw(&mut self, ui: &mut Ui, masters: &Masters) -> ScreenAction {
        let base = ui.scope_id().with("item-invoice");
        let escape_wanted = self.confirm_discard || self.searching() || !self.draft.is_blank();
        let mut labels = self.pick_labels(masters);
        let keys = read_keys(ui, !labels.is_empty(), escape_wanted, self.confirm_discard);
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
        ui.heading(self.title());
        if self.confirm_discard {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "Warning: discard this invoice? Press Y to discard, N to keep it.",
            );
        }
        let mut events = Vec::new();
        let enabled = !self.confirm_discard;
        ui.add_enabled_ui(enabled, |ui| {
            self.draw_header(ui, base, &labels, &mut events);
            ui.separator();
            self.draw_rows(ui, base, masters, &labels, &mut events);
            ui.separator();
            self.draw_gst(ui, masters);
            ui.separator();
            self.draw_footer(ui, base, masters, &mut events);
        });
        self.focus_pending = false;
        if matches!(self.apply_events(masters, events), ScreenAction::Pop) {
            action = ScreenAction::Pop;
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

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        if self.posting.is_some() {
            self.wait_for_post(ui);
            return ScreenAction::Stay;
        }
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
