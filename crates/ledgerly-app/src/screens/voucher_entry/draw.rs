//! Drawing the entry screen with the kit: the header, the lines, the footer and the create form.

use egui::{Id, Ui};
use ledgerly_core::SignedAmount;
use ledgerly_model::Masters;

use super::amount::eval_amount;
use super::display::{format_amount, format_signed, side_label};
use super::draft::{Balance, DraftRow, LedgerRef};
use super::keys::{UiEvent, read_keys};
use super::search::PickerItem;
use super::state::{Field, Notice, VoucherEntryScreen};
use crate::{Screen, ScreenAction};

/// What `draw_row` needs to know about the screen.
struct RowCtx<'a> {
    base: Id,
    want: Option<Field>,
    focused: Field,
    items: &'a [PickerItem],
    pick: usize,
    balance: Option<SignedAmount>,
}

/// Which text field to draw.
struct FieldSpec {
    id: Id,
    field: Field,
    width: f32,
    hint: &'static str,
}

fn item_label(item: &PickerItem) -> String {
    match item {
        PickerItem::Ledger(hit) => {
            let mut parts = vec![hit.name.clone()];
            if let Some(alias) = &hit.via_alias {
                parts.push(format!("(alias {alias})"));
            }
            if let Some(balance) = hit.balance {
                parts.push(format_signed(balance));
            }
            if let Some(gstin) = &hit.gstin {
                parts.push(gstin.clone());
            }
            parts.join("   ")
        }
        PickerItem::Create { name } => format!("+ Create ledger \"{name}\"   (Alt+C)"),
    }
}

/// A single-line field that can take the keyboard on request. Reports focus and Enter as events
/// and returns whether the text changed.
fn text_field(
    ui: &mut Ui,
    spec: &FieldSpec,
    text: &mut String,
    want: Option<Field>,
    events: &mut Vec<UiEvent>,
) -> bool {
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
    if response.lost_focus() && ui.ctx().input(|i| i.key_pressed(Key::Enter)) {
        let back = ui.ctx().input(|i| i.modifiers.shift);
        events.push(UiEvent::Submit {
            field: spec.field,
            back,
        });
    }
    response.changed()
}

fn draw_row(ui: &mut Ui, i: usize, row: &mut DraftRow, rc: &RowCtx<'_>, events: &mut Vec<UiEvent>) {
    ui.horizontal(|ui| {
        if ui
            .button(side_label(row.side))
            .on_hover_text("Switch Dr / Cr")
            .clicked()
        {
            events.push(UiEvent::Side(i));
        }
        let ledger_spec = FieldSpec {
            id: rc.base.with(("ledger", i)),
            field: Field::Ledger(i),
            width: 320.0,
            hint: "Ledger: type to search",
        };
        if text_field(ui, &ledger_spec, &mut row.ledger_text, rc.want, events) {
            row.ledger = None;
            events.push(UiEvent::LedgerTyped);
        }
        let amount_spec = FieldSpec {
            id: rc.base.with(("amount", i)),
            field: Field::Amount(i),
            width: 150.0,
            hint: "Amount, e.g. 1200*18%",
        };
        text_field(ui, &amount_spec, &mut row.amount_text, rc.want, events);
        if let Some(balance) = rc.balance {
            ui.weak(format!("Balance {}", format_signed(balance)));
        }
        if let Some(LedgerRef::Pending(_)) = row.ledger {
            ui.weak("new ledger, not created yet");
        }
        let plain = row
            .amount_text
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.');
        if !plain && let Ok(paise) = eval_amount(&row.amount_text) {
            ui.weak(format!("= {}", format_amount(paise)));
        }
    });
    if rc.focused == Field::Ledger(i) && !rc.items.is_empty() {
        ui.group(|ui| {
            for (j, item) in rc.items.iter().enumerate() {
                if ui
                    .selectable_label(j == rc.pick, item_label(item))
                    .clicked()
                {
                    events.push(UiEvent::Pick(j));
                }
            }
        });
    }
}

impl VoucherEntryScreen {
    pub(super) fn balance_text(&self) -> String {
        match self.draft.balance() {
            Ok(Balance::Balanced) => {
                "Lines as typed balance (the engine checks again when it posts).".to_owned()
            }
            Ok(Balance::Needs { side, amount }) => format!(
                "Lines as typed do not balance: a {} line of {} would balance them.",
                side_label(side),
                format_amount(amount)
            ),
            Err(e) => format!("Difference not available: {e}"),
        }
    }

