//! The bank chooser: which bank account to reconcile, when the company has several.
//!
//! Behaviour: the reconciliation screens ask for the bank ledger and let the user pick another
//! one later (the help page "Bank Reconciliation reports", <https://help.tallysolutions.com/bank-recon-reports/>,
//! read 2026-11-25: a list of the company's banks, and F4 "Bank Ledger" to switch). The look is
//! ours: the page head, then one card with a line per bank in the reconciliation statement's
//! `table.t` style (`design/mock-v2/index.html` lines 148-158).
//!
//! Every balance is the engine's: each bank's "balance as per company books" from `Api::brs`
//! (rule C4, the screen adds up nothing). A bank the engine cannot answer for shows the reason,
//! never a zero.

use egui::{FontId, Frame, Rect, Sense, Stroke, Ui, pos2, vec2};
use ledgerly_core::{Date, LedgerId, Outcome, Paise};

use super::{PAGE_TITLE, format_line_date};
use crate::screen::KeyHint;
use crate::screens::wire::{Done, Latest, unsupported_text};
use crate::shell::navigate;
use crate::theme::{RADIUS_M, TEXT_BODY, TEXT_META, border_width, shadow_1};
use crate::ui::badge::{BadgeKind, paint_badge};
use crate::ui::format;
use crate::ui::note::{NoteKind, note};
use crate::ui::page_head::page_head;
use crate::ui::table::{self, CellPlace, RowState};
use crate::ui::{bold_font, kind, tokens};
use crate::{AppCtx, Screen, ScreenAction};

/// The page head's title (`design/mock-v2` bank-chooser).
const HEAD_TITLE: &str = "Choose a bank account";

/// The line under the card.
const FOOTNOTE: &str = "The balance is the engine's balance on this computer, as at the last voucher date. Import a statement before matching.";

/// One bank ledger the chooser offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankChoice {
    ledger: LedgerId,
    name: String,
    account: String,
}

impl BankChoice {
    /// A bank ledger `ledger` named `name`, whose account number is shown as `account` (already
    /// masked, such as `XXXX 4471`).
    #[must_use]
    pub fn new(ledger: LedgerId, name: &str, account: &str) -> Self {
        Self {
            ledger,
            name: name.to_owned(),
            account: account.to_owned(),
        }
    }
}

/// What a row's balance cell shows.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Balance {
    /// The engine has not answered yet.
    Loading,
    /// The engine's balance as per company books, debit-positive.
    Value(Paise),
    /// Why there is no figure: the engine's `Unsupported` reason or the worker's error.
    Missing(String),
}

/// One line of the chooser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChooserRow {
    ledger: LedgerId,
    name: String,
    account: String,
    balance: Balance,
    statement: Statement,
}

impl ChooserRow {
    /// The statement status of the account.
    #[must_use]
    pub fn statement(&self) -> &Statement {
        &self.statement
    }

    /// The bank ledger.
    #[must_use]
    pub fn ledger(&self) -> LedgerId {
        self.ledger
    }

    /// The ledger's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The balance as per company books, or `None` while loading or when the engine gave none.
    #[must_use]
    pub fn balance(&self) -> Option<Paise> {
        match self.balance {
            Balance::Value(p) => Some(p),
            Balance::Loading | Balance::Missing(_) => None,
        }
    }
}

/// Whether a bank account has a statement imported, as the engine holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    /// The engine has not answered yet.
    Loading,
    /// No statement is stored for the account.
    None,
    /// A statement is stored; its first and last line dates when it has lines.
    Imported(Option<(Date, Date)>),
    /// The stored statements could not be read, and why (never shown as "no statement").
    Unreadable(String),
}

type Answer = Vec<(LedgerId, Result<Outcome<Paise>, String>, Statement)>;

/// The bank chooser screen.
#[derive(Debug)]
pub struct BankChooserScreen {
    rows: Vec<ChooserRow>,
    as_of: Date,
    focus: usize,
    started: bool,
    load: Latest<Answer>,
    notice: Option<String>,
}

