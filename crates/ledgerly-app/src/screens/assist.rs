//! Assistant panel. **Card U11** implements this module (spec 6.8).
//!
//! Frozen interface (plan card U11):
//!
//! ```text
//! pub struct <Name>Screen { .. }
//! impl Screen for <Name>Screen { .. }
//! ```
//!
//! Here `<Name>` is `Assist`: [`AssistScreen`].
//!
//! What the panel shows, and what it never does:
//!
//! * Everything the assistant read or inferred sits under the label [`LABEL_AI_PROPOSED`], with its
//!   source and confidence. A value below the confidence threshold arrives as `None` and is drawn
//!   as blank ([`BLANK_TEXT`]), never as a guess (spec 6.8: "below threshold -> blank").
//! * Every number the engine recomputed sits under the label [`LABEL_ENGINE_COMPUTED`]. The panel
//!   never adds, subtracts or rounds anything (rule C4): it draws the `Paise` it was handed.
//! * Accept is disabled unless the engine's check of a voucher draft is [`EngineCheck::Ok`]. A
//!   blocked, unsupported or not-yet-checked draft cannot be accepted. A question about the books
//!   is an answer, not a proposal to accept, so it has no Accept at all.
//! * The panel never posts anything. Accept and Reject are recorded as a [`PanelAction`] that the
//!   Wave 2 wiring takes with [`AssistScreen::take_action`]; the engine posts, after a person
//!   confirms (spec 6.8, rule A10).
//!
//! # Why these view types exist
//!
//! `ledgerly-app` does not depend on `ledgerly-assist` yet (the manifest is frozen), so the panel
//! carries its own plain view types ([`DraftView`], [`EngineCheck`], ...). The wiring card builds
//! them from `ledgerly_assist::{Proposal, DraftCheck}`: `DraftCheck::Ok(voucher)` becomes
//! [`EngineCheck::Ok`] with the voucher's engine-computed figures, `DraftCheck::Blocked` becomes
//! [`EngineCheck::Blocked`], and `AssistError::Unsupported` becomes [`EngineCheck::Unsupported`].
//! See `docs/interface-requests.md` (U11).

use egui::{Button, Key, Modifiers, RichText, TextEdit, Ui};
use ledgerly_core::format::format_inr;
use ledgerly_core::{DrCr, Paise};

use crate::{AppCtx, Screen, ScreenAction};

/// Heading of everything the assistant proposed.
pub const LABEL_AI_PROPOSED: &str = "AI proposed";
/// Heading of everything the engine computed.
pub const LABEL_ENGINE_COMPUTED: &str = "Engine computed";
/// Shown in place of a value the assistant was not confident enough to give.
pub const BLANK_TEXT: &str = "(blank: confidence too low)";
/// Shown when the assistant is turned off for the company.
pub const OFF_TEXT: &str = "The assistant is turned off for this company.";

/// The company's assistant tier (spec 6.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tier {
    /// No assistant.
    #[default]
    Off,
    /// On-device model.
    Local,
    /// The user's own cloud key, with PII redaction.
    Cloud,
}

impl Tier {
    /// Short description shown in the panel header.
    #[must_use]
    pub fn describe(self) -> &'static str {
        match self {
            Tier::Off => "Assistant: off",
            Tier::Local => "Assistant: local model",
            Tier::Cloud => "Assistant: cloud (your own key, personal data redacted)",
        }
    }
}

/// One value the assistant proposed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedField {
    /// What the value is, such as `Party` or `Invoice number`.
    pub label: String,
    /// The value as text; `None` when it fell below the confidence threshold (blank).
    pub value: Option<String>,
    /// Confidence in per-mille (0..=1000).
    pub confidence_permille: u16,
    /// Where it was read, in words (`Typed text, characters 5-9`, `Page 1 of the bill`).
    pub source: String,
}

impl ProposedField {
    /// The text to draw: the value, or [`BLANK_TEXT`] when there is none.
    #[must_use]
    pub fn display_value(&self) -> &str {
        self.value.as_deref().unwrap_or(BLANK_TEXT)
    }

