//! Read-only display of one voucher, opened by a drill from a report or a palette pick (card C5).
//!
//! # What Tally does, and what this screen does
//!
//! Tally opens a voucher from the last level of a report. Read 2026-11-21 on the keyboard
//! shortcuts page <https://help.tallysolutions.com/keyboard-shortcuts-tally-prime/>: Enter is "To
//! drill-down and open a voucher or master from the last level details of a report", and
//! Ctrl+Enter (TallyPrime) is "To drill-down and open a voucher for display". The Day Book page
//! (<https://help.tallysolutions.com/tally-prime/accounting-financial-reports/day-book-tally/>,
//! read 2026-11-21) says "Press Enter to open the transaction, and check the required details".
//! The pages do not say which mode Enter opens a voucher in.
//!
//! Ledgerly has no screen that alters an existing voucher yet (the entry screen creates new
//! vouchers only, and `AlterVoucher` is not wired to any screen). So Enter opens this screen,
//! which only shows the stored voucher. This is a deliberate gap, not Tally's behaviour: altering
//! from the drill is pending the alteration screen.
//!
//! # No figures of its own (rule C4)
//!
//! The screen shows the lines exactly as stored. It adds nothing up, so it shows no total; a
//! voucher's lines balance by the engine's invariant, not by anything drawn here. Parts of a
//! voucher it cannot show yet (stock lines, stored GST) are named in words, never dropped
//! silently.

use egui::{Frame, Key, Margin, Modifiers, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use ledgerly_api::{ChangeKind, EntryOrigin, VoucherDetail};
use ledgerly_core::{Date, DrCr, Paise, Qty, Rate, SignedAmount, VoucherId, format::format_inr};
use ledgerly_model::{BaseVoucherKind, Books, InventoryLine, VoucherStatus};
use thiserror::Error;

use super::Notice;
use crate::screens::print_job::PrintJob;
use crate::screens::wire::{Done, Latest, unsupported_text};
use crate::theme::{RADIUS_M, TEXT_BODY, TEXT_H2, TEXT_META, border_width, shadow_1};
use crate::ui::badge::{BadgeKind, badge};
use crate::ui::button::{ButtonKind, button};
use crate::ui::note::NoteKind;
use crate::ui::page_head::page_head;
use crate::ui::table::{self, CellPlace, RowState, T_HEAD_HEIGHT, T_ROW_HEIGHT};
use crate::ui::{bold_font, kind, tokens};
use crate::worker::{ApiWorker, Poll, Ticket};
use crate::{AppCtx, KeyHint, Screen, ScreenAction};
use ledgerly_core::Outcome;

/// Why a voucher could not be shown.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VoucherViewError {
    /// The voucher is not in the books.
    #[error("voucher {id} is not in the books")]
    NotFound {
        /// The voucher id asked for.
        id: u64,
    },
    /// A line or the party names a ledger that is not in the masters.
    #[error("voucher {voucher} refers to ledger {ledger}, which is not in the books")]
    UnknownLedger {
        /// The voucher id.
        voucher: u64,
        /// The ledger id that is missing.
        ledger: u64,
    },
    /// A stock line names an item that is not in the masters.
    #[error("voucher {voucher} has a stock line for item {item}, which is not in the books")]
    UnknownItem {
        /// The voucher id.
        voucher: u64,
        /// The stock item id that is missing.
        item: u64,
    },
    /// A stock line names a unit that is not in the masters.
    #[error("voucher {voucher} has a stock line in unit {unit}, which is not in the books")]
    UnknownUnit {
        /// The voucher id.
        voucher: u64,
        /// The unit id that is missing.
        unit: u64,
    },
    /// The voucher's type is not in the masters.
    #[error("voucher {voucher} has voucher type {type_id}, which is not in the books")]
    UnknownType {
        /// The voucher id.
        voucher: u64,
        /// The voucher type id that is missing.
        type_id: u64,
    },
}

/// One ledger line of a voucher, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoucherViewLine {
    /// The ledger's name.
    pub ledger: String,
    /// The stored amount with its side.
    pub amount: SignedAmount,
}

/// What the screen shows for one voucher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoucherView {
    /// The voucher type's name.
    pub type_name: String,
    /// The voucher number, once allocated.
    pub number: Option<String>,
    /// The voucher date.
    pub date: Date,
    /// The status in words.
    pub status: &'static str,
    /// The party ledger's name, when the voucher has one.
    pub party: Option<String>,
    /// The narration.
    pub narration: String,
    /// The ledger lines in stored order.
    pub lines: Vec<VoucherViewLine>,
    /// The stock item lines in stored order (empty for a voucher with no items).
    pub items: Vec<VoucherViewItem>,
}

