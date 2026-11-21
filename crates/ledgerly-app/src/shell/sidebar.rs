//! The side bar: the groups of [`navigate::sidebar`] (208 px, or a 60 px rail with Ctrl+\ that
//! shows each entry's first letter, with the full name as hover text).
//!
//! Three things live here:
//!
//! * [`command_route`] says what a keymap command does in the shell: the shell runs it, a screen
//!   opens for it, or nothing does and it is hidden. The palette and the sidebar both use it, so a
//!   row that does nothing is never shown (rule B3).
//! * [`screen_for`] turns a target into the screen it opens. A sidebar click and a palette pick both
//!   go through it.
//! * [`selected_label`] names the entry for the screen on top of the stack, which is the one drawn
//!   as selected.

use egui::{Color32, Pos2, Rect, Ui, Vec2};
use ledgerly_core::{FinancialYear, Outcome};
use ledgerly_model::{BaseVoucherKind, Books};

use super::chrome::ChromeCmd;
use super::navigate::{self, NavError, SidebarEntry, SidebarGroup};
use crate::keymap::Action;
use crate::palette::PaletteTarget;
use crate::screens::item_invoice::ItemInvoiceScreen;
use crate::screens::voucher_entry::VoucherEntryScreen;
use crate::theme::{TEXT_CAP, Tokens};
use crate::ui::keycap;
use crate::{AppCtx, Screen};

/// Width of the sidebar, in points (`--navw` in the mock, 208 px).
pub const WIDTH: f32 = 208.0;
/// Width of the rail, in points (`.app.rail` in the mock, 60 px).
pub const RAIL_WIDTH: f32 = 60.0;
/// Height of one sidebar row, in points.
const ROW_HEIGHT: f32 = 32.0;

/// What a keymap command does in the shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRoute {
    /// The shell itself runs the command (tabs, Esc, the palette).
    Shell,
    /// A screen opens for the command (a new voucher, the keyboard map).
    Screen,
    /// Nothing opens for the command yet, so the palette and the sidebar do not show it. The keys
    /// that belong to a screen (Save, Print, Alter) stay on that screen.
    Hidden,
}

/// What each keymap command does (rule B3: a command with no route is hidden, not listed).
///
/// `Screen`: a new sales, payment, receipt, contra or journal voucher opens the voucher entry
/// screen (sales opens the item invoice when the company keeps inventory), and Help opens the
/// keyboard map. `Hidden`: a purchase voucher (no screen enters it yet), the period and company
/// changes (no screens), and every key that belongs to a screen.
#[must_use]
pub fn command_route(action: Action) -> CommandRoute {
    match action {
        Action::NewTab | Action::CloseTab | Action::Back | Action::OpenPalette | Action::SwitchPalette => {
            CommandRoute::Shell
        }
        Action::NewContra
        | Action::NewPayment
        | Action::NewReceipt
        | Action::NewJournal
        | Action::NewSales
        | Action::Help => CommandRoute::Screen,
        Action::NewPurchase
        | Action::ChangePeriod
        | Action::SwitchCompany
        | Action::CompanyFeatures
        | Action::Configure
        | Action::Save
        | Action::CreateMaster
        | Action::AlterMaster
        | Action::MoreDetails
        | Action::DeleteLine
        | Action::SwitchSide
        | Action::DeleteVoucher
        | Action::CancelVoucher
        | Action::DuplicateVoucher
        | Action::InsertVoucher
        | Action::Undo
        | Action::ToggleDetail
        | Action::Print
        | Action::Export
        | Action::Email => CommandRoute::Hidden,
    }
}

/// Whether a palette row or sidebar entry for `target` is shown: a command with no route is not.
#[must_use]
pub fn target_is_shown(target: &PaletteTarget) -> bool {
    match target {
        PaletteTarget::Command(action) => command_route(*action) != CommandRoute::Hidden,
        PaletteTarget::Report(_)
        | PaletteTarget::Master(_)
        | PaletteTarget::Voucher(_)
        | PaletteTarget::Setting(_)
        | PaletteTarget::AiIntent(_) => true,
    }
}

/// The sidebar's groups in order, with the entries that have no route left out and any group
/// that becomes empty dropped.
#[must_use]
pub fn visible_groups() -> Vec<SidebarGroup> {
    navigate::sidebar()
        .into_iter()
        .map(|mut group| {
            group.entries.retain(|entry| target_is_shown(&entry.target));
            group
        })
        .filter(|group| !group.entries.is_empty())
        .collect()
}

