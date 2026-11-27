//! Purchase from a bill photo with the assistant (round s21, plan round D; mock states
//! `purchase-bill.choose`, `.reading`, `.ledger-lines`, `.rejected`, `.error` and `.accepted`).
//!
//! The person types the path of a PNG bill photo. The file is read on the worker: a file that is
//! not a PNG is refused with what failed, before anything reaches the assistant. The bytes go
//! through the engine's one gate (`Api::read_bill_photo`); the provider returns the bill's text
//! lines and the bill reader drafts a purchase, which is stored and waits in the review queue. The
//! card shows the printed figures as read, each labelled "from bill", with a box on the image for
//! every value read. The taxable value names no purchase ledger, so the person chooses one; the
//! engine checks the draft after every choice and its verdict is the check line. The engine total
//! is the party line of the engine's voucher and is shown only when the engine made one; the
//! screen computes no figure. Ctrl+A accepts through the engine, Alt+D rejects; both are logged.

use std::path::PathBuf;
use std::sync::Arc;

use egui::{
    Align2, Color32, ColorImage, FontId, Frame, Key, Margin, Modifiers, Rect, RichText, Stroke,
    StrokeKind, TextEdit, TextureHandle, TextureOptions, Ui, vec2,
};
use ledgerly_api::{
    Accepted, AiGate, Api, BillRead, DraftCheck, DraftEdits, DraftField, Proposal, ProviderPlace,
    SourceRef, VoucherDraft, needs_review,
};
use ledgerly_core::{CancelToken, Date, DrCr, EventSeq, LedgerId, Outcome, Paise};

use crate::i18n::lit;
use crate::keymap::Action;
use crate::screen::KeyHint;
use crate::screens::ai_gate;
use crate::screens::wire::{Done, Latest, unsupported_text};
use crate::theme::{RADIUS_M, TEXT_BODY, TEXT_H2, TEXT_META, border_width};
use crate::ui::button::{ButtonKind, button};
use crate::ui::card::card;
use crate::ui::conf::{Confidence, conf};
use crate::ui::icon::{Icon, icon, icon_sized};
use crate::ui::note::{NoteKind, note};
use crate::ui::prop::{Prop, PropAction, PropState};
use crate::ui::prov::{ProvKind, prov};
use crate::ui::skeleton::skeleton_rows;
use crate::ui::{bold_font, format, kind, para_text, tokens};
use crate::worker::ApiWorker;
use crate::{AppCtx, ScreenAction};

/// The image column's width inside its padding (400 points less 2 x 18, as the screen without
/// the assistant).
const IMAGE_WIDTH: f32 = 364.0;
/// The image column's least height inside its padding.
const IMAGE_HEIGHT: f32 = 484.0;
/// The gap between the columns.
const COLUMN_GAP: f32 = 16.0;
/// The eight bytes every PNG file starts with.
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// One draft line as the card shows it: the printed amount and the ledger, or the choice to make.
#[derive(Debug, Clone)]
struct BillLine {
    /// Dr or Cr.
    side: DrCr,
    /// The ledger's name, when the reader matched one or the person chose one.
    ledger: Option<String>,
    /// The words the line was read from.
    read: String,
    /// The amount as printed on the bill.
    amount: Paise,
    /// Whether the reader left the line's ledger open, so the person chooses it.
    open: bool,
}

/// What the engine said of the draft with the person's choices.
#[derive(Debug, Clone)]
struct Checked {
    /// The draft lines with the chosen ledgers.
    lines: Vec<BillLine>,
    /// The party line of the engine's voucher, when the engine made one.
    engine_total: Option<Paise>,
    /// `Ok(())`, or why the engine does not make the voucher.
    verdict: Result<(), String>,
    /// The voucher's date and the lock date in force.
    dates: (Option<Date>, Option<Date>),
}

/// A read bill as the screen shows it.
#[derive(Debug, Clone)]
struct BillView {
    /// The stored draft.
    id: EventSeq,
    /// The draft as read, kept to check it again with the person's choices.
    draft: Box<VoucherDraft>,
    /// Where the provider runs.
    place: ProviderPlace,
    /// The reader's lowest confidence, per mille.
    confidence: u16,
    /// The ledgers the person chose, by draft line.
    chosen: Vec<(usize, LedgerId)>,
    /// The fields of the draft as read that need review (read by the OCR reader).
    review: Vec<DraftField>,
    /// The fields the person confirmed against the photo.
    confirmed: Vec<DraftField>,
    /// Every ledger of the company, by name, for the open lines' lists.
    choices: Vec<(LedgerId, String)>,
    /// The engine's word on the draft with `chosen`.
    checked: Checked,
}

/// The photo, decoded on the worker, and the boxes of the values read from it.
#[derive(Debug, Clone)]
struct Photo {
    /// The file name, as the AI log keeps it.
    file: String,
    /// The decoded image.
    image: Arc<ColorImage>,
    /// A label and a box `[x, y, w, h]` in image pixels for each value read.
    boxes: Vec<(String, [u32; 4])>,
}

/// What the person did on the card: chose a ledger for an open line, or confirmed a value.
#[derive(Debug, Clone, Copy)]
enum Pick {
    /// A ledger for the draft line at this index.
    Ledger(usize, LedgerId),
    /// A value read from the bill, checked against the photo.
    Confirm(DraftField),
}

/// What a read gave the screen.
#[derive(Debug)]
enum ReadDone {
    /// A stored draft and the engine's check of it.
    Drafted(Box<Photo>, Box<BillView>),
    /// The gate refused.
    Refused(AiGate),
    /// The provider or the reader could not read the photo.
    Failed(String, String),
    /// The file is not one this build reads; nothing reached the assistant.
    FileError(String, String),
    /// The person cancelled before the provider ran.
    Cancelled,
}

/// The worker's answers.
#[derive(Debug)]
enum Reply {
    /// The gate for bill reading.
    Gate(Result<AiGate, String>),
    /// The read of a file.
    Read(Result<ReadDone, String>),
    /// The engine's check after the person chose a ledger.
    Checked(Result<Checked, String>),
    /// The accept: the posted voucher's number, or why nothing was posted.
    Accepted(Result<Result<String, String>, String>),
    /// The reject was recorded.
    Rejected(Result<(), String>),
}

/// Where the screen is.
#[derive(Debug, Clone)]
enum Phase {
    /// Waiting for a file path.
    Choose,
    /// The file is with the assistant.
    Reading(String),
    /// A draft waits for the person.
    Draft(Box<Photo>, Box<BillView>),
    /// Accepted; the engine posted the voucher with this number.
    Posted(Box<Photo>, Box<BillView>, String),
    /// Rejected; nothing was posted.
    Rejected(Box<Photo>),
    /// The file could not be read (the name, and what failed).
    Failed(String, String),
}

