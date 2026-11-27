//! AI proposal card (`.prop` in the mock). Every AI result uses this one component, with the same
//! three keys everywhere: Accept `Ctrl+A`, Edit `Enter`, Reject `Alt+D`.
//!
//! The card lays out what the caller gives it: a head (kind, source, confidence), a body, a list
//! of checks and a footer. It never computes or formats an amount. The caller decides whether
//! Accept is allowed, from the engine's checks, and passes that in with [`Prop::accept`].

use egui::{
    CornerRadius, Frame, Key, Margin, Modifiers, Rect, RichText, Shape, Stroke, StrokeKind, Ui,
    pos2,
};

use super::button::{ButtonKind, button};
use super::{kind, tokens};
use crate::theme::{RADIUS_M, SPACE_2, TEXT_META, ThemeKind, Tokens, border_width};

/// The three actions of a proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropAction {
    /// Save the proposal as a voucher or a record. Key `Ctrl+A`.
    Accept,
    /// Open the proposal's fields for editing. Key `Enter`.
    Edit,
    /// Discard the proposal. Key `Alt+D`.
    Reject,
}

impl PropAction {
    /// The button label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            PropAction::Accept => "Accept",
            PropAction::Edit => "Edit",
            PropAction::Reject => "Reject",
        }
    }

    /// The key shown on the button, in words.
    #[must_use]
    pub fn shortcut(self) -> &'static str {
        match self {
            PropAction::Accept => "Ctrl+A",
            PropAction::Edit => "Enter",
            PropAction::Reject => "Alt+D",
        }
    }

    /// The modifiers and key that trigger this action.
    #[must_use]
    pub fn key_press(self) -> (Modifiers, Key) {
        match self {
            PropAction::Accept => (Modifiers::CTRL, Key::A),
            PropAction::Edit => (Modifiers::NONE, Key::Enter),
            PropAction::Reject => (Modifiers::ALT, Key::D),
        }
    }
}

/// Where a proposal is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PropState {
    /// Waiting for a decision. Dashed AI outline.
    #[default]
    Open,
    /// Accepted. Solid green outline; the buttons are off.
    Done,
    /// Rejected. Solid grey outline; the buttons are off.
    Rejected,
}

/// Width of the heavy bar on the left edge, in points (6 in high contrast).
const BAR_WIDTH: f32 = 4.0;
/// Width of the heavy bar on the left edge in high contrast.
const BAR_WIDTH_HIGH_CONTRAST: f32 = 6.0;
/// Dash and gap lengths of the AI outline, in points.
const DASH: f32 = 6.0;
/// Gap between dashes.
const GAP: f32 = 4.0;

/// Builder for one proposal card. Create it with [`Prop::new`], set what applies, then call
/// [`Prop::show`].
#[derive(Debug, Clone, Copy)]
pub struct Prop<'a> {
    /// The footer text: why the proposal exists, or the reason Accept is blocked.
    why: &'a str,
    /// Where the proposal is.
    state: PropState,
    /// Whether Accept may fire. False while a check blocks it.
    accept_enabled: bool,
    /// Why Accept is blocked, in words. Shown in the footer while blocked.
    accept_reason: Option<&'a str>,
    /// Whether this card takes the proposal keys this frame. Off by default, so only the card
    /// the user is working on reacts to Ctrl+A. Turn it on only when no text field has focus.
    handles_keys: bool,
    /// The words on the Accept, Edit and Reject buttons. The keys stay the same.
    labels: [&'a str; 3],
}

impl<'a> Prop<'a> {
    /// A proposal card with footer text `why`, open, with Accept allowed and no key handling.
    #[must_use]
    pub fn new(why: &'a str) -> Self {
        Self {
            why,
            state: PropState::Open,
            accept_enabled: true,
            accept_reason: None,
            handles_keys: false,
            labels: [
                PropAction::Accept.label(),
                PropAction::Edit.label(),
                PropAction::Reject.label(),
            ],
        }
    }

    /// Sets the words on the Accept, Edit and Reject buttons, for a proposal whose actions the
    /// generic words do not name (Ask your books: Save as report, Open the full report, Dismiss).
    /// The keys stay Ctrl+A, Enter and Alt+D.
    #[must_use]
    pub fn labels(mut self, accept: &'a str, edit: &'a str, reject: &'a str) -> Self {
        self.labels = [accept, edit, reject];
        self
    }

    /// Sets the state of the proposal.
    #[must_use]
    pub fn state(mut self, state: PropState) -> Self {
        self.state = state;
        self
    }

    /// Sets whether Accept may fire, and the reason it may not, if the caller has one.
    #[must_use]
    pub fn accept(mut self, enabled: bool, reason: Option<&'a str>) -> Self {
        self.accept_enabled = enabled;
        self.accept_reason = reason;
        self
    }

    /// Sets whether this card takes the proposal keys this frame.
    #[must_use]
    pub fn handles_keys(mut self, handles: bool) -> Self {
        self.handles_keys = handles;
        self
    }

