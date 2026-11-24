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

use egui::{Key, Modifiers, RichText, Ui};
use ledgerly_core::{Date, SignedAmount, VoucherId};
use ledgerly_model::{Books, VoucherStatus};
use thiserror::Error;

use super::Notice;
use super::text::{amount_text, date_text};
use crate::screens::print_job::PrintJob;
use crate::ui::button::{ButtonKind, button};
use crate::{AppCtx, Screen, ScreenAction};

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
    /// Parts of the voucher this screen cannot show yet, in words.
    pub not_shown: Vec<String>,
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
    let mut not_shown = Vec::new();
    if !voucher.inventory_lines.is_empty() {
        not_shown.push("Stock item lines are not shown on this screen yet.".to_owned());
    }
    if voucher.gst.is_some() {
        not_shown.push("GST details are not shown on this screen yet.".to_owned());
    }
    Ok(VoucherView {
        type_name,
        number: voucher.number.clone(),
        date: voucher.date,
        status: status_text(voucher.status),
        party,
        narration: voucher.narration.clone(),
        lines,
        not_shown,
    })
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
}

impl VoucherViewScreen {
    /// A screen for voucher `id`. It reads the voucher from the shared books each frame.
    #[must_use]
    pub fn new(id: VoucherId) -> Self {
        Self {
            id,
            origin: None,
            print: PrintJob::default(),
            notice: None,
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

    fn draw_view(ui: &mut Ui, view: &VoucherView) {
        let number = view.number.as_deref().unwrap_or("(no number yet)");
        ui.heading(format!("{} {number}", view.type_name));
        ui.label(format!(
            "Date {}    Status {}",
            date_text(view.date),
            view.status
        ));
        if let Some(party) = view.party.as_deref() {
            ui.label(format!("Party {party}"));
        }
        ui.separator();
        egui::Grid::new(("voucher-view-lines", view.lines.len())).show(ui, |ui| {
            for line in &view.lines {
                ui.label(line.ledger.as_str());
                ui.label(amount_text(line.amount));
                ui.end_row();
            }
        });
        if !view.narration.is_empty() {
            ui.separator();
            ui.label(format!("Narration: {}", view.narration));
        }
        for note in &view.not_shown {
            ui.label(RichText::new(note.as_str()).italics());
        }
        ui.separator();
        ui.label(
            RichText::new("Display only. Altering a voucher from a report is not available yet.")
                .italics(),
        );
    }
}

impl Screen for VoucherViewScreen {
    fn title(&self) -> String {
        format!("Voucher #{}", self.id.0)
    }

    fn sidebar_title(&self) -> String {
        self.origin.clone().unwrap_or_else(|| self.title())
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        if let Some(note) = self.print.tick() {
            self.notice = Some(note);
        }
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
                    self.print
                        .start(handle, &title, move |api| api.print_voucher(id));
                }
            }
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
                Ok(view) => Self::draw_view(ui, &view),
                Err(e) => {
                    ui.label(e.to_string());
                }
            },
        }
        ScreenAction::Stay
    }
}