/// The assistant half of the Purchase from bill photo screen.
#[derive(Debug)]
pub(crate) struct AiBill {
    /// The gate, once read.
    gate: Option<Result<AiGate, String>>,
    /// The path being typed.
    path: String,
    /// Where the screen is.
    phase: Phase,
    /// A line for the person about the last action.
    notice: Option<String>,
    /// The one call in flight.
    latest: Latest<Reply>,
    /// The token of the read in flight; Esc cancels it.
    cancel: Option<CancelToken>,
    /// Whether the person cancelled the read in flight.
    cancelled: bool,
    /// Whether a choice of ledger waits for the engine's check.
    recheck: bool,
    /// Whether the path field took focus once it could be typed in.
    focused_once: bool,
    /// The photo's texture, once uploaded.
    texture: Option<Texture>,
}

/// The photo's texture (egui's handle has no `Debug`).
#[derive(Clone)]
struct Texture(TextureHandle);

impl std::fmt::Debug for Texture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Texture").field(&self.0.id()).finish()
    }
}

impl Default for AiBill {
    fn default() -> Self {
        Self {
            gate: None,
            path: String::new(),
            phase: Phase::Choose,
            notice: None,
            latest: Latest::default(),
            cancel: None,
            cancelled: false,
            recheck: false,
            focused_once: false,
            texture: None,
        }
    }
}

/// What the screen asked the stack for.
enum Ask {
    /// Nothing.
    Nothing,
    /// Open the purchase form.
    Purchase,
    /// Open AI and privacy.
    Settings,
    /// Read another bill.
    Another,
    /// Read the typed path.
    Read,
}

impl AiBill {
    /// Whether the gate has been read.
    pub(crate) fn loaded(&self) -> bool {
        self.gate.is_some()
    }

    /// The key bar's name for where the screen is (its hint with no key).
    pub(crate) fn key_bar_name(&self) -> String {
        self.keys()
            .into_iter()
            .find(|hint| hint.keys.is_empty())
            .map_or_default(|hint| hint.label)
    }

    /// The key bar for where the screen is (mock `SCREENS['purchase-bill'].keys`). Esc is the
    /// shell's, except while reading, where it cancels.
    pub(crate) fn keys(&self) -> Vec<KeyHint> {
        match &self.phase {
            Phase::Choose | Phase::Failed(..) => vec![
                KeyHint::new("", lit("Read a bill")),
                KeyHint::new("Enter", lit("Read")),
                KeyHint::new("F9", lit("Type the bill instead")),
            ],
            Phase::Reading(_) => vec![
                KeyHint::new("", lit("Reading bill")),
                KeyHint::new("Esc", lit("Cancel")),
            ],
            Phase::Draft(..) => vec![
                KeyHint::new("", lit("Draft purchase")),
                KeyHint::new("Ctrl+A", lit("Accept and post")),
                KeyHint::new("Alt+D", lit("Reject draft")),
            ],
            Phase::Rejected(_) => vec![
                KeyHint::new("", lit("Rejected")),
                KeyHint::new("Ctrl+Alt+O", lit("Read another")),
                KeyHint::new("F9", lit("Type the bill instead")),
            ],
            Phase::Posted(..) => vec![
                KeyHint::new("", lit("Posted")),
                KeyHint::new("Ctrl+Alt+O", lit("Read another")),
            ],
        }
    }

    /// Chooses `ledger` for the draft's line `line`, as the line's ledger list does; the engine
    /// checks the draft again on the next frame.
    pub(crate) fn choose_ledger(&mut self, line: usize, ledger: LedgerId) {
        if let Phase::Draft(_, view) = &mut self.phase {
            view.chosen.retain(|&(n, _)| n != line);
            view.chosen.push((line, ledger));
            self.recheck = true;
        }
    }

    /// Confirms a value read from the bill (the Confirm button beside it); the engine checks the
    /// draft again.
    pub(crate) fn confirm_field(&mut self, field: DraftField) {
        if let Phase::Draft(_, view) = &mut self.phase
            && view.review.contains(&field)
            && !view.confirmed.contains(&field)
        {
            view.confirmed.push(field);
            self.recheck = true;
        }
    }

    fn start_gate(&mut self, worker: &ApiWorker) {
        self.latest.start(worker, |api: &mut Api| {
            Reply::Gate(
                api.ai_gate(ledgerly_api::AiFeature::BillReading)
                    .map_err(|e| e.to_string()),
            )
        });
    }

    fn start_read(&mut self, worker: &ApiWorker) {
        let path = PathBuf::from(self.path.trim());
        if path.as_os_str().is_empty() {
            return;
        }
        let file = file_name(&path);
        let cancel = CancelToken::new();
        self.cancel = Some(cancel.clone());
        self.cancelled = false;
        self.phase = Phase::Reading(file);
        self.notice = None;
        self.texture = None;
        self.latest.start(worker, move |api: &mut Api| {
            Reply::Read(read(api, &path, &cancel).map_err(|e| e.to_string()))
        });
    }

    fn start_check(&mut self, worker: &ApiWorker) {
        let Phase::Draft(_, view) = &self.phase else {
            return;
        };
        let id = view.id;
        let draft = view.draft.clone();
        let chosen = view.chosen.clone();
        let confirmed = view.confirmed.clone();
        self.recheck = false;
        self.latest.start(worker, move |api: &mut Api| {
            Reply::Checked(check(api, id, &draft, &chosen, &confirmed).map_err(|e| e.to_string()))
        });
    }

    fn start_accept(&mut self, worker: &ApiWorker, view: &BillView) {
        let id = view.id;
        let edits = DraftEdits {
            line_ledgers: view.chosen.clone(),
            confirmed: view.confirmed.clone(),
            ..DraftEdits::default()
        };
        self.latest.start(worker, move |api: &mut Api| {
            Reply::Accepted(
                api.accept_ai_draft(id, &edits)
                    .map(|out| match out {
                        Outcome::Value(Accepted::Posted(v)) => {
                            Ok(v.number.unwrap_or_else(|| v.id.0.to_string()))
                        }
                        Outcome::Value(Accepted::Blocked { reason, .. }) => Err(reason),
                        Outcome::Unsupported(u) => Err(unsupported_text(&u)),
                    })
                    .map_err(|e| e.to_string()),
            )
        });
    }

    fn start_reject(&mut self, worker: &ApiWorker, id: EventSeq) {
        self.latest.start(worker, move |api: &mut Api| {
            Reply::Rejected(api.reject_ai_draft(id, None).map_err(|e| e.to_string()))
        });
    }

