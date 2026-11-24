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
//! * Accept is disabled unless the engine's check is [`EngineCheck::Ok`]: for a voucher draft, a
//!   bank match and a mapping alike. A blocked, unsupported or not-yet-checked proposal cannot be
//!   accepted. A voucher draft also needs every field and line filled and no ledger still to be
//!   created. A question about the books is an answer, not a proposal to accept, so it has no
//!   Accept at all.
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

use crate::i18n::Tr;
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
    /// The description in the words of `tr`, shown in the panel header.
    #[must_use]
    pub fn describe_in(self, tr: &Tr) -> String {
        tr.t(match self {
            Tier::Off => "assist.tier.off",
            Tier::Local => "assist.tier.local",
            Tier::Cloud => "assist.tier.cloud",
        })
    }

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

    /// The text to draw in the words of `tr`: the value, or the blank text when there is none.
    #[must_use]
    pub fn display_value_in(&self, tr: &Tr) -> String {
        self.value.clone().unwrap_or_else(|| tr.t("assist.blank"))
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
    /// The engine's verdict on the match or mapping. Accept is enabled only on
    /// [`EngineCheck::Ok`], as for a voucher draft: a match whose amounts disagree is `Blocked`
    /// (spec 6.8). Until the engine has checked it, it is [`EngineCheck::NotChecked`].
    pub check: EngineCheck,
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

    /// Updates the engine's verdict on the proposal on show: the voucher draft, or the bank match
    /// or mapping. Does nothing for an answer to a question, which has no verdict to update.
    pub fn set_check(&mut self, check: EngineCheck) {
        match self.proposal.as_mut() {
            Some(ProposalView::Voucher(draft)) => draft.check = check,
            Some(ProposalView::BankMatch(view) | ProposalView::Mapping(view)) => {
                view.check = check;
            }
            Some(ProposalView::Report(_)) | None => {}
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

    /// Sends the text box content as a request, in English: [`Self::submit_input_in`] with the
    /// English file.
    pub fn submit_input(&mut self) {
        self.submit_input_in(&Tr::default());
    }

    /// Sends the text box content as a request. Does nothing for blank text. When the assistant
    /// is off, shows the off text of `tr` instead and sends nothing.
    pub fn submit_input_in(&mut self, tr: &Tr) {
        let text = self.input.trim().to_owned();
        if text.is_empty() {
            return;
        }
        match self.tier {
            Tier::Off => self.error = Some(tr.t("assist.off")),
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

    /// Why Accept is disabled, in English: [`Self::accept_blocked_reason_in`] with the English
    /// file.
    #[must_use]
    pub fn accept_blocked_reason(&self) -> Option<String> {
        self.accept_blocked_reason_in(&Tr::default())
    }

    /// Why Accept is disabled in the words of `tr`, or `None` when it is enabled.
    ///
    /// A voucher draft needs an engine verdict of `Ok` and also no blank field or line and no
    /// ledger still to be created: a posting with any of those would carry an unresolved value.
    /// A bank match or a mapping needs `Ok` on its own [`SuggestionView::check`]. An answer to a
    /// question has nothing to accept.
    #[must_use]
    pub fn accept_blocked_reason_in(&self, tr: &Tr) -> Option<String> {
        if self.tier == Tier::Off {
            return Some(tr.t("assist.off"));
        }
        match self.proposal.as_ref() {
            None => Some(tr.t("assist.nothing_has_been_proposed")),
            Some(ProposalView::Voucher(draft)) => voucher_blocked_reason(draft, tr),
            Some(ProposalView::BankMatch(view)) => {
                check_blocked_reason(&view.check, &tr.t("assist.noun.match"), tr)
            }
            Some(ProposalView::Mapping(view)) => {
                check_blocked_reason(&view.check, &tr.t("assist.noun.mapping"), tr)
            }
            Some(ProposalView::Report(_)) => Some(tr.t("assist.nothing_to_accept")),
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

/// Why an engine verdict blocks Accept, in the words of `tr`, or `None` for `Ok`. `noun` names the
/// thing checked, already in the words of `tr`.
fn check_blocked_reason(check: &EngineCheck, noun: &str, tr: &Tr) -> Option<String> {
    match check {
        EngineCheck::Ok { .. } => None,
        EngineCheck::NotChecked => Some(tr.tf("assist.check.not_checked", &[("noun", noun)])),
        EngineCheck::Blocked { field, reason } => Some(tr.tf(
            "assist.check.blocked",
            &[("field", field.as_str()), ("reason", reason.as_str())],
        )),
        EngineCheck::Unsupported { what, reason } => Some(tr.tf(
            "assist.check.unsupported",
            &[("what", what.as_str()), ("reason", reason.as_str())],
        )),
    }
}

/// Why a voucher draft cannot be accepted, or `None`. The engine's verdict comes first. Then a
/// blank field or line (below the confidence threshold) and a ledger still to be created block
/// Accept: the posting path would otherwise get a value nobody confirmed.
fn voucher_blocked_reason(draft: &DraftView, tr: &Tr) -> Option<String> {
    if let Some(reason) = check_blocked_reason(&draft.check, &tr.t("assist.noun.draft"), tr) {
        return Some(reason);
    }
    if let Some(field) = draft.fields.iter().find(|f| f.is_blank()) {
        return Some(tr.tf("assist.field_blank", &[("label", field.label.as_str())]));
    }
    if draft
        .lines
        .iter()
        .any(|line| line.ledger.is_blank() || line.amount.is_blank())
    {
        return Some(tr.t("assist.line_blank"));
    }
    draft
        .create_ledgers
        .first()
        .map(|name| tr.tf("assist.create_ledger_first", &[("name", name.as_str())]))
}

/// The percentage text of a confidence in per-mille, `92.5%` for `925`, without floats.
fn percent_text(permille: u16) -> String {
    format!("{}.{}%", permille / 10, permille % 10)
}

fn draw_field_row(ui: &mut Ui, field: &ProposedField, tr: &Tr) {
    ui.label(&field.label);
    if field.is_blank() {
        ui.label(RichText::new(field.display_value_in(tr)).italics().weak());
    } else {
        ui.label(field.display_value_in(tr));
    }
    ui.weak(format!("{}  {}", field.confidence_text(), field.source));
    ui.end_row();
}

fn draw_line_row(ui: &mut Ui, line: &ProposedLine, tr: &Tr) {
    let side = match line.side {
        DrCr::Dr => "Dr",
        DrCr::Cr => "Cr",
    };
    ui.label(side);
    ui.label(line.ledger.display_value_in(tr));
    ui.label(line.amount.display_value_in(tr));
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

fn draw_check(ui: &mut Ui, check: &EngineCheck, tr: &Tr) {
    ui.label(RichText::new(tr.t("assist.label.engine_computed")).strong());
    match check {
        EngineCheck::NotChecked => {
            ui.weak(tr.t("assist.check.not_checked_yet"));
        }
        EngineCheck::Ok { figures } => {
            ui.label(RichText::new(tr.t("assist.check.ok")).strong());
            draw_figures(ui, "u11_engine_figures", figures);
        }
        EngineCheck::Blocked { field, reason } => {
            let warn = ui.visuals().error_fg_color;
            ui.label(
                RichText::new(tr.tf("assist.check.blocked_at", &[("field", field.as_str())]))
                    .color(warn)
                    .strong(),
            );
            ui.label(reason);
        }
        EngineCheck::Unsupported { what, reason } => {
            let warn = ui.visuals().warn_fg_color;
            ui.label(
                RichText::new(tr.tf("assist.check.unsupported_title", &[("what", what.as_str())]))
                    .color(warn)
                    .strong(),
            );
            ui.label(reason);
        }
    }
}

fn draw_draft(ui: &mut Ui, draft: &DraftView, tr: &Tr) {
    ui.label(RichText::new(tr.t("assist.label.ai_proposed")).strong());
    ui.label(tr.tf("assist.voucher", &[("kind", draft.kind.as_str())]));
    egui::Grid::new("u11_draft_fields")
        .num_columns(3)
        .show(ui, |ui| {
            for field in &draft.fields {
                draw_field_row(ui, field, tr);
            }
        });
    if !draft.lines.is_empty() {
        ui.separator();
        egui::Grid::new("u11_draft_lines")
            .num_columns(4)
            .show(ui, |ui| {
                for line in &draft.lines {
                    draw_line_row(ui, line, tr);
                }
            });
    }
    for name in &draft.create_ledgers {
        ui.label(tr.tf("assist.create_ledger", &[("name", name.as_str())]));
    }
    ui.separator();
    draw_check(ui, &draft.check, tr);
}

fn draw_suggestion(ui: &mut Ui, view: &SuggestionView, id: &str, tr: &Tr) {
    ui.label(RichText::new(tr.t("assist.label.ai_proposed")).strong());
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
    ui.weak(tr.tf(
        "assist.confidence",
        &[("value", percent_text(view.confidence_permille).as_str())],
    ));
    ui.separator();
    draw_check(ui, &view.check, tr);
}

fn draw_report(ui: &mut Ui, view: &ReportView, tr: &Tr) {
    ui.label(RichText::new(tr.t("assist.label.ai_proposed")).strong());
    ui.label(tr.tf("assist.query", &[("query", view.query.as_str())]));
    ui.separator();
    ui.label(RichText::new(tr.t("assist.label.engine_computed")).strong());
    match &view.answer {
        ReportAnswer::Pending => {
            ui.weak(tr.t("assist.report.not_run"));
        }
        ReportAnswer::Computed(figures) => draw_figures(ui, "u11_report_figures", figures),
        ReportAnswer::Unsupported { what, reason } => {
            let warn = ui.visuals().warn_fg_color;
            ui.label(
                RichText::new(tr.tf("assist.check.unsupported_title", &[("what", what.as_str())]))
                    .color(warn)
                    .strong(),
            );
            ui.label(reason);
        }
    }
}

fn draw_proposal(ui: &mut Ui, proposal: &ProposalView, tr: &Tr) {
    match proposal {
        ProposalView::Voucher(draft) => draw_draft(ui, draft, tr),
        ProposalView::BankMatch(view) => draw_suggestion(ui, view, "u11_bank_match", tr),
        ProposalView::Mapping(view) => draw_suggestion(ui, view, "u11_mapping", tr),
        ProposalView::Report(view) => draw_report(ui, view, tr),
    }
}

impl AssistScreen {
    /// The input row. Returns whether the person asked to send.
    fn draw_input(&mut self, ui: &mut Ui, tr: &Tr) -> bool {
        let mut send = false;
        ui.horizontal(|ui| {
            let edit = ui.add(
                TextEdit::singleline(&mut self.input)
                    .hint_text(tr.t("assist.input_hint"))
                    .desired_width(ui.available_width() - 80.0),
            );
            if edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                send = true;
            }
            let enabled = self.tier != Tier::Off && !self.waiting;
            if ui
                .add_enabled(enabled, Button::new(tr.t("assist.ask")))
                .clicked()
            {
                send = true;
            }
        });
        send
    }

    /// Whether the Accept button is drawn: a proposal other than an answer to a question.
    fn shows_accept(&self) -> bool {
        self.proposal
            .as_ref()
            .is_some_and(|p| !matches!(p, ProposalView::Report(_)))
    }

    /// The Accept and Reject buttons. Returns `(accept, reject)` clicks.
    fn draw_buttons(&self, ui: &mut Ui, tr: &Tr) -> (bool, bool) {
        let mut accept = false;
        let mut reject = false;
        let can_accept = self.can_accept();
        let is_report = matches!(self.proposal, Some(ProposalView::Report(_)));
        ui.horizontal(|ui| {
            if !is_report {
                let button = ui.add_enabled(can_accept, Button::new(tr.t("assist.accept")));
                accept = button.clicked();
            }
            let label = if is_report {
                tr.t("assist.dismiss")
            } else {
                tr.t("assist.reject")
            };
            reject = ui.button(label).clicked();
        });
        let reason = self.accept_blocked_reason_in(tr).filter(|_| !is_report);
        if let Some(reason) = reason {
            ui.weak(tr.tf("assist.accept_disabled", &[("reason", reason.as_str())]));
        }
        (accept, reject)
    }
}

impl Screen for AssistScreen {
    fn title(&self, tr: &Tr) -> String {
        tr.t("assist.title")
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut AppCtx) -> ScreenAction {
        if self.closing {
            self.closing = false;
            return ScreenAction::Pop;
        }
        let tr = ctx.tr.clone();
        // Ctrl+A is read before the input row draws, so the text field does not also select all.
        // voucher_entry.rs reads its save key the same way. It is read only while Accept is shown,
        // so Ctrl+A keeps its normal meaning in the text field otherwise.
        let key_accept =
            self.shows_accept() && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::A));
        ui.heading(tr.t("assist.title"));
        ui.weak(self.tier.describe_in(&tr));
        if self.draw_input(ui, &tr) {
            self.submit_input_in(&tr);
        }
        if self.waiting {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(tr.t("assist.waiting"));
            });
        }
        if let Some(message) = self.error.as_deref() {
            let color = ui.visuals().error_fg_color;
            ui.label(RichText::new(tr.tf("assist.problem", &[("message", message)])).color(color));
        }
        ui.separator();
        match self.proposal.as_ref() {
            Some(proposal) => draw_proposal(ui, proposal, &tr),
            None => {
                ui.weak(tr.t("assist.nothing_proposed"));
            }
        }
        if self.proposal.is_none() {
            return ScreenAction::Stay;
        }
        ui.separator();
        let (accept, reject) = self.draw_buttons(ui, &tr);
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