    /// Whether the field is blank.
    #[must_use]
    pub fn is_blank(&self) -> bool {
        self.value.is_none()
    }

    /// The confidence as a percentage with one decimal, from per-mille, without floats
    /// (`925` -> `92.5%`).
    #[must_use]
    pub fn confidence_text(&self) -> String {
        format!(
            "{}.{}%",
            self.confidence_permille / 10,
            self.confidence_permille % 10
        )
    }
}

/// One proposed accounting line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedLine {
    /// The ledger.
    pub ledger: ProposedField,
    /// Side.
    pub side: DrCr,
    /// The amount as proposed (text from the assistant's transcription).
    pub amount: ProposedField,
}

/// A figure the engine computed, drawn as given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputedFigure {
    /// What it is, such as `Total` or `CGST`.
    pub label: String,
    /// The engine's amount.
    pub amount: Paise,
}

/// What the engine made of a draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineCheck {
    /// The engine has not been run on this draft yet.
    NotChecked,
    /// The engine rebuilt the voucher and reproduced the document's figures.
    Ok {
        /// The figures the engine computed.
        figures: Vec<ComputedFigure>,
    },
    /// The draft cannot be accepted as it stands.
    Blocked {
        /// The draft field at fault.
        field: String,
        /// What is wrong and what to do.
        reason: String,
    },
    /// The engine does not support what the draft needs.
    Unsupported {
        /// The construct that is not handled.
        what: String,
        /// Why, and what the user can do.
        reason: String,
    },
}

/// A proposed voucher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftView {
    /// Voucher kind, in words.
    pub kind: String,
    /// Header values (date, party, invoice number, document total, ...).
    pub fields: Vec<ProposedField>,
    /// Accounting lines.
    pub lines: Vec<ProposedLine>,
    /// Ledgers the user would have to create first.
    pub create_ledgers: Vec<String>,
    /// The engine's verdict.
    pub check: EngineCheck,
}

/// A bank-line match or an import column mapping: rows of `(left, right)` text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestionView {
    /// What is being suggested.
    pub heading: String,
    /// The suggested pairs.
    pub rows: Vec<(String, String)>,
    /// Why, when the assistant said.
    pub reason: Option<String>,
    /// Confidence in per-mille.
    pub confidence_permille: u16,
}

/// What a question about the books came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportAnswer {
    /// The engine has not run the query yet.
    Pending,
    /// The engine ran it.
    Computed(Vec<ComputedFigure>),
    /// The engine cannot answer it.
    Unsupported {
        /// The construct that is not handled.
        what: String,
        /// Why.
        reason: String,
    },
}

/// A question answered by an engine query. The query is always shown with the answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportView {
    /// The query the engine ran, in words (`trial_balance, 01-04-2025 to 30-04-2025`).
    pub query: String,
    /// The answer.
    pub answer: ReportAnswer,
}

/// What the assistant proposed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalView {
    /// A voucher to review.
    Voucher(DraftView),
    /// A bank line match.
    BankMatch(SuggestionView),
    /// An import column mapping.
    Mapping(SuggestionView),
    /// A report the engine ran for a question.
    Report(ReportView),
}

/// What the person decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelAction {
    /// Hand the proposal to the engine to post (voucher) or apply (match, mapping).
    Accept,
    /// Throw the proposal away.
    Reject,
}

/// The assistant panel.
#[derive(Debug, Default)]
pub struct AssistScreen {
    tier: Tier,
    input: String,
    proposal: Option<ProposalView>,
    request: Option<String>,
    action: Option<PanelAction>,
    waiting: bool,
    error: Option<String>,
    closing: bool,
}

impl AssistScreen {
    /// A panel for a company whose assistant tier is `tier`.
    #[must_use]
    pub fn new(tier: Tier) -> Self {
        Self {
            tier,
            ..Self::default()
        }
    }