    fn tick(&mut self) {
        let reply = match self.latest.poll() {
            None => return,
            Some(Done::Lost) => {
                self.notice = Some(lit("The company file stopped responding; reopen it."));
                self.phase = Phase::Choose;
                return;
            }
            Some(Done::Ready(reply)) => reply,
        };
        match reply {
            Reply::Gate(gate) => self.gate = Some(gate),
            Reply::Read(done) => self.read_done(done),
            Reply::Checked(result) => {
                if let Phase::Draft(_, view) = &mut self.phase {
                    match result {
                        Ok(checked) => view.checked = checked,
                        Err(reason) => self.notice = Some(reason),
                    }
                }
            }
            Reply::Accepted(result) => {
                if let Phase::Draft(photo, view) = &self.phase {
                    match result {
                        Ok(Ok(number)) => {
                            self.phase = Phase::Posted(photo.clone(), view.clone(), number);
                        }
                        Ok(Err(reason)) | Err(reason) => self.notice = Some(reason),
                    }
                }
            }
            Reply::Rejected(result) => {
                if let Phase::Draft(photo, _) = &self.phase {
                    match result {
                        Ok(()) => self.phase = Phase::Rejected(photo.clone()),
                        Err(reason) => self.notice = Some(reason),
                    }
                }
            }
        }
    }

    fn read_done(&mut self, done: Result<ReadDone, String>) {
        self.cancel = None;
        if self.cancelled {
            // Esc was pressed while the read ran: the screen is back at the file field.
            self.cancelled = false;
            self.notice = Some(match done {
                Ok(ReadDone::Drafted(..)) => lit(
                    "The read finished before it could stop; the draft waits in the review queue.",
                ),
                Ok(
                    ReadDone::Refused(_)
                    | ReadDone::Failed(..)
                    | ReadDone::FileError(..)
                    | ReadDone::Cancelled,
                )
                | Err(_) => lit("Reading cancelled. Nothing was kept."),
            });
            return;
        }
        match done {
            Ok(ReadDone::Drafted(photo, view)) => self.phase = Phase::Draft(photo, view),
            Ok(ReadDone::Refused(gate)) => {
                self.gate = Some(Ok(gate));
                self.phase = Phase::Choose;
            }
            Ok(ReadDone::Failed(file, reason) | ReadDone::FileError(file, reason)) => {
                self.phase = Phase::Failed(file, reason);
            }
            Ok(ReadDone::Cancelled) => {
                self.notice = Some(lit("Reading cancelled. Nothing was kept."));
                self.phase = Phase::Choose;
            }
            Err(reason) => {
                self.notice = Some(reason);
                self.phase = Phase::Choose;
            }
        }
    }

    /// Draws the screen with the assistant and returns what it asked the stack for.
    pub(crate) fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        self.tick();
        if let Some(worker) = ctx.api.as_ref()
            && !self.latest.is_pending()
        {
            if self.gate.is_none() {
                self.start_gate(worker);
            } else if self.recheck {
                self.start_check(worker);
            }
        }
        let mut asked = self.screen_keys(ui);