/// One stock item line of a voucher, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoucherViewItem {
    /// The item's name.
    pub name: String,
    /// The quantity, in `unit`.
    pub qty: Qty,
    /// The unit's symbol.
    pub unit: String,
    /// The rate per `unit`, in rupees.
    pub rate: Rate,
    /// The line amount, as stored (never recomputed from quantity and rate here).
    pub amount: Paise,
}

/// The quantity, rate and amount of an item line as the Item table shows them (index.html:1626):
/// `25 roll`, `3,850.00`, `96,250.00`. The digits are the stored ones; this adds commas, a unit
/// and trailing zeros only.
#[must_use]
pub fn item_texts(item: &VoucherViewItem) -> (String, String, String) {
    let rate = item.rate.0.to_string();
    (
        format!("{} {}", item.qty.0.normalize(), item.unit),
        crate::ui::format::grouped_text(&rate).unwrap_or(rate),
        format_inr(item.amount),
    )
}

/// `text` ended with one full stop: a name such as "Ananya K." already ends with one.
fn sentence(mut text: String) -> String {
    if !text.ends_with('.') {
        text.push('.');
    }
    text
}

/// The Audit card's bullets for a voucher (index.html:1629): who created it and how, what changed
/// it since, whether the period is locked, then the log record. Every fact is the audit log's.
#[must_use]
pub fn audit_lines(detail: &VoucherDetail) -> Vec<String> {
    let lock = if detail.period_locked() {
        "Period locked."
    } else {
        "Alter is allowed: the period is open."
    };
    let Some(audit) = &detail.audit else {
        return vec![
            "The audit log holds no record of who entered this voucher.".to_owned(),
            lock.to_owned(),
        ];
    };
    let day = crate::ui::format::date(audit.posted_at.date());
    let by = &audit.entered_by;
    let created = match audit.origin {
        EntryOrigin::Typed => format!("Created {day} by {by}, typed by hand."),
        EntryOrigin::AiDraft => {
            sentence(format!("Created {day} from an AI draft, accepted by {by}"))
        }
        EntryOrigin::Imported => format!("Created {day} by {by}, through an import."),
    };
    let mut lines = vec![created];
    if audit.changes.is_empty() {
        lines.push(format!("Not altered since. {lock}"));
    } else {
        for change in &audit.changes {
            let word = match change.kind {
                ChangeKind::Altered => "Altered",
                ChangeKind::Cancelled => "Cancelled",
            };
            lines.push(sentence(format!(
                "{word} {} by {}",
                crate::ui::format::date(change.at.date()),
                change.by
            )));
        }
        lines.push(lock.to_owned());
    }
    let short: String = audit.hash.chars().take(12).collect();
    lines.push(format!("Audit log entry {}, hash {short}.", audit.event.0));
    lines
}

/// The status as words.
fn status_text(status: VoucherStatus) -> &'static str {
    match status {
        VoucherStatus::Regular => "Regular",
        VoucherStatus::Optional => "Optional (not in the books)",
        VoucherStatus::PostDated => "Post-dated",
        VoucherStatus::Cancelled => "Cancelled",
    }
}

/// The display model of voucher `id`, copied from `books`.
///
/// # Errors
/// [`VoucherViewError`] when the voucher, its type, its party or one of its ledgers is missing
/// from `books`.
pub fn voucher_view(books: &Books, id: VoucherId) -> Result<VoucherView, VoucherViewError> {
    let voucher = books
        .vouchers
        .get(&id)
        .ok_or(VoucherViewError::NotFound { id: id.0 })?;
    let type_name = books
        .masters
        .voucher_types
        .get(&voucher.type_id)
        .map(|t| t.name.clone())
        .ok_or(VoucherViewError::UnknownType {
            voucher: id.0,
            type_id: voucher.type_id.0,
        })?;
    let ledger_name = |ledger: ledgerly_core::LedgerId| {
        books
            .masters
            .ledgers
            .get(&ledger)
            .map(|l| l.name.clone())
            .ok_or(VoucherViewError::UnknownLedger {
                voucher: id.0,
                ledger: ledger.0,
            })
    };
    let party = voucher.party.map(ledger_name).transpose()?;
    let lines = voucher
        .ledger_lines
        .iter()
        .map(|line| {
            Ok(VoucherViewLine {
                ledger: ledger_name(line.ledger)?,
                amount: line.amount,
            })
        })
        .collect::<Result<Vec<_>, VoucherViewError>>()?;
    let items = voucher
        .inventory_lines
        .iter()
        .map(|line| item_of(books, id, line))
        .collect::<Result<Vec<_>, VoucherViewError>>()?;
    Ok(VoucherView {
        type_name,
        number: voucher.number.clone(),
        date: voucher.date,
        status: status_text(voucher.status),
        party,
        narration: voucher.narration.clone(),
        lines,
        items,
    })
}

