//! Drawing the entry screen with the kit: the page head, the voucher card, the lines and the
//! notes. A single-entry voucher (payment or receipt) draws the account once, then the particulars
//! lines; the other kinds draw every line with its own Dr / Cr switch.

use egui::{Id, Key, Ui};
use ledgerly_core::SignedAmount;
use ledgerly_model::Masters;

use super::amount::eval_amount;
use super::create::{FormAnswer, create_form_ui};
use super::date::parse_entry_date;
use super::display::{format_amount, format_signed, side_label};
use super::draft::{Balance, DraftRow, LedgerRef};
use super::keys::{UiEvent, read_keys};
use super::search::PickerItem;
use super::state::{Field, Notice, VoucherEntryScreen};
use crate::keymap::Action;
use crate::ui::button::{ButtonKind, button};
use crate::ui::card::card;
use crate::ui::format;
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::page_head;
use crate::{Screen, ScreenAction};

/// What the line drawing needs to know about the screen.
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

/// The type-ahead list under the ledger field of line `i`, when that field has the keyboard.
fn draw_picker(ui: &mut Ui, i: usize, rc: &RowCtx<'_>, events: &mut Vec<UiEvent>) {
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

/// The name of the group the line's ledger is under, from the masters. `None` while no ledger is
/// picked, so the column stays blank rather than guessing.
fn group_name(masters: &Masters, row: &DraftRow) -> Option<String> {
    match &row.ledger {
        Some(LedgerRef::Chosen(id)) => masters
            .ledgers
            .get(id)
            .and_then(|ledger| masters.groups.get(&ledger.group))
            .map(|group| group.name.clone()),
        Some(LedgerRef::Pending(_)) | None => None,
    }
}

/// One line of a double-entry voucher: its Dr / Cr switch, ledger, amount and the entry aids.
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
        let entry = if plain {
            None
        } else {
            eval_amount(&row.amount_text).ok()
        };
        if let Some(paise) = entry {
            ui.weak(format!("= {}", format_amount(paise)));
        }
    });
    draw_picker(ui, i, rc, events);
}

/// The account of a single-entry voucher: picked once, with no amount of its own.
fn draw_account(ui: &mut Ui, row: &mut DraftRow, rc: &RowCtx<'_>, events: &mut Vec<UiEvent>) {
    let spec = FieldSpec {
        id: rc.base.with(("ledger", 0)),
        field: Field::Ledger(0),
        width: 320.0,
        hint: "Cash or bank ledger",
    };
    let changed = ui
        .horizontal(|ui| text_field(ui, &spec, &mut row.ledger_text, rc.want, events))
        .inner;
    if changed {
        row.ledger = None;
        events.push(UiEvent::LedgerTyped);
    }
    draw_picker(ui, 0, rc, events);
}

/// One particulars line of a single-entry voucher: its number, the ledger, the ledger's group and
/// the amount. The line has no Dr / Cr switch: its side follows the account.
fn draw_single_line(
    ui: &mut Ui,
    i: usize,
    row: &mut DraftRow,
    group: Option<&str>,
    rc: &RowCtx<'_>,
    events: &mut Vec<UiEvent>,
) {
    ui.horizontal(|ui| {
        ui.weak(format!("{i}"));
        let ledger_spec = FieldSpec {
            id: rc.base.with(("ledger", i)),
            field: Field::Ledger(i),
            width: 300.0,
            hint: "Ledger: type to search",
        };
        if text_field(ui, &ledger_spec, &mut row.ledger_text, rc.want, events) {
            row.ledger = None;
            events.push(UiEvent::LedgerTyped);
        }
        ui.weak(group.unwrap_or(""));
        let amount_spec = FieldSpec {
            id: rc.base.with(("amount", i)),
            field: Field::Amount(i),
            width: 150.0,
            hint: "Amount, e.g. 1200*18%",
        };
        text_field(ui, &amount_spec, &mut row.amount_text, rc.want, events);
        if let Some(LedgerRef::Pending(_)) = row.ledger {
            ui.weak("new ledger, not created yet");
        }
        let plain = row
            .amount_text
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.');
        let entry = if plain {
            None
        } else {
            eval_amount(&row.amount_text).ok()
        };
        if let Some(paise) = entry {
            ui.weak(format!("= {}", format_amount(paise)));
        }
    });
    draw_picker(ui, i, rc, events);
}

impl VoucherEntryScreen {
    /// The entry aid under the lines: whether the lines as typed balance.
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

    /// The date and financial year of a double-entry voucher.
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

    /// Every line of a double-entry voucher, each with its own Dr / Cr.
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

    /// The narration and the entry aid of a double-entry voucher.
    pub(super) fn draw_footer(&mut self, ui: &mut Ui, base: Id, events: &mut Vec<UiEvent>) {
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
    }

