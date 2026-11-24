//! Keyboard input: which keys the screen reads and what each key and event does.

use super::{Field, ItemInvoiceScreen, KeyCmd, UiEvent};
use crate::ScreenAction;
use egui::{Event, InputState, Key, Modifiers, Ui};
use ledgerly_model::Masters;

/// Save: Ctrl+A and Ctrl+S (spec 7 names Ctrl+A). Ctrl+Enter is not here: spec 7 gives it to
/// "alter master". Hard-coded until the keymap card U2 is wired (see `docs/interface-requests.md`).
const SAVE_KEYS: [(Modifiers, Key); 2] =
    [(Modifiers::COMMAND, Key::A), (Modifiers::COMMAND, Key::S)];

/// Consumes the press of exactly `key` with exactly `modifiers` held, and reports whether there was
/// one. egui's `consume_key` also accepts extra Alt or Shift, so the discard question would take
/// Shift+Y as a yes. A held key that repeats is not a new press, so a held Enter cannot post a
/// save on its own.
fn consume_exact(input: &mut InputState, key: Key, modifiers: Modifiers) -> bool {
    let before = input.events.len();
    input.events.retain(|event| {
        !matches!(
            event,
            Event::Key { key: pressed_key, modifiers: held, pressed: true, repeat: false, .. }
                if *pressed_key == key && held.matches_exact(modifiers)
        )
    });
    input.events.len() != before
}

/// The questions the screen is asking, which change what Y, N and Enter do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Prompt {
    /// "Discard this invoice?" is showing.
    pub(super) discard: bool,
    /// "Save this invoice?" is showing.
    pub(super) save: bool,
}

pub(super) fn read_keys(
    ui: &Ui,
    picker_open: bool,
    escape_wanted: bool,
    prompt: Prompt,
) -> Vec<KeyCmd> {
    let mut out = Vec::new();
    ui.ctx().input_mut(|i| {
        if prompt.discard {
            // Plain keys only: Shift+Y must not discard the invoice.
            if consume_exact(i, Key::Y, Modifiers::NONE) {
                out.push(KeyCmd::DiscardYes);
            }
            if consume_exact(i, Key::N, Modifiers::NONE) {
                out.push(KeyCmd::DiscardNo);
            }
        }
        if prompt.save && consume_exact(i, Key::Enter, Modifiers::NONE) {
            out.push(KeyCmd::ConfirmSave);
        }
        let mut save = false;
        for (modifiers, key) in SAVE_KEYS {
            save |= i.consume_key(modifiers, key);
        }
        if save {
            out.push(KeyCmd::Save);
        }
        if picker_open && i.consume_key(Modifiers::NONE, Key::ArrowDown) {
            out.push(KeyCmd::PickDown);
        }
        if picker_open && i.consume_key(Modifiers::NONE, Key::ArrowUp) {
            out.push(KeyCmd::PickUp);
        }
        if !prompt.discard && !prompt.save && consume_exact(i, Key::C, Modifiers::ALT) {
            out.push(KeyCmd::Create);
        }
        if !prompt.discard && !prompt.save && consume_exact(i, Key::H, Modifiers::COMMAND) {
            out.push(KeyCmd::ChangeMode);
        }
        if escape_wanted && i.consume_key(Modifiers::NONE, Key::Escape) {
            out.push(KeyCmd::Escape);
        }
    });
    out
}

impl ItemInvoiceScreen {
    pub(super) fn handle_key(
        &mut self,
        masters: &Masters,
        cmd: KeyCmd,
        list_len: usize,
    ) -> ScreenAction {
        match cmd {
            KeyCmd::Save | KeyCmd::ConfirmSave => self.save(masters),
            KeyCmd::PickUp => self.pick_index = self.pick_index.saturating_sub(1),
            KeyCmd::PickDown => {
                let last = list_len.saturating_sub(1);
                self.pick_index = self.pick_index.saturating_add(1).min(last);
            }
            KeyCmd::DiscardYes => {
                if self.answer_discard(true) {
                    return ScreenAction::Pop;
                }
            }
            KeyCmd::DiscardNo => {
                self.answer_discard(false);
            }
            KeyCmd::Create => self.begin_create(),
            KeyCmd::ChangeMode => self.change_mode(),
            KeyCmd::Escape => {
                if self.confirm_discard {
                    self.confirm_discard = false;
                } else if self.create.is_some() {
                    self.cancel_create();
                } else if self.confirming_save {
                    self.confirming_save = false;
                } else if self.searching() {
                    self.clear_search();
                } else if self.request_leave() {
                    return ScreenAction::Pop;
                }
            }
        }
        ScreenAction::Stay
    }

    pub(super) fn apply_events(&mut self, masters: &Masters, events: Vec<UiEvent>) {
        for event in events {
            match event {
                UiEvent::Focus(field) => {
                    if self.focus != field {
                        self.focus = field;
                        self.pick_index = 0;
                        self.confirming_save = false;
                    }
                }
                UiEvent::Typed(field) => {
                    self.confirming_save = false;
                    self.typed(field);
                }
                UiEvent::Submit { field, back } => {
                    self.focus = field;
                    if back {
                        self.step_back();
                    } else {
                        self.submit(masters);
                    }
                }
                UiEvent::Pick(j) => {
                    self.pick_index = j;
                    self.submit(masters);
                }
            }
        }
    }

    /// Typing into a picked field un-picks it.
    fn typed(&mut self, field: Field) {
        self.pick_index = 0;
        match field {
            Field::Party => self.draft.party = None,
            Field::Account => self.draft.account = None,
            Field::Item(i) => {
                if let Some(row) = self.draft.rows.get_mut(i) {
                    row.item = None;
                }
            }
            Field::Ledger(i) => {
                if let Some(row) = self.draft.particulars.get_mut(i) {
                    row.ledger = None;
                }
            }
            Field::Date
            | Field::Number
            | Field::Place
            | Field::Reference
            | Field::Qty(_)
            | Field::Rate(_)
            | Field::Discount(_)
            | Field::Amount(_)
            | Field::LedgerAmount(_)
            | Field::Narration => {}
        }
    }
}
