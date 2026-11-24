//! Drawing the entry screen to the voucher sheet of the mock (`design/mock-v2/index.html`,
//! `payScreen`): the page head, then one sheet with the head (date and account), the particulars
//! table and the foot (narration and notices). A single-entry voucher (payment or receipt) draws
//! the account once, then the particulars lines; the other kinds draw every line with its own
//! Dr / Cr switch. The type-ahead list floats under the field it belongs to.

use egui::{Id, Key, Rect, Response, Ui};
use ledgerly_model::Masters;

use super::amount::eval_amount;
use super::create::{FormAnswer, create_form_ui};
use super::display::{format_amount, format_signed, side_label};
use super::draft::{Balance, DraftRow, LedgerRef};
use super::keys::{UiEvent, read_keys};
use super::kind::EntryKind;
use super::search::PickerItem;
use super::state::{Field, Notice, VoucherEntryScreen};
use crate::keymap::Action;
use crate::screens::item_invoice::extras::BalanceShown;
use crate::screens::voucher_sheet::{self as sheet, Col, InputSpec, Look, Total};
use crate::theme::TEXT_BODY;
use crate::ui::button::{ButtonKind, button};
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::page_head_numbered;
use crate::ui::prov::ProvKind;
use crate::ui::{para_text, tokens};
use crate::{Screen, ScreenAction};

/// The single-entry particulars table (index.html:1395): # Particulars Group Amount.
const SINGLE_COLS: [Col; 4] = [
    Col::text("#", 36.0),
    Col::rest("Particulars"),
    Col::text("Group", 200.0),
    Col::num("Amount", 170.0),
];

/// The double-entry table: each line has its own Dr / Cr switch and shows the ledger's balance.
const DOUBLE_COLS: [Col; 4] = [
    Col::text("Dr / Cr", 80.0),
    Col::rest("Ledger"),
    Col::num("Balance", 170.0),
    Col::num("Amount", 220.0),
];

/// Room kept for the "= amount" aid beside a typed expression.
const AID_WIDTH: f32 = 96.0;

