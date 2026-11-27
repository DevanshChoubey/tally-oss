//! Plain-words entry with the assistant (round s21, plan round C; mock states
//! `ai.quick-entry.proposal`, `.unknown-ledger`, `.loading`, `.rejected` and `.not-installed`).
//!
//! The sentence goes through the engine's one gate (`Api::read_words`): the tier, the feature's
//! switch and the provider's status. A draft is stored at once (it also waits in the review
//! queue) and checked by the engine with today's date offered, as for any new voucher. The card
//! shows the engine's voucher when it balances, or the draft and the engine's reason when it does
//! not. Amounts are the ones the person typed (copied by the reader, parsed by its own rule) or
//! the engine's; the screen computes none. Ctrl+A accepts through the engine, Alt+D rejects and
//! records it; both go into the AI activity log.

use egui::text::{LayoutJob, TextFormat};
use egui::{FontId, Frame, Key, Margin, Modifiers, RichText, Stroke, TextEdit, Ui};
use ledgerly_api::{
    Accepted, AiGate, Api, DraftCheck, DraftEdits, Proposal, ProviderPlace, SourceRef,
    VoucherDraft, WordsRead,
};
use ledgerly_core::{CancelToken, Date, DrCr, EventSeq, LedgerId, Outcome, Paise};
use ledgerly_model::BaseVoucherKind;

use crate::i18n::lit;
use crate::keymap::Action;
use crate::screen::KeyHint;
use crate::screens::ai_gate;
use crate::screens::wire::{Done, Latest, unsupported_text};
use crate::theme::{RADIUS_M, TEXT_BODY, TEXT_H2, TEXT_INPUT, TEXT_META, border_width};
use crate::ui::button::{ButtonKind, button};
use crate::ui::card::card;
use crate::ui::conf::{Confidence, conf};
use crate::ui::icon::{Icon, icon};
use crate::ui::note::{NoteKind, note};
use crate::ui::prop::{Prop, PropAction, PropState};
use crate::ui::prov::{ProvKind, prov};
use crate::ui::skeleton::skeleton_rows;
use crate::ui::{bold_font, format, kind, para_text, tokens};
use crate::worker::ApiWorker;
use crate::{AppCtx, ScreenAction};

/// One line of the card's table: the side, the ledger (or the name no ledger has), the amount and
/// where the amount came from.
#[derive(Debug, Clone)]
struct Line {
    /// Dr or Cr.
    side: DrCr,
    /// The ledger's name, or `None` when no ledger has the typed name.
    ledger: Option<String>,
    /// The name as typed.
    typed: String,
    /// The amount.
    amount: Paise,
    /// Copied from the person's words, or the engine's balancing figure.
    from: ProvKind,
}

/// A draft as the card shows it.
#[derive(Debug, Clone)]
struct DraftView {
    /// The stored draft.
    id: EventSeq,
    /// The voucher kind.
    kind: BaseVoucherKind,
    /// Where the provider runs.
    place: ProviderPlace,
    /// The reader's lowest confidence, per mille.
    confidence: u16,
    /// The table lines.
    lines: Vec<Line>,
    /// The narration the reader wrote from the typed words.
    narration: String,
    /// Names the reader found no ledger for.
    unknown: Vec<String>,
    /// The date offered (today), when no date was typed.
    offered_date: Option<Date>,
    /// The engine's verdict: `Ok(())`, or why it blocks the draft.
    verdict: Result<(), String>,
    /// The character ranges of the sentence the reader took values from.
    marks: Vec<(usize, usize)>,
}

/// What a read gave the screen.
#[derive(Debug)]
enum ReadDone {
    /// A stored draft and the engine's check of it.
    Drafted(Box<DraftView>),
    /// The gate refused.
    Refused(AiGate),
    /// The provider ran and could not read the sentence.
    Failed(String),
}

/// The worker's answers.
#[derive(Debug)]
enum Reply {
    /// The gate for plain words.
    Gate(Result<AiGate, String>),
    /// The read of a sentence.
    Read(Result<ReadDone, String>),
    /// The accept: the posted voucher's number, or why nothing was posted.
    Accepted(Result<Result<String, String>, String>),
    /// The reject was recorded.
    Rejected(Result<(), String>),
}

