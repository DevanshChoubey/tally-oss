//! The Start screen: the companies opened lately, shown when the app starts and the list is not
//! empty (mock `SCREENS.start`, state `start.recent`). With no list the app starts on company
//! setup instead.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use egui::{Key, Modifiers, Rect, RichText, Sense, vec2};
use ledgerly_api::Api;

use crate::recent::{self, RecentCompany};
use crate::shell::navigate::{self, IMPORT_KEY};
use crate::shell::{GatewayScreen, KeyHint, hand_over, spawn_file_job};
use crate::theme::{RADIUS_S, TEXT_BODY, TEXT_H1, TEXT_META};
use crate::ui::button::{ButtonKind, button};
use crate::ui::icon::{self, Icon};
use crate::ui::{bold_font, card::card, para, tokens};
use crate::{AppCtx, Screen, ScreenAction};

/// What a press on the Start screen asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Want {
    /// Open the selected company.
    Open,
    /// Create a company.
    Create,
    /// Import from another app.
    Import,
    /// Pick a file.
    PickFile,
}

/// The Start screen.
pub struct StartScreen {
    dir: PathBuf,
    entries: Vec<RecentCompany>,
    selected: usize,
    picker: fn() -> Option<PathBuf>,
    job: Option<Receiver<Result<Api, String>>>,
    notice: Option<String>,
    opened: bool,
}

impl std::fmt::Debug for StartScreen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StartScreen")
            .field("entries", &self.entries.len())
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}

impl StartScreen {
    /// The screen for `entries`, read from the list in `dir`; `dir` is where an opened company is
    /// recorded again.
    #[must_use]
    pub fn new(dir: PathBuf, entries: Vec<RecentCompany>) -> Self {
        Self {
            dir,
            entries,
            selected: 0,
            picker: crate::screens::setup::default_file_dialog,
            job: None,
            notice: None,
            opened: false,
        }
    }

    /// The same screen with `picker` as the file dialog of "Open a file".
    #[must_use]
    pub fn with_picker(mut self, picker: fn() -> Option<PathBuf>) -> Self {
        self.picker = picker;
        self
    }

    /// The names listed, top first.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.entries.iter().map(|c| c.name.clone()).collect()
    }

    /// The row the keyboard is on.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Moves the selection by `delta` rows and stays inside the list.
    pub fn move_by(&mut self, delta: isize) {
        let last = self.entries.len().saturating_sub(1);
        self.selected = self.selected.saturating_add_signed(delta).min(last);
    }

    /// Starts opening the file at `path` in the background.
    fn open_path(&mut self, path: PathBuf) {
        if self.job.is_some() {
            return;
        }
        let dir = self.dir.clone();
        match spawn_file_job(move || recent::open_and_record(&path, Some(dir.as_path()))) {
            Ok(rx) => {
                self.job = Some(rx);
                self.notice = Some("Opening the company file...".to_owned());
            }
            Err(reason) => self.notice = Some(reason),
        }
    }

    /// Starts opening the selected company.
    pub fn open_selected(&mut self) {
        if let Some(entry) = self.entries.get(self.selected) {
            self.open_path(entry.path.clone());
        }
    }

    fn collect(&mut self, ctx: &mut AppCtx) {
        let Some(rx) = self.job.as_ref() else { return };
        let answer = match rx.try_recv() {
            Ok(answer) => answer,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                Err("The company file could not be opened; try again.".to_owned())
            }
        };
        self.job = None;
        match answer.and_then(|api| hand_over(ctx, api)) {
            Ok(()) => self.opened = true,
            Err(reason) => self.notice = Some(reason),
        }
    }

    fn take_keys(&mut self, ui: &egui::Ui) -> Option<Want> {
        ui.input_mut(|i| {
            if i.consume_key(Modifiers::COMMAND, Key::N) {
                return Some(Want::Create);
            }
            if i.consume_key(Modifiers::NONE, Key::Enter) {
                return Some(Want::Open);
            }
            if i.consume_key(Modifiers::NONE, Key::J)
                || i.consume_key(Modifiers::NONE, Key::ArrowDown)
            {
                self.move_by(1);
            }
            if i.consume_key(Modifiers::NONE, Key::K)
                || i.consume_key(Modifiers::NONE, Key::ArrowUp)
            {
                self.move_by(-1);
            }
            None
        })
    }

    /// The company rows. A double click asks to open the row.
    fn draw_rows(&mut self, ui: &mut egui::Ui) -> Option<Want> {
        let t = tokens(ui);
        let mut want = None;
        for (i, c) in self.entries.iter().enumerate() {
            let (rect, response) =
                ui.allocate_exact_size(vec2(ui.available_width(), 52.0), Sense::click());
            if i == self.selected {
                ui.painter().rect_filled(rect, RADIUS_S, t.accent_soft);
            }
            let left = rect.min + vec2(10.0, 6.0);
            ui.painter().text(
                left,
                egui::Align2::LEFT_TOP,
                &c.name,
                bold_font(ui, TEXT_BODY),
                t.text,
            );
            ui.painter().text(
                left + vec2(0.0, 20.0),
                egui::Align2::LEFT_TOP,
                &c.detail,
                egui::FontId::proportional(TEXT_META),
                t.text_3,
            );
            ui.painter().text(
                egui::pos2(rect.right() - 160.0, rect.top() + 6.0),
                egui::Align2::RIGHT_TOP,
                &c.financial_year,
                egui::FontId::proportional(TEXT_BODY),
                t.text_2,
            );
            ui.painter().text(
                egui::pos2(rect.right() - 10.0, rect.top() + 6.0),
                egui::Align2::RIGHT_TOP,
                &c.last_opened,
                egui::FontId::proportional(TEXT_BODY),
                t.text_2,
            );
            if response.clicked() {
                self.selected = i;
            }
            if response.double_clicked() {
                want = Some(Want::Open);
            }
        }
        want
    }

    fn draw(&mut self, ui: &mut egui::Ui) -> Option<Want> {
        let mut want = None;
        ui.add_space(40.0);
        let side = ((ui.available_width() - 760.0) / 2.0).max(0.0);
        ui.horizontal_top(|ui| {
            ui.add_space(side);
            ui.vertical(|ui| {
                ui.set_width(ui.available_width().min(760.0));
                card(ui, |ui| {
                    let t = tokens(ui);
                    draw_title_row(ui, &t);
                    ui.add_space(12.0);
                    ui.label(RichText::new("Recent companies").font(bold_font(ui, TEXT_BODY)));
                    ui.add_space(6.0);
                    let (head, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
                    for (x, align, text) in [
                        (head.left() + 10.0, egui::Align2::LEFT_CENTER, "Company"),
                        (
                            head.right() - 160.0,
                            egui::Align2::RIGHT_CENTER,
                            "Financial year",
                        ),
                        (
                            head.right() - 10.0,
                            egui::Align2::RIGHT_CENTER,
                            "Last opened",
                        ),
                    ] {
                        ui.painter().text(
                            egui::pos2(x, head.center().y),
                            align,
                            text,
                            bold_font(ui, TEXT_META),
                            t.text_3,
                        );
                    }
                    if let Some(w) = self.draw_rows(ui) {
                        want = Some(w);
                    }
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if button(ui, "Open selected", Some("Enter"), ButtonKind::Primary).clicked()
                        {
                            want = Some(Want::Open);
                        }
                        if button(ui, "Create company", Some("Ctrl+N"), ButtonKind::Secondary)
                            .clicked()
                        {
                            want = Some(Want::Create);
                        }
                        if button(ui, "Import from another app", None, ButtonKind::Secondary)
                            .clicked()
                        {
                            want = Some(Want::Import);
                        }
                        if button(ui, "Open a file", None, ButtonKind::Secondary).clicked() {
                            want = Some(Want::PickFile);
                        }
                    });
                    if let Some(text) = &self.notice {
                        ui.add_space(8.0);
                        para(ui, text, TEXT_META, t.text_3);
                    }
                });
            });
        });
        want
    }
}