/// Where the columns sit in the card, as fractions of its width (`design/mock-v2` bank-chooser:
/// the number column starts 35% in, the balance ends 63% in, the statement starts 65% in).
const NUMBER_AT: f32 = 0.35;
/// Right edge of the balance column.
const BALANCE_TO: f32 = 0.633;
/// Left edge of the statement column.
const STATEMENT_AT: f32 = 0.654;

impl BankChooserScreen {
    /// A chooser over `choices`, in the given order, with each bank's books balance as of
    /// `as_of`. The row of `focus` is the one selected first, else the first row.
    #[must_use]
    pub fn new(choices: Vec<BankChoice>, as_of: Date, focus: Option<LedgerId>) -> Self {
        let rows: Vec<ChooserRow> = choices
            .into_iter()
            .map(|c| ChooserRow {
                ledger: c.ledger,
                name: c.name,
                account: c.account,
                balance: Balance::Loading,
                statement: Statement::Loading,
            })
            .collect();
        let focus = focus
            .and_then(|l| rows.iter().position(|r| r.ledger == l))
            .unwrap_or(0);
        Self {
            rows,
            as_of,
            focus,
            started: false,
            load: Latest::default(),
            notice: None,
        }
    }

    /// The lines, top to bottom.
    #[must_use]
    pub fn rows(&self) -> &[ChooserRow] {
        &self.rows
    }

    /// The bank ledger of the selected line.
    #[must_use]
    pub fn focused(&self) -> Option<LedgerId> {
        self.rows.get(self.focus).map(|r| r.ledger)
    }

    /// Whether the engine's balances are still on their way.
    #[must_use]
    pub fn is_loading(&self) -> bool {
        !self.started || self.load.is_pending()
    }

    /// Starts the balance request once, and takes its answer when it arrives.
    fn tick(&mut self, ctx: &AppCtx) {
        if let Some(worker) = ctx.api.as_ref() {
            if !self.started {
                self.started = true;
                let ledgers: Vec<LedgerId> = self.rows.iter().map(|r| r.ledger).collect();
                let as_of = self.as_of;
                self.load.start(worker, move |api| {
                    ledgers
                        .into_iter()
                        .map(|ledger| {
                            let answer = api
                                .brs(ledger, as_of)
                                .map(|o| o.map(|brs| brs.books_balance))
                                .map_err(|e| e.to_string());
                            let statements = match api.stored_bank_statements(ledger) {
                                Ok(statements) => statements,
                                Err(e) => {
                                    return (ledger, answer, Statement::Unreadable(e.to_string()));
                                }
                            };
                            let period = statements.iter().filter_map(|(_, s)| s.period()).fold(
                                None,
                                |acc: Option<(Date, Date)>, (a, b)| match acc {
                                    None => Some((a, b)),
                                    Some((from, to)) => Some((from.min(a), to.max(b))),
                                },
                            );
                            let statement = if statements.is_empty() {
                                Statement::None
                            } else {
                                Statement::Imported(period)
                            };
                            (ledger, answer, statement)
                        })
                        .collect()
                });
            }
        } else if !self.started {
            self.started = true;
            self.notice = Some("No company is open, so the balances cannot be read.".to_owned());
            for row in &mut self.rows {
                row.balance = Balance::Missing("no company is open".to_owned());
            }
        }
        match self.load.poll() {
            Some(Done::Ready(answers)) => self.apply(answers),
            Some(Done::Lost) => {
                self.notice = Some("The worker stopped before it gave the balances.".to_owned());
                for row in &mut self.rows {
                    row.balance = Balance::Missing("the worker stopped".to_owned());
                }
            }
            None => {}
        }
    }

    fn apply(&mut self, answers: Answer) {
        for (ledger, answer, statement) in answers {
            let Some(row) = self.rows.iter_mut().find(|r| r.ledger == ledger) else {
                continue;
            };
            row.statement = statement;
            row.balance = match answer {
                Ok(Outcome::Value(p)) => Balance::Value(p),
                Ok(Outcome::Unsupported(u)) => Balance::Missing(unsupported_text(&u)),
                Err(text) => Balance::Missing(text),
            };
        }
    }