    pub(super) fn draw_header(&mut self, ui: &mut Ui, base: Id, events: &mut Vec<UiEvent>) {
        let want = self.focus_pending.then_some(self.focus);
        let spec = FieldSpec {
            id: base.with("date"),
            field: Field::Date,
            width: 110.0,
            hint: "dd-mm-yyyy",
        };
        ui.horizontal(|ui| {
            ui.label("Date");
            text_field(ui, &spec, &mut self.draft.date_text, want, events);
            ui.weak(format!("Financial year {}", self.fy.label()));
        });
    }

    pub(super) fn draw_rows(
        &mut self,
        ui: &mut Ui,
        base: Id,
        items: &[PickerItem],
        events: &mut Vec<UiEvent>,
    ) {
        let want = self.focus_pending.then_some(self.focus);
        for (i, row) in self.draft.rows.iter_mut().enumerate() {
            let balance = match &row.ledger {
                Some(LedgerRef::Chosen(id)) => self.balances.get(id).copied(),
                Some(LedgerRef::Pending(_)) | None => None,
            };
            let rc = RowCtx {
                base,
                want,
                focused: self.focus,
                items,
                pick: self.pick_index,
                balance,
            };
            draw_row(ui, i, row, &rc, events);
        }
    }

    pub(super) fn draw_footer(
        &mut self,
        ui: &mut Ui,
        base: Id,
        masters: &Masters,
        events: &mut Vec<UiEvent>,
    ) {
        let want = self.focus_pending.then_some(self.focus);
        let spec = FieldSpec {
            id: base.with("narration"),
            field: Field::Narration,
            width: 480.0,
            hint: "Optional",
        };
        ui.horizontal(|ui| {
            ui.label("Narration");
            text_field(ui, &spec, &mut self.draft.narration, want, events);
        });
        ui.label(self.balance_text());
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
                events.push(UiEvent::Leave);
            }
        });
    }

    pub(super) fn draw_create_form(&mut self, ui: &mut Ui, masters: &Masters) {
        let Some(form) = self.create.as_mut() else {
            return;
        };
        let mut confirmed = false;
        let mut cancelled = false;
        ui.group(|ui| {
            ui.label("Create ledger");
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut form.name).desired_width(280.0));
            });
            let selected = form
                .group
                .and_then(|g| masters.groups.get(&g))
                .map_or("Choose a group", |g| g.name.as_str());
            egui::ComboBox::from_id_salt("u4-create-group")
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for (id, group) in &masters.groups {
                        ui.selectable_value(&mut form.group, Some(*id), group.name.as_str());
                    }
                });
            ui.horizontal(|ui| {
                confirmed = ui.button("Create").clicked();
                cancelled = ui.button("Cancel (Esc)").clicked();
            });
        });
        let (name, group) = (form.name.clone(), form.group);
        if confirmed {
            self.confirm_create(masters, &name, group);
        } else if cancelled {
            self.create = None;
        }
    }

    pub(super) fn draw(&mut self, ui: &mut Ui, masters: &Masters) -> ScreenAction {
        let base = ui.scope_id().with("voucher-entry");
        let escape_wanted = self.create.is_some()
            || self.confirm_discard
            || self.searching()
            || !self.draft.is_blank();
        // The type-ahead list is computed once per frame (twice when a key changed the state it
        // depends on), never once per use: it ranks every ledger (rule D1).
        let mut items = self.picker_items(masters);
        let keys = read_keys(ui, !items.is_empty(), escape_wanted, self.confirm_discard);
        let mut action = ScreenAction::Stay;
        for cmd in &keys {
            if matches!(
                self.handle_key(masters, cmd, items.len()),
                ScreenAction::Pop
            ) {
                action = ScreenAction::Pop;
            }
        }
        if !keys.is_empty() {
            items = self.picker_items(masters);
        }
        ui.heading(self.title());
        if self.confirm_discard {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "Warning: discard this voucher? Press Y to discard, N to keep it.",
            );
        }
        let mut events = Vec::new();
        let enabled = !self.confirm_discard && self.create.is_none();
        ui.add_enabled_ui(enabled, |ui| {
            self.draw_header(ui, base, &mut events);
            ui.separator();
            self.draw_rows(ui, base, &items, &mut events);
            ui.separator();
            self.draw_footer(ui, base, masters, &mut events);
        });
        self.draw_create_form(ui, masters);
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
