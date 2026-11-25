//! Drawing the entry screen to the voucher sheet of the mock (`design/mock-v2/index.html`,
//! `payScreen`): the page head, then one sheet with the head (date and account), the particulars
//! table and the foot (narration and notices). A single-entry voucher (payment or receipt) draws
//! the account once, then the particulars lines; the other kinds draw every line with its own
//! Dr / Cr switch. The type-ahead list floats under the field it belongs to.

use egui::{Id, Key, Rect, Response, Ui};
use ledgerly_core::SignedAmount;
use ledgerly_model::Masters;

use super::amount::eval_amount;
use super::create::{FormAnswer, create_form_ui};
use super::display::{format_amount, format_balance, format_signed, side_label};
use super::draft::{Balance, DraftRow, LedgerRef, MODES, mode_label, parse_mode};
use super::keys::{UiEvent, read_keys};
use super::kind::EntryKind;
use super::search::PickerItem;
use super::state::{Field, Notice, VoucherEntryScreen};
use crate::keymap::Action;
use crate::screens::item_invoice::extras::BalanceShown;
use crate::screens::voucher_sheet::{self as sheet, Col, InputSpec, Look, Total};
use crate::theme::TEXT_BODY;
use crate::ui::button::{ButtonKind, button_icon};
use crate::ui::icon::Icon;
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

/// The double-entry table (index.html:2058): each line has its own Dr / Cr, which a click on it
/// switches, and the group of its ledger.
const DOUBLE_COLS: [Col; 5] = [
    Col::text("#", 36.0),
    Col::rest("Ledger"),
    Col::text("Dr / Cr", 70.0),
    Col::text("Group", 200.0),
    Col::num("Amount", 170.0),
];

/// The contra's table: its second heading reads "Account" (index.html:2074).
const CONTRA_COLS: [Col; 5] = [
    Col::text("#", 36.0),
    Col::rest("Account"),
    Col::text("Dr / Cr", 70.0),
    Col::text("Group", 200.0),
    Col::num("Amount", 170.0),
];

/// What the empty line at the end of the list says (index.html:2060, `Next ledger, or Enter on
/// empty to finish`).
const NEXT_LEDGER_HINT: &str = "Next ledger, or Enter on empty to finish";

/// Room kept for the "= amount" aid beside a typed expression.
const AID_WIDTH: f32 = 96.0;

/// Room kept beside the Mode box for the arrow that opens the list of instruments.
const MODE_ARROW: f32 = 52.0;

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
pub(super) struct FieldSpec {
    pub(super) id: Id,
    pub(super) field: Field,
    /// The caption the field is drawn under; its accessible name.
    pub(super) name: &'static str,
    pub(super) hint: &'static str,
}

/// What every field of a frame reports to: the field the keyboard is wanted on, the field it is on,
/// the events, and where the type-ahead list for the focused field hangs.
pub(super) struct Cx<'a> {
    pub(super) want: Option<Field>,
    pub(super) focus: Field,
    pub(super) events: &'a mut Vec<UiEvent>,
    pub(super) anchor: &'a mut Option<Rect>,
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
pub(super) fn text_field(
    ui: &mut Ui,
    cx: &mut Cx<'_>,
    spec: &FieldSpec,
    text: &mut String,
    look: Look,
    numeric: bool,
    width: Option<f32>,
) -> Response {
    text_field_ink(ui, cx, spec, text, look, numeric, width, false)
}