/// The display line of one stock item line.
fn item_of(
    books: &Books,
    voucher: VoucherId,
    line: &InventoryLine,
) -> Result<VoucherViewItem, VoucherViewError> {
    let name = books
        .masters
        .stock_items
        .get(&line.item)
        .map(|i| i.name.clone())
        .ok_or(VoucherViewError::UnknownItem {
            voucher: voucher.0,
            item: line.item.0,
        })?;
    let unit = books
        .masters
        .units
        .get(&line.unit)
        .map(|u| u.symbol.clone())
        .ok_or(VoucherViewError::UnknownUnit {
            voucher: voucher.0,
            unit: line.unit.0,
        })?;
    Ok(VoucherViewItem {
        name,
        qty: line.qty,
        unit,
        rate: line.rate,
        amount: line.amount,
    })
}

/// The page title of a voucher (index.html:1574: "Voucher SI/0412"); a voucher with no number yet
/// says so.
#[must_use]
pub fn voucher_title(view: &VoucherView) -> String {
    match view.number.as_deref() {
        Some(number) => format!("Voucher {number}"),
        None => "Voucher (no number yet)".to_owned(),
    }
}

/// A posting's amount as the Debit and Credit columns show it: the figure in the column of its
/// side and nothing in the other (index.html:1621), bare of any Dr or Cr word. A line is never
/// added to another.
#[must_use]
pub fn posting_columns(amount: SignedAmount) -> (String, String) {
    let figure = format_inr(amount.amount);
    match amount.side {
        DrCr::Dr => (figure, String::new()),
        DrCr::Cr => (String::new(), figure),
    }
}

/// What the screen holds of the engine's totals and audit facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DetailState {
    /// Not asked yet, or the answer is on its way.
    Loading,
    /// The engine's answer.
    Ready(VoucherDetail),
    /// The engine could not give it; the text says why.
    Unavailable(String),
}

/// The read-only voucher screen.
#[derive(Debug)]
pub struct VoucherViewScreen {
    id: VoucherId,
    /// The sidebar title of the report the voucher was opened from (see
    /// [`Screen::sidebar_title`]).
    origin: Option<String>,
    /// The print in flight (Ctrl+P), and the last notice about a print.
    print: PrintJob,
    notice: Option<Notice>,
    /// Ctrl+E: the e-invoice JSON being made, the dialog that names its file, and what was said
    /// about the last export (shown as a note under the head, as index.html's
    /// `balance-sheet.voucher-einvoice-exported`).
    export: Option<Ticket<Result<Outcome<String>, String>>>,
    export_picker: JsonPicker,
    export_note: Option<Notice>,
    /// The voucher's number and page title, once the books have been read (the breadcrumb is
    /// asked for without the books).
    shown: std::cell::RefCell<Option<Shown>>,
    /// The totals and audit facts, asked of the engine on the worker.
    detail: Latest<Result<Outcome<VoucherDetail>, String>>,
    detail_state: DetailState,
    asked: bool,
}

/// What the screen read from the books for its heads.
#[derive(Debug, Clone)]
struct Shown {
    /// The voucher number (or its title when it has none): the crumb.
    number: String,
    /// The voucher view's page title, "Voucher SI/0412".
    title: String,
    /// The print preview's crumb: the entry screen's name for the type, "Sales invoice".
    type_label: String,
    /// The print preview's page title, "Sales invoice SI/0412".
    preview_title: String,
    /// The print preview's subtitle, "Saved · Shree Ganesh Traders · 14 Nov 2026".
    preview_subtitle: String,
}