impl Screen for StartScreen {
    fn title(&self) -> String {
        "Start".to_owned()
    }

    fn keys(&self) -> Vec<KeyHint> {
        vec![
            KeyHint::new("", "Start"),
            KeyHint::new("J / K", "Move"),
            KeyHint::new("Enter", "Open"),
            KeyHint::new("Ctrl+N", "Create"),
            KeyHint::new("Esc", "Quit"),
        ]
    }

    fn cursor_label(&self, _ctx: &AppCtx) -> Option<String> {
        self.entries.get(self.selected).map(|c| c.name.clone())
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut AppCtx) -> ScreenAction {
        self.collect(ctx);
        if std::mem::take(&mut self.opened) {
            return ScreenAction::Replace(Box::new(GatewayScreen::new()));
        }
        if self.job.is_some() {
            ui.ctx().request_repaint();
        }
        let keyed = self.take_keys(ui);
        let drawn = self.draw(ui);
        match keyed.or(drawn) {
            None => ScreenAction::Stay,
            Some(Want::Open) => {
                self.open_selected();
                ScreenAction::Stay
            }
            Some(Want::Create) => ScreenAction::Replace(Box::new(
                crate::screens::setup::SetupScreen::default().with_recent_dir(self.dir.clone()),
            )),
            Some(Want::Import) => match navigate::screen_for_key(IMPORT_KEY, ctx) {
                Some(Ok(screen)) => ScreenAction::Push(screen),
                Some(Err(e)) => {
                    self.notice = Some(e.to_string());
                    ScreenAction::Stay
                }
                None => {
                    self.notice = Some("that screen is not available yet".to_owned());
                    ScreenAction::Stay
                }
            },
            Some(Want::PickFile) => {
                if let Some(path) = (self.picker)() {
                    self.open_path(path);
                }
                ScreenAction::Stay
            }
        }
    }
}

/// The mock's `.spread` row: the name at the left, the meta text and a check at the right, painted
/// in place on one row.
fn draw_title_row(ui: &mut egui::Ui, t: &crate::theme::Tokens) {
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::hover());
    ui.painter().text(
        row.left_center(),
        egui::Align2::LEFT_CENTER,
        "Ledgerly",
        bold_font(ui, TEXT_H1),
        t.text,
    );
    let check = Rect::from_center_size(
        egui::pos2(row.right() - 7.0, row.center().y),
        vec2(14.0, 14.0),
    );
    icon::paint(ui.painter(), check, Icon::Check, t.text_3);
    ui.painter().text(
        egui::pos2(check.left() - 6.0, row.center().y),
        egui::Align2::RIGHT_CENTER,
        "Saved on this computer",
        egui::FontId::proportional(TEXT_META),
        t.text_3,
    );
}
