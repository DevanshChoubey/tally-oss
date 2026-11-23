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
use crate::screens::voucher_entry::VoucherEntryScreen;
use crate::theme::{RADIUS_S, TEXT_BODY, TEXT_CAP, Tokens};
use crate::ui::icon::{self, ICON_SIZE, Icon};
use crate::ui::{bold_font, keycap};
use crate::{AppCtx, Screen};

/// Width of the sidebar, in points (`--navw` in the mock, 208 px).
pub const WIDTH: f32 = 208.0;
/// Width of the rail, in points (`.app.rail` in the mock, 60 px).
pub const RAIL_WIDTH: f32 = 60.0;
/// Height of one sidebar row, in points.
const ROW_HEIGHT: f32 = 32.0;
/// Side padding inside a row (`padding:0 10px`).
const ROW_PAD_X: f32 = 10.0;
/// Gap between a row's icon, label and keys (`gap:10px`).
const ROW_GAP: f32 = 10.0;

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
/// `Screen`: a new payment, receipt, contra or journal voucher opens the voucher entry screen; a
/// new sales, purchase, credit note or debit note opens the invoice screen (item mode when the
/// company keeps inventory, accounting mode otherwise); Help opens the keyboard map. `Hidden`:
/// the period and company changes (no screens), and every key that belongs to a screen.
#[must_use]
pub fn command_route(action: Action) -> CommandRoute {
    match action {
        Action::NewTab
        | Action::CloseTab
        | Action::Back
        | Action::OpenPalette
        | Action::SwitchPalette => CommandRoute::Shell,
        Action::NewContra
        | Action::NewPayment
        | Action::NewReceipt
        | Action::NewJournal
        | Action::NewSales
        | Action::NewPurchase
        | Action::NewCreditNote
        | Action::NewDebitNote
        | Action::Help => CommandRoute::Screen,
        Action::ChangePeriod
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
        | PaletteTarget::AiIntent(_)
        | PaletteTarget::Fill(_) => true,
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
        | PaletteTarget::AiIntent(_)
        | PaletteTarget::Fill(_) => navigate::open_target(target, ctx),
    }
}

/// The screen a command opens; [`NavError::NotNavigation`] for a command with no screen.
fn command_screen(action: Action, ctx: &AppCtx) -> Result<Box<dyn Screen>, NavError> {
    match action {
        Action::NewSales => navigate::sales_screen(loaded(ctx)?, navigate::today()),
        Action::NewPurchase => invoice(ctx, BaseVoucherKind::Purchase),
        Action::NewCreditNote => invoice(ctx, BaseVoucherKind::CreditNote),
        Action::NewDebitNote => invoice(ctx, BaseVoucherKind::DebitNote),
        Action::NewContra => entry_screen(ctx, BaseVoucherKind::Contra),
        Action::NewPayment => entry_screen(ctx, BaseVoucherKind::Payment),
        Action::NewReceipt => entry_screen(ctx, BaseVoucherKind::Receipt),
        Action::NewJournal => entry_screen(ctx, BaseVoucherKind::Journal),
        Action::Help => {
            navigate::open_target(&PaletteTarget::Report(navigate::KEYS_KEY.to_owned()), ctx)
        }
        Action::NewTab
        | Action::CloseTab
        | Action::Back
        | Action::OpenPalette
        | Action::SwitchPalette
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
fn type_of(books: &Books, kind: BaseVoucherKind) -> Result<ledgerly_core::VoucherTypeId, NavError> {
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
/// The invoice screen for a new voucher of `kind`.
fn invoice(ctx: &AppCtx, kind: BaseVoucherKind) -> Result<Box<dyn Screen>, NavError> {
    navigate::invoice_screen(loaded(ctx)?, kind, navigate::today())
}

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

/// The sidebar entry that names the screen on top of a stack, by its title: the title equals the
/// entry's label, starts with it and a colon (`Masters: Ledgers`, `Bank reconciliation: HDFC`),
/// or is the label and " invoice" (`Sales invoice`: the invoice screen of the Sales row),
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
            top == label
                || top.starts_with(&format!("{label}:"))
                || top == format!("{label} invoice")
        })
        .map(|entry| entry.label)
}

/// The icon of a sidebar entry, by its label (`NAV` in the mock). `None` for a label the mock has
/// no icon for.
#[must_use]
pub fn entry_icon(label: &str) -> Option<Icon> {
    let icon = match label {
        "Home" => Icon::Home,
        "Review queue" => Icon::Inbox,
        "Sales" => Icon::Sale,
        "Purchase" => Icon::Purchase,
        "Payment" => Icon::Pay,
        "Receipt" => Icon::Recv,
        "Plain-words entry" => Icon::Chat,
        "Day book" => Icon::Book,
        "Balance sheet" => Icon::Scale,
        "Masters" => Icon::Users,
        "GST returns" => Icon::Gst,
        "Bank reconciliation" => Icon::Bank,
        "AI and privacy" => Icon::Shield,
        "Keyboard map" => Icon::Kbd,
        "Company setup" => Icon::Building,
        _ => return None,
    };
    Some(icon)
}

