//! The side card of the ledger drill (`design/mock-v2/index.html` `balance-sheet.ledger`, lines
//! 1618-1622): the ledger's name, its group, GSTIN and credit days from the masters, the open
//! bills the engine's outstanding report lists, and the "Alter ledger" button.
//!
//! Nothing is worked out here. The group, GSTIN and credit days are fields of the ledger master
//! copied as stored; the open bills are the engine's ([`PartyBills`]), asked on the worker; a
//! field the master does not hold shows "None" and a bill list the engine could not give shows
//! why.

use egui::{Align, Layout, RichText, Ui, vec2};
use ledgerly_core::{LedgerId, Period};
use ledgerly_model::Books;

use crate::screens::masters::bills::PartyBills;
use crate::screens::masters::open_bills_body;
use crate::theme::{SPACE_2, TEXT_BODY, TEXT_H2, TEXT_META};
use crate::ui::button::{ButtonKind, button};
use crate::ui::card::card;
use crate::ui::table::{self, CellPlace, RowState, T_ROW_HEIGHT};
use crate::ui::{bold_font, tokens};
use crate::worker::ApiWorker;

/// What the card says for a field the master does not hold.
const NONE: &str = "None";

/// The master fields the card shows, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SideInfo {
    /// The ledger.
    pub ledger: LedgerId,
    /// The ledger's name.
    pub name: String,
    /// The name of its group.
    pub group: String,
    /// Its GSTIN, when the master holds one.
    pub gstin: Option<String>,
    /// Its default credit days, when the master holds them.
    pub credit_days: Option<u32>,
}

/// The card's fields for `ledger`, or `None` when the books do not hold it or its group.
pub(super) fn side_info(books: &Books, ledger: LedgerId) -> Option<SideInfo> {
    let l = books.masters.ledgers.get(&ledger)?;
    let group = books.masters.groups.get(&l.group?)?;
    Some(SideInfo {
        ledger,
        name: l.name.clone(),
        group: group.name.clone(),
        gstin: l
            .party
            .as_ref()
            .and_then(|p| p.gstin.as_ref())
            .map(|g| g.as_str().to_owned()),
        credit_days: l.bill_wise.as_ref().and_then(|b| b.default_credit_days),
    })
}

/// The card's state: the open bills and which ledger and period they were asked for.
#[derive(Debug, Default)]
pub(super) struct LedgerSide {
    info: Option<SideInfo>,
    bills: PartyBills,
    asked: Option<(LedgerId, Period)>,
}

impl LedgerSide {
    /// Sets the card for `info` (or none) and asks for the bills when the ledger or period moved.
    pub(super) fn follow(&mut self, info: Option<SideInfo>, period: Period) {
        match &info {
            Some(i) => {
                if self.asked != Some((i.ledger, period)) {
                    self.bills.request(i.ledger, period);
                    self.asked = Some((i.ledger, period));
                }
            }
            None => self.asked = None,
        }
        self.info = info;
    }

    /// Takes a finished answer and sends a waiting request.
    pub(super) fn tick(&mut self, worker: Option<&ApiWorker>) {
        self.bills.tick(worker);
    }

    /// The ledger the card is for.
    pub(super) fn ledger(&self) -> Option<LedgerId> {
        self.info.as_ref().map(|i| i.ledger)
    }

    /// Draws the card; true when Alter ledger was clicked.
    pub(super) fn draw(&self, ui: &mut Ui) -> bool {
        let Some(info) = &self.info else {
            return false;
        };
        let mut alter = false;
        card(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let t = tokens(ui);
            let h2 = bold_font(ui, TEXT_H2);
            ui.label(RichText::new(&info.name).font(h2).color(t.text));
            ui.add_space(SPACE_2);
            let days = info.credit_days.map(|d| d.to_string());
            prop_row(ui, "Group", &info.group);
            prop_row(ui, "GSTIN", info.gstin.as_deref().unwrap_or(NONE));
            prop_row(ui, "Credit days", days.as_deref().unwrap_or(NONE));
            ui.add_space(SPACE_2 * 2.0);
            let h3 = bold_font(ui, TEXT_BODY);
            ui.label(RichText::new("Open bills").font(h3).color(t.text));
            ui.add_space(SPACE_2);
            open_bills_body(ui, &self.bills);
            ui.add_space(SPACE_2 * 2.0);
            ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                alter = button(
                    ui,
                    "Alter ledger",
                    Some("Ctrl+Enter"),
                    ButtonKind::Secondary,
                )
                .clicked();
            });
        });
        alter
    }
}

/// One `label  value` line in the `table.t` rules.
fn prop_row(ui: &mut Ui, label: &str, value: &str) {
    let t = tokens(ui);
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, T_ROW_HEIGHT), egui::Sense::hover());
    table::paint_row(
        ui,
        rect,
        RowState {
            selected: false,
            hovered: false,
            sub: false,
        },
    );
    let left = egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + 110.0, rect.bottom()));
    let right = egui::Rect::from_min_max(egui::pos2(rect.left() + 110.0, rect.top()), rect.max);
    table::paint_cell_text(
        ui,
        left,
        1.0,
        label,
        egui::FontId::proportional(TEXT_META + 1.0),
        t.text_2,
        CellPlace::Left(table::CELL_PAD_X),
    );
    table::paint_cell_text(
        ui,
        right,
        1.0,
        value,
        egui::FontId::proportional(TEXT_BODY),
        t.text,
        CellPlace::Left(table::CELL_PAD_X),
    );
}
