//! Keys and widget events: what the keys do, and how drawn events are applied.

use egui::{Key, Modifiers, Ui};
use ledgerly_model::Masters;

use super::state::{Field, VoucherEntryScreen};
use crate::ScreenAction;
use crate::keymap::{Action, Keymap};

/// What the user did in the widgets this frame, applied after drawing.
pub(super) enum UiEvent {
    Focus(Field),
    LedgerTyped,
    Submit { field: Field, back: bool },
    Pick(usize),
    Side(usize),
}

/// Keys read before the widgets see them.
pub(super) enum KeyCmd {
    Save,
    DeleteLine,
    CreateMaster,
    PickUp,
    PickDown,
    Escape,
    DiscardYes,
    DiscardNo,
}

pub(super) fn read_keys(
    ui: &Ui,
    keymap: &Keymap,
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
        // Every key comes from the keymap, so a rebound key (spec 7) works here too. Consuming
        // the event keeps one press from reaching a widget as well.
        if keymap.consume_action(i, Action::Save) {
            out.push(KeyCmd::Save);
        }
        if keymap.consume_action(i, Action::DeleteLine) {
            out.push(KeyCmd::DeleteLine);
        }
        if keymap.consume_action(i, Action::CreateMaster) {
            out.push(KeyCmd::CreateMaster);
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

impl VoucherEntryScreen {
    /// `picker_len` is the length of the type-ahead list this frame already computed.
    pub(super) fn handle_key(
        &mut self,
        masters: &Masters,
        cmd: &KeyCmd,
        picker_len: usize,
    ) -> ScreenAction {
        match cmd {
            KeyCmd::Save => self.save(masters),
            KeyCmd::DeleteLine => self.delete_line(),
            KeyCmd::CreateMaster => self.begin_create(),
            KeyCmd::PickUp => self.shift_pick(picker_len, -1),
            KeyCmd::PickDown => self.shift_pick(picker_len, 1),
            KeyCmd::DiscardYes => {
                if self.answer_discard(true) {
                    return ScreenAction::Pop;
                }
            }
            KeyCmd::DiscardNo => {
                self.answer_discard(false);
            }
            KeyCmd::Escape => return self.handle_escape(),
        }
        ScreenAction::Stay
    }

    pub(super) fn handle_escape(&mut self) -> ScreenAction {
        if self.create.is_some() {
            self.create = None;
        } else if self.panel.is_some() {
            self.close_bills();
        } else if self.confirm_discard {
            self.confirm_discard = false;
        } else if self.searching() {
            if let Field::Ledger(i) = self.focus {
                self.set_ledger_text(i, "");
            }
        } else if self.request_leave() {
            return ScreenAction::Pop;
        }
        ScreenAction::Stay
    }

    /// `true` while the ledger field that has the keyboard holds an unfinished search.
    pub(super) fn searching(&self) -> bool {
        let Field::Ledger(i) = self.focus else {
            return false;
        };
        self.draft
            .rows
            .get(i)
            .is_some_and(|r| r.ledger.is_none() && !r.ledger_text.trim().is_empty())
    }

    pub(super) fn apply_events(&mut self, masters: &Masters, events: Vec<UiEvent>) {
        for event in events {
            match event {
                UiEvent::Focus(field) => {
                    if self.focus != field {
                        self.focus = field;
                        self.pick_index = 0;
                    }
                }
                UiEvent::LedgerTyped => self.pick_index = 0,
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
                UiEvent::Side(i) => self.toggle_side(i),
            }
        }
    }
}