/// Draws the sidebar and returns the click, if any: the clicked entry replaces the top screen.
/// The footer (`nav.side .foot`) sits at the bottom.
pub(super) fn sidebar(
    ui: &mut Ui,
    rail: bool,
    selected: Option<&'static str>,
    t: &Tokens,
) -> Vec<ChromeCmd> {
    let mut cmds = Vec::new();
    ui.spacing_mut().item_spacing.y = 1.0;
    for group in visible_groups() {
        if !rail {
            group_heading(ui, group.title, t);
        }
        for entry in &group.entries {
            let is_selected = selected == Some(entry.label);
            if let Some(cmd) = entry_row(ui, entry, rail, is_selected, t) {
                cmds.push(cmd);
            }
        }
    }
    if !rail {
        footer(ui, t);
    }
    cmds
}

/// A group heading (`nav.side .grp`): caption size, weight 600, `text-3`, padded 12 10 4.
#[allow(clippy::float_arithmetic, reason = "padding")]
fn group_heading(ui: &mut Ui, title: &str, t: &Tokens) {
    let font = bold_font(ui, TEXT_CAP);
    let galley = ui
        .painter()
        .layout_no_wrap(title.to_owned(), font, t.text_3);
    let height = 12.0 + galley.size().y + 4.0;
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        egui::Sense::hover(),
    );
    ui.painter().galley(
        Pos2::new(rect.min.x + ROW_PAD_X, rect.min.y + 12.0),
        galley,
        t.text_3,
    );
}

/// The footer at the bottom of the sidebar (`nav.side .foot`): the app's version, and where the
/// company file is kept.
fn footer(ui: &mut Ui, t: &Tokens) {
    let text = format!(
        "Ledgerly {}\nSaved on this computer",
        env!("CARGO_PKG_VERSION")
    );
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(ROW_PAD_X);
            ui.label(egui::RichText::new(text).size(TEXT_CAP).color(t.text_3));
        });
    });
}

/// One sidebar row (`nav.side button`): 32 points high, the icon, the label, then the count and
/// the keycap at the right. The selected row has the accent-soft fill, accent ink and weight
/// 600; a hovered row the hover fill and text ink.
#[allow(clippy::float_arithmetic, reason = "laying out the row")]
fn entry_row(
    ui: &mut Ui,
    entry: &SidebarEntry,
    rail: bool,
    selected: bool,
    t: &Tokens,
) -> Option<ChromeCmd> {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), ROW_HEIGHT),
        egui::Sense::click(),
    );
    let hovered = response.hovered();
    let (ink, fill) = if selected {
        (t.accent_ink, t.accent_soft)
    } else if hovered {
        (t.text, t.hover)
    } else {
        (t.text_2, Color32::TRANSPARENT)
    };
    let painter = ui.painter();
    painter.rect_filled(rect, RADIUS_S, fill);
    let mid = rect.center().y;
    let icon_x = if rail {
        rect.center().x - ICON_SIZE / 2.0
    } else {
        rect.min.x + ROW_PAD_X
    };
    if let Some(which) = entry_icon(entry.label) {
        let r = Rect::from_min_size(
            Pos2::new(icon_x, mid - ICON_SIZE / 2.0),
            Vec2::splat(ICON_SIZE),
        );
        icon::paint(painter, r, which, ink);
    }
    if !rail {
        let font = if selected {
            bold_font(ui, TEXT_BODY)
        } else {
            egui::FontId::proportional(TEXT_BODY)
        };
        painter.text(
            Pos2::new(icon_x + ICON_SIZE + ROW_GAP, mid),
            egui::Align2::LEFT_CENTER,
            entry.label,
            font,
            ink,
        );
        let mut right = rect.max.x - ROW_PAD_X;
        if let Some(hint) = entry.hint {
            // `nav.side button kbd{background:transparent}`.
            right = paint_at_right(
                ui,
                right,
                mid,
                hint,
                Color32::TRANSPARENT,
                t.border,
                t.text_2,
            );
            right -= ROW_GAP;
        }
        if let Some(count) = entry.count {
            paint_count(ui, right, mid, count, t);
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

/// Paints a keycap with `label` ending at `right`, centred on `mid`. Returns its left edge.
#[allow(clippy::float_arithmetic, reason = "placing the keycap")]
fn paint_at_right(
    ui: &Ui,
    right: f32,
    mid: f32,
    label: &str,
    fill: Color32,
    edge: Color32,
    ink: Color32,
) -> f32 {
    let width = keycap::width(ui, label);
    let half = keycap::KEYCAP_HEIGHT / 2.0;
    let rect = Rect::from_min_max(
        Pos2::new(right - width, mid - half),
        Pos2::new(right, mid + half),
    );
    keycap::paint(ui.painter(), rect, label, fill, edge, ink);
    rect.min.x
}

/// Paints the count pill (`nav.side .count`: caption size, weight 700, `ai-soft` fill, `ai-ink`
/// text, 18 points high, 6 points of side padding) ending at `right`.
#[allow(clippy::float_arithmetic, reason = "placing the pill")]
fn paint_count(ui: &Ui, right: f32, mid: f32, count: u64, t: &Tokens) {
    let galley = ui
        .painter()
        .layout_no_wrap(count.to_string(), bold_font(ui, TEXT_CAP), t.ai_ink);
    let width = galley.size().x + 12.0;
    let rect = Rect::from_min_max(
        Pos2::new(right - width, mid - 9.0),
        Pos2::new(right, mid + 9.0),
    );
    ui.painter().rect_filled(rect, 9.0, t.ai_soft);
    let size = galley.size();
    ui.painter()
        .galley(rect.center() - size / 2.0, galley, t.ai_ink);
}