/// Where the screen is.
#[derive(Debug, Clone)]
enum Phase {
    /// Waiting for a sentence.
    Idle,
    /// The sentence is with the assistant.
    Reading,
    /// A draft waits for the person.
    Draft(Box<DraftView>),
    /// Accepted; the engine posted the voucher with this number.
    Posted(Box<DraftView>, String),
    /// Rejected; nothing was posted.
    Rejected(Box<DraftView>),
    /// The provider could not read the sentence.
    Failed(String),
}

/// The assistant half of the Plain-words entry screen.
#[derive(Debug)]
pub(crate) struct AiEntry {
    /// The gate, once read.
    gate: Option<Result<AiGate, String>>,
    /// The sentence being typed.
    text: String,
    /// The sentence the last read was of.
    read_text: String,
    /// Where the screen is.
    phase: Phase,
    /// A line for the person about the last action.
    notice: Option<String>,
    /// The one call in flight.
    latest: Latest<Reply>,
    /// Whether the box took focus once it could be typed in.
    focused_once: bool,
}

impl Default for AiEntry {
    fn default() -> Self {
        Self {
            gate: None,
            text: String::new(),
            read_text: String::new(),
            phase: Phase::Idle,
            notice: None,
            latest: Latest::default(),
            focused_once: false,
        }
    }
}

/// What the screen asked the stack for.
enum Ask {
    /// Nothing.
    Nothing,
    /// Open a voucher form.
    Voucher(Action),
    /// Open AI and privacy.
    Settings,
}

impl AiEntry {
    /// Whether the gate has been read.
    pub(crate) fn loaded(&self) -> bool {
        self.gate.is_some()
    }

    /// The key bar's name for where the screen is (mock `keys(st)`: its hint with no key).
    pub(crate) fn key_bar_name(&self) -> String {
        self.keys()
            .into_iter()
            .find(|hint| hint.keys.is_empty())
            .map_or_default(|hint| hint.label)
    }

    /// The key bar for where the screen is (mock `SCREENS['quick-entry'].keys`).
    pub(crate) fn keys(&self) -> Vec<KeyHint> {
        match &self.phase {
            Phase::Draft(_) => vec![
                KeyHint::new("", lit("Plain-words draft")),
                KeyHint::new("Ctrl+A", lit("Accept and post")),
                KeyHint::new("Enter", lit("Edit in the voucher form")),
                KeyHint::new("Alt+D", lit("Reject")),
            ],
            Phase::Rejected(_) => vec![
                KeyHint::new("", lit("Rejected")),
                KeyHint::new("F5", lit("Type it myself")),
            ],
            Phase::Idle | Phase::Reading | Phase::Posted(..) | Phase::Failed(_) => vec![
                KeyHint::new("", lit(super::quick_entry::TITLE)),
                KeyHint::new("F5", lit("Payment")),
                KeyHint::new("F6", lit("Receipt")),
            ],
        }
    }

    fn ready(&self) -> bool {
        matches!(self.gate, Some(Ok(AiGate::Ready(_))))
    }

    fn start_gate(&mut self, worker: &ApiWorker) {
        self.latest.start(worker, |api: &mut Api| {
            Reply::Gate(
                api.ai_gate(ledgerly_api::AiFeature::PlainWords)
                    .map_err(|e| e.to_string()),
            )
        });
    }

    fn start_read(&mut self, worker: &ApiWorker, today: Date) {
        let text = self.text.trim().to_owned();
        if text.is_empty() {
            return;
        }
        self.read_text.clone_from(&text);
        self.phase = Phase::Reading;
        self.notice = None;
        self.latest.start(worker, move |api: &mut Api| {
            Reply::Read(read(api, &text, today).map_err(|e| e.to_string()))
        });
    }

