//! Keys and widget events: what the keys do, and how drawn events are applied.

use egui::{Key, Modifiers, Ui};
use ledgerly_model::Masters;

use super::state::{Field, VoucherEntryScreen};
use crate::ScreenAction;

/// What the user did in the widgets this frame, applied after drawing.
pub(super) enum UiEvent {
    Focus(Field),
    LedgerTyped,
    Submit { field: Field, back: bool },
    Pick(usize),
    Side(usize),
    Leave,
}

/// The keys this screen reads. They are hard-coded here, in one table, until the keymap (card U2,
/// `keymap.rs`) is merged and the screen can ask it for its bindings (spec 7: keys are rebindable;
/// the need is filed in `docs/interface-requests.md`). Ctrl+Enter is deliberately absent: spec 7
/// gives it to "alter master", which this screen does not do, so it is left to the shell.
mod bindings {
    use egui::{Key, Modifiers};

    /// Save: Ctrl+A and Ctrl+S (spec 7 names Ctrl+A; Ctrl+S is the plan's second binding).
    pub(super) const SAVE: [(Modifiers, Key); 2] =
        [(Modifiers::COMMAND, Key::A), (Modifiers::COMMAND, Key::S)];
    /// Create a ledger inline (spec 7: Alt+C).
    pub(super) const CREATE_MASTER: (Modifiers, Key) = (Modifiers::ALT, Key::C);
    // Switching a line between Dr and Cr has no keyboard binding. Read 2026-11-21: Tally's
    // shortcut page (https://help.tallysolutions.com/tally-prime/keyboard-shortcuts/keyboard-shortcuts-tally-prime/) gives Alt+S to Stock Query and lists no side-switch key. The
    // contra page (https://help.tallysolutions.com/record-contra-entry/) sets a line's side by the field it is entered in. The binding is pending;
    // the on-screen Dr / Cr button does the switch.
    /// Delete the line. Provisional: no source for this binding yet (spec 7 gives Alt+D to
    /// "delete" a voucher, not a line), so it is only a placeholder until the keymap is merged.
    pub(super) const DELETE_LINE: (Modifiers, Key) = (Modifiers::COMMAND, Key::D);
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
        for (modifiers, key) in bindings::SAVE {
            // Consume every binding, so one key press never reaches a widget.
            save |= i.consume_key(modifiers, key);
        }
        if save {
            out.push(KeyCmd::Save);
        }
        if i.consume_key(bindings::DELETE_LINE.0, bindings::DELETE_LINE.1) {
            out.push(KeyCmd::DeleteLine);
        }
        if i.consume_key(bindings::CREATE_MASTER.0, bindings::CREATE_MASTER.1) {
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
                UiEvent::Leave => {
                    if self.request_leave() {
                        action = ScreenAction::Pop;
                    }
                }
            }
        }
        action
    }
}