    /// The company's tier.
    #[must_use]
    pub fn tier(&self) -> Tier {
        self.tier
    }

    /// Changes the tier (the company setting changed).
    pub fn set_tier(&mut self, tier: Tier) {
        self.tier = tier;
    }

    /// Replaces the text box content.
    pub fn set_input(&mut self, text: &str) {
        text.clone_into(&mut self.input);
    }

    /// Shows a proposal and clears any waiting state, error and earlier decision.
    pub fn set_proposal(&mut self, proposal: ProposalView) {
        self.proposal = Some(proposal);
        self.waiting = false;
        self.error = None;
        self.action = None;
    }

    /// Updates the engine's verdict on the voucher draft on show. Does nothing when the proposal
    /// on show is not a voucher.
    pub fn set_check(&mut self, check: EngineCheck) {
        if let Some(ProposalView::Voucher(draft)) = self.proposal.as_mut() {
            draft.check = check;
        }
    }

    /// Shows an error from the assistant (a refused request, a network problem) and stops waiting.
    pub fn set_error(&mut self, message: &str) {
        self.error = Some(message.to_owned());
        self.waiting = false;
    }

    /// The proposal on show.
    #[must_use]
    pub fn proposal(&self) -> Option<&ProposalView> {
        self.proposal.as_ref()
    }

