//! Keyboard input: which keys the screen reads and what each key and event does.

use super::{Field, ItemInvoiceScreen, KeyCmd, UiEvent};
use crate::ScreenAction;
use egui::{Key, Modifiers, Ui};
use ledgerly_model::Masters;

/// Save: Ctrl+A and Ctrl+S (spec 7 names Ctrl+A). Ctrl+Enter is not here: spec 7 gives it to
/// "alter master". Hard-coded until the keymap card U2 is wired (see `docs/interface-requests.md`).
const SAVE_KEYS: [(Modifiers, Key); 2] =
    [(Modifiers::COMMAND, Key::A), (Modifiers::COMMAND, Key::S)];

pub(super) fn read_keys(
    ui: &Ui,
    picker_open: bool,
    escape_wanted: bool,
    confirming: bool,
) -> Vec<KeyCmd> {
    let mut out = Vec::new();
    ui.ctx().input_mut(|i| {
        if confirming {
            if i.consume_key(Modifiers::NONE, Key::Y) {
                out.push(KeyCmd::DiscardYes);
            }
            if i.consume_key(Modifiers::NONE, Key::N) {
                out.push(KeyCmd::DiscardNo);
            }
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
            KeyCmd::Save => self.save(masters),
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
            KeyCmd::Escape => {
                if self.confirm_discard {
                    self.confirm_discard = false;
                } else if self.searching() {
                    self.clear_search();
                } else if self.request_leave() {
                    return ScreenAction::Pop;
                }
            }
        }
        ScreenAction::Stay
    }

    pub(super) fn apply_events(&mut self, masters: &Masters, events: Vec<UiEvent>) -> ScreenAction {
        let mut action = ScreenAction::Stay;
        for event in events {
            match event {
                UiEvent::Focus(field) => {
                    if self.focus != field {
                        self.focus = field;
                        self.pick_index = 0;
                    }
                }
                UiEvent::Typed(field) => self.typed(field),
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
                UiEvent::Leave => {
                    if self.request_leave() {
                        action = ScreenAction::Pop;
                    }
                }
            }
        }
        action
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
            Field::Date
            | Field::Number
            | Field::Place
            | Field::Reference
            | Field::Qty(_)
            | Field::Rate(_)
            | Field::Discount(_)
            | Field::Amount(_)
            | Field::Narration => {}
        }
    }
}