    fn start_accept(&mut self, worker: &ApiWorker, view: &DraftView) {
        let id = view.id;
        let edits = DraftEdits {
            date: view.offered_date,
            place_of_supply: None,
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
                self.phase = Phase::Idle;
                return;
            }
            Some(Done::Ready(reply)) => reply,
        };
        match reply {
            Reply::Gate(gate) => self.gate = Some(gate),
            Reply::Read(Ok(ReadDone::Drafted(view))) => self.phase = Phase::Draft(view),
            Reply::Read(Ok(ReadDone::Refused(gate))) => {
                self.gate = Some(Ok(gate));
                self.phase = Phase::Idle;
            }
            Reply::Read(Ok(ReadDone::Failed(reason))) => self.phase = Phase::Failed(reason),
            Reply::Read(Err(reason)) => {
                self.notice = Some(reason);
                self.phase = Phase::Idle;
            }
            Reply::Accepted(result) => {
                if let Phase::Draft(view) = &self.phase {
                    match result {
                        Ok(Ok(number)) => self.phase = Phase::Posted(view.clone(), number),
                        Ok(Err(reason)) | Err(reason) => self.notice = Some(reason),
                    }
                }
            }
            Reply::Rejected(result) => {
                if let Phase::Draft(view) = &self.phase {
                    match result {
                        Ok(()) => self.phase = Phase::Rejected(view.clone()),
                        Err(reason) => self.notice = Some(reason),
                    }
                }
            }
        }
    }

    /// Draws the screen with the assistant and returns what it asked the stack for.
    pub(crate) fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        self.tick();
        if let Some(worker) = ctx.api.as_ref()
            && self.gate.is_none()
            && !self.latest.is_pending()
        {
            self.start_gate(worker);
        }
        let (f5, f6) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::F5),
                i.consume_key(Modifiers::NONE, Key::F6),
            )
        });
        if f5 {
            return opened(
                &mut self.notice,
                ai_gate::open_voucher(Action::NewPayment, ctx),
            );
        }
        if f6 {
            return opened(
                &mut self.notice,
                ai_gate::open_voucher(Action::NewReceipt, ctx),
            );
        }

        crate::ui::page_head::page_head(
            ui,
            &lit(super::quick_entry::TITLE),
            Some(&lit(super::quick_entry::SUBTITLE)),
            |_| {},
        );
        let typing = self.ready() && !matches!(self.phase, Phase::Reading);
        let (submitted, box_focused) = self.sentence_box(ui, typing);
        if submitted && let Some(worker) = ctx.api.as_ref() {
            self.start_read(worker, crate::shell::navigate::today());
        }
        self.read_as(ui);
        ui.add_space(14.0);

        let mut asked = Ask::Nothing;
        let mut action = None;
        egui_extras::StripBuilder::new(ui)
            .size(egui_extras::Size::relative(super::quick_entry::LEFT_SHARE))
            .size(egui_extras::Size::remainder())
            .horizontal(|mut strip| {
                strip.cell(|ui| {
                    (asked, action) = self.left(ui, &ctx.tr, !box_focused);
                });
                strip.cell(super::quick_entry::draw_rules);
            });
        if let Some(action) = action {
            self.act(action, ctx);
        }
        match asked {
            Ask::Nothing => ScreenAction::Stay,
            Ask::Voucher(a) => opened(&mut self.notice, ai_gate::open_voucher(a, ctx)),
            Ask::Settings => opened(&mut self.notice, ai_gate::open_settings(ctx)),
        }
    }

    fn act(&mut self, action: PropAction, ctx: &AppCtx) {
        let Phase::Draft(view) = &self.phase else {
            return;
        };
        let view = view.clone();
        let Some(worker) = ctx.api.as_ref() else {
            return;
        };
        if self.latest.is_pending() {
            return;
        }
        match action {
            PropAction::Accept => self.start_accept(worker, &view),
            PropAction::Reject => self.start_reject(worker, view.id),
            PropAction::Edit => {
                self.notice = Some(lit(
                    "Opening a draft in the voucher form is not built yet; accept it, or reject it and type it with F5.",
                ));
            }
        }
    }

    /// The sentence box. Returns whether Enter submitted it, and whether it has focus.
    fn sentence_box(&mut self, ui: &mut Ui, typing: bool) -> (bool, bool) {
        let t = tokens(ui);
        let mut submitted = false;
        let mut focused = false;
        Frame::new()
            .fill(t.surface)
            .stroke(Stroke::new(border_width(kind(ui)), t.border_strong))
            .corner_radius(RADIUS_M)
            .inner_margin(Margin {
                left: 12,
                right: 8,
                top: 8,
                bottom: 8,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    icon(ui, Icon::Chat, t.text_2);
                    ui.add_space(8.0);
                    let hint = if typing {
                        lit("Type an entry, as you would say it")
                    } else {
                        lit("No local model can run in this build")
                    };
                    let response = ui.add_enabled(
                        typing,
                        TextEdit::singleline(&mut self.text)
                            .hint_text(hint)
                            .font(FontId::proportional(TEXT_INPUT))
                            .frame(Frame::NONE)
                            .desired_width(f32::INFINITY),
                    );
                    if typing && !self.focused_once {
                        response.request_focus();
                        self.focused_once = true;
                    }
                    submitted = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    focused = response.has_focus();
                });
            });
        (submitted, focused)
    }

    /// "Read as:" with the parts the reader took values from marked (mock `markText`).
    fn read_as(&self, ui: &mut Ui) {
        let t = tokens(ui);
        let (text, marks): (&str, &[(usize, usize)]) = match &self.phase {
            Phase::Draft(v) | Phase::Posted(v, _) | Phase::Rejected(v) => {
                (&self.read_text, &v.marks)
            }
            Phase::Reading => (&self.read_text, &[]),
            Phase::Idle | Phase::Failed(_) => ("", &[]),
        };
        let mut job = LayoutJob::default();
        let plain = TextFormat::simple(FontId::proportional(TEXT_META), t.text_3);
        let marked = TextFormat {
            background: t.ai_soft,
            color: t.ai_ink,
            ..TextFormat::simple(FontId::proportional(TEXT_META), t.ai_ink)
        };
        job.append(&lit("Read as: "), 4.0, plain.clone());
        if text.is_empty() {
            job.append("…", 0.0, plain);
        } else {
            for (n, ch) in text.chars().enumerate() {
                let inside = marks.iter().any(|&(s, e)| n >= s && n < e);
                let format = if inside {
                    marked.clone()
                } else {
                    plain.clone()
                };
                job.append(&ch.to_string(), 0.0, format);
            }
        }
        ui.add_space(6.0);
        ui.label(job);
    }

    /// The left card for where the screen is. Returns what it asked for and the proposal action.
    fn left(
        &mut self,
        ui: &mut Ui,
        tr: &crate::i18n::Tr,
        keys_free: bool,
    ) -> (Ask, Option<PropAction>) {
        let mut asked = Ask::Nothing;
        let mut action = None;
        if let Some(line) = self.notice.clone() {
            note(ui, NoteKind::Warn, &line);
            ui.add_space(8.0);
        }
        let gate = self.gate.clone();
        match (&gate, &self.phase) {
            (None, _) => {
                card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    skeleton_rows(ui, 3, ui.available_width());
                });
            }
            (Some(Err(reason)), _) => {
                note(ui, NoteKind::Warn, reason);
            }
            (Some(Ok(AiGate::Ready(_))), Phase::Reading) => reading_card(ui),
            (Some(Ok(AiGate::Ready(_))), Phase::Idle) => idle_card(ui),
            (Some(Ok(AiGate::Ready(_))), Phase::Failed(reason)) => {
                note(
                    ui,
                    NoteKind::Warn,
                    &format!("{} {reason}", lit("The assistant could not read it:")),
                );
                asked = type_it_myself(ui);
            }
            (Some(Ok(_)), Phase::Draft(view)) => {
                let view = view.clone();
                action = draft_card(ui, &view, PropState::Open, keys_free, tr);
            }
            (Some(Ok(_)), Phase::Posted(view, number)) => {
                let view = view.clone();
                let number = number.clone();
                let _ = draft_card(ui, &view, PropState::Done, false, tr);
                ui.add_space(8.0);
                note(
                    ui,
                    NoteKind::Ok,
                    &format!("{} {number} {}", kind_word(view.kind), lit("posted.")),
                );
            }
            (Some(Ok(_)), Phase::Rejected(view)) => {
                let view = view.clone();
                let _ = draft_card(ui, &view, PropState::Rejected, false, tr);
                ui.add_space(8.0);
                if rejected_card(ui) {
                    asked = Ask::Voucher(Action::NewPayment);
                }
            }
            (Some(Ok(refused)), Phase::Idle | Phase::Reading | Phase::Failed(_)) => {
                card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if refusal_note(ui, refused, tr) {
                        asked = Ask::Settings;
                    }
                    ui.add_space(12.0);
                    if let Ask::Voucher(a) = type_it_myself(ui) {
                        asked = Ask::Voucher(a);
                    }
                });
            }
        }
        (asked, action)
    }
}