        crate::ui::page_head::page_head(
            ui,
            &lit(super::purchase_bill::PAGE_TITLE),
            Some(&lit(super::purchase_bill::SUBTITLE)),
            |_| {},
        );
        let mut action = None;
        let mut chose = None;
        let keys_free = !ui.ctx().memory(|m| m.focused().is_some());
        // Two cells of fixed widths, as the plain-words screen lays out its card: the card's
        // head chips would otherwise widen it past the window.
        egui_extras::StripBuilder::new(ui)
            .size(egui_extras::Size::exact(IMAGE_WIDTH + 36.0))
            .size(egui_extras::Size::exact(COLUMN_GAP))
            .size(egui_extras::Size::remainder())
            .horizontal(|mut strip| {
                strip.cell(|ui| self.image_column(ui));
                strip.empty();
                strip.cell(|ui| {
                    let (a, act, c) = self.right(ui, &ctx.tr, keys_free);
                    if !matches!(a, Ask::Nothing) {
                        asked = a;
                    }
                    action = act;
                    chose = c;
                });
            });
        match chose {
            Some(Pick::Ledger(line, ledger)) => self.choose_ledger(line, ledger),
            Some(Pick::Confirm(field)) => self.confirm_field(field),
            None => {}
        }
        if let Some(action) = action {
            self.act(action, ctx);
        }
        match asked {
            Ask::Nothing => ScreenAction::Stay,
            Ask::Read => {
                if let Some(worker) = ctx.api.as_ref()
                    && !self.latest.is_pending()
                {
                    self.start_read(worker);
                }
                ScreenAction::Stay
            }
            Ask::Another => {
                self.phase = Phase::Choose;
                self.texture = None;
                self.focused_once = false;
                self.notice = None;
                ScreenAction::Stay
            }
            Ask::Purchase => opened(
                &mut self.notice,
                ai_gate::open_voucher(Action::NewPurchase, ctx),
            ),
            Ask::Settings => opened(&mut self.notice, ai_gate::open_settings(ctx)),
        }
    }

    /// F9, Ctrl+Alt+O and Esc while reading; Esc cancels the read here.
    fn screen_keys(&mut self, ui: &Ui) -> Ask {
        let reading = matches!(self.phase, Phase::Reading(_));
        let (f9, another, esc) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::F9),
                i.consume_key(Modifiers::COMMAND | Modifiers::ALT, Key::O),
                reading && i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if esc {
            if let Some(token) = &self.cancel {
                token.cancel();
            }
            self.cancelled = true;
            self.phase = Phase::Choose;
            self.notice = Some(lit("Reading cancelled. Nothing was kept."));
        }
        match (f9, another) {
            (true, _) if !matches!(self.phase, Phase::Draft(..)) => Ask::Purchase,
            (_, true) if !reading => Ask::Another,
            _ => Ask::Nothing,
        }
    }

    fn act(&mut self, action: PropAction, ctx: &AppCtx) {
        let Phase::Draft(_, view) = &self.phase else {
            return;
        };
        let view = view.clone();
        let Some(worker) = ctx.api.as_ref() else {
            return;
        };
        if self.latest.is_pending() || self.recheck {
            return;
        }
        match action {
            PropAction::Accept => self.start_accept(worker, &view),
            PropAction::Reject => self.start_reject(worker, view.id),
            PropAction::Edit => {
                self.notice = Some(lit(
                    "Opening a draft in the voucher form is not built yet; accept it, or reject it and type the bill with F9.",
                ));
            }
        }
    }

    /// The image column: the photo with a box on each value read, or the empty state.
    fn image_column(&mut self, ui: &mut Ui) {
        let t = tokens(ui);
        let photo = match &self.phase {
            Phase::Draft(photo, _) | Phase::Posted(photo, ..) | Phase::Rejected(photo) => {
                Some(photo.clone())
            }
            Phase::Choose | Phase::Reading(_) | Phase::Failed(..) => None,
        };
        Frame::new()
            .fill(t.sunken)
            .corner_radius(RADIUS_M)
            .inner_margin(Margin::same(18))
            .show(ui, |ui| {
                ui.set_width(IMAGE_WIDTH);
                ui.set_min_height(IMAGE_HEIGHT);
                if let Some(photo) = photo {
                    self.draw_photo(ui, &photo);
                } else {
                    empty_photo(ui, &self.phase);
                }
            });
    }

    /// The photo scaled to the column, with the boxes over it.
    fn draw_photo(&mut self, ui: &mut Ui, photo: &Photo) {
        let t = tokens(ui);
        let texture = self
            .texture
            .get_or_insert_with(|| {
                Texture(ui.ctx().load_texture(
                    format!("bill-{}", photo.file),
                    ColorImage::clone(&photo.image),
                    TextureOptions::LINEAR,
                ))
            })
            .0
            .clone();
        let [w, h] = photo.image.size;
        if w == 0 || h == 0 {
            return;
        }
        let scale = IMAGE_WIDTH / pixels(w);
        let size = vec2(IMAGE_WIDTH, pixels(h) * scale);
        let response = ui.add(egui::Image::new((texture.id(), size)));
        let origin = response.rect.min;
        let painter = ui.painter_at(response.rect.expand(4.0));
        for (label, [x, y, bw, bh]) in &photo.boxes {
            let rect = Rect::from_min_size(
                origin + vec2(box_pixels(*x) * scale, box_pixels(*y) * scale),
                vec2(box_pixels(*bw) * scale, box_pixels(*bh) * scale),
            )
            .expand(3.0);
            painter.rect(
                rect,
                2.0,
                t.ai_soft.gamma_multiply(0.12),
                Stroke::new(1.5, t.ai),
                StrokeKind::Outside,
            );
            painter.text(
                rect.right_center() + vec2(4.0, 0.0),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(10.0),
                t.ai_ink,
            );
        }
    }

    /// The right column for where the screen is. Returns what it asked for, the proposal action
    /// and a ledger chosen for a line.
    fn right(
        &mut self,
        ui: &mut Ui,
        tr: &crate::i18n::Tr,
        keys_free: bool,
    ) -> (Ask, Option<PropAction>, Option<Pick>) {
        let mut asked = Ask::Nothing;
        let mut action = None;
        let mut chose = None;
        if let Some(line) = self.notice.clone() {
            note(ui, NoteKind::Warn, &line);
            ui.add_space(8.0);
        }
        let gate = self.gate.clone();
        let phase = self.phase.clone();
        match (&gate, &phase) {
            (None, _) => {
                card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    skeleton_rows(ui, 3, ui.available_width());
                });
            }
            (Some(Err(reason)), _) => {
                note(ui, NoteKind::Warn, reason);
            }
            (Some(Ok(AiGate::Ready(_))), Phase::Choose) => {
                asked = self.choose_card(ui);
            }
            (Some(Ok(_)), Phase::Reading(_)) => {
                reading_card(ui);
            }
            (Some(Ok(_)), Phase::Failed(file, reason)) => {
                asked = failed_note(ui, file, reason);
            }
            (Some(Ok(_)), Phase::Draft(photo, view)) => {
                (action, chose) = bill_card(
                    ui,
                    photo,
                    view,
                    PropState::Open,
                    keys_free && !self.recheck,
                    tr,
                );
            }
            (Some(Ok(_)), Phase::Posted(photo, view, number)) => {
                let _ = bill_card(ui, photo, view, PropState::Done, false, tr);
                ui.add_space(8.0);
                note(
                    ui,
                    NoteKind::Ok,
                    &format!("{} {number} {}", lit("Purchase"), lit("posted.")),
                );
            }
            (Some(Ok(_)), Phase::Rejected(_)) => {
                asked = rejected_note(ui);
            }
            (Some(Ok(refused)), Phase::Choose) => {
                card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if refusal_note(ui, refused, tr) {
                        asked = Ask::Settings;
                    }
                    ui.add_space(12.0);
                    if button(
                        ui,
                        &lit("Type the bill instead"),
                        Some("F9"),
                        ButtonKind::Secondary,
                    )
                    .clicked()
                    {
                        asked = Ask::Purchase;
                    }
                });
            }
        }
        (asked, action, chose)
    }

    /// The card that asks for a file (mock `purchase-bill.choose`).
    fn choose_card(&mut self, ui: &mut Ui) -> Ask {
        let mut asked = Ask::Nothing;
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            let t = tokens(ui);
            ui.label(
                RichText::new(lit("Read a bill"))
                    .font(bold_font(ui, TEXT_H2))
                    .color(t.text),
            );
            ui.add_space(4.0);
            ui.add(
                egui::Label::new(para_text(
                    &lit("Type the path of a bill photo (PNG). It is read on this computer. The image is not kept with the voucher yet."),
                    TEXT_BODY,
                    t.text_2,
                ))
                .wrap(),
            );
            ui.add_space(10.0);
            ui.label(RichText::new(lit("File")).size(TEXT_META).color(t.text_3));
            let response = ui.add(
                TextEdit::singleline(&mut self.path)
                    .hint_text("/home/you/Bills/IMG_2041.png")
                    .desired_width(f32::INFINITY),
            );
            if !self.focused_once {
                response.request_focus();
                self.focused_once = true;
            }
            if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                asked = Ask::Read;
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if button(ui, &lit("Read"), Some("Enter"), ButtonKind::Primary).clicked() {
                    asked = Ask::Read;
                }
                if button(
                    ui,
                    &lit("Type the bill instead"),
                    Some("F9"),
                    ButtonKind::Secondary,
                )
                .clicked()
                {
                    asked = Ask::Purchase;
                }
            });
        });
        asked
    }
}

/// The loading card (mock `purchase-bill.reading`).
fn reading_card(ui: &mut Ui) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        let t = tokens(ui);
        ui.horizontal(|ui| {
            icon(ui, Icon::Cpu, t.text_2);
            ui.label(
                RichText::new(lit("Reading on this computer"))
                    .font(bold_font(ui, TEXT_BODY))
                    .color(t.text_2),
            );
        });
        ui.add_space(8.0);
        skeleton_rows(ui, 9, ui.available_width());
        ui.add_space(10.0);
        ui.label(
            RichText::new(lit(
                "Esc cancels. The image never leaves this computer in the local tier.",
            ))
            .size(TEXT_META)
            .color(t.text_3),
        );
    });
}

