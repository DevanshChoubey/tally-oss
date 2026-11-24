//! The bill-wise details of a payment or receipt line: the open bills of the party and the part of
//! the line each one settles (the popup of `design/mock-v2/index.html`, `payScreen`, lines
//! 1398-1403). It opens by itself when the ledger of the line tracks bills, as TallyPrime's
//! Bill-wise Details do (<https://help.tallysolutions.com/advances-received/>, read 2026-11-25).
//! The open amounts come from the engine; what is settled is typed, and starts as the line's
//! amount (or the bill's open amount when that is less) on the first open bill.

use egui::{Area, Frame, Margin, Order, RichText, Stroke, Ui, pos2};
use ledgerly_api::OpenBill;

use super::amount::eval_amount;
use super::display::format_amount;
use super::draft::BillDraft;
use super::draw::{Cx, FieldSpec, text_field};
use super::kind::EntryKind;
use super::state::{Field, VoucherEntryScreen};
use crate::screens::voucher_sheet::Look;
use crate::theme::{RADIUS_M, TEXT_BODY, TEXT_CAP, border_width, shadow_2};
use crate::ui::{bold_font, kind, para_text, tokens};

/// The width of the popup, as the mock's (`width:460px`).
const WIDTH: f32 = 460.0;

impl VoucherEntryScreen {
    /// Draws the bill-wise details when they are open. Does nothing otherwise.
    pub(super) fn draw_bill_panel(&mut self, ui: &mut Ui, base: egui::Id, cx: &mut Cx<'_>) {
        let Some(panel) = self.panel else {
            return;
        };
        let open: Vec<OpenBill> = self.open_bills().map_or_else(Vec::new, <[_]>::to_vec);
        let answered = self.open_bills().is_some();
        let (name, this) = self.panel_names(panel.row);
        let t = tokens(ui);
        let k = kind(ui);
        let at = pos2(
            ui.max_rect().right().min(ui.ctx().content_rect().right()) - 40.0 - WIDTH,
            ui.max_rect().top() + 190.0,
        );
        Area::new(base.with("bill-panel"))
            .order(Order::Foreground)
            .fade_in(false)
            .fixed_pos(at)
            .constrain(true)
            .show(ui.ctx(), |ui| {
                Frame::new()
                    .fill(t.surface)
                    .stroke(Stroke::new(border_width(k), t.border_strong))
                    .corner_radius(RADIUS_M)
                    .shadow(shadow_2(k))
                    .inner_margin(Margin::same(14))
                    .show(ui, |ui| {
                        ui.set_width(WIDTH - 28.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("Bill-wise details for {name}"))
                                    .font(bold_font(ui, TEXT_BODY + 1.0))
                                    .color(t.text),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new("Esc").font(bold_font(ui, TEXT_CAP)).color(t.text_3));
                            });
                        });
                        ui.label(para_text(
                            "Opened automatically because this party tracks bills. Enter accepts.",
                            TEXT_CAP,
                            t.text_3,
                        ));
                        ui.add_space(6.0);
                        if !answered {
                            ui.label(para_text("Reading the open bills.", TEXT_BODY, t.text_2));
                        } else if open.is_empty() {
                            ui.label(para_text(
                                "No bill is open for this party. Enter leaves the amount On Account.",
                                TEXT_BODY,
                                t.text_2,
                            ));
                        } else {
                            self.draw_bill_rows(ui, base, cx, panel.row, &open, this);
                        }
                    });
            });
    }

    /// The party's name and the word for what the line is ("payment" or "receipt").
    fn panel_names(&self, row: usize) -> (String, &'static str) {
        let name = self
            .draft
            .rows
            .get(row)
            .map_or_else(String::new, |r| r.ledger_text.clone());
        let this = match self.draft.kind {
            EntryKind::Receipt => "receipt",
            EntryKind::Payment
            | EntryKind::Contra
            | EntryKind::Journal
            | EntryKind::Memorandum
            | EntryKind::ReversingJournal => "payment",
        };
        (name, this)
    }

    /// The table of open bills: type, bill, date, open amount and the part settled, with a line
    /// under each saying what is left open on it.
    #[expect(
        clippy::too_many_lines,
        reason = "one table drawn row by row; splitting would only pass the same borrows around"
    )]
    fn draw_bill_rows(
        &mut self,
        ui: &mut Ui,
        base: egui::Id,
        cx: &mut Cx<'_>,
        row: usize,
        open: &[OpenBill],
        this: &str,
    ) {
        let t = tokens(ui);
        egui::Grid::new(base.with("bill-grid"))
            .num_columns(5)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                for head in ["Type", "Bill", "Date", "Open"] {
                    ui.label(
                        RichText::new(head)
                            .font(bold_font(ui, TEXT_CAP))
                            .color(t.text_3),
                    );
                }
                ui.label(
                    RichText::new(format!("This {this}"))
                        .font(bold_font(ui, TEXT_CAP))
                        .color(t.text_3),
                );
                ui.end_row();
                for (k, bill) in open.iter().enumerate() {
                    ui.label(para_text("Against", TEXT_BODY, t.text));
                    ui.label(para_text(&bill.name, TEXT_BODY, t.text));
                    ui.label(para_text(
                        &crate::ui::format::date(bill.date),
                        TEXT_BODY,
                        t.text_2,
                    ));
                    ui.label(para_text(
                        &format_amount(bill.open.amount),
                        TEXT_BODY,
                        t.text,
                    ));
                    let Some(r) = self.draft.rows.get_mut(row) else {
                        return;
                    };
                    let at = if let Some(at) = r.bills.iter().position(|b| b.name == bill.name) {
                        at
                    } else {
                        r.bills.push(BillDraft {
                            name: bill.name.clone(),
                            amount_text: String::new(),
                        });
                        r.bills.len().saturating_sub(1)
                    };
                    let spec = FieldSpec {
                        id: base.with(("bill-amount", k)),
                        field: Field::Bill(k),
                        name: "Settled against bill",
                        hint: "0.00",
                    };
                    if let Some(b) = r.bills.get_mut(at) {
                        text_field(
                            ui,
                            cx,
                            &spec,
                            &mut b.amount_text,
                            Look::Boxed,
                            true,
                            Some(110.0),
                        );
                    }
                    ui.end_row();
                    let left = self
                        .draft
                        .rows
                        .get(row)
                        .and_then(|r| r.bills.iter().find(|b| b.name == bill.name))
                        .and_then(|b| eval_amount(&b.amount_text).ok())
                        .and_then(|part| bill.open.amount.checked_sub(part).ok());
                    // The mock's note runs across the first four columns (colspan 4), so it is
                    // painted from the first cell without asking the column for its width.
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(1.0, crate::ui::line_height(TEXT_CAP)),
                        egui::Sense::hover(),
                    );
                    ui.painter().text(
                        rect.left_center(),
                        egui::Align2::LEFT_CENTER,
                        format!("Left open on {} after saving", bill.name),
                        egui::FontId::proportional(TEXT_CAP),
                        t.text_3,
                    );
                    ui.label("");
                    ui.label("");
                    ui.label("");
                    ui.label(para_text(
                        &left.map_or_else(String::new, format_amount),
                        TEXT_CAP,
                        t.text_3,
                    ));
                    ui.end_row();
                }
            });
    }
}