/// The screen a target opens, as a sidebar click and a palette pick both open it.
///
/// # Errors
/// As [`navigate::open_target`] for a report, master, voucher, setting or assistant request. A
/// command: [`NavError::NoCompany`] when a voucher needs books and none are loaded;
/// [`NavError::Unsupported`] with the reason when the company cannot enter that voucher;
/// [`NavError::NotNavigation`] for a command the shell runs itself or that has no screen.
pub fn screen_for(target: &PaletteTarget, ctx: &AppCtx) -> Result<Box<dyn Screen>, NavError> {
    match target {
        PaletteTarget::Command(action) => command_screen(*action, ctx),
        PaletteTarget::Report(_)
        | PaletteTarget::Master(_)
        | PaletteTarget::Voucher(_)
        | PaletteTarget::Setting(_)
        | PaletteTarget::AiIntent(_) => navigate::open_target(target, ctx),
    }
}

/// The screen a command opens; [`NavError::NotNavigation`] for a command with no screen.
fn command_screen(action: Action, ctx: &AppCtx) -> Result<Box<dyn Screen>, NavError> {
    match action {
        Action::NewSales => sales_screen(ctx),
        Action::NewContra => entry_screen(ctx, BaseVoucherKind::Contra),
        Action::NewPayment => entry_screen(ctx, BaseVoucherKind::Payment),
        Action::NewReceipt => entry_screen(ctx, BaseVoucherKind::Receipt),
        Action::NewJournal => entry_screen(ctx, BaseVoucherKind::Journal),
        Action::Help => navigate::open_target(&PaletteTarget::Report(navigate::KEYS_KEY.to_owned()), ctx),
        Action::NewTab
        | Action::CloseTab
        | Action::Back
        | Action::OpenPalette
        | Action::SwitchPalette
        | Action::NewPurchase
        | Action::ChangePeriod
        | Action::SwitchCompany
        | Action::CompanyFeatures
        | Action::Configure
        | Action::Save
        | Action::CreateMaster
        | Action::AlterMaster
        | Action::MoreDetails
        | Action::DeleteLine
        | Action::SwitchSide
        | Action::DeleteVoucher
        | Action::CancelVoucher
        | Action::DuplicateVoucher
        | Action::InsertVoucher
        | Action::Undo
        | Action::ToggleDetail
        | Action::Print
        | Action::Export
        | Action::Email => Err(NavError::NotNavigation),
    }
}

/// The books of the open company, or why there are none.
fn loaded(ctx: &AppCtx) -> Result<&Books, NavError> {
    ctx.books.as_ref().ok_or(NavError::NoCompany)
}

/// The financial year of the company.
fn year_of(books: &Books) -> FinancialYear {
    FinancialYear {
        start: books.company.fy_start,
    }
}

/// The first active voucher type of `kind`, by id.
fn type_of(
    books: &Books,
    kind: BaseVoucherKind,
) -> Result<ledgerly_core::VoucherTypeId, NavError> {
    books
        .masters
        .voucher_types
        .values()
        .find(|t| t.base == kind && t.active)
        .map(|t| t.id)
        .ok_or_else(|| NavError::Unsupported {
            what: format!("a new {kind:?} voucher"),
            reason: "the company has no active voucher type of this kind".to_owned(),
        })
}

/// Voucher entry for a new voucher of `kind`, dated today.
fn entry_screen(ctx: &AppCtx, kind: BaseVoucherKind) -> Result<Box<dyn Screen>, NavError> {
    let books = loaded(ctx)?;
    let id = type_of(books, kind)?;
    match VoucherEntryScreen::open(books, id, year_of(books), navigate::today()) {
        Ok(Outcome::Value(screen)) => Ok(Box::new(screen)),
        Ok(Outcome::Unsupported(u)) => Err(NavError::Unsupported {
            what: u.what,
            reason: u.reason,
        }),
        Err(e) => Err(NavError::Unsupported {
            what: format!("a new {kind:?} voucher"),
            reason: e.to_string(),
        }),
    }
}