/// The loading card: what is happening and skeleton rows (mock `quick-entry.loading`).
fn reading_card(ui: &mut Ui) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        let t = tokens(ui);
        ui.horizontal(|ui| {
            icon(ui, Icon::Cpu, t.text_2);
            ui.label(
                RichText::new(lit("Drafting on this computer…"))
                    .size(TEXT_BODY)
                    .color(t.text_2),
            );
        });
        ui.add_space(8.0);
        skeleton_rows(ui, 6, ui.available_width());
    });
}

/// The card before any sentence is read.
fn idle_card(ui: &mut Ui) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        let t = tokens(ui);
        ui.label(para_text(
            &lit("Press Enter to draft. The draft waits for you; nothing is posted until you accept."),
            TEXT_BODY,
            t.text_2,
        ));
    });
}

/// The card under a rejected draft. Returns whether "Type it myself" was clicked.
fn rejected_card(ui: &mut Ui) -> bool {
    let mut clicked = false;
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        let t = tokens(ui);
        ui.label(
            RichText::new(lit(
                "Nothing was posted. The rejection is in the AI activity log.",
            ))
            .size(TEXT_BODY)
            .color(t.text_2),
        );
        ui.add_space(6.0);
        clicked = button(
            ui,
            &lit("Type it myself"),
            Some("F5"),
            ButtonKind::Secondary,
        )
        .clicked();
    });
    clicked
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