    /// Draws the card and returns the action taken this frame, if any. A click on a button and
    /// a key press both count. A blocked Accept never returns [`PropAction::Accept`].
    pub fn show(
        self,
        ui: &mut Ui,
        head: impl FnOnce(&mut Ui),
        body: impl FnOnce(&mut Ui),
        checks: impl FnOnce(&mut Ui),
    ) -> Option<PropAction> {
        let t = tokens(ui);
        let mut clicked = None;
        let frame = Frame::new()
            .fill(t.surface)
            .corner_radius(RADIUS_M)
            .show(ui, |ui| {
                head_section(ui, self.state, &t, head);
                Frame::new()
                    .inner_margin(Margin::symmetric(14, 12))
                    .show(ui, |ui| {
                        body(ui);
                        ui.add_space(SPACE_2);
                        checks(ui);
                    });
                ui.separator();
                clicked = footer(ui, &self, &t);
            });
        let k = kind(ui);
        paint_edges(ui, frame.response.rect, self.state, &t, k);
        if self.handles_keys && self.state == PropState::Open {
            clicked = take_key_action(ui)
                .and_then(|action| resolve(action, self.accept_enabled))
                .or(clicked);
        }
        clicked
    }
}

/// The action a key press asks for, given whether Accept is allowed. Returns `None` for a blocked
/// Accept, so a key cannot save a proposal that a check has blocked.
#[must_use]
pub fn resolve(action: PropAction, accept_enabled: bool) -> Option<PropAction> {
    match action {
        PropAction::Accept => accept_enabled.then_some(PropAction::Accept),
        PropAction::Edit | PropAction::Reject => Some(action),
    }
}

/// Consumes the first proposal key pressed this frame and returns its action. Call only when no
/// text field has focus, or `Enter` would be taken from the field.
pub fn take_key_action(ui: &mut Ui) -> Option<PropAction> {
    ui.input_mut(|input| {
        [PropAction::Accept, PropAction::Edit, PropAction::Reject]
            .into_iter()
            .find(|action| {
                let (modifiers, key) = action.key_press();
                input.consume_key(modifiers, key)
            })
    })
}

/// The head: a tinted band whose text colour follows the state.
fn head_section(ui: &mut Ui, state: PropState, t: &Tokens, head: impl FnOnce(&mut Ui)) {
    let (bg, fg) = match state {
        PropState::Open => (t.ai_soft, t.ai_ink),
        PropState::Done => (t.ok_soft, t.ok),
        PropState::Rejected => (t.sunken, t.text_2),
    };
    Frame::new()
        .fill(bg)
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.visuals_mut().override_text_color = Some(fg);
            ui.horizontal_wrapped(head);
        });
}

/// The footer: the reason (or the block reason), then Edit, Reject and Accept with their keys.
/// Returns the button clicked this frame.
fn footer(ui: &mut Ui, prop: &Prop<'_>, t: &Tokens) -> Option<PropAction> {
    let open = prop.state == PropState::Open;
    let accept_on = open && prop.accept_enabled;
    let mut clicked = None;
    Frame::new()
        .fill(t.surface_2)
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let text = match prop.accept_reason {
                    Some(reason) if !prop.accept_enabled => reason,
                    _ => prop.why,
                };
                ui.label(RichText::new(text).size(TEXT_META).color(t.text_2));
                clicked = footer_buttons(ui, open, accept_on, prop.labels);
            });
        });
    clicked
}

/// The three buttons of the footer, in the order Edit, Reject, Accept, after the reason. Each is
/// placed straight in the footer's wrapping row (no child scope), so a narrow card wraps the
/// buttons onto the next line, as the mock's footer wraps, instead of widening the card. Accept is
/// enabled only when the card is open, so disabling the row is monotonic: the whole row when it is
/// not open, then Accept when it is blocked.
fn footer_buttons(
    ui: &mut Ui,
    open: bool,
    accept_on: bool,
    [accept, edit, reject]: [&str; 3],
) -> Option<PropAction> {
    let mut clicked = None;
    if !open {
        ui.disable();
    }
    if button(
        ui,
        edit,
        Some(PropAction::Edit.shortcut()),
        ButtonKind::Secondary,
    )
    .clicked()
    {
        clicked = Some(PropAction::Edit);
    }
    if button(
        ui,
        reject,
        Some(PropAction::Reject.shortcut()),
        ButtonKind::Danger,
    )
    .clicked()
    {
        clicked = Some(PropAction::Reject);
    }
    if !accept_on {
        ui.disable();
    }
    if button(
        ui,
        accept,
        Some(PropAction::Accept.shortcut()),
        ButtonKind::Primary,
    )
    .clicked()
    {
        clicked = Some(PropAction::Accept);
    }
    clicked
}

/// Paints the left bar and the outline over the card's rectangle `rect`.
fn paint_edges(ui: &Ui, rect: Rect, state: PropState, t: &Tokens, k: ThemeKind) {
    let (color, rim_is_dashed) = match state {
        PropState::Open => (t.ai, true),
        PropState::Done => (t.ok, false),
        PropState::Rejected => (t.border_strong, false),
    };
    let bar = if k == ThemeKind::HighContrast {
        BAR_WIDTH_HIGH_CONTRAST
    } else {
        BAR_WIDTH
    };
    let rim = Stroke::new(border_width(k), color);
    let painter = ui.painter();
    painter.rect_filled(
        Rect::from_min_max(rect.min, pos2(rect.min.x + bar, rect.max.y)),
        CornerRadius::ZERO,
        color,
    );
    if rim_is_dashed {
        let corners = [
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
            rect.left_top(),
        ];
        painter.extend(Shape::dashed_line(&corners, rim, DASH, GAP));
    } else {
        painter.rect_stroke(rect, RADIUS_M, rim, StrokeKind::Inside);
    }
}