/// A new sales invoice: the item invoice when the company keeps inventory, otherwise voucher entry.
fn sales_screen(ctx: &AppCtx) -> Result<Box<dyn Screen>, NavError> {
    let books = loaded(ctx)?;
    if !books.company.features.inventory {
        return entry_screen(ctx, BaseVoucherKind::Sales);
    }
    let id = type_of(books, BaseVoucherKind::Sales)?;
    match ItemInvoiceScreen::open(books, id, year_of(books), navigate::today()) {
        Ok(Outcome::Value(screen)) => Ok(Box::new(screen)),
        Ok(Outcome::Unsupported(u)) => Err(NavError::Unsupported {
            what: u.what,
            reason: u.reason,
        }),
        Err(e) => Err(NavError::Unsupported {
            what: "a new sales invoice".to_owned(),
            reason: e.to_string(),
        }),
    }
}

/// The sidebar entry that names the screen on top of a stack, by its title: the title equals the
/// entry's label, or starts with it and a colon (`Masters: Ledgers`, `Bank reconciliation: HDFC`),
/// ignoring case. `None` when no entry names it (the screen is not in the sidebar).
#[must_use]
pub fn selected_label(top_title: &str) -> Option<&'static str> {
    let top = top_title.to_lowercase();
    let groups = visible_groups();
    groups
        .iter()
        .flat_map(|group| group.entries.iter())
        .find(|entry| {
            let label = entry.label.to_lowercase();
            top == label || top.starts_with(&format!("{label}:"))
        })
        .map(|entry| entry.label)
}

/// Draws the sidebar and returns the click, if any: the clicked entry replaces the top screen.
pub(super) fn sidebar(
    ui: &mut Ui,
    rail: bool,
    selected: Option<&'static str>,
    t: &Tokens,
) -> Vec<ChromeCmd> {
    let mut cmds = Vec::new();
    for group in visible_groups() {
        if !rail {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(group.title)
                    .size(TEXT_CAP)
                    .color(t.text_3),
            );
        }
        for entry in &group.entries {
            let is_selected = selected == Some(entry.label);
            if let Some(cmd) = entry_row(ui, entry, rail, is_selected, t) {
                cmds.push(cmd);
            }
        }
    }
    cmds
}

/// One sidebar row. Selected rows use the accent-soft fill with accent ink; the key hint is a
/// keycap at the right edge, and the count (when there is one) a pill beside it.
fn entry_row(
    ui: &mut Ui,
    entry: &SidebarEntry,
    rail: bool,
    selected: bool,
    t: &Tokens,
) -> Option<ChromeCmd> {
    let (ink, fill) = if selected {
        (t.accent_ink, t.accent_soft)
    } else {
        (t.text_2, Color32::TRANSPARENT)
    };
    let text = if rail {
        entry.label.chars().take(1).collect::<String>()
    } else {
        entry.label.to_owned()
    };
    let width = ui.available_width();
    let response = ui
        .scope(|ui| {
            ui.visuals_mut().override_text_color = Some(ink);
            ui.add(
                egui::Button::new(text.as_str())
                    .fill(fill)
                    .min_size(Vec2::new(width, ROW_HEIGHT)),
            )
        })
        .inner;
    if !rail {
        if let Some(hint) = entry.hint {
            paint_at_right(ui, &response, hint, t.surface_2, t.border, t.text_2);
        }
        if let Some(count) = entry.count {
            paint_at_right(
                ui,
                &response,
                &count.to_string(),
                t.ai_soft,
                t.ai_soft,
                t.ai_ink,
            );
        }
    }
    let response = if rail {
        response.on_hover_text(entry.label)
    } else {
        response
    };
    if response.clicked() {
        Some(ChromeCmd::Replace(entry.target.clone()))
    } else {
        None
    }
}

/// Paints a keycap-sized box with `label` at the right edge of a sidebar row.
fn paint_at_right(
    ui: &Ui,
    row: &egui::Response,
    label: &str,
    fill: Color32,
    edge: Color32,
    ink: Color32,
) {
    let width = keycap::width(ui, label);
    let mid = row.rect.center().y;
    let half = keycap::KEYCAP_HEIGHT / 2.0;
    let right = row.rect.max.x - 8.0;
    let rect = Rect::from_min_max(
        Pos2::new(right - width, mid - half),
        Pos2::new(right, mid + half),
    );
    keycap::paint(ui.painter(), rect, label, fill, edge, ink);
}