/// The name the entry screen gives a voucher's type: a sales voucher is entered as an invoice
/// (index.html `addPrintPreview("sales-invoice", ...)`: "Sales invoice"); the other types keep
/// their own name.
fn entry_name(books: &Books, id: VoucherId, view: &VoucherView) -> String {
    let sales = books
        .vouchers
        .get(&id)
        .and_then(|v| books.masters.voucher_types.get(&v.type_id))
        .is_some_and(|t| t.base == BaseVoucherKind::Sales);
    if sales {
        format!("{} invoice", view.type_name)
    } else {
        view.type_name.clone()
    }
}

/// The width of the Debit and Credit columns of the posting table (index.html:1621, `.num`).
const AMOUNT_W: f32 = 170.0;
/// The Item table's Qty, Rate and Amount columns.
const ITEM_COLS: [f32; 3] = [110.0, 112.0, 150.0];

/// The save dialog of the e-invoice export (a closure has no `Debug`).
struct JsonPicker(crate::screens::print_job::SavePicker);

impl std::fmt::Debug for JsonPicker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("JsonPicker")
    }
}

/// The system save dialog, offering JSON.
fn save_json_dialog(suggested: &str) -> Option<std::path::PathBuf> {
    crate::ui::dialogs::save_file(
        "Export e-invoice JSON",
        Some(suggested),
        &[("JSON", &["json"])],
    )
}

impl VoucherViewScreen {
    /// Replaces the save dialog the e-invoice export opens.
    pub fn set_export_picker(&mut self, picker: crate::screens::print_job::SavePicker) {
        self.export_picker = JsonPicker(picker);
    }

    /// What was said about the last e-invoice export, when there was one.
    #[must_use]
    pub fn export_note(&self) -> Option<&Notice> {
        self.export_note.as_ref()
    }

    /// Takes the e-invoice JSON when the worker has made it, asks where to save it, writes it and
    /// says where it went. A voucher that cannot be an e-invoice says why; nothing is written.
    fn tick_export(&mut self) {
        let Some(ticket) = self.export.as_mut() else {
            return;
        };
        let answer = match ticket.poll() {
            Poll::Pending | Poll::Taken => return,
            Poll::Ready(reply) => reply,
            Poll::Lost => Err("the engine stopped before it answered".to_owned()),
        };
        self.export = None;
        self.export_note = Some(match answer {
            Ok(Outcome::Value(json)) => {
                let number = self
                    .shown
                    .borrow()
                    .as_ref()
                    .map_or_else(|| self.id.0.to_string(), |s| s.number.replace('/', "-"));
                match (self.export_picker.0)(&format!("e-invoice-{number}.json")) {
                    // A cancelled dialog writes nothing and says nothing.
                    None => return,
                    Some(path) => match std::fs::write(&path, json) {
                        Ok(()) => Notice::Info(format!(
                            "Exported the e-invoice JSON (INV-01) to {}. Upload it on the \
                             government portal's bulk page; nothing is sent from here.",
                            path.display()
                        )),
                        Err(e) => Notice::Error(format!(
                            "The e-invoice JSON was not saved: \"{}\" could not be written ({e}). \
                             Nothing was written.",
                            path.display()
                        )),
                    },
                }
            }
            Ok(Outcome::Unsupported(u)) => {
                Notice::Error(format!("The e-invoice JSON was not written. {}", u.reason))
            }
            Err(e) => Notice::Error(format!("The e-invoice JSON was not written: {e}")),
        });
    }

    /// Replaces the save dialog the print opens.
    pub fn set_print_picker(&mut self, picker: crate::screens::print_job::SavePicker) {
        self.print.set_picker(picker);
    }

    /// The pages the print preview shows, when it is open.
    #[must_use]
    pub fn print_preview(&self) -> Option<&ledgerly_api::PrintedDocument> {
        self.print.preview()
    }

    /// The settings typed in the print preview's card.
    pub fn print_form_mut(&mut self) -> &mut crate::screens::print_preview::PrintForm {
        self.print.form_mut()
    }

    /// Why the last Update preview was refused, when it was.
    #[must_use]
    pub fn print_setup_error(&self) -> Option<&str> {
        self.print.setup_error()
    }

    /// `true` while the PDF is being prepared (the busy note is on screen).
    #[must_use]
    pub fn is_print_rendering(&self) -> bool {
        self.print.is_rendering()
    }