/// The note for a file that could not be read (mock `purchase-bill.error`). Returns what its
/// buttons asked for.
fn failed_note(ui: &mut Ui, file: &str, reason: &str) -> Ask {
    let mut asked = Ask::Nothing;
    note(
        ui,
        NoteKind::Bad,
        &format!(
            "{} {file}: {reason}\n{}",
            lit("Could not read"),
            lit("Nothing was stored except the file name in the AI log, when the assistant ran.")
        ),
    );
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if button(
            ui,
            &lit("Choose another file"),
            Some("Ctrl+Alt+O"),
            ButtonKind::Secondary,
        )
        .clicked()
        {
            asked = Ask::Another;
        }
        if button(
            ui,
            &lit("Type the bill instead"),
            Some("F9"),
            ButtonKind::Secondary,
        )
        .clicked()
        {
            asked = Ask::Purchase;
        }
    });
    asked
}

/// The note after a rejection (mock `purchase-bill.rejected`).
fn rejected_note(ui: &mut Ui) -> Ask {
    let mut asked = Ask::Nothing;
    note(
        ui,
        NoteKind::Info,
        &format!(
            "{}\n{}",
            lit("Draft rejected. Nothing was posted."),
            lit("The rejection is in the AI activity log.")
        ),
    );
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if button(
            ui,
            &lit("Read another"),
            Some("Ctrl+Alt+O"),
            ButtonKind::Primary,
        )
        .clicked()
        {
            asked = Ask::Another;
        }
        if button(
            ui,
            &lit("Type the bill instead"),
            Some("F9"),
            ButtonKind::Secondary,
        )
        .clicked()
        {
            asked = Ask::Purchase;
        }
    });
    asked
}

/// The note for a refused gate. Returns whether its AI and privacy button was clicked.
fn refusal_note(ui: &mut Ui, gate: &AiGate, tr: &crate::i18n::Tr) -> bool {
    let line = match gate {
        AiGate::NotChosen => {
            return ai_gate::gate_note_in(
                ui,
                ai_gate::Feature::BillReading,
                &ai_gate::Gate::Unset,
                tr,
            );
        }
        AiGate::Off => {
            return ai_gate::gate_note_in(
                ui,
                ai_gate::Feature::BillReading,
                &ai_gate::Gate::Off,
                tr,
            );
        }
        // The shared sentence for this computer's tier: no reader is installed (the screens of
        // `ai_gate` say the same for every feature).
        AiGate::NotInstalled(_) => {
            return ai_gate::gate_note_in(
                ui,
                ai_gate::Feature::BillReading,
                &ai_gate::Gate::Local,
                tr,
            );
        }
        AiGate::FeatureOff => lit("Bill reading is switched off in AI and privacy."),
        AiGate::NotConfigured(reason) => {
            format!("{} {reason}.", lit("The cloud provider is not set up:"))
        }
        AiGate::Ready(_) => return false,
    };
    note(ui, NoteKind::Warn, &line);
    ui.add_space(6.0);
    button(ui, &lit("AI and privacy"), None, ButtonKind::Secondary).clicked()
}

/// Keeps the reason when a screen does not open.
fn opened(notice: &mut Option<String>, result: Result<ScreenAction, String>) -> ScreenAction {
    match result {
        Ok(action) => action,
        Err(why) => {
            *notice = Some(why);
            ScreenAction::Stay
        }
    }
}

/// The source chip's word for where the provider runs.
fn place_word(place: ProviderPlace) -> String {
    match place {
        ProviderPlace::ThisComputer => lit("Local model"),
        ProviderPlace::Cloud => lit("Your cloud provider"),
        ProviderPlace::Test => lit("Test provider"),
    }
}

/// The confidence band (the review queue's rule).
fn band(permille: u16) -> Confidence {
    if permille >= 850 {
        Confidence::High
    } else if permille >= ledgerly_api::AI_CONFIDENCE_THRESHOLD_PERMILLE {
        Confidence::Medium
    } else {
        Confidence::Low
    }
}

/// The proposal card (mock `purchase-bill.ledger-lines`). Returns the action and a ledger the
/// person chose for an open line.
fn bill_card(
    ui: &mut Ui,
    photo: &Photo,
    view: &BillView,
    state: PropState,
    keys: bool,
    tr: &crate::i18n::Tr,
) -> (Option<PropAction>, Option<Pick>) {
    let blocked = view.checked.verdict.as_ref().err().map(String::as_str);
    let why = lit("Fix the red items first.");
    let mut chose = None;
    let action = Prop::new(&why)
        .state(state)
        .accept(blocked.is_none(), blocked)
        .handles_keys(keys)
        .show(
            ui,
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    let head = match state {
                        PropState::Open => lit("Draft purchase voucher"),
                        PropState::Done => lit("Accepted"),
                        PropState::Rejected => lit("Rejected"),
                    };
                    ui.label(RichText::new(head).strong());
                    if state == PropState::Open {
                        ui.label(
                            RichText::new(lit("suggested by the bill reader")).size(TEXT_META),
                        );
                    }
                    // The sources on their own row: in one row with the kind they widen the
                    // card past its cell.
                    ui.end_row();
                    ui.label(
                        RichText::new(format!("{}, {}", photo.file, lit("page 1"))).size(TEXT_META),
                    );
                    ui.label(RichText::new(place_word(view.place)).size(TEXT_META));
                    if state == PropState::Open {
                        let _ = conf(ui, band(view.confidence), tr);
                    }
                });
            },
            |ui| chose = bill_body(ui, photo, view, state == PropState::Open),
            |ui| bill_checks(ui, view),
        );
    (action, chose)
}

/// The values read from the bill head, each with where it came from.
fn bill_fields(ui: &mut Ui, photo: &Photo, view: &BillView, open: bool) -> Option<Pick> {
    let t = tokens(ui);
    let draft = &view.draft;
    let mut picked = None;
    let mut field =
        |ui: &mut Ui, label: &str, kind: Option<(ProvKind, DraftField)>, value: &str| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(lit(label)).size(TEXT_META).color(t.text_3));
                    if let Some((kind, f)) = kind {
                        picked = review_tag(ui, view, f, kind, open).or(picked);
                    }
                });
                ui.label(RichText::new(value).size(TEXT_BODY).color(t.text));
            });
        };
    let unread = lit("Not read");
    egui::Grid::new(("bill-fields", view.id.0))
        .num_columns(2)
        .spacing([24.0, 8.0])
        .show(ui, |ui| {
            field(
                ui,
                "Supplier",
                Some((ProvKind::Ai, DraftField::PartyName)),
                draft.party_name.as_ref().map_or(&unread, |p| &p.value),
            );
            field(
                ui,
                "Supplier bill no.",
                Some((ProvKind::FromBill, DraftField::InvoiceNumber)),
                draft.invoice_number.as_ref().map_or(&unread, |p| &p.value),
            );
            ui.end_row();
            let date = draft
                .date
                .as_ref()
                .map_or_else(|| unread.clone(), |d| format::date(d.value));
            field(
                ui,
                "Bill date",
                Some((ProvKind::FromBill, DraftField::Date)),
                &date,
            );
            field(ui, "File", None, &photo.file);
            ui.end_row();
            field(
                ui,
                "Supplier GSTIN",
                // A value not read has no source to show.
                draft
                    .supplier_gstin
                    .as_ref()
                    .map(|_| (ProvKind::FromBill, DraftField::SupplierGstin)),
                draft.supplier_gstin.as_ref().map_or(&unread, |g| &g.value),
            );
            ui.end_row();
        });
    picked
}