    /// Opens the bank of line `index` in place of the chooser.
    fn open(&mut self, index: usize, ctx: &AppCtx) -> ScreenAction {
        let Some(row) = self.rows.get(index) else {
            return ScreenAction::Stay;
        };
        let Some(books) = ctx.books.as_ref() else {
            self.notice = Some("The company's books are not loaded yet.".to_owned());
            return ScreenAction::Stay;
        };
        match navigate::bank_screen(books, row.ledger) {
            Ok(screen) => ScreenAction::Replace(Box::new(screen)),
            Err(e) => {
                self.notice = Some(e.to_string());
                ScreenAction::Stay
            }
        }
    }

    fn draw_card(&self, ui: &mut Ui) -> Option<usize> {
        let t = tokens(ui);
        let k = kind(ui);
        let mut clicked = None;
        Frame::new()
            .fill(t.surface)
            .stroke(Stroke::new(border_width(k), t.border))
            .corner_radius(RADIUS_M)
            .shadow(shadow_1(k))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                let width = ui.available_width();
                let (head, _) =
                    ui.allocate_exact_size(vec2(width, table::T_HEAD_HEIGHT), Sense::hover());
                Self::paint_head(ui, head);
                for (i, row) in self.rows.iter().enumerate() {
                    let (rect, response) =
                        ui.allocate_exact_size(vec2(width, table::T_ROW_HEIGHT), Sense::click());
                    let state = RowState {
                        selected: i == self.focus,
                        hovered: response.hovered(),
                        sub: false,
                    };
                    paint_line(ui, rect, row, state);
                    let name = format!("{}, {}, {}", row.name, row.account, balance_text(row));
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::accesskit::Role::Row, true, &name)
                    });
                    if response.clicked() {
                        clicked = Some(i);
                    }
                }
            });
        clicked
    }

    fn paint_head(ui: &Ui, rect: Rect) {
        let t = tokens(ui);
        table::paint_head_rule(ui, rect);
        let font = bold_font(ui, TEXT_META);
        let cols = columns(rect);
        table::paint_cell_text(
            ui,
            rect,
            1.0,
            "Account",
            font.clone(),
            t.text_3,
            CellPlace::Left(table::indent(0)),
        );
        table::paint_cell_text(
            ui,
            cols.number,
            1.0,
            "Number",
            font.clone(),
            t.text_3,
            CellPlace::Left(0.0),
        );
        table::paint_cell_text(
            ui,
            cols.balance,
            1.0,
            "Balance per books",
            font.clone(),
            t.text_3,
            CellPlace::Right,
        );
        table::paint_cell_text(
            ui,
            cols.statement,
            1.0,
            "Statement",
            font,
            t.text_3,
            CellPlace::Left(0.0),
        );
    }
}

/// The three columns after the name, as rectangles of the line.
struct Columns {
    number: Rect,
    balance: Rect,
    statement: Rect,
}

fn columns(rect: Rect) -> Columns {
    let at = |f: f32| rect.left() + rect.width() * f;
    let band = |from: f32, to: f32| {
        Rect::from_min_max(pos2(at(from), rect.top()), pos2(at(to), rect.bottom()))
    };
    Columns {
        number: band(NUMBER_AT, BALANCE_TO - 0.15),
        balance: band(NUMBER_AT, BALANCE_TO),
        statement: band(STATEMENT_AT, 1.0),
    }
}

/// What the balance cell of `row` says.
fn balance_text(row: &ChooserRow) -> String {
    match &row.balance {
        Balance::Loading => "Loading".to_owned(),
        Balance::Value(p) => format::amount(*p),
        Balance::Missing(why) => why.clone(),
    }
}

/// The words after the badge in the statement column: the period, or that there is none.
#[must_use]
pub(super) fn statement_text(statement: &Statement) -> String {
    match statement {
        Statement::Loading => "Loading".to_owned(),
        Statement::None => "No statement imported".to_owned(),
        Statement::Imported(None) => "Yes".to_owned(),
        Statement::Imported(Some((from, to))) => {
            let from_text = if from.year() == to.year() {
                format!(
                    "{} {}",
                    from.day(),
                    crate::ui::format::month_abbrev(from.month())
                )
            } else {
                format_line_date(*from)
            };
            format!("Yes, {from_text} to {}", format_line_date(*to))
        }
        Statement::Unreadable(why) => format!("Could not be read: {why}"),
    }
}