/// "Type it myself" with the two voucher forms the reader drafts.
fn type_it_myself(ui: &mut Ui) -> Ask {
    let t = tokens(ui);
    let mut asked = Ask::Nothing;
    ui.label(
        RichText::new(lit(super::quick_entry::TYPE_IT_TITLE))
            .font(bold_font(ui, TEXT_H2))
            .color(t.text),
    );
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if button(ui, &lit("Payment"), Some("F5"), ButtonKind::Secondary).clicked() {
            asked = Ask::Voucher(Action::NewPayment);
        }
        if button(ui, &lit("Receipt"), Some("F6"), ButtonKind::Secondary).clicked() {
            asked = Ask::Voucher(Action::NewReceipt);
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
                ai_gate::Feature::PlainWords,
                &ai_gate::Gate::Unset,
                tr,
            );
        }
        AiGate::Off => {
            return ai_gate::gate_note_in(
                ui,
                ai_gate::Feature::PlainWords,
                &ai_gate::Gate::Off,
                tr,
            );
        }
        AiGate::NotInstalled(reason) => format!(
            "{} {}. {}",
            lit("No local model can run."),
            sentence_case(reason),
            lit("Choose a model file in AI and privacy; nothing is downloaded.")
        ),
        AiGate::FeatureOff => lit("Plain-words entry is switched off in AI and privacy."),
        AiGate::NotConfigured(reason) => {
            format!("{} {reason}.", lit("The cloud provider is not set up:"))
        }
        AiGate::Ready(_) => return false,
    };
    note(ui, NoteKind::Warn, &line);
    ui.add_space(6.0);
    button(ui, &lit("AI and privacy"), None, ButtonKind::Secondary).clicked()
}