/// The tag beside a value read from the bill: its usual provenance, or, for a value that needs
/// review, "needs review" and a Confirm button until the person confirms it, then "you
/// confirmed". Returns the confirmation when the button was pressed.
fn review_tag(
    ui: &mut Ui,
    view: &BillView,
    field: DraftField,
    usual: ProvKind,
    open: bool,
) -> Option<Pick> {
    if !view.review.contains(&field) {
        let _ = prov(ui, usual);
        return None;
    }
    if view.confirmed.contains(&field) {
        let _ = prov(ui, ProvKind::YouConfirmed);
        return None;
    }
    let _ = prov(ui, ProvKind::NeedsReview);
    let pressed = open
        && ui
            .add(
                egui::Button::new(
                    RichText::new(format!("\u{2713} {}", lit("Confirm"))).size(TEXT_META),
                )
                .small(),
            )
            .on_hover_text(lit("Confirm this value"))
            .clicked();
    pressed.then_some(Pick::Confirm(field))
}

/// The draft lines; an open line has a ledger list the person chooses from.
fn bill_lines(ui: &mut Ui, view: &BillView, open: bool) -> Option<Pick> {
    let t = tokens(ui);
    let mut chose = None;
    egui::Grid::new(("bill-lines", view.id.0))
        .num_columns(3)
        .spacing([16.0, 6.0])
        .min_col_width(40.0)
        .show(ui, |ui| {
            for head in ["Dr / Cr", "Ledger", "Amount"] {
                ui.label(RichText::new(lit(head)).size(TEXT_META).color(t.text_3));
            }
            ui.end_row();
            for (n, line) in view.checked.lines.iter().enumerate() {
                ui.label(match line.side {
                    DrCr::Dr => "Dr",
                    DrCr::Cr => "Cr",
                });
                ui.vertical(|ui| {
                    if line.open && open {
                        chose = ledger_chooser(ui, view, n, line).or(chose);
                    } else {
                        ui.horizontal(|ui| {
                            match &line.ledger {
                                Some(name) => ui.label(name),
                                None => ui.label(
                                    RichText::new(format!(
                                        "{} {}",
                                        lit("No ledger named"),
                                        line.read
                                    ))
                                    .color(t.bad),
                                ),
                            };
                            if !line.open && view.review.contains(&DraftField::LineLedger(n)) {
                                chose = review_tag(
                                    ui,
                                    view,
                                    DraftField::LineLedger(n),
                                    ProvKind::FromBill,
                                    open,
                                )
                                .or(chose);
                            }
                        });
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(format::amount(line.amount));
                    chose = review_tag(
                        ui,
                        view,
                        DraftField::LineAmount(n),
                        ProvKind::FromBill,
                        open,
                    )
                    .or(chose);
                });
                ui.end_row();
            }
        });
    chose
}

/// The ledger list of an open line, with the words it was read from. Returns the ledger chosen.
fn ledger_chooser(ui: &mut Ui, view: &BillView, n: usize, line: &BillLine) -> Option<Pick> {
    let t = tokens(ui);
    let mut chose = None;
    let shown = line
        .ledger
        .clone()
        .unwrap_or_else(|| lit("Choose a ledger"));
    egui::ComboBox::from_id_salt(("bill-line", view.id.0, n))
        .selected_text(shown)
        .width(220.0)
        .show_ui(ui, |ui| {
            for (id, name) in &view.choices {
                let on = line.ledger.as_deref() == Some(name.as_str());
                if ui.selectable_label(on, name).clicked() {
                    chose = Some(Pick::Ledger(n, *id));
                }
            }
        });
    ui.label(
        RichText::new(format!(
            "{} \"{}\": {}",
            lit("Read"),
            line.read,
            lit("the bill names no ledger for it")
        ))
        .size(TEXT_META)
        .color(t.text_3),
    );
    chose
}

/// The card's fields, lines and totals. Returns a ledger chosen for an open line.
fn bill_body(ui: &mut Ui, photo: &Photo, view: &BillView, open: bool) -> Option<Pick> {
    let t = tokens(ui);
    let mut chose = bill_fields(ui, photo, view, open);
    ui.add_space(10.0);
    chose = bill_lines(ui, view, open).or(chose);
    ui.add_space(10.0);
    ui.columns(2, |cols| {
        if let [left, right] = cols {
            left.add(
                egui::Label::new(para_text(
                    &if view.review.is_empty() {
                        lit("Item rows are not read yet. Tax ledgers come from your masters; nothing is created.")
                    } else {
                        lit("The OCR reader gives no confidence, so nothing it read is kept until you confirm it against the photo. Tab to a Confirm button; Space confirms.")
                    },
                    TEXT_META,
                    t.text_2,
                ))
                .wrap(),
            );
            chose = totals_box(right, view, open).or(chose);
        }
    });
    for name in &view.draft.create_ledgers {
        ui.add_space(6.0);
        note(
            ui,
            NoteKind::Warn,
            &format!(
                "{} \"{name}\". {}",
                lit("No ledger named"),
                lit("The assistant does not invent ledgers. Pick one, or create it in Masters.")
            ),
        );
    }
    chose
}

/// An amount as read from the bill, or "Not read".
fn read_amount(ui: &Ui, read: Option<&ledgerly_api::Transcribed<Paise>>) -> RichText {
    let t = tokens(ui);
    read.map_or_else(
        || {
            RichText::new(lit("Not read"))
                .size(TEXT_META)
                .color(t.text_3)
        },
        |p| {
            RichText::new(format::amount(p.value))
                .size(TEXT_BODY)
                .color(t.text)
        },
    )
}

/// The printed total as read, and the engine's total only when the engine made the voucher.
fn totals_box(ui: &mut Ui, view: &BillView, open: bool) -> Option<Pick> {
    let mut picked = None;
    let t = tokens(ui);
    Frame::new()
        .fill(t.surface_2)
        .stroke(Stroke::new(border_width(kind(ui)), t.border))
        .corner_radius(RADIUS_M)
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // The label and its tag on one line, the figure (or why there is none) under it,
            // wrapped: a row that does not wrap widens the card past its cell.
            let row = |ui: &mut Ui, label: &str, kind: ProvKind, value: RichText| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(label).size(TEXT_META).color(t.text_2));
                    let _ = prov(ui, kind);
                });
                ui.add(egui::Label::new(value).wrap());
                ui.add_space(4.0);
            };
            let printed = read_amount(ui, view.draft.document_total.as_ref());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(lit("Printed total"))
                        .size(TEXT_META)
                        .color(t.text_2),
                );
                picked = review_tag(
                    ui,
                    view,
                    DraftField::DocumentTotal,
                    ProvKind::FromBill,
                    open,
                );
            });
            ui.add(egui::Label::new(printed).wrap());
            ui.add_space(4.0);
            let tax = read_amount(ui, view.draft.document_tax.as_ref());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(lit("Printed tax"))
                        .size(TEXT_META)
                        .color(t.text_2),
                );
                if view.draft.document_tax.is_some() {
                    picked =
                        review_tag(ui, view, DraftField::DocumentTax, ProvKind::FromBill, open)
                            .or(picked);
                }
            });
            ui.add(egui::Label::new(tax).wrap());
            ui.add_space(4.0);
            let engine = view.checked.engine_total.map_or_else(
                || {
                    RichText::new(lit("Shown when the engine makes the voucher"))
                        .size(TEXT_META)
                        .color(t.text_3)
                },
                |total| {
                    RichText::new(format::amount(total))
                        .size(TEXT_BODY)
                        .color(t.text)
                        .strong()
                },
            );
            row(ui, &lit("Engine total"), ProvKind::Engine, engine);
        });
    picked
}