    /// The notice about the last print, when there is one.
    #[must_use]
    pub fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref()
    }

    /// The print preview in place of the voucher: the page head, then the engine's pages and the
    /// card that saves them (the same preview as every report).
    fn draw_print_preview(&mut self, ui: &mut Ui) -> ScreenAction {
        page_head(ui, &self.page_title(), self.subtitle().as_deref(), |_| {});
        self.print.draw_preview(ui);
        ui.ctx().request_repaint();
        ScreenAction::Stay
    }

    /// A screen for voucher `id`. It reads the voucher from the shared books each frame.
    #[must_use]
    pub fn new(id: VoucherId) -> Self {
        Self {
            id,
            origin: None,
            print: PrintJob::default(),
            notice: None,
            export: None,
            export_picker: JsonPicker(Box::new(save_json_dialog)),
            export_note: None,
            shown: std::cell::RefCell::new(None),
            detail: Latest::default(),
            detail_state: DetailState::Loading,
            asked: false,
        }
    }

    /// The same screen, opened from a drill that started at the screen titled `origin`.
    #[must_use]
    pub fn with_origin(mut self, origin: impl Into<String>) -> Self {
        self.origin = Some(origin.into());
        self
    }

    /// The voucher this screen shows.
    #[must_use]
    pub fn voucher_id(&self) -> VoucherId {
        self.id
    }

    /// Takes a finished answer, and asks the worker the first time it can.
    fn tick_detail(&mut self, worker: Option<&ApiWorker>) {
        match self.detail.poll() {
            Some(Done::Ready(Ok(Outcome::Value(d)))) => self.detail_state = DetailState::Ready(d),
            Some(Done::Ready(Ok(Outcome::Unsupported(u)))) => {
                self.detail_state = DetailState::Unavailable(unsupported_text(&u));
            }
            Some(Done::Ready(Err(e))) => self.detail_state = DetailState::Unavailable(e),
            Some(Done::Lost) => {
                self.detail_state = DetailState::Unavailable(
                    "the background worker stopped before the audit log was read".to_owned(),
                );
            }
            None => {}
        }
        if self.asked {
            return;
        }
        let Some(handle) = worker else {
            self.detail_state = DetailState::Unavailable(
                "no company file is open, so the audit log cannot be read".to_owned(),
            );
            self.asked = true;
            return;
        };
        self.asked = true;
        let id = self.id;
        self.detail.start(handle, move |api| {
            api.voucher_detail(id).map_err(|e| e.to_string())
        });
    }

    /// The voucher as the mock's two cards (index.html:1612-1630): the voucher, its items and
    /// postings on the left, the Audit card on the right.
    /// Ctrl+P prints the voucher (spec 11 M5) and Ctrl+E exports its e-invoice JSON; both
    /// run on the API worker.
    fn handle_keys(&mut self, ui: &mut Ui, ctx: &mut AppCtx) {
        // Ctrl+P prints the voucher (spec 11 M5). The PDF comes from the API worker.
        if ui
            .ctx()
            .input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::P))
        {
            match ctx.api.as_ref() {
                None => {
                    self.notice = Some(Notice::Error(
                        "No company is open, so the voucher cannot be printed.".to_owned(),
                    ));
                }
                Some(handle) => {
                    let id = self.id;
                    let title = format!("Voucher {}", id.0);
                    self.print.start(handle, &title, move |api, setup| {
                        api.print_voucher_document_with(id, setup)
                    });
                }
            }
        }
        if ui
            .ctx()
            .input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::E))
        {
            match ctx.api.as_ref() {
                None => {
                    self.export_note = Some(Notice::Error(
                        "No company is open, so the e-invoice cannot be exported.".to_owned(),
                    ));
                }
                Some(handle) => {
                    let id = self.id;
                    self.export = Some(
                        handle.run(move |api| api.einvoice_json(id).map_err(|e| e.to_string())),
                    );
                }
            }
        }
    }

    fn draw_view(&self, ui: &mut Ui, view: &VoucherView) {
        let gap = 12.0;
        let total = ui.available_width();
        // `.g-main`: minmax(0, 1.55fr) minmax(320px, 1fr).
        let right = ((total - gap) / 2.55).max(320.0).min(total - gap);
        let left = total - gap - right;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = vec2(gap, 0.0);
            let down = egui::Layout::top_down(egui::Align::Min);
            ui.allocate_ui_with_layout(vec2(left, 0.0), down, |ui| {
                self.draw_voucher_card(ui, view, left);
            });
            ui.allocate_ui_with_layout(vec2(right, 0.0), down, |ui| {
                self.draw_audit_card(ui);
            });
        });
    }

    fn card_frame(ui: &Ui) -> Frame {
        let t = tokens(ui);
        let k = kind(ui);
        Frame::new()
            .fill(t.surface)
            .stroke(Stroke::new(border_width(k), t.border))
            .corner_radius(RADIUS_M)
            .shadow(shadow_1(k))
    }

    fn draw_voucher_card(&self, ui: &mut Ui, view: &VoucherView, width: f32) {
        let t = tokens(ui);
        Self::card_frame(ui).show(ui, |ui| {
            ui.set_width(width - 2.0);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            // `.vhead`: four fields in a row, 16 px around, a rule under.
            let number = view.number.as_deref().unwrap_or("(no number yet)");
            let date = crate::ui::format::date(view.date);
            Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                ui.set_width(width - 34.0);
                ui.spacing_mut().item_spacing = vec2(16.0, 12.0);
                let cell = (width - 34.0 - 3.0 * 16.0) / 4.0;
                ui.horizontal_top(|ui| {
                    for (label, value) in [
                        ("Type", view.type_name.as_str()),
                        ("Number", number),
                        ("Date", date.as_str()),
                    ] {
                        ui.allocate_ui(vec2(cell, 0.0), |ui| {
                            ui.set_min_width(cell);
                            field(ui, label, |ui| {
                                ui.label(RichText::new(value).size(TEXT_BODY).color(t.text));
                            });
                        });
                    }
                    ui.allocate_ui(vec2(cell, 0.0), |ui| {
                        ui.set_min_width(cell);
                        field(ui, "Status", |ui| {
                            let (kind_of, word) = if view.status == "Regular" {
                                (BadgeKind::Ok, "Posted")
                            } else {
                                (BadgeKind::Warn, view.status)
                            };
                            badge(ui, kind_of, word);
                        });
                    });
                });
            });
            let (rule, _) = ui.allocate_exact_size(vec2(width - 2.0, 1.0), Sense::hover());
            ui.painter()
                .hline(rule.x_range(), rule.center().y, Stroke::new(1.0, t.border));
            if !view.items.is_empty() {
                Self::draw_items(ui, view, width - 2.0);
                ui.add_space(8.0);
            }
            self.draw_postings(ui, view, width - 2.0);
            if !view.narration.is_empty() {
                Frame::new()
                    .inner_margin(Margin::symmetric(16, 10))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("Narration: {}", view.narration))
                                .size(TEXT_META)
                                .color(t.text_2),
                        );
                    });
            }
        });
    }

    /// A heading line: `label` on the left, `heads` over the right-hand columns.
    fn paint_head(ui: &mut Ui, width: f32, label: &str, heads: &[(&str, f32)], engine: bool) {
        let t = tokens(ui);
        let (head, _) = ui.allocate_exact_size(vec2(width, T_HEAD_HEIGHT), Sense::hover());
        table::paint_head_rule(ui, head);
        let font = bold_font(ui, TEXT_META);
        let (left, cols) = split_columns(head, &heads.iter().map(|h| h.1).collect::<Vec<_>>());
        table::paint_cell_text(
            ui,
            left,
            1.0,
            label,
            font.clone(),
            t.text_3,
            CellPlace::Left(table::CELL_PAD_X),
        );
        if engine {
            table::paint_engine_tag(ui, left, label, &font);
        }
        for ((text, _), rect) in heads.iter().zip(cols) {
            table::paint_cell_text(
                ui,
                rect,
                1.0,
                text,
                font.clone(),
                t.text_3,
                CellPlace::Right,
            );
        }
    }

    /// One body line: `label` on the left and `cells` in the right-hand columns.
    fn paint_line(ui: &mut Ui, width: f32, label: &str, cells: &[(String, f32)]) {
        let t = tokens(ui);
        let (rect, _) = ui.allocate_exact_size(vec2(width, T_ROW_HEIGHT), Sense::hover());
        table::paint_row(
            ui,
            rect,
            RowState {
                selected: false,
                hovered: false,
                sub: false,
            },
        );
        let (left, cols) = split_columns(rect, &cells.iter().map(|c| c.1).collect::<Vec<_>>());
        let font = egui::FontId::proportional(TEXT_BODY);
        table::paint_cell_text(
            ui,
            left,
            1.0,
            label,
            font.clone(),
            t.text,
            CellPlace::Left(table::CELL_PAD_X),
        );
        for ((text, _), rect) in cells.iter().zip(cols) {
            table::paint_cell_text(ui, rect, 1.0, text, font.clone(), t.text, CellPlace::Right);
        }
    }

    /// The Item table (index.html:1613): item, quantity with its unit, rate and amount as stored.
    fn draw_items(ui: &mut Ui, view: &VoucherView, width: f32) {
        let [qty_w, rate_w, amount_w] = ITEM_COLS;
        Self::paint_head(
            ui,
            width,
            "Item",
            &[("Qty", qty_w), ("Rate", rate_w), ("Amount", amount_w)],
            false,
        );
        for item in &view.items {
            let (qty, rate, amount) = item_texts(item);
            Self::paint_line(
                ui,
                width,
                &item.name,
                &[(qty, qty_w), (rate, rate_w), (amount, amount_w)],
            );
        }
    }

    /// The ledger lines, one per posting, in the Debit or Credit column of their side, then the
    /// engine's totals of the two sides (never added here).
    fn draw_postings(&self, ui: &mut Ui, view: &VoucherView, width: f32) {
        Self::paint_head(
            ui,
            width,
            "Ledger posting",
            &[("Debit", AMOUNT_W), ("Credit", AMOUNT_W)],
            true,
        );
        for line in &view.lines {
            let (dr, cr) = posting_columns(line.amount);
            Self::paint_line(ui, width, &line.ledger, &[(dr, AMOUNT_W), (cr, AMOUNT_W)]);
        }
        let t = tokens(ui);
        match &self.detail_state {
            DetailState::Ready(detail) => {
                let (rect, _) =
                    ui.allocate_exact_size(vec2(width, table::T_TOTAL_HEIGHT), Sense::hover());
                table::paint_total_rules(ui, rect);
                let font = bold_font(ui, TEXT_BODY);
                let (left, cols) = split_columns(rect, &[AMOUNT_W, AMOUNT_W]);
                table::paint_cell_text(
                    ui,
                    left,
                    3.0,
                    "Total",
                    font.clone(),
                    t.text,
                    CellPlace::Left(table::CELL_PAD_X),
                );
                for (total, cell) in [detail.debit_total, detail.credit_total]
                    .into_iter()
                    .zip(cols)
                {
                    table::paint_cell_text(
                        ui,
                        cell,
                        3.0,
                        &format_inr(total),
                        font.clone(),
                        t.text,
                        CellPlace::Right,
                    );
                }
            }
            DetailState::Loading => {}
            DetailState::Unavailable(why) => {
                Frame::new()
                    .inner_margin(Margin::symmetric(16, 10))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("Total not shown: {why}."))
                                .size(TEXT_META)
                                .color(t.text_2),
                        );
                    });
            }
        }
    }

    /// The right-hand Audit card (index.html:1629): the audit log's facts about this voucher.
    fn draw_audit_card(&self, ui: &mut Ui) {
        let t = tokens(ui);
        Self::card_frame(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            Frame::new()
                .inner_margin(Margin::symmetric(16, 14))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing = vec2(0.0, 6.0);
                    ui.label(
                        RichText::new("Audit")
                            .font(bold_font(ui, TEXT_H2))
                            .color(t.text),
                    );
                    ui.add_space(4.0);
                    let mut lines = match &self.detail_state {
                        DetailState::Ready(detail) => audit_lines(detail),
                        DetailState::Loading => vec!["Reading the audit log.".to_owned()],
                        DetailState::Unavailable(why) => vec![format!(
                            "The audit log could not be read: {why}. Nothing is guessed here."
                        )],
                    };
                    lines.push(
                        "Read-only here. Altering a voucher from a report is not available yet."
                            .to_owned(),
                    );
                    for line in lines {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            ui.label(RichText::new("\u{2022}").color(t.text_2));
                            ui.add(
                                egui::Label::new(
                                    RichText::new(line).size(TEXT_BODY).color(t.text_2),
                                )
                                .wrap(),
                            );
                        });
                    }
                });
        });
    }
}