/// Where parts of the screen were drawn in the last frame, for layout tests.
#[derive(Debug, Clone, Default)]
pub(super) struct Shown {
    /// The head's fields and the narration by caption.
    pub(super) fields: Vec<(&'static str, Rect)>,
    /// The line table's headings in drawing order.
    pub(super) heads: Vec<(&'static str, Rect)>,
    /// The open type-ahead list.
    pub(super) picker: Option<Rect>,
}

/// Which text field to draw.
struct FieldSpec {
    id: Id,
    field: Field,
    /// The caption the field is drawn under; its accessible name.
    name: &'static str,
    hint: &'static str,
}

/// What every field of a frame reports to: the field the keyboard is wanted on, the field it is on,
/// the events, and where the type-ahead list for the focused field hangs.
struct Cx<'a> {
    want: Option<Field>,
    focus: Field,
    events: &'a mut Vec<UiEvent>,
    anchor: &'a mut Option<Rect>,
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

/// A single-line field that can take the keyboard on request. Reports focus and Enter as events,
/// notes where it sits so the list can hang under it, and returns the response.
fn text_field(
    ui: &mut Ui,
    cx: &mut Cx<'_>,
    spec: &FieldSpec,
    text: &mut String,
    look: Look,
    numeric: bool,
    width: Option<f32>,
) -> Response {
    let response = sheet::input(
        ui,
        text,
        &InputSpec {
            id: spec.id,
            hint: spec.hint,
            look,
            numeric,
            invalid: false,
            width,
        },
    );
    crate::ui::a11y::name(ui, &response, egui::accesskit::Role::TextInput, spec.name);
    if cx.want == Some(spec.field) {
        response.request_focus();
    }
    if response.has_focus() {
        cx.events.push(UiEvent::Focus(spec.field));
    }
    if cx.focus == spec.field {
        *cx.anchor = Some(response.rect);
    }
    if response.lost_focus() && ui.ctx().input(|i| i.key_pressed(Key::Enter)) {
        let back = ui.ctx().input(|i| i.modifiers.shift);
        cx.events.push(UiEvent::Submit {
            field: spec.field,
            back,
        });
    }
    response
}

/// The ledger field of line `i`: typing there forgets the ledger that was picked.
fn ledger_field(
    ui: &mut Ui,
    cx: &mut Cx<'_>,
    base: Id,
    i: usize,
    row: &mut DraftRow,
    hint: &'static str,
    look: Look,
) {
    let spec = FieldSpec {
        id: base.with(("ledger", i)),
        field: Field::Ledger(i),
        name: "Ledger",
        hint,
    };
    let response = text_field(ui, cx, &spec, &mut row.ledger_text, look, false, None);
    if response.changed() {
        row.ledger = None;
        cx.events.push(UiEvent::LedgerTyped);
    }
}

/// The amount field of line `i`, with the value of a typed expression (`1200*18%`) beside it.
fn amount_field(ui: &mut Ui, cx: &mut Cx<'_>, base: Id, i: usize, row: &mut DraftRow) {
    let spec = FieldSpec {
        id: base.with(("amount", i)),
        field: Field::Amount(i),
        name: "Amount",
        hint: "Amount, e.g. 1200*18%",
    };
    let plain = row
        .amount_text
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.');
    let entry = if plain {
        None
    } else {
        eval_amount(&row.amount_text).ok()
    };
    let width = entry.map(|_| (ui.available_width() - AID_WIDTH).max(80.0));
    text_field(ui, cx, &spec, &mut row.amount_text, Look::Cell, true, width);
    if let Some(paise) = entry {
        cell_text(ui, &format!("= {}", format_amount(paise)));
    }
}

/// A cell of plain text in the quiet ink.
fn cell_text(ui: &mut Ui, text: &str) {
    let t = tokens(ui);
    ui.label(para_text(text, TEXT_BODY, t.text_2));
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

/// The titles of the open list.
const LIST_CAPTION: &str = "Ledgers";

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

    /// The head: the date, and for a single-entry voucher the account across two columns.
    fn draw_head(&mut self, ui: &mut Ui, base: Id, cx: &mut Cx<'_>) {
        let single = self.draft.is_single_entry();
        let account_balance = match self.draft.rows.first().and_then(|r| r.ledger.as_ref()) {
            Some(LedgerRef::Chosen(id)) => self.balances.get(id).copied(),
            Some(LedgerRef::Pending(_)) | None => None,
        };
        let date_spec = FieldSpec {
            id: base.with("date"),
            field: Field::Date,
            name: "Date",
            hint: "dd-mm-yyyy",
        };
        let mut fields = Vec::new();
        sheet::head(ui, |ui, total| {
            sheet::head_row(ui, |ui| {
                let shown = sheet::fld(
                    ui,
                    total,
                    1,
                    |ui| sheet::caption(ui, "Date", Some("F2")),
                    |ui| {
                        // Bare like the mock's read-only date until the keyboard or pointer is on
                        // it; it stays a field so Tab and a screen reader still reach it.
                        // Not being edited, it reads as the mock writes it (`21 Nov 2026`).
                        let mut long = super::parse_entry_date(&self.draft.date_text, self.fy)
                            .ok()
                            .map(crate::ui::format::date);
                        match long.as_mut() {
                            Some(text) if cx.focus != Field::Date => {
                                text_field(ui, cx, &date_spec, text, Look::Cell, false, None);
                            }
                            Some(_) | None => {
                                text_field(
                                    ui,
                                    cx,
                                    &date_spec,
                                    &mut self.draft.date_text,
                                    Look::Cell,
                                    false,
                                    None,
                                );
                            }
                        }
                    },
                );
                fields.push(("Date", shown.response.rect));
                if single && let Some(account) = self.draft.rows.first_mut() {
                    let shown = sheet::fld(
                        ui,
                        total,
                        2,
                        |ui| sheet::caption(ui, "Account", None),
                        |ui| {
                            ledger_field(
                                ui,
                                cx,
                                base,
                                0,
                                account,
                                "Cash or bank ledger",
                                Look::Boxed,
                            );
                            // The line is always there, so the fields under it never move when a
                            // balance appears.
                            let text = account_balance.map_or_else(String::new, |b| {
                                format!("balance now {} (engine)", format_signed(b))
                            });
                            sheet::hint(ui, if text.is_empty() { " " } else { &text });
                        },
                    );
                    fields.push(("Account", shown.response.rect));
                }
            });
        });
        self.shown.fields = fields;
    }

    /// The lines table: the particulars of a single-entry voucher, or every line of a double-entry
    /// one.
    fn draw_table(&mut self, ui: &mut Ui, base: Id, masters: &Masters, cx: &mut Cx<'_>) {
        let single = self.draft.is_single_entry();
        let skip = usize::from(single);
        let focused_row = match self.focus {
            Field::Ledger(i) | Field::Amount(i) => Some(i),
            Field::Date | Field::Narration => None,
        };
        let balances = &self.balances;
        let rows = &mut self.draft.rows;
        let count = rows.len().saturating_sub(skip);
        let cols: &[Col] = if single { &SINGLE_COLS } else { &DOUBLE_COLS };
        let rects = sheet::lines(
            ui,
            cols,
            count,
            |r| focused_row == Some(r.saturating_add(skip)),
            |ui, r, c| {
                let i = r.saturating_add(skip);
                let Some(row) = rows.get_mut(i) else {
                    return;
                };
                if single {
                    match c {
                        0 => cell_text(ui, &i.to_string()),
                        1 => {
                            ledger_field(
                                ui,
                                cx,
                                base,
                                i,
                                row,
                                "Ledger: type to search",
                                Look::Cell,
                            );
                        }
                        2 => cell_text(ui, group_name(masters, row).as_deref().unwrap_or("")),
                        3 => amount_field(ui, cx, base, i, row),
                        _ => {}
                    }
                } else {
                    match c {
                        0 => {
                            if button(ui, side_label(row.side), None, ButtonKind::Secondary)
                                .clicked()
                            {
                                cx.events.push(UiEvent::Side(i));
                            }
                        }
                        1 => {
                            ledger_field(
                                ui,
                                cx,
                                base,
                                i,
                                row,
                                "Ledger: type to search",
                                Look::Cell,
                            );
                        }
                        2 => {
                            let text = match &row.ledger {
                                Some(LedgerRef::Chosen(id)) => {
                                    balances.get(id).copied().map(format_signed)
                                }
                                Some(LedgerRef::Pending(_)) => {
                                    Some("new ledger, not created yet".to_owned())
                                }
                                None => None,
                            };
                            cell_text(ui, text.as_deref().unwrap_or(""));
                        }
                        3 => amount_field(ui, cx, base, i, row),
                        _ => {}
                    }
                }
            },
        );
        self.shown.heads = cols.iter().map(|c| c.label).zip(rects).collect();
    }

    /// The foot: the narration, the entry aid and the notice of the last action.
    fn draw_foot(&mut self, ui: &mut Ui, base: Id, masters: &Masters, cx: &mut Cx<'_>) {
        let single = self.draft.is_single_entry();
        let aid = if single {
            "The account takes the total of the particulars. The engine checks the voucher when it posts."
                .to_owned()
        } else {
            self.balance_text()
        };
        let spec = FieldSpec {
            id: base.with("narration"),
            field: Field::Narration,
            name: "Narration",
            hint: "Optional",
        };
        let box_rows = self.balance_box(masters);
        let mut narration = None;
        sheet::foot(ui, |left, right| {
            sheet::fld(
                left,
                left.available_width(),
                4,
                |ui| sheet::caption(ui, "Narration", None),
                |ui| {
                    narration = Some(
                        text_field(
                            ui,
                            cx,
                            &spec,
                            &mut self.draft.narration,
                            Look::Boxed,
                            false,
                            None,
                        )
                        .rect,
                    );
                },
            );
            sheet::hint(left, &aid);
            self.draw_notice(left);
            if let Some((rows, after)) = &box_rows {
                sheet::taxbox(
                    right,
                    rows,
                    Some(&Total {
                        label: "Balance after",
                        value: after,
                        tag: Some(ProvKind::Engine),
                    }),
                );
            }
        });
        if let Some(rect) = narration {
            self.shown.fields.push(("Narration", rect));
        }
    }

    /// The box at the right of a single-entry voucher's foot (index.html:1409): the total paid out
    /// or received, the account's balance now and, as the total row, its balance after saving, the
    /// engine's own. `None` until the engine has answered for the voucher as it stands.
    fn balance_box(&self, masters: &Masters) -> Option<(Vec<(String, String)>, String)> {
        let (account, voucher) = self.balance_request(masters)?;
        let BalanceShown::Change(change) = self.extras.balance_for(account, &voucher)? else {
            return None;
        };
        let total = self.draft.particulars_total().ok().flatten()?;
        let label = match self.draft.kind {
            EntryKind::Payment => "Paid out",
            EntryKind::Receipt => "Received",
            EntryKind::Contra
            | EntryKind::Journal
            | EntryKind::Memorandum
            | EntryKind::ReversingJournal => return None,
        };
        Some((
            vec![
                (label.to_owned(), format_amount(total)),
                ("Balance now".to_owned(), format_signed(change.now)),
            ],
            format_signed(change.after),
        ))
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

    #[expect(
        clippy::too_many_lines,
        reason = "one sheet drawn top to bottom; splitting would only pass the same borrows around"
    )]
    pub(super) fn draw(&mut self, ui: &mut Ui, masters: &Masters) -> ScreenAction {
        self.shown = Shown::default();
        let base = ui.scope_id().with("voucher-entry");
        // While a save is out the sheet stays on screen but takes no keys.
        let busy = self.posting.is_some();
        let escape_wanted = self.create.is_some()
            || self.confirm_discard
            || self.searching()
            || !self.draft.is_blank();
        // The type-ahead list is computed once per frame (twice when a key changed the state it
        // depends on), never once per use: it ranks every ledger (rule D1).
        let mut items = self.picker_items(masters);
        let keys = if busy {
            Vec::new()
        } else {
            read_keys(
                ui,
                &self.keymap,
                !items.is_empty(),
                escape_wanted,
                self.confirm_discard,
            )
        };
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
        let number = self.extras.number.clone();
        let subtitle = self.subtitle();
        let save_key = self.keymap.shortcut_label(Action::Save);
        if self.confirm_discard {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "Warning: discard this voucher? Press Y to discard, N to keep it.",
            );
        }
        let mut events = Vec::new();
        let mut anchor = None;
        let mut save_clicked = false;
        let enabled = !self.confirm_discard && self.create.is_none() && !busy;
        let (focus, pick) = (self.focus, self.pick_index);
        let want = self.focus_pending.then_some(self.focus);
        ui.add_enabled_ui(enabled, |ui| {
            page_head_numbered(ui, &title, number.as_deref(), subtitle.as_deref(), |ui| {
                save_clicked =
                    button(ui, "Save", save_key.as_deref(), ButtonKind::Primary).clicked();
            });
            let mut cx = Cx {
                want,
                focus,
                events: &mut events,
                anchor: &mut anchor,
            };
            sheet::sheet(ui, |ui| {
                self.draw_head(ui, base, &mut cx);
                self.draw_table(ui, base, masters, &mut cx);
                self.draw_foot(ui, base, masters, &mut cx);
            });
        });
        if let Some(anchor) = anchor
            && !items.is_empty()
        {
            let labels: Vec<String> = items.iter().map(item_label).collect();
            let (rect, clicked) =
                sheet::type_ahead(ui, base.with("list"), anchor, LIST_CAPTION, &labels, pick);
            self.shown.picker = Some(rect);
            if let Some(j) = clicked {
                events.push(UiEvent::Pick(j));
            }
        }
        self.draw_create_form(ui, masters);
        self.focus_pending = false;
        if save_clicked {
            self.save(masters);
        }
        self.apply_events(masters, events);
        if !keys.is_empty() || self.focus_pending {
            ui.ctx().request_repaint();
        }
        action
    }
}