    /// The error on show.
    #[must_use]
    pub fn error_text(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Whether a request has been sent and no answer has arrived.
    #[must_use]
    pub fn is_waiting(&self) -> bool {
        self.waiting
    }

    /// Sends the text box content as a request. Does nothing for blank text. When the assistant
    /// is off, shows [`OFF_TEXT`] instead and sends nothing.
    pub fn submit_input(&mut self) {
        let text = self.input.trim().to_owned();
        if text.is_empty() {
            return;
        }
        match self.tier {
            Tier::Off => OFF_TEXT.clone_into(self.error.get_or_insert_with(String::new)),
            Tier::Local | Tier::Cloud => {
                self.error = None;
                self.waiting = true;
                self.request = Some(text);
            }
        }
    }

    /// The request to send to the assistant, once.
    pub fn take_request(&mut self) -> Option<String> {
        self.request.take()
    }

    /// Why Accept is disabled, or `None` when it is enabled.
    ///
    /// For a voucher draft this follows the engine's verdict alone. Blank fields (below the
    /// confidence threshold) and ledgers still to be created are the engine check's business:
    /// the host must answer `Blocked` for a draft with either, and only an `Ok` verdict enables
    /// Accept. The panel does not second-guess the engine with rules of its own.
    #[must_use]
    pub fn accept_blocked_reason(&self) -> Option<String> {
        if self.tier == Tier::Off {
            return Some(OFF_TEXT.to_owned());
        }
        match self.proposal.as_ref() {
            None => Some("Nothing has been proposed.".to_owned()),
            Some(ProposalView::Voucher(draft)) => match &draft.check {
                EngineCheck::Ok { .. } => None,
                EngineCheck::NotChecked => {
                    Some("The engine has not checked this draft yet.".to_owned())
                }
                EngineCheck::Blocked { field, reason } => {
                    Some(format!("Blocked at {field}: {reason}"))
                }
                EngineCheck::Unsupported { what, reason } => {
                    Some(format!("Not supported yet ({what}): {reason}"))
                }
            },
            Some(ProposalView::BankMatch(_) | ProposalView::Mapping(_)) => None,
            Some(ProposalView::Report(_)) => Some("An answer has nothing to accept.".to_owned()),
        }
    }

    /// Whether Accept is enabled: the assistant is on and, for a voucher draft, the engine's check
    /// passed.
    #[must_use]
    pub fn can_accept(&self) -> bool {
        self.accept_blocked_reason().is_none()
    }

    /// Records Accept. Returns `false` and records nothing while Accept is disabled.
    pub fn accept(&mut self) -> bool {
        if self.can_accept() {
            self.action = Some(PanelAction::Accept);
            true
        } else {
            false
        }
    }

    /// Records Reject, drops the proposal and closes the panel on the next frame.
    pub fn reject(&mut self) {
        self.action = Some(PanelAction::Reject);
        self.proposal = None;
        self.closing = true;
    }

    /// The person's decision, once.
    pub fn take_action(&mut self) -> Option<PanelAction> {
        self.action.take()
    }
}

// ---------------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------------

fn draw_field_row(ui: &mut Ui, field: &ProposedField) {
    ui.label(&field.label);
    if field.is_blank() {
        ui.label(RichText::new(field.display_value()).italics().weak());
    } else {
        ui.label(field.display_value());
    }
    ui.weak(format!("{}  {}", field.confidence_text(), field.source));
    ui.end_row();
}

fn draw_line_row(ui: &mut Ui, line: &ProposedLine) {
    let side = match line.side {
        DrCr::Dr => "Dr",
        DrCr::Cr => "Cr",
    };
    ui.label(side);
    ui.label(line.ledger.display_value());
    ui.label(line.amount.display_value());
    ui.weak(format!(
        "{}  {}",
        line.amount.confidence_text(),
        line.amount.source
    ));
    ui.end_row();
}

fn draw_figures(ui: &mut Ui, id: &str, figures: &[ComputedFigure]) {
    egui::Grid::new(id).num_columns(2).show(ui, |ui| {
        for figure in figures {
            ui.label(&figure.label);
            ui.label(format_inr(figure.amount));
            ui.end_row();
        }
    });
}

fn draw_check(ui: &mut Ui, check: &EngineCheck) {
    ui.label(RichText::new(LABEL_ENGINE_COMPUTED).strong());
    match check {
        EngineCheck::NotChecked => {
            ui.weak("Not checked yet. No figure is shown until the engine has run.");
        }
        EngineCheck::Ok { figures } => {
            ui.label(RichText::new("Checked: the engine reproduced the figures.").strong());
            draw_figures(ui, "u11_engine_figures", figures);
        }
        EngineCheck::Blocked { field, reason } => {
            let warn = ui.visuals().error_fg_color;
            ui.label(
                RichText::new(format!("Blocked at {field}"))
                    .color(warn)
                    .strong(),
            );
            ui.label(reason);
        }
        EngineCheck::Unsupported { what, reason } => {
            let warn = ui.visuals().warn_fg_color;
            ui.label(
                RichText::new(format!("Not supported yet: {what}"))
                    .color(warn)
                    .strong(),
            );
            ui.label(reason);
        }
    }
}

fn draw_draft(ui: &mut Ui, draft: &DraftView) {
    ui.label(RichText::new(LABEL_AI_PROPOSED).strong());
    ui.label(format!("Voucher: {}", draft.kind));
    egui::Grid::new("u11_draft_fields")
        .num_columns(3)
        .show(ui, |ui| {
            for field in &draft.fields {
                draw_field_row(ui, field);
            }
        });
    if !draft.lines.is_empty() {
        ui.separator();
        egui::Grid::new("u11_draft_lines")
            .num_columns(4)
            .show(ui, |ui| {
                for line in &draft.lines {
                    draw_line_row(ui, line);
                }
            });
    }
    for name in &draft.create_ledgers {
        ui.label(format!("You would need to create the ledger: {name}"));
    }
    ui.separator();
    draw_check(ui, &draft.check);
}

fn draw_suggestion(ui: &mut Ui, view: &SuggestionView, id: &str) {
    ui.label(RichText::new(LABEL_AI_PROPOSED).strong());
    ui.label(&view.heading);
    egui::Grid::new(id).num_columns(2).show(ui, |ui| {
        for (left, right) in &view.rows {
            ui.label(left);
            ui.label(right);
            ui.end_row();
        }
    });
    if let Some(reason) = view.reason.as_deref() {
        ui.weak(reason);
    }
    ui.weak(format!(
        "Confidence {}.{}%",
        view.confidence_permille / 10,
        view.confidence_permille % 10
    ));
}

fn draw_report(ui: &mut Ui, view: &ReportView) {
    ui.label(RichText::new(LABEL_AI_PROPOSED).strong());
    ui.label(format!("Query: {}", view.query));
    ui.separator();
    ui.label(RichText::new(LABEL_ENGINE_COMPUTED).strong());
    match &view.answer {
        ReportAnswer::Pending => {
            ui.weak("The engine has not run this query yet.");
        }
        ReportAnswer::Computed(figures) => draw_figures(ui, "u11_report_figures", figures),
        ReportAnswer::Unsupported { what, reason } => {
            let warn = ui.visuals().warn_fg_color;
            ui.label(
                RichText::new(format!("Not supported yet: {what}"))
                    .color(warn)
                    .strong(),
            );
            ui.label(reason);
        }
    }
}

fn draw_proposal(ui: &mut Ui, proposal: &ProposalView) {
    match proposal {
        ProposalView::Voucher(draft) => draw_draft(ui, draft),
        ProposalView::BankMatch(view) => draw_suggestion(ui, view, "u11_bank_match"),
        ProposalView::Mapping(view) => draw_suggestion(ui, view, "u11_mapping"),
        ProposalView::Report(view) => draw_report(ui, view),
    }
}

impl AssistScreen {
    /// The input row. Returns whether the person asked to send.
    fn draw_input(&mut self, ui: &mut Ui) -> bool {
        let mut send = false;
        ui.horizontal(|ui| {
            let edit = ui.add(
                TextEdit::singleline(&mut self.input)
                    .hint_text("Describe a voucher or ask a question")
                    .desired_width(ui.available_width() - 80.0),
            );
            if edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                send = true;
            }
            let enabled = self.tier != Tier::Off && !self.waiting;
            if ui.add_enabled(enabled, Button::new("Ask")).clicked() {
                send = true;
            }
        });
        send
    }