/// [`text_field`] with the text drawn in the error colour when `bad_ink`.
#[expect(
    clippy::too_many_arguments,
    reason = "text_field's own arguments and the ink"
)]
pub(super) fn text_field_ink(
    ui: &mut Ui,
    cx: &mut Cx<'_>,
    spec: &FieldSpec,
    text: &mut String,
    look: Look,
    numeric: bool,
    width: Option<f32>,
    bad_ink: bool,
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
            bad_ink,
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
fn amount_field(ui: &mut Ui, cx: &mut Cx<'_>, base: Id, i: usize, row: &mut DraftRow, bad: bool) {
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
    // Once the keyboard has left the line, a plain amount reads as the mock writes it
    // (`6,500.00`): the parsed value, formatted. While it is edited, or when it does not parse,
    // the typed text stays. The draft keeps what was typed.
    let committed = if plain && cx.focus != spec.field && !row.amount_text.trim().is_empty() {
        eval_amount(&row.amount_text).ok().map(format_amount)
    } else {
        None
    };
    if let Some(mut shown) = committed {
        text_field_ink(ui, cx, &spec, &mut shown, Look::Cell, true, width, bad);
    } else {
        text_field_ink(
            ui,
            cx,
            &spec,
            &mut row.amount_text,
            Look::Cell,
            true,
            width,
            bad,
        );
    }
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

/// An account number as the mock writes it (`XXXX 4471`): only the last four digits are shown.
fn masked_account(number: &str) -> String {
    let digits: Vec<char> = number.chars().filter(|c| !c.is_whitespace()).collect();
    if digits.is_empty() {
        return String::new();
    }
    let tail: String = digits.iter().skip(digits.len().saturating_sub(4)).collect();
    format!("XXXX {tail}")
}

/// The box at the right of the foot: rows, then the total row.
struct FootBox {
    rows: Vec<(String, String)>,
    label: String,
    value: String,
    bad: bool,
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

    /// The line under the Account field (index.html:1394): the bank's masked account number, then
    /// "balance now ... (engine)": the host's balance for the ledger, else the engine's answer for
    /// the voucher as it stands. `None` while neither is known, so no figure is ever guessed.
    fn account_hint(&self, masters: &Masters, held: Option<SignedAmount>) -> Option<String> {
        let id = match self.draft.rows.first()?.ledger.as_ref()? {
            LedgerRef::Chosen(id) => *id,
            LedgerRef::Pending(_) => return None,
        };
        let now = held.or_else(|| {
            let (account, voucher) = self.balance_request(masters)?;
            match self.extras.balance_for(account, &voucher)? {
                BalanceShown::Change(change) => Some(change.now),
                BalanceShown::Missing(_) => None,
            }
        })?;
        let number = masters
            .ledgers
            .get(&id)
            .and_then(|l| l.bank.as_ref())
            .map(|b| masked_account(&b.account_number))
            .filter(|n| !n.is_empty());
        let balance = format!("balance now \u{20b9}{} (engine)", format_signed(now));
        Some(match number {
            Some(n) => format!("{n} \u{b7} {balance}"),
            None => balance,
        })
    }

    /// The head: the date, and for a single-entry voucher the account across two columns.
    #[expect(
        clippy::too_many_lines,
        reason = "one head row drawn left to right; splitting would only pass the same borrows around"
    )]
    fn draw_head(&mut self, ui: &mut Ui, base: Id, masters: &Masters, cx: &mut Cx<'_>) {
        let single = self.draft.is_single_entry();
        let account_balance = match self.draft.rows.first().and_then(|r| r.ledger.as_ref()) {
            Some(LedgerRef::Chosen(id)) => self.balances.get(id).copied(),
            Some(LedgerRef::Pending(_)) | None => None,
        };
        let account_hint = self.account_hint(masters, account_balance);
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
                            let text = account_hint.as_deref().unwrap_or(" ");
                            sheet::hint(ui, text);
                        },
                    );
                    fields.push(("Account", shown.response.rect));
                    let mode_spec = FieldSpec {
                        id: base.with("mode"),
                        field: Field::Mode,
                        name: "Mode",
                        hint: "NEFT, UPI, cheque",
                    };
                    let shown = sheet::fld(
                        ui,
                        total,
                        1,
                        |ui| sheet::caption(ui, "Mode", None),
                        |ui| {
                            // The mock writes the mode as plain text (index.html:1405, "NEFT /
                            // UPI"); it stays a field with a list. Until the keyboard is on it, a
                            // chosen mode reads bare, as the date does; with the keyboard on it,
                            // or while none is chosen, it is the box and the arrow lists them all.
                            let plain = cx.focus != Field::Mode
                                && parse_mode(&self.draft.mode_text).is_some();
                            if plain {
                                text_field(
                                    ui,
                                    cx,
                                    &mode_spec,
                                    &mut self.draft.mode_text,
                                    Look::Cell,
                                    false,
                                    None,
                                );
                                return;
                            }
                            ui.horizontal(|ui| {
                                let room = (ui.available_width() - MODE_ARROW).max(60.0);
                                text_field(
                                    ui,
                                    cx,
                                    &mode_spec,
                                    &mut self.draft.mode_text,
                                    Look::Boxed,
                                    false,
                                    Some(room),
                                );
                                egui::ComboBox::from_id_salt(base.with("mode-list"))
                                    .selected_text("")
                                    .width(24.0)
                                    .show_ui(ui, |ui| {
                                        for kind in MODES {
                                            let label = mode_label(kind);
                                            let on =
                                                parse_mode(&self.draft.mode_text) == Some(kind);
                                            if ui.selectable_label(on, label).clicked() {
                                                self.choose_mode(kind);
                                            }
                                        }
                                    });
                            });
                        },
                    );
                    fields.push(("Mode", shown.response.rect));
                }
            });
        });
        self.shown.fields = fields;
    }

    /// The lines table: the particulars of a single-entry voucher, or every line of a double-entry
    /// one.
    fn draw_table(&mut self, ui: &mut Ui, base: Id, masters: &Masters, cx: &mut Cx<'_>) {
        let single = self.draft.is_single_entry();
        let journal_like = match self.draft.kind {
            EntryKind::Journal | EntryKind::Memorandum | EntryKind::ReversingJournal => true,
            EntryKind::Contra | EntryKind::Payment | EntryKind::Receipt => false,
        };
        let skip = usize::from(single);
        let focused_row = match self.focus {
            Field::Ledger(i) | Field::Amount(i) => Some(i),
            Field::Bill(_) => self.panel.map(|p| p.row),
            Field::Date | Field::Mode | Field::Narration => None,
        };
        // The side that is short while the lines do not balance: its amounts are drawn in the
        // error colour (the mock reddens the credit amount of its unbalanced journal,
        // index.html:2057). The side comes from the draft's own balance check.
        let short = match self.draft.balance() {
            Ok(Balance::Needs { side, .. }) if journal_like => Some(side),
            Ok(Balance::Needs { .. } | Balance::Balanced) | Err(_) => None,
        };
        let refused = self.contra_refused();
        let rows = &mut self.draft.rows;
        let count = rows.len().saturating_sub(skip);
        // The list always ends with the empty line that says how to finish, as the mock draws it:
        // a drawn hint when the last line is filled, or the last line's own field when it is empty.
        let hint_row = count > 0 && rows.last().is_some_and(|r| !r.is_blank());
        let cols: &[Col] = if single {
            &SINGLE_COLS
        } else if self.draft.kind == EntryKind::Contra {
            &CONTRA_COLS
        } else {
            &DOUBLE_COLS
        };
        let rects = sheet::lines(
            ui,
            cols,
            count.saturating_add(usize::from(hint_row)),
            |r| focused_row == Some(r.saturating_add(skip)),
            |ui, r, c| {
                let i = r.saturating_add(skip);
                let Some(row) = rows.get_mut(i) else {
                    if c == 0 {
                        cell_text(ui, &r.saturating_add(1).to_string());
                    } else if c == 1 {
                        let t = tokens(ui);
                        ui.label(para_text(NEXT_LEDGER_HINT, TEXT_BODY, t.text_3));
                    }
                    return;
                };
                let ledger_hint = line_hint(refused, r);
                if single {
                    match c {
                        0 => cell_text(ui, &i.to_string()),
                        1 => ledger_field(ui, cx, base, i, row, ledger_hint, Look::Cell),
                        2 => cell_text(ui, group_name(masters, row).as_deref().unwrap_or("")),
                        3 => amount_field(ui, cx, base, i, row, false),
                        _ => {}
                    }
                } else {
                    match c {
                        0 => cell_text(ui, &r.saturating_add(1).to_string()),
                        1 => ledger_field(ui, cx, base, i, row, ledger_hint, Look::Cell),
                        2 => side_switch(ui, cx, i, row.side),
                        3 => cell_text(ui, group_name(masters, row).as_deref().unwrap_or("")),
                        4 if refused
                            && row.amount_text.trim().is_empty()
                            && cx.focus != Field::Amount(i) =>
                        {
                            refused_amount(ui, r);
                        }
                        4 => amount_field(ui, cx, base, i, row, short == Some(row.side)),
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
        let foot_box = self.foot_box(masters);
        let overdraft = self.overdraft_note(masters);
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
            if let Some(text) = &overdraft {
                note(left, NoteKind::Warn, text);
            }
            self.draw_notice(left);
            if let Some(foot) = &foot_box {
                sheet::taxbox(
                    right,
                    &foot.rows,
                    Some(&Total {
                        label: &foot.label,
                        value: &foot.value,
                        tag: Some(ProvKind::Engine),
                        bad: foot.bad,
                    }),
                );
            }
        });
        if let Some(rect) = narration {
            self.shown.fields.push(("Narration", rect));
        }
    }

    /// The box at the right of the foot. A single-entry voucher (index.html:1409) shows the total
    /// paid out or received, the account's balance now and, as the total row, its balance after
    /// saving, the engine's own. A journal (index.html:2065) shows the debit and credit totals and
    /// their difference, from the model's totals of the lines. A contra (index.html:2081) shows the
    /// transfer and the credited account's balance after. `None` until there is something to show.
    fn foot_box(&self, masters: &Masters) -> Option<FootBox> {
        match self.draft.kind {
            EntryKind::Payment | EntryKind::Receipt => self.balance_box(masters),
            EntryKind::Contra => self.contra_box(masters),
            EntryKind::Journal | EntryKind::Memorandum | EntryKind::ReversingJournal => {
                self.totals_box(masters)
            }
        }
    }

    /// The journal's foot box: "Debit total", "Credit total" and the difference. While a line is
    /// incomplete the lines cannot be totalled, and the box says "Not computed" rather than a
    /// figure.
    fn totals_box(&self, masters: &Masters) -> Option<FootBox> {
        if self.draft.rows.iter().all(DraftRow::is_blank) {
            return None;
        }
        let not_computed = || "Not computed".to_owned();
        let totals = self
            .draft
            .build_lines(masters, self.fy, self.books_begin)
            .ok()
            .and_then(|built| match built {
                ledgerly_core::Outcome::Value((_, lines)) => {
                    ledgerly_model::validate::LineTotals::of(&lines).ok()
                }
                ledgerly_core::Outcome::Unsupported(_) => None,
            });
        let Some(totals) = totals else {
            return Some(FootBox {
                rows: vec![
                    ("Debit total".to_owned(), not_computed()),
                    ("Credit total".to_owned(), not_computed()),
                ],
                label: "Difference".to_owned(),
                value: not_computed(),
                bad: false,
            });
        };
        let difference = totals.difference().ok()?;
        Some(FootBox {
            rows: vec![
                ("Debit total".to_owned(), format_amount(totals.debit)),
                ("Credit total".to_owned(), format_amount(totals.credit)),
            ],
            label: "Difference".to_owned(),
            value: format_amount(difference.map_or(ledgerly_core::Paise::ZERO, |d| d.amount)),
            bad: difference.is_some(),
        })
    }

    /// The contra's foot box: the amount moved and what the credited account holds after it, the
    /// engine's figure. `None` until the engine has answered for the voucher as it stands.
    fn contra_box(&self, masters: &Masters) -> Option<FootBox> {
        if self.contra_refused() {
            // Nothing can be transferred yet, so the engine has no figure (index.html:2161).
            return Some(FootBox {
                rows: vec![("Transfer".to_owned(), "Not computed".to_owned())],
                label: "Cash after".to_owned(),
                value: "Not computed".to_owned(),
                bad: false,
            });
        }
        let (account, voucher) = self.balance_request(masters)?;
        let BalanceShown::Change(change) = self.extras.balance_for(account, &voucher)? else {
            return None;
        };
        let totals = ledgerly_model::validate::LineTotals::of(&voucher.ledger_lines).ok()?;
        let name = masters.ledgers.get(&account)?.name.clone();
        Some(FootBox {
            rows: vec![("Transfer".to_owned(), format_amount(totals.debit))],
            label: format!("{name} after"),
            value: format_balance(change.after),
            bad: change.after.side == ledgerly_core::DrCr::Cr && !change.after.amount.is_zero(),
        })
    }

    /// The payment or receipt's foot box (index.html:1409).
    fn balance_box(&self, masters: &Masters) -> Option<FootBox> {
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
        Some(FootBox {
            rows: vec![
                (label.to_owned(), format_amount(total)),
                ("Balance now".to_owned(), format_balance(change.now)),
            ],
            label: "Balance after".to_owned(),
            value: format_balance(change.after),
            // Below zero is drawn in the error colour, as the mock's overdraft state draws it.
            bad: change.after.side == ledgerly_core::DrCr::Cr && !change.after.amount.is_zero(),
        })
    }

    /// The warning of `payScreen`'s overdraft state (index.html:1407): the account would be below
    /// zero once the voucher is saved, by the engine's own figures. Saving is not blocked: the
    /// company has no overdraft limit or block policy yet.
    fn overdraft_note(&self, masters: &Masters) -> Option<String> {
        let (account, voucher) = self.balance_request(masters)?;
        let BalanceShown::Change(change) = self.extras.balance_for(account, &voucher)? else {
            return None;
        };
        if change.after.side != ledgerly_core::DrCr::Cr || change.after.amount.is_zero() {
            return None;
        }
        let name = masters.ledgers.get(&account)?.name.clone();
        let lead = if change.now.side == ledgerly_core::DrCr::Cr && !change.now.amount.is_zero() {
            format!("{name} is below zero.")
        } else {
            format!("This takes {name} below zero.")
        };
        Some(format!(
            "{lead} Shortfall {} (engine). You can still save: the company has no overdraft limit or block policy yet.",
            format_amount(change.after.amount)
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
            || self.panel.is_some()
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
        let differ = matches!(
            self.draft.kind,
            EntryKind::Journal | EntryKind::Memorandum | EntryKind::ReversingJournal
        ) && self.totals_box(masters).is_some_and(|b| b.bad);
        let mut events = Vec::new();
        let mut anchor = None;
        let mut save_clicked = false;
        let enabled = !self.confirm_discard && self.create.is_none() && !busy;
        let (focus, pick) = (self.focus, self.pick_index);
        let want = self.focus_pending.then_some(self.focus);
        ui.add_enabled_ui(enabled, |ui| {
            page_head_numbered(ui, &title, number.as_deref(), subtitle.as_deref(), |ui| {
                // The mock draws Save disabled while the debits and credits differ.
                save_clicked = ui
                    .add_enabled_ui(!differ, |ui| {
                        button_icon(
                            ui,
                            Some(Icon::Save),
                            "Save",
                            save_key.as_deref(),
                            ButtonKind::Primary,
                        )
                        .clicked()
                    })
                    .inner;
            });
            let mut cx = Cx {
                want,
                focus,
                events: &mut events,
                anchor: &mut anchor,
            };
            sheet::sheet(ui, |ui| {
                self.draw_head(ui, base, masters, &mut cx);
                self.draw_table(ui, base, masters, &mut cx);
                self.draw_foot(ui, base, masters, &mut cx);
            });
            self.draw_bill_panel(ui, base, &mut cx);
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
        let before = self.focus;
        self.apply_events(masters, events);
        // A click that moved the keyboard changes what the list under the field shows; the frame
        // that saw the click has already drawn the old list.
        if !keys.is_empty() || self.focus_pending || self.focus != before {
            ui.ctx().request_repaint();
        }
        action
    }
}

/// The hint a ledger field shows while it is empty. After a refused contra each line says what it
/// needs (index.html `SCREENS.contra`, validation-error).
fn line_hint(refused: bool, row: usize) -> &'static str {
    if refused {
        if row == 0 {
            "Pick the account the money goes to"
        } else {
            "Pick the account the money comes from"
        }
    } else if row >= 1 {
        NEXT_LEDGER_HINT
    } else {
        "Ledger: type to search"
    }
}

/// The amount of an empty line after a refused contra: "blank" on the first, "0.00" on the second,
/// both in the error colour (index.html:2155-2156).
fn refused_amount(ui: &mut Ui, row: usize) {
    let t = tokens(ui);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.label(para_text(
            if row == 0 { "blank" } else { "0.00" },
            TEXT_BODY,
            t.bad,
        ));
    });
}

/// The Dr/Cr cell: plain text like the mock's, still the switch (a click flips the side).
fn side_switch(ui: &mut Ui, cx: &mut Cx<'_>, i: usize, side: ledgerly_core::DrCr) {
    let t = tokens(ui);
    let label = ui.add(
        egui::Label::new(para_text(side_label(side), TEXT_BODY, t.text_2))
            .sense(egui::Sense::click()),
    );
    if label.on_hover_text("Click to switch Dr and Cr").clicked() {
        cx.events.push(UiEvent::Side(i));
    }
}

#[cfg(test)]
mod tests {
    use super::masked_account;

    // provenance: the mock writes the bank's account as `XXXX 4471` (index.html:537); the
    // sample's number 99990004471 ends in those four digits. derived: the last four characters.
    #[test]
    fn an_account_number_shows_only_its_last_four_digits() {
        assert_eq!(masked_account("99990004471"), "XXXX 4471");
        assert_eq!(masked_account("12 34"), "XXXX 1234");
        assert_eq!(masked_account(""), "");
    }
}
