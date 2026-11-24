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
use ledgerly_core::{Date, DrCr, SignedAmount, VoucherId, format::format_inr};
use ledgerly_model::{Books, VoucherStatus};
use thiserror::Error;

use super::Notice;
use super::text::date_text;
use crate::screens::print_job::PrintJob;
use crate::theme::{RADIUS_M, TEXT_BODY, TEXT_H2, TEXT_META, border_width, shadow_1};
use crate::ui::badge::{BadgeKind, badge};
use crate::ui::button::{ButtonKind, button};
use crate::ui::page_head::page_head;
use crate::ui::table::{self, CellPlace, RowState, T_HEAD_HEIGHT, T_ROW_HEIGHT};
use crate::ui::{bold_font, kind, tokens};
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
    /// The voucher's number and page title, once the books have been read (the breadcrumb is
    /// asked for without the books).
    shown: std::cell::RefCell<Option<(String, String)>>,
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
            shown: std::cell::RefCell::new(None),
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

    /// The voucher as the mock's two cards (index.html:1612-1630): the voucher and its postings
    /// on the left, what this screen cannot do on the right.
    fn draw_view(ui: &mut Ui, view: &VoucherView) {
        let gap = 12.0;
        let total = ui.available_width();
        // `.g-main`: minmax(0, 1.55fr) minmax(320px, 1fr).
        let right = ((total - gap) / 2.55).max(320.0).min(total - gap);
        let left = total - gap - right;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = vec2(gap, 0.0);
            let down = egui::Layout::top_down(egui::Align::Min);
            ui.allocate_ui_with_layout(vec2(left, 0.0), down, |ui| {
                Self::draw_voucher_card(ui, view, left);
            });
            ui.allocate_ui_with_layout(vec2(right, 0.0), down, |ui| {
                Self::draw_notes_card(ui, view);
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

    fn draw_voucher_card(ui: &mut Ui, view: &VoucherView, width: f32) {
        let t = tokens(ui);
        Self::card_frame(ui).show(ui, |ui| {
            ui.set_width(width - 2.0);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            // `.vhead`: four fields in a row, 16 px around, a rule under.
            let number = view.number.as_deref().unwrap_or("(no number yet)");
            let date = date_text(view.date);
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
                            field(ui, label, |ui| {
                                ui.label(RichText::new(value).size(TEXT_BODY).color(t.text));
                            });
                        });
                    }
                    ui.allocate_ui(vec2(cell, 0.0), |ui| {
                        field(ui, "Status", |ui| {
                            let kind_of = if view.status == "Regular" {
                                BadgeKind::Ok
                            } else {
                                BadgeKind::Warn
                            };
                            badge(ui, kind_of, view.status);
                        });
                    });
                });
                if let Some(party) = view.party.as_deref() {
                    field(ui, "Party", |ui| {
                        ui.label(RichText::new(party).size(TEXT_BODY).color(t.text));
                    });
                }
            });
            let (rule, _) = ui.allocate_exact_size(vec2(width - 2.0, 1.0), Sense::hover());
            ui.painter()
                .hline(rule.x_range(), rule.center().y, Stroke::new(1.0, t.border));
            Self::draw_postings(ui, view, width - 2.0);
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

    /// The ledger lines, one per posting, in the Debit or Credit column of their side. No total
    /// is drawn: the screen never adds figures.
    fn draw_postings(ui: &mut Ui, view: &VoucherView, width: f32) {
        let t = tokens(ui);
        let amount_w = 150.0_f32.min(width / 3.0);
        let split = |rect: Rect| {
            let credit = Rect::from_min_max(pos2(rect.right() - amount_w, rect.top()), rect.max);
            let debit = Rect::from_min_max(
                pos2(credit.left() - amount_w, rect.top()),
                pos2(credit.left(), rect.bottom()),
            );
            let label = Rect::from_min_max(rect.min, pos2(debit.left(), rect.bottom()));
            (label, debit, credit)
        };
        let (head, _) = ui.allocate_exact_size(vec2(width, T_HEAD_HEIGHT), Sense::hover());
        table::paint_head_rule(ui, head);
        let (label, debit, credit) = split(head);
        let font = bold_font(ui, TEXT_META);
        table::paint_cell_text(
            ui,
            label,
            1.0,
            "Ledger posting",
            font.clone(),
            t.text_3,
            CellPlace::Left(table::CELL_PAD_X),
        );
        table::paint_cell_text(
            ui,
            debit,
            1.0,
            "Debit",
            font.clone(),
            t.text_3,
            CellPlace::Right,
        );
        table::paint_cell_text(ui, credit, 1.0, "Credit", font, t.text_3, CellPlace::Right);
        for line in &view.lines {
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
            let (label, debit, credit) = split(rect);
            let font = egui::FontId::proportional(TEXT_BODY);
            table::paint_cell_text(
                ui,
                label,
                1.0,
                &line.ledger,
                font.clone(),
                t.text,
                CellPlace::Left(table::CELL_PAD_X),
            );
            let (dr, cr) = posting_columns(line.amount);
            table::paint_cell_text(ui, debit, 1.0, &dr, font.clone(), t.text, CellPlace::Right);
            table::paint_cell_text(ui, credit, 1.0, &cr, font, t.text, CellPlace::Right);
        }
    }

    /// The right-hand card: what the screen is (read-only) and what it leaves out.
    fn draw_notes_card(ui: &mut Ui, view: &VoucherView) {
        let t = tokens(ui);
        Self::card_frame(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            Frame::new()
                .inner_margin(Margin::symmetric(16, 14))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing = vec2(0.0, 6.0);
                    ui.label(
                        RichText::new("Read-only")
                            .font(bold_font(ui, TEXT_H2))
                            .color(t.text),
                    );
                    ui.add_space(4.0);
                    let lines = view.not_shown.iter().map(String::as_str).chain([
                        "Display only. Altering a voucher from a report is not available yet.",
                    ]);
                    for line in lines {
                        ui.horizontal_top(|ui| {
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
            Some((number, _)) => number.clone(),
            None => format!("Voucher #{}", self.id.0),
        }
    }

    fn page_title(&self) -> String {
        match self.shown.borrow().as_ref() {
            Some((_, title)) => title.clone(),
            None => format!("Voucher #{}", self.id.0),
        }
    }

    fn subtitle(&self) -> Option<String> {
        Some("Read-only. Esc goes back up".to_owned())
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
                Ok(view) => {
                    let title = voucher_title(&view);
                    let number = view.number.clone().unwrap_or_else(|| title.clone());
                    *self.shown.borrow_mut() = Some((number, title.clone()));
                    page_head(ui, &title, self.subtitle().as_deref(), |_| {});
                    Self::draw_view(ui, &view);
                }
                Err(e) => {
                    ui.label(e.to_string());
                }
            },
        }
        ScreenAction::Stay
    }
}