/// `text` with its first letter upper case, for an engine reason that starts a sentence.
fn sentence_case(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// The voucher kind as the card's head names it.
fn kind_word(kind: BaseVoucherKind) -> String {
    match kind {
        BaseVoucherKind::Payment => lit("Payment"),
        BaseVoucherKind::Receipt => lit("Receipt"),
        BaseVoucherKind::Purchase
        | BaseVoucherKind::Sales
        | BaseVoucherKind::Contra
        | BaseVoucherKind::Journal
        | BaseVoucherKind::CreditNote
        | BaseVoucherKind::DebitNote
        | BaseVoucherKind::Memorandum
        | BaseVoucherKind::ReversingJournal
        | BaseVoucherKind::ReceiptNote
        | BaseVoucherKind::DeliveryNote
        | BaseVoucherKind::RejectionIn
        | BaseVoucherKind::RejectionOut
        | BaseVoucherKind::StockJournal
        | BaseVoucherKind::PhysicalStock
        | BaseVoucherKind::SalesOrder
        | BaseVoucherKind::PurchaseOrder => lit("Voucher"),
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

/// The proposal card (mock `proposal()`): head, the table, narration, date, checks, footer.
fn draft_card(
    ui: &mut Ui,
    view: &DraftView,
    state: PropState,
    keys: bool,
    tr: &crate::i18n::Tr,
) -> Option<PropAction> {
    let blocked = view.verdict.as_ref().err().map(String::as_str);
    let why = lit("Amount copied from your words, not computed");
    let kind_line = format!("{} {}", lit("Draft"), kind_word(view.kind).to_lowercase());
    Prop::new(&why)
        .state(state)
        .accept(blocked.is_none(), blocked)
        .handles_keys(keys)
        .show(
            ui,
            |ui| {
                ui.horizontal(|ui| {
                    let head = match state {
                        PropState::Open => kind_line.clone(),
                        PropState::Done => lit("Accepted"),
                        PropState::Rejected => lit("Rejected"),
                    };
                    ui.label(RichText::new(head).strong());
                    if state == PropState::Open {
                        ui.label(RichText::new(lit("suggested")).size(TEXT_META));
                    }
                    // The chips follow the kind in the row, not pinned right: a right-to-left
                    // layout here takes the band's whole width and widens the card past its cell.
                    ui.add_space(12.0);
                    ui.label(RichText::new(lit("Your words")).size(TEXT_META));
                    ui.label(RichText::new(place_word(view.place)).size(TEXT_META));
                    if state == PropState::Open {
                        let _ = conf(ui, band(view.confidence), tr);
                    }
                });
            },
            |ui| draft_body(ui, view),
            |ui| draft_checks(ui, view),
        )
}

/// The card's table, narration and date.
fn draft_body(ui: &mut Ui, view: &DraftView) {
    let t = tokens(ui);
    egui::Grid::new(("qe-lines", view.id.0))
        .num_columns(3)
        .spacing([16.0, 6.0])
        .min_col_width(40.0)
        .show(ui, |ui| {
            for head in ["Dr / Cr", "Ledger", "Amount"] {
                ui.label(RichText::new(lit(head)).size(TEXT_META).color(t.text_3));
            }
            ui.end_row();
            for line in &view.lines {
                ui.label(match line.side {
                    DrCr::Dr => "Dr",
                    DrCr::Cr => "Cr",
                });
                ui.horizontal(|ui| match &line.ledger {
                    Some(name) => {
                        ui.label(name);
                        let _ = prov(ui, ProvKind::Ai);
                    }
                    None => {
                        ui.label(
                            RichText::new(format!("{} {}", lit("No ledger named"), line.typed))
                                .color(t.bad),
                        );
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(format::amount(line.amount));
                    let _ = prov(ui, line.from);
                });
                ui.end_row();
            }
        });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(lit("Narration"))
                .size(TEXT_META)
                .color(t.text_3),
        );
        let _ = prov(ui, ProvKind::Ai);
        ui.label(&view.narration);
    });
    if let Some(date) = view.offered_date {
        ui.label(
            RichText::new(format!(
                "{} {}{}",
                lit("Date:"),
                format::date(date),
                lit(", today, as for any new voucher; no date was typed.")
            ))
            .size(TEXT_META)
            .color(t.text_2),
        );
    }
    for name in &view.unknown {
        ui.add_space(6.0);
        note(
            ui,
            NoteKind::Warn,
            &format!(
                "{} \"{name}\". {}",
                lit("No party or ledger named"),
                lit("The assistant does not invent ledgers. Pick one, or create it in Masters.")
            )
            .replacen(&format!("\"{name}\". "), &format!("\"{name}\".\n"), 1),
        );
    }
}

/// The engine's checks under the table.
fn draft_checks(ui: &mut Ui, view: &DraftView) {
    let t = tokens(ui);
    let (glyph, color, line) = match &view.verdict {
        Ok(()) => (Icon::Check, t.ok, lit("Dr equals Cr (engine).")),
        Err(reason) => (Icon::X, t.bad, reason.clone()),
    };
    ui.horizontal_wrapped(|ui| {
        icon(ui, glyph, color);
        ui.label(RichText::new(line).size(TEXT_META).color(color));
    });
}

/// Reads `text` on the worker: the gate, the read, and the engine's check with today offered.
fn read(api: &mut Api, text: &str, today: Date) -> Result<ReadDone, ledgerly_api::ApiError> {
    let place = match api.ai_gate(ledgerly_api::AiFeature::PlainWords)? {
        AiGate::Ready(info) => info.place,
        refused @ (AiGate::NotChosen
        | AiGate::Off
        | AiGate::FeatureOff
        | AiGate::NotInstalled(_)
        | AiGate::NotConfigured(_)) => return Ok(ReadDone::Refused(refused)),
    };
    let (id, proposal) = match api.read_words(text, &CancelToken::new())? {
        WordsRead::Drafted { id, proposal } => (id, proposal),
        WordsRead::Refused(gate) => return Ok(ReadDone::Refused(gate)),
        WordsRead::Failed(reason) => return Ok(ReadDone::Failed(reason)),
    };
    let draft = match proposal {
        Proposal::VoucherDraft(d) => *d,
        Proposal::MatchSuggestion(_)
        | Proposal::ReportQuery(_)
        | Proposal::MappingSuggestion(_) => {
            return Ok(ReadDone::Failed(lit(
                "the assistant did not draft a voucher",
            )));
        }
    };
    let offered_date = draft.date.is_none().then_some(today);
    let edits = DraftEdits {
        date: offered_date,
        place_of_supply: None,
    };
    let check = api.check_ai_draft(id, &edits)?;
    let name_of = |id: LedgerId| -> Option<String> {
        match api.books() {
            Ok(Outcome::Value(b)) => b.masters.ledgers.get(&id).map(|l| l.name.clone()),
            Ok(Outcome::Unsupported(_)) | Err(_) => None,
        }
    };
    let (lines, verdict) = match &check {
        Outcome::Value(DraftCheck::Ok(voucher)) => (engine_lines(voucher, &name_of), Ok(())),
        Outcome::Value(DraftCheck::Blocked { reason, .. }) => {
            (draft_lines(&draft, &name_of), Err(reason.clone()))
        }
        Outcome::Unsupported(u) => (draft_lines(&draft, &name_of), Err(unsupported_text(u))),
    };
    let confidence = lowest_confidence(&draft);
    Ok(ReadDone::Drafted(Box::new(DraftView {
        id,
        kind: draft.kind,
        place,
        confidence,
        lines,
        narration: draft.narration.clone(),
        unknown: draft.create_ledgers.clone(),
        offered_date,
        verdict,
        marks: marks_of(&draft),
    })))
}

/// The engine's voucher as table lines: the first amount is the typed one, the other the engine's
/// balancing figure.
fn engine_lines(
    voucher: &ledgerly_model::Voucher,
    name_of: &dyn Fn(LedgerId) -> Option<String>,
) -> Vec<Line> {
    voucher
        .ledger_lines
        .iter()
        .enumerate()
        .map(|(n, l)| Line {
            side: l.amount.side,
            ledger: name_of(l.ledger),
            typed: String::new(),
            amount: l.amount.amount,
            from: if n == 0 {
                ProvKind::YourWords
            } else {
                ProvKind::Engine
            },
        })
        .collect()
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

/// The draft's own lines, when the engine did not make a voucher of it.
fn draft_lines(draft: &VoucherDraft, name_of: &dyn Fn(LedgerId) -> Option<String>) -> Vec<Line> {
    draft
        .lines
        .iter()
        .map(|l| Line {
            side: l.side,
            ledger: l.ledger.and_then(name_of),
            typed: l.ledger_name.value.clone(),
            amount: l.amount.value,
            from: ProvKind::YourWords,
        })
        .collect()
}

/// The character ranges of the sentence the draft's values came from.
fn marks_of(draft: &VoucherDraft) -> Vec<(usize, usize)> {
    let typed = |s: &SourceRef| match s {
        SourceRef::Typed { start, end } => Some((*start, *end)),
        SourceRef::Document { .. } | SourceRef::StatementLine { .. } | SourceRef::Edited => None,
    };
    draft
        .lines
        .iter()
        .flat_map(|l| [typed(&l.amount.source), typed(&l.ledger_name.source)])
        .chain(draft.date.iter().map(|d| typed(&d.source)))
        .flatten()
        .collect()
}
