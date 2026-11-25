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

    /// The table of open bills drawn as the mock's `table.t` (index.html:1427-1429): a heading
    /// line with a rule under it (Open and This payment right-aligned), one selected line per bill
    /// (the soft fill, the accent bar at its left, a rule under it) and, under each, a quiet line
    /// "Left open on ... after saving" with the figure in the last column.
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
        use crate::ui::table::{
            CELL_PAD_X, RowState, T_HEAD_HEIGHT, T_ROW_HEIGHT, paint_head_rule, paint_row,
        };
        use egui::{Align, Align2, FontId, Layout, Rect, UiBuilder, vec2};
        let t = tokens(ui);
        let total = ui.available_width();
        // Type, Bill, Date, Open, This payment (the last two hold figures).
        let widths = [
            66.0_f32,
            82.0,
            96.0,
            84.0,
            total - 66.0 - 82.0 - 96.0 - 84.0,
        ];
        let x_of = |c: usize| widths.iter().take(c).sum::<f32>();
        let heads = ["Type", "Bill", "Date", "Open"];
        let this_head = format!("This {this}");
        let (head, _) = ui.allocate_exact_size(vec2(total, T_HEAD_HEIGHT), egui::Sense::hover());
        for c in 0..5 {
            let label = if c < 4 {
                heads.get(c).copied().unwrap_or("")
            } else {
                this_head.as_str()
            };
            let numeric = c >= 3;
            let (x, align) = if numeric {
                (
                    head.left() + x_of(c) + widths.get(c).copied().unwrap_or(0.0) - CELL_PAD_X,
                    Align2::RIGHT_CENTER,
                )
            } else {
                (head.left() + x_of(c) + CELL_PAD_X, Align2::LEFT_CENTER)
            };
            ui.painter().text(
                egui::pos2(x, head.center().y),
                align,
                label,
                bold_font(ui, TEXT_CAP),
                t.text_3,
            );
        }
        paint_head_rule(ui, head);
        for (k, bill) in open.iter().enumerate() {
            let (line, _) = ui.allocate_exact_size(vec2(total, T_ROW_HEIGHT), egui::Sense::hover());
            paint_row(
                ui,
                line,
                RowState {
                    selected: true,
                    hovered: false,
                    sub: false,
                },
            );
            let text_at = |ui: &Ui, c: usize, text: &str, ink: egui::Color32| {
                ui.painter().text(
                    egui::pos2(line.left() + x_of(c) + CELL_PAD_X, line.center().y),
                    Align2::LEFT_CENTER,
                    text,
                    FontId::proportional(TEXT_BODY),
                    ink,
                );
            };
            text_at(ui, 0, "Against", t.text);
            text_at(ui, 1, &bill.name, t.text);
            text_at(ui, 2, &crate::ui::format::date(bill.date), t.text_2);
            ui.painter().text(
                egui::pos2(line.left() + x_of(4) - CELL_PAD_X, line.center().y),
                Align2::RIGHT_CENTER,
                format_amount(bill.open.amount),
                FontId::proportional(TEXT_BODY),
                t.text,
            );
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
            let cell = Rect::from_min_size(
                egui::pos2(line.left() + x_of(4), line.top() + 2.0),
                vec2(widths[4] - 4.0, T_ROW_HEIGHT - 5.0),
            );
            if let Some(b) = r.bills.get_mut(at) {
                ui.scope_builder(
                    UiBuilder::new()
                        .max_rect(cell)
                        .layout(Layout::left_to_right(Align::Center)),
                    |ui| {
                        // The offered default and any amount the keyboard has left read with
                        // Indian grouping; what the user is typing stays as typed.
                        let plain = !b.amount_text.trim().is_empty()
                            && b.amount_text
                                .chars()
                                .all(|c| c.is_ascii_digit() || c == '.');
                        let canonical = |p: ledgerly_core::Paise| {
                            p.to_rupees().to_string() == b.amount_text.trim()
                        };
                        let shown = eval_amount(&b.amount_text)
                            .ok()
                            .filter(|p| plain && (cx.focus != spec.field || canonical(*p)));
                        if let Some(paise) = shown {
                            let mut grouped = format_amount(paise);
                            let r = text_field(
                                ui,
                                cx,
                                &spec,
                                &mut grouped,
                                Look::Cell,
                                true,
                                Some(cell.width()),
                            );
                            if r.changed() {
                                b.amount_text = grouped.chars().filter(|c| *c != ',').collect();
                            }
                        } else {
                            text_field(
                                ui,
                                cx,
                                &spec,
                                &mut b.amount_text,
                                Look::Cell,
                                true,
                                Some(cell.width()),
                            );
                        }
                    },
                );
            }
            let left = self
                .draft
                .rows
                .get(row)
                .and_then(|r| r.bills.iter().find(|b| b.name == bill.name))
                .and_then(|b| eval_amount(&b.amount_text).ok())
                .and_then(|part| bill.open.amount.checked_sub(part).ok());
            let (quiet, _) =
                ui.allocate_exact_size(vec2(total, T_ROW_HEIGHT), egui::Sense::hover());
            paint_row(
                ui,
                quiet,
                RowState {
                    selected: false,
                    hovered: false,
                    sub: false,
                },
            );
            ui.painter().text(
                egui::pos2(quiet.left() + CELL_PAD_X, quiet.center().y),
                Align2::LEFT_CENTER,
                format!("Left open on {} after saving", bill.name),
                FontId::proportional(TEXT_BODY),
                t.text_2,
            );
            ui.painter().text(
                egui::pos2(
                    quiet.left() + x_of(4) + widths[4] - CELL_PAD_X,
                    quiet.center().y,
                ),
                Align2::RIGHT_CENTER,
                left.map_or_else(String::new, format_amount),
                FontId::proportional(TEXT_BODY),
                t.text_2,
            );
        }
    }
}