    /// The single-entry voucher: the date and the account, the particulars, then the narration.
    /// The account's amount is the particulars total, which the screen never shows as its own
    /// figure: there is no engine total yet, so only the entry aid is said in words.
    pub(super) fn draw_single(
        &mut self,
        ui: &mut Ui,
        base: Id,
        masters: &Masters,
        items: &[PickerItem],
        events: &mut Vec<UiEvent>,
    ) {
        let want = self.focus_pending.then_some(self.focus);
        let focused = self.focus;
        let pick = self.pick_index;
        let typed_date = parse_entry_date(&self.draft.date_text, self.fy).ok();
        let account_balance = match self.draft.rows.first().and_then(|r| r.ledger.as_ref()) {
            Some(LedgerRef::Chosen(id)) => self.balances.get(id).copied(),
            Some(LedgerRef::Pending(_)) | None => None,
        };
        let date_spec = FieldSpec {
            id: base.with("date"),
            field: Field::Date,
            width: 110.0,
            hint: "dd-mm-yyyy",
        };
        let narration_spec = FieldSpec {
            id: base.with("narration"),
            field: Field::Narration,
            width: 480.0,
            hint: "Optional",
        };
        let date_text = &mut self.draft.date_text;
        let narration = &mut self.draft.narration;
        let rows = &mut self.draft.rows;
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.weak("Date");
                    text_field(ui, &date_spec, date_text, want, events);
                    if let Some(date) = typed_date {
                        ui.weak(format::date(date));
                    }
                });
                ui.add_space(24.0);
                ui.vertical(|ui| {
                    ui.weak("Account");
                    if let Some((account, lines)) = rows.split_first_mut() {
                        let rc = RowCtx {
                            base,
                            want,
                            focused,
                            items,
                            pick,
                            balance: account_balance,
                        };
                        draw_account(ui, account, &rc, events);
                        if let Some(balance) = account_balance {
                            ui.weak(format!("Balance now {}", format_signed(balance)));
                        }
                        ui.separator();
                        ui.weak("#   Particulars");
                        for (offset, row) in lines.iter_mut().enumerate() {
                            let i = offset.saturating_add(1);
                            let group = group_name(masters, row);
                            let rc = RowCtx {
                                base,
                                want,
                                focused,
                                items,
                                pick,
                                balance: None,
                            };
                            draw_single_line(ui, i, row, group.as_deref(), &rc, events);
                        }
                    }
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Narration");
                text_field(ui, &narration_spec, narration, want, events);
            });
            ui.weak(
                "The account takes the total of the particulars. The engine checks the voucher when it posts.",
            );
        });
    }

    /// The notice of the last action, as a kit note: an error or information.
    pub(super) fn draw_notice(&self, ui: &mut Ui) {
        match &self.notice {
            Some(Notice::Error(s)) => {
                note(ui, NoteKind::Bad, s);
            }
            Some(Notice::Info(s)) => {
                note(ui, NoteKind::Info, s);
            }
            None => {}
        }
    }

    pub(super) fn draw_create_form(&mut self, ui: &mut Ui, masters: &Masters) {
        let Some(form) = self.create.as_mut() else {
            return;
        };
        match create_form_ui(ui, form, masters, "u4-create-group") {
            FormAnswer::Open => {}
            FormAnswer::Confirm => {
                let (name, group) = (form.name.clone(), form.group);
                self.confirm_create(masters, &name, group);
            }
            FormAnswer::Cancel => self.create = None,
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
        let keys = read_keys(
            ui,
            &self.keymap,
            !items.is_empty(),
            escape_wanted,
            self.confirm_discard,
        );
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
        let title = self.page_title();
        let subtitle = self.subtitle();
        let save_key = self.keymap.shortcut_label(Action::Save);
        let back_key = self.keymap.shortcut_label(Action::Back);
        if self.confirm_discard {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "Warning: discard this voucher? Press Y to discard, N to keep it.",
            );
        }
        let mut events = Vec::new();
        let mut save_clicked = false;
        let mut discard_clicked = false;
        let enabled = !self.confirm_discard && self.create.is_none();
        ui.add_enabled_ui(enabled, |ui| {
            page_head(ui, &title, subtitle.as_deref(), |ui| {
                save_clicked =
                    button(ui, "Save", save_key.as_deref(), ButtonKind::Primary).clicked();
                discard_clicked =
                    button(ui, "Discard", back_key.as_deref(), ButtonKind::Secondary).clicked();
            });
            if self.draft.is_single_entry() {
                self.draw_single(ui, base, masters, &items, &mut events);
            } else {
                card(ui, |ui| {
                    self.draw_header(ui, base, &mut events);
                    ui.separator();
                    self.draw_rows(ui, base, &items, &mut events);
                    ui.separator();
                    self.draw_footer(ui, base, &mut events);
                });
            }
        });
        self.draw_notice(ui);
        self.draw_create_form(ui, masters);
        self.focus_pending = false;
        if save_clicked {
            self.save(masters);
        }
        if discard_clicked {
            events.push(UiEvent::Leave);
        }
        if matches!(self.apply_events(masters, events), ScreenAction::Pop) {
            action = ScreenAction::Pop;
        }
        if !keys.is_empty() || self.focus_pending {
            ui.ctx().request_repaint();
        }
        action
    }
}