/// The engine's checks under the card.
fn bill_checks(ui: &mut Ui, view: &BillView) {
    let t = tokens(ui);
    let waiting = view
        .review
        .iter()
        .filter(|f| !view.confirmed.contains(f) && !chosen_covers(view, **f))
        .count();
    let mut lines: Vec<(Icon, Color32, String)> = Vec::new();
    if waiting > 0 {
        let words = if waiting == 1 {
            lit("value read from the bill needs review: confirm it against the photo.")
        } else {
            lit("values read from the bill need review: confirm each against the photo.")
        };
        lines.push((Icon::X, t.bad, format!("{waiting} {words}")));
    }
    lines.push(match &view.checked.verdict {
        Ok(()) => (Icon::Check, t.ok, lit("Dr equals Cr (engine).")),
        Err(_) if waiting > 0 => (
            Icon::X,
            t.bad,
            lit("Confirm each value read from the bill."),
        ),
        Err(reason) => (Icon::X, t.bad, sentence_case(reason)),
    });
    if let (Some(date), Some(lock)) = view.checked.dates {
        lines.push(if date > lock {
            (
                Icon::Check,
                t.ok,
                format!("{} {}", format::date(date), lit("is after the lock date.")),
            )
        } else {
            (
                Icon::X,
                t.bad,
                format!(
                    "{} {} {}{}",
                    format::date(date),
                    lit("is on or before the lock date,"),
                    format::date(lock),
                    lit("; the engine refuses it.")
                ),
            )
        });
    }
    for (glyph, color, line) in lines {
        ui.horizontal_wrapped(|ui| {
            icon(ui, glyph, color);
            ui.label(RichText::new(line).size(TEXT_META).color(color));
        });
    }
}

/// Whether `field` is settled by a ledger choice rather than a confirmation: the ledger of a line
/// the person chose one for, or of a line that names none and waits for their choice.
fn chosen_covers(view: &BillView, field: DraftField) -> bool {
    match field {
        // A line that names no ledger is the person's choice, made in its select and checked
        // as "Choose a ledger": it is not a value to confirm (the mock's count leaves it out).
        DraftField::LineLedger(n) => {
            view.chosen.iter().any(|(i, _)| *i == n)
                || view.draft.lines.get(n).is_some_and(|l| l.ledger.is_none())
        }
        DraftField::Date
        | DraftField::PartyName
        | DraftField::InvoiceNumber
        | DraftField::SupplierGstin
        | DraftField::LineAmount(_)
        | DraftField::DocumentTotal
        | DraftField::DocumentTax => false,
    }
}

/// `text` with its first letter upper case, for an engine reason that starts a sentence.
fn sentence_case(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// A pixel count as points, for scaling the photo.
#[expect(
    clippy::cast_precision_loss,
    reason = "image sides are far below 2^24, where f32 is exact"
)]
fn pixels(n: usize) -> f32 {
    n as f32
}

/// A box coordinate as points, for scaling the photo.
fn box_pixels(n: u32) -> f32 {
    pixels(usize::try_from(n).unwrap_or(usize::MAX))
}

/// The file name of `path`, as the AI log keeps it.
fn file_name(path: &std::path::Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Why this build does not read `bytes`, or `None` for a PNG.
fn not_read(bytes: &[u8]) -> Option<String> {
    if bytes.starts_with(&PNG_SIGNATURE) {
        None
    } else if bytes.starts_with(b"%PDF") {
        Some(lit(
            "a PDF is not read in this build; save the page as a PNG, or type the bill.",
        ))
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(lit(
            "a JPEG photo is not read in this build; save it as a PNG, or type the bill.",
        ))
    } else {
        Some(lit(
            "the file is not a PNG image; this build reads PNG bill photos.",
        ))
    }
}

/// Reads the file at `path` on the worker: the file, the gate's read, and the engine's check.
fn read(
    api: &mut Api,
    path: &std::path::Path,
    cancel: &CancelToken,
) -> Result<ReadDone, ledgerly_api::ApiError> {
    let file = file_name(path);
    let place = match api.ai_gate(ledgerly_api::AiFeature::BillReading)? {
        AiGate::Ready(info) => info.place,
        refused @ (AiGate::NotChosen
        | AiGate::Off
        | AiGate::FeatureOff
        | AiGate::NotInstalled(_)
        | AiGate::NotConfigured(_)) => return Ok(ReadDone::Refused(refused)),
    };
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => {
            return Ok(ReadDone::FileError(
                file,
                format!("{} {e}", lit("the file could not be opened:")),
            ));
        }
    };
    if let Some(reason) = not_read(&bytes) {
        return Ok(ReadDone::FileError(file, reason));
    }
    let image = match decode_png(&bytes) {
        Ok(image) => image,
        Err(reason) => return Ok(ReadDone::FileError(file, reason)),
    };
    let (id, proposal) = match api.read_bill_photo(&bytes, &file, cancel)? {
        BillRead::Drafted { id, proposal, .. } => (id, proposal),
        BillRead::Refused(gate) => return Ok(ReadDone::Refused(gate)),
        BillRead::Failed(reason) => return Ok(ReadDone::Failed(file, reason)),
        BillRead::Cancelled => return Ok(ReadDone::Cancelled),
    };
    let draft = match proposal {
        Proposal::VoucherDraft(d) => d,
        Proposal::MatchSuggestion(_)
        | Proposal::ReportQuery(_)
        | Proposal::MappingSuggestion(_) => {
            return Ok(ReadDone::Failed(
                file,
                lit("the assistant did not draft a voucher"),
            ));
        }
    };
    let checked = check(api, id, &draft, &[], &[])?;
    let choices = ledger_choices(api)?;
    let photo = Photo {
        file,
        image: Arc::new(image),
        boxes: boxes_of(&draft),
    };
    let view = BillView {
        id,
        review: needs_review(&draft),
        confirmed: Vec::new(),
        confidence: lowest_confidence(&draft),
        draft,
        place,
        chosen: Vec::new(),
        choices,
        checked,
    };
    Ok(ReadDone::Drafted(Box::new(photo), Box::new(view)))
}