/// One bank line: fill and rule, the name, the account number, the balance and the statement.
fn paint_line(ui: &Ui, rect: Rect, row: &ChooserRow, state: RowState) {
    let t = tokens(ui);
    table::paint_row(ui, rect, state);
    let font = FontId::proportional(TEXT_BODY);
    let cols = columns(rect);
    table::paint_cell_text(
        ui,
        rect,
        1.0,
        &row.name,
        font.clone(),
        t.text,
        CellPlace::Left(table::indent(0)),
    );
    table::paint_cell_text(
        ui,
        cols.number,
        1.0,
        &row.account,
        font.clone(),
        t.text_2,
        CellPlace::Left(0.0),
    );
    let colour = match row.balance {
        Balance::Value(_) => t.text,
        Balance::Loading | Balance::Missing(_) => t.text_3,
    };
    table::paint_cell_text(
        ui,
        cols.balance,
        1.0,
        &balance_text(row),
        font,
        colour,
        CellPlace::Right,
    );
    let y = rect.center().y;
    let mut x = cols.statement.left();
    let badge = match row.statement {
        Statement::Imported(_) => Some((BadgeKind::Ok, "Imported")),
        Statement::None => Some((BadgeKind::Neutral, "Not imported")),
        Statement::Unreadable(_) => Some((BadgeKind::Bad, "Unreadable")),
        Statement::Loading => None,
    };
    if let Some((kind_of, label)) = badge {
        x += paint_badge(ui, x, y, kind_of, label) + 6.0;
    }
    let meta = match &row.statement {
        Statement::Unreadable(why) => why.clone(),
        Statement::Imported(_) | Statement::None | Statement::Loading => {
            statement_text(&row.statement)
        }
    };
    table::paint_cell_text(
        ui,
        Rect::from_min_max(pos2(x, rect.top()), pos2(rect.right(), rect.bottom())),
        1.0,
        &meta,
        FontId::proportional(TEXT_META),
        t.text_3,
        CellPlace::Left(0.0),
    );
}

impl Screen for BankChooserScreen {
    fn cursor_label(&self, _ctx: &AppCtx) -> Option<String> {
        self.rows.get(self.focus).map(|r| r.name.clone())
    }

    fn title(&self) -> String {
        PAGE_TITLE.to_owned()
    }

    fn page_title(&self) -> String {
        HEAD_TITLE.to_owned()
    }

    fn key_bar_name(&self) -> String {
        "Choose account".to_owned()
    }

    fn subtitle(&self) -> Option<String> {
        Some(
            "Each account reconciles on its own statement. Enter opens the one selected."
                .to_owned(),
        )
    }

    fn keys(&self) -> Vec<KeyHint> {
        vec![
            KeyHint::new("J / K", "Move"),
            KeyHint::new("Enter", "Open reconciliation"),
            KeyHint::new("Esc", "Back"),
        ]
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        self.tick(ctx);
        if self.is_loading() {
            ui.request_repaint();
        }
        let (enter, down, up) = ui.input_mut(|i| {
            let none = egui::Modifiers::NONE;
            (
                i.consume_key(none, egui::Key::Enter),
                i.consume_key(none, egui::Key::J) | i.consume_key(none, egui::Key::ArrowDown),
                i.consume_key(none, egui::Key::K) | i.consume_key(none, egui::Key::ArrowUp),
            )
        });
        if down && self.focus + 1 < self.rows.len() {
            self.focus += 1;
        }
        if up {
            self.focus = self.focus.saturating_sub(1);
        }
        let sub = self.subtitle();
        page_head(ui, HEAD_TITLE, sub.as_deref(), |_| {});
        let clicked = self.draw_card(ui);
        let t = tokens(ui);
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(FOOTNOTE)
                .size(TEXT_META)
                .color(t.text_3),
        );
        if let Some(text) = self.notice.as_deref() {
            note(ui, NoteKind::Bad, text);
        }
        if let Some(i) = clicked {
            self.focus = i;
            return self.open(i, ctx);
        }
        if enter {
            return self.open(self.focus, ctx);
        }
        ScreenAction::Stay
    }
}