/// Splits `rect` into the label area and right-aligned columns of `widths` (the last width is the
/// rightmost column).
fn split_columns(rect: Rect, widths: &[f32]) -> (Rect, Vec<Rect>) {
    let mut right = rect.right();
    let mut cols = Vec::with_capacity(widths.len());
    for w in widths.iter().rev() {
        cols.push(Rect::from_min_max(
            pos2(right - w, rect.top()),
            pos2(right, rect.bottom()),
        ));
        right -= w;
    }
    cols.reverse();
    (
        Rect::from_min_max(rect.min, pos2(right, rect.bottom())),
        cols,
    )
}

/// One field of the voucher head (`.fld`): its label in the meta style, then the value.
fn field(ui: &mut Ui, label: &str, value: impl FnOnce(&mut Ui)) {
    let t = tokens(ui);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 4.0);
        ui.label(RichText::new(label).size(TEXT_META).color(t.text_2));
        value(ui);
    });
}

impl Screen for VoucherViewScreen {
    fn title(&self) -> String {
        match self.shown.borrow().as_ref() {
            Some(s) if self.print.preview().is_some() => s.type_label.clone(),
            Some(s) => s.number.clone(),
            None => format!("Voucher #{}", self.id.0),
        }
    }

    fn page_title(&self) -> String {
        match self.shown.borrow().as_ref() {
            Some(s) if self.print.preview().is_some() => s.preview_title.clone(),
            Some(s) => s.title.clone(),
            None => format!("Voucher #{}", self.id.0),
        }
    }