/// The PNG decoded for the screen, or why it could not be.
fn decode_png(bytes: &[u8]) -> Result<ColorImage, String> {
    match image::load_from_memory_with_format(bytes, image::ImageFormat::Png) {
        Ok(decoded) => {
            let rgba = decoded.to_rgba8();
            let size = [
                usize::try_from(rgba.width()).unwrap_or(0),
                usize::try_from(rgba.height()).unwrap_or(0),
            ];
            Ok(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
        }
        Err(e) => Err(format!("{} {e}", lit("the PNG could not be decoded:"))),
    }
}

/// The company's ledgers by name, for the list an open line chooses from.
fn ledger_choices(api: &mut Api) -> Result<Vec<(LedgerId, String)>, ledgerly_api::ApiError> {
    let mut choices: Vec<(LedgerId, String)> = match api.books()? {
        Outcome::Value(books) => books
            .masters
            .ledgers
            .values()
            .map(|l| (l.id, l.name.clone()))
            .collect(),
        Outcome::Unsupported(_) => Vec::new(),
    };
    choices.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(choices)
}

/// The engine's check of the draft with the person's choices, and the lines as the card shows
/// them.
fn check(
    api: &Api,
    id: EventSeq,
    draft: &VoucherDraft,
    chosen: &[(usize, LedgerId)],
    confirmed: &[DraftField],
) -> Result<Checked, ledgerly_api::ApiError> {
    let edits = DraftEdits {
        line_ledgers: chosen.to_vec(),
        confirmed: confirmed.to_vec(),
        ..DraftEdits::default()
    };
    let outcome = api.check_ai_draft(id, &edits)?;
    let names: Vec<(LedgerId, String)> = match api.books()? {
        Outcome::Value(books) => books
            .masters
            .ledgers
            .values()
            .map(|l| (l.id, l.name.clone()))
            .collect(),
        Outcome::Unsupported(_) => Vec::new(),
    };
    let name_of = |id: LedgerId| {
        names
            .iter()
            .find(|(n, _)| *n == id)
            .map(|(_, name)| name.clone())
    };
    let lines = draft
        .lines
        .iter()
        .enumerate()
        .map(|(n, l)| {
            let picked = chosen.iter().find(|(i, _)| *i == n).map(|(_, id)| *id);
            BillLine {
                side: l.side,
                ledger: picked.or(l.ledger).and_then(name_of),
                read: l.ledger_name.value.clone(),
                amount: l.amount.value,
                open: l.ledger.is_none(),
            }
        })
        .collect();
    let (engine_total, verdict, date) = match &outcome {
        Outcome::Value(DraftCheck::Ok(voucher)) => {
            let total = voucher
                .ledger_lines
                .iter()
                .find(|l| Some(l.ledger) == voucher.party && l.amount.side == DrCr::Cr)
                .map(|l| l.amount.amount);
            (total, Ok(()), Some(voucher.date))
        }
        Outcome::Value(DraftCheck::Blocked { reason, .. }) => (
            None,
            Err(reason.clone()),
            draft.date.as_ref().map(|d| d.value),
        ),
        Outcome::Unsupported(u) => (
            None,
            Err(unsupported_text(u)),
            draft.date.as_ref().map(|d| d.value),
        ),
    };
    Ok(Checked {
        lines,
        engine_total,
        verdict,
        dates: (date, api.lock_date()),
    })
}

/// The reader's lowest confidence over the draft's amounts and ledger names, per mille.
fn lowest_confidence(draft: &VoucherDraft) -> u16 {
    draft
        .lines
        .iter()
        .map(|l| {
            l.amount
                .confidence_permille
                .min(l.ledger_name.confidence_permille)
        })
        .min()
        .unwrap_or(0)
}

/// A label and a box for each value the draft read from the photo, once per box.
fn boxes_of(draft: &VoucherDraft) -> Vec<(String, [u32; 4])> {
    let on_photo = |s: &SourceRef| match s {
        SourceRef::Document { bbox, .. } => Some(*bbox),
        SourceRef::Typed { .. } | SourceRef::StatementLine { .. } | SourceRef::Edited => None,
    };
    let mut boxes: Vec<(String, [u32; 4])> = Vec::new();
    let mut add = |label: String, source: &SourceRef| {
        if let Some(bbox) = on_photo(source)
            && !boxes.iter().any(|(_, b)| *b == bbox)
        {
            boxes.push((label, bbox));
        }
    };
    if let Some(p) = &draft.party_name {
        add(lit("Supplier"), &p.source);
    }
    if let Some(p) = &draft.invoice_number {
        add(lit("Bill no."), &p.source);
    }
    if let Some(p) = &draft.date {
        add(lit("Date"), &p.source);
    }
    if let Some(p) = &draft.document_total {
        add(lit("Total"), &p.source);
    }
    for line in &draft.lines {
        add(line.ledger_name.value.clone(), &line.amount.source);
    }
    boxes
}

/// The image column before a photo is read: what the column will show, or what is happening.
fn empty_photo(ui: &mut Ui, phase: &Phase) {
    let t = tokens(ui);
    let (glyph, title, line) = match phase {
        Phase::Reading(file) => (Icon::Cpu, file.clone(), lit("Reading on this computer.")),
        Phase::Failed(file, _) => (
            Icon::File,
            file.clone(),
            lit("Nothing was read from this file."),
        ),
        Phase::Choose | Phase::Draft(..) | Phase::Posted(..) | Phase::Rejected(_) => (
            Icon::Camera,
            lit("No bill chosen"),
            lit("The photo shows here with a box on each value read from it."),
        ),
    };
    ui.vertical_centered(|ui| {
        ui.add_space(180.0);
        icon_sized(ui, glyph, t.text_3, 36.0);
        ui.add_space(10.0);
        ui.label(
            RichText::new(title)
                .font(bold_font(ui, TEXT_H2))
                .color(t.text),
        );
        ui.add_space(4.0);
        ui.add(egui::Label::new(para_text(&line, TEXT_BODY, t.text_2)).wrap());
    });
}