    /// The Accept and Reject buttons. Returns `(accept, reject)` clicks.
    fn draw_buttons(&self, ui: &mut Ui) -> (bool, bool) {
        let mut accept = false;
        let mut reject = false;
        let can_accept = self.can_accept();
        let is_report = matches!(self.proposal, Some(ProposalView::Report(_)));
        ui.horizontal(|ui| {
            if !is_report {
                let button = ui.add_enabled(can_accept, Button::new("Accept (Ctrl+Enter)"));
                accept = button.clicked();
            }
            let label = if is_report { "Dismiss" } else { "Reject" };
            reject = ui.button(label).clicked();
        });
        let reason = self.accept_blocked_reason().filter(|_| !is_report);
        if let Some(reason) = reason {
            ui.weak(format!("Accept is disabled. {reason}"));
        }
        (accept, reject)
    }
}

impl Screen for AssistScreen {
    fn title(&self) -> String {
        "Assistant".to_owned()
    }

    fn ui(&mut self, ui: &mut Ui, _ctx: &mut AppCtx) -> ScreenAction {
        if self.closing {
            self.closing = false;
            return ScreenAction::Pop;
        }
        ui.heading("Assistant");
        ui.weak(self.tier.describe());
        if self.draw_input(ui) {
            self.submit_input();
        }
        if self.waiting {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Waiting for the assistant...");
            });
        }
        if let Some(message) = self.error.as_deref() {
            let color = ui.visuals().error_fg_color;
            ui.label(RichText::new(format!("Problem: {message}")).color(color));
        }
        ui.separator();
        match self.proposal.as_ref() {
            Some(proposal) => draw_proposal(ui, proposal),
            None => {
                ui.weak("Nothing proposed yet.");
            }
        }
        if self.proposal.is_none() {
            return ScreenAction::Stay;
        }
        ui.separator();
        let (accept, reject) = self.draw_buttons(ui);
        let key_accept = ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
        if accept || key_accept {
            self.accept();
        }
        if reject {
            self.reject();
            self.closing = false;
            return ScreenAction::Pop;
        }
        ScreenAction::Stay
    }
}