    fn subtitle(&self) -> Option<String> {
        match self.shown.borrow().as_ref() {
            Some(s) if self.print.preview().is_some() => Some(s.preview_subtitle.clone()),
            Some(_) | None => Some("Read-only. Esc goes back up".to_owned()),
        }
    }

    fn key_bar_name(&self) -> String {
        if self.print.preview().is_some() {
            return "Print preview".to_owned();
        }
        self.page_title()
    }

    fn keys(&self) -> Vec<KeyHint> {
        if self.print.preview().is_some() {
            // index.html:2272: the preview's key bar.
            return vec![
                KeyHint::new("Ctrl+P", "Save as PDF"),
                KeyHint::new("Esc", "Close"),
            ];
        }
        Vec::new()
    }

    fn sidebar_title(&self) -> String {
        self.origin.clone().unwrap_or_else(|| self.title())
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        let was_previewing = self.print.preview().is_some();
        if let Some(note) = self.print.tick() {
            self.notice = Some(note);
        }
        if !was_previewing && self.print.preview().is_some() {
            // The preview opens on a clean page: an older note is not about this print.
            self.notice = None;
        }
        self.print.run_update(ctx.api.as_ref());
        self.tick_export();
        self.tick_detail(ctx.api.as_ref());
        if self.export.is_some() {
            ui.ctx().request_repaint();
        }
        if self.detail.is_pending() {
            ui.ctx().request_repaint();
        }
        if self.print.preview().is_some() {
            return self.draw_print_preview(ui);
        }
        self.handle_keys(ui, ctx);
        if self.print.is_rendering() {
            crate::screens::print_preview::draw_busy(ui);
        }
        if let Some(note) = &self.notice {
            ui.label(note.text());
        }
        if self.print.saved_path().is_some()
            && button(ui, "Open PDF", None, ButtonKind::Secondary).clicked()
            && let Some(note) = self.print.open_saved()
        {
            self.notice = Some(note);
        }
        match ctx.books.as_ref() {
            None => {
                ui.label("The company's books are not loaded, so the voucher cannot be shown.");
            }
            Some(books) => match voucher_view(books, self.id) {
                Ok(view) => {
                    let title = voucher_title(&view);
                    let number = view.number.clone().unwrap_or_else(|| title.clone());
                    let type_label = entry_name(books, self.id, &view);
                    let status = if view.status == "Regular" {
                        "Saved"
                    } else {
                        view.status
                    };
                    let mut parts = vec![status.to_owned()];
                    parts.extend(view.party.clone());
                    parts.push(crate::ui::format::date(view.date));
                    *self.shown.borrow_mut() = Some(Shown {
                        preview_title: format!("{type_label} {number}"),
                        preview_subtitle: parts.join(" \u{b7} "),
                        type_label,
                        number,
                        title: title.clone(),
                    });
                    page_head(ui, &title, self.subtitle().as_deref(), |_| {});
                    if let Some(note) = &self.export_note {
                        let kind = match note {
                            Notice::Info(_) => NoteKind::Info,
                            Notice::Error(_) => NoteKind::Bad,
                        };
                        crate::ui::note::note(ui, kind, note.text());
                    }
                    self.draw_view(ui, &view);
                }
                Err(e) => {
                    ui.label(e.to_string());
                }
            },
        }
        ScreenAction::Stay
    }
}
