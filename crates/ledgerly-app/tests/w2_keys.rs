//! Wave 2 item S-J: the standard preset, loading presets by name, the text-field rule (decision
//! 10) and the keyboard map's layout.
//!
//! Provenance of the expected values: `design/mock-v2/index.html`, the `KEYMAP` table at lines
//! 1835-1841 (column 1 is the classic keys, column 2 the standard keys), the Ctrl+S save and
//! Ctrl+Shift voucher keys in `design/mock-v2/DESIGN.md` section 4 (lines 170-183), and
//! `docs/status/2026-11-22-ui-parity-report.md` section 8, decisions 4 and 10. Each expected chord
//! is typed here from those sources. The text-field behaviour is egui 0.37's own: a focused
//! single-line `TextEdit` selects all on Ctrl+A (`egui-0.37.0/src/text_selection/cursor_range.rs`).
#![allow(clippy::unwrap_used, reason = "test code: rule B2 lets tests unwrap")]

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_app::keymap::{
    Action, Chord, DEFAULT_PRESET, Keymap, KeymapError, PRESET_NAMES, classic, from_toml,
    is_text_edit_chord, preset, standard, to_toml,
};
use ledgerly_app::screens::keymap_map::layout;

/// A frame whose input is one key press.
fn press(key: Key, modifiers: Modifiers) -> RawInput {
    RawInput {
        events: vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..RawInput::default()
    }
}

/// Runs one frame of `input` with `frame` drawing into it.
fn run(ctx: &egui::Context, input: RawInput, frame: impl FnMut(&mut egui::Ui)) {
    let out = ctx.run_ui(input, frame);
    out.drop_without_applying_deltas();
}

/// The text of every chord bound to `action`, in order.
fn chord_texts(km: &Keymap, action: Action) -> Vec<String> {
    km.chords(action).iter().map(Chord::to_string).collect()
}

/// Whether `action` fires on `key` with `modifiers` when no text field has focus.
fn fires_without_text_focus(km: &Keymap, action: Action, key: Key, modifiers: Modifiers) -> bool {
    let ctx = egui::Context::default();
    let mut fired = false;
    run(&ctx, press(key, modifiers), |ui| {
        fired = ui.input_mut(|i| km.consume_action_outside_text(i, action, false));
    });
    fired
}

/// Focuses a single-line text field holding "Hello", then presses `key` with `modifiers` in the
/// next frame, the way the screens read keys (before the widgets draw). Returns whether `action`
/// fired, and the two ends of the field's selection afterwards, sorted.
fn press_in_text_field(
    km: &Keymap,
    action: Action,
    key: Key,
    modifiers: Modifiers,
) -> (bool, [usize; 2]) {
    let ctx = egui::Context::default();
    let id = egui::Id::unique("s-j-text-field");
    let mut text = String::from("Hello");

    // Frame 1: the field draws and asks for focus.
    run(&ctx, RawInput::default(), |ui| {
        ui.add(egui::TextEdit::singleline(&mut text).id(id))
            .request_focus();
    });

    // Frame 2: the key arrives while the field has focus.
    let mut fired = false;
    let mut focused = false;
    run(&ctx, press(key, modifiers), |ui| {
        focused = ui.ctx().text_edit_focused();
        fired = ui.input_mut(|i| km.consume_action_outside_text(i, action, focused));
        ui.add(egui::TextEdit::singleline(&mut text).id(id));
    });
    assert!(focused, "the text field should have focus in frame 2");

    let state = egui::text_edit::TextEditState::load(&ctx, id).unwrap();
    let range = state.cursor.char_range().unwrap();
    // `CharIndex` is a newtype over the character offset (epaint `text/index.rs`).
    let mut cursors = [range.primary.index.0, range.secondary.index.0];
    cursors.sort_unstable();
    (fired, cursors)
}

// ---- the standard preset ---------------------------------------------------------------------

#[test]
fn standard_saves_with_ctrl_s_and_leaves_ctrl_a_to_text_fields() {
    // provenance: derived — decision 4 (docs/status/2026-11-22-ui-parity-report.md section 8)
    // makes Ctrl+S the standard save; DESIGN.md section 4 gives "Save voucher" Ctrl+S in the
    // standard column. Ctrl+A is select-all in a text field, so the standard preset must not
    // bind it.
    let km = standard();
    assert_eq!(
        km.action_for(Chord::key(Key::S).with_ctrl()),
        Some(Action::Save)
    );
    assert_eq!(km.action_for(Chord::key(Key::A).with_ctrl()), None);
    assert_eq!(chord_texts(&km, Action::Save), ["Ctrl+S"]);
}

#[test]
fn standard_voucher_keys_follow_the_mock_with_function_key_aliases() {
    // provenance: derived — design/mock-v2/index.html line 1837 (KEYMAP, Vouchers): standard
    // Ctrl+Shift+S Sales, P Purchase, Y Payment, R Receipt, C Contra, J Journal. DESIGN.md
    // section 4: "F-keys stay as aliases", so the F-keys of the classic preset stay on the
    // same actions.
    let km = standard();
    assert_eq!(chord_texts(&km, Action::NewSales), ["Ctrl+Shift+S", "F8"]);
    assert_eq!(
        chord_texts(&km, Action::NewPurchase),
        ["Ctrl+Shift+P", "F9"]
    );
    assert_eq!(chord_texts(&km, Action::NewPayment), ["Ctrl+Shift+Y", "F5"]);
    assert_eq!(chord_texts(&km, Action::NewReceipt), ["Ctrl+Shift+R", "F6"]);
    assert_eq!(chord_texts(&km, Action::NewContra), ["Ctrl+Shift+C", "F4"]);
    assert_eq!(chord_texts(&km, Action::NewJournal), ["Ctrl+Shift+J", "F7"]);
}

#[test]
fn standard_company_and_palette_keys_follow_the_mock() {
    // provenance: derived — design/mock-v2/index.html lines 1836 and 1837 (KEYMAP, Go anywhere):
    // standard Ctrl+Shift+O switches company, and Ctrl+K opens the palette. The mock gives the
    // standard column one palette key; Ctrl+G is kept for switch-to, because the two palette
    // actions need separate chords.
    let km = standard();
    assert_eq!(chord_texts(&km, Action::SwitchCompany), ["Ctrl+Shift+O"]);
    assert_eq!(chord_texts(&km, Action::OpenPalette), ["Ctrl+K"]);
    assert_eq!(chord_texts(&km, Action::SwitchPalette), ["Ctrl+G"]);
}

#[test]
fn standard_binds_every_action_except_the_pending_side_switch() {
    // provenance: derived — the same pending list as the classic preset (switch-side, see
    // assets/keymap-classic.toml): no Tally source gives a side-switch key.
    let km = standard();
    for action in Action::ALL.iter().copied() {
        if action == Action::SwitchSide {
            assert!(km.chords(action).is_empty(), "switch-side stays unbound");
            continue;
        }
        assert!(
            !km.chords(action).is_empty(),
            "{} is unbound in standard",
            action.id()
        );
    }
}

#[test]
fn the_standard_file_round_trips_through_toml() {
    // provenance: derived — to_toml and from_toml are inverses (keymap.rs module docs).
    let km = standard();
    assert_eq!(from_toml(&to_toml(&km)).unwrap(), km);
    assert_eq!(km.name(), "standard");
}

// ---- presets by name -------------------------------------------------------------------------

#[test]
fn the_shipped_presets_are_classic_and_standard_with_classic_as_default() {
    // provenance: derived — decision 4 and the task brief: classic is the default, standard is the
    // second preset.
    assert_eq!(PRESET_NAMES, ["classic", "standard"]);
    assert_eq!(DEFAULT_PRESET, "classic");
}

#[test]
fn a_preset_loads_by_name() {
    // provenance: derived — the names are the two presets above; each must equal the function
    // that builds it directly.
    assert_eq!(preset("classic").unwrap(), classic());
    assert_eq!(preset("standard").unwrap(), standard());
    assert_eq!(preset("classic").unwrap().name(), "classic");
    assert_eq!(preset("standard").unwrap().name(), "standard");
}

#[test]
fn an_unknown_preset_name_is_an_error_not_a_default() {
    // provenance: derived — a name that ships is the only way to get a keymap; anything else
    // must fail, not fall back.
    assert_eq!(
        preset("dvorak"),
        Err(KeymapError::UnknownPreset {
            name: "dvorak".to_owned()
        })
    );
}

// ---- decision 10: Ctrl+A in a text field -----------------------------------------------------

#[test]
fn text_edit_chords_are_ctrl_with_a_c_v_x_z_or_y_only() {
    // provenance: derived — the keys a single-line text field edits itself with Ctrl held
    // (select all, copy, paste, cut, undo, redo). Alt or Shift changes the chord, so it is not one.
    assert!(is_text_edit_chord(Chord::key(Key::A).with_ctrl()));
    assert!(is_text_edit_chord(Chord::key(Key::Z).with_ctrl()));
    assert!(!is_text_edit_chord(
        Chord::key(Key::A).with_ctrl().with_shift()
    ));
    assert!(!is_text_edit_chord(
        Chord::key(Key::A).with_ctrl().with_alt()
    ));
    assert!(!is_text_edit_chord(Chord::key(Key::S).with_ctrl()));
    assert!(!is_text_edit_chord(Chord::key(Key::F4)));
    assert!(!is_text_edit_chord(Chord::key(Key::A)));
}

#[test]
fn ctrl_a_saves_when_no_text_field_has_focus() {
    // provenance: derived — the classic save key (spec section 7, "Ctrl+A save").
    assert!(fires_without_text_focus(
        &classic(),
        Action::Save,
        Key::A,
        Modifiers::COMMAND
    ));
}

#[test]
fn ctrl_a_in_a_focused_text_field_selects_all_and_fires_no_save() {
    // provenance: derived — decision 10 (parity report section 8): Ctrl+A in a focused single-line
    // text field selects all, and no save is fired. "Hello" has five characters, so the
    // selection is [0, 5].
    let (fired, selection) =
        press_in_text_field(&classic(), Action::Save, Key::A, Modifiers::COMMAND);
    assert!(!fired, "Ctrl+A must not save while a text field has focus");
    assert_eq!(selection, [0, 5], "Ctrl+A must select all the text");
}

#[test]
fn ctrl_s_still_saves_in_a_text_field_on_the_standard_preset() {
    // provenance: derived — Ctrl+S is not a text-editing chord, so the standard save works while a
    // field has focus, as a desktop save does.
    let (fired, _) = press_in_text_field(&standard(), Action::Save, Key::S, Modifiers::COMMAND);
    assert!(fired);
}

#[test]
fn alt_c_still_creates_a_master_in_a_text_field() {
    // provenance: derived — spec section 7 "Alt+C create master inline". Alt is not a text-editing
    // modifier, so the field does not take the key.
    let (fired, _) = press_in_text_field(&classic(), Action::CreateMaster, Key::C, Modifiers::ALT);
    assert!(fired);
}

// ---- the keyboard map's layout ---------------------------------------------------------------

#[test]
fn the_map_lists_every_action_exactly_once() {
    // provenance: derived — the map must show every Action; a key with no card would be hidden.
    let listed: Vec<Action> = layout()
        .iter()
        .flat_map(|group| group.rows.iter().map(|row| row.action))
        .collect();
    assert_eq!(listed.len(), Action::ALL.len());
    for action in Action::ALL.iter().copied() {
        let count = listed.iter().filter(|a| **a == action).count();
        assert_eq!(count, 1, "{} is listed {count} times", action.id());
    }
}

#[test]
fn the_map_groups_follow_the_mock_titles_in_order() {
    // provenance: derived — design/mock-v2/index.html lines 1836-1841: the first four cards are
    // "Go anywhere", "Vouchers", "Inside a voucher" and "Reports"; any action without a mock row
    // goes under "Other actions" last.
    let titles: Vec<&str> = layout().iter().map(|group| group.title).collect();
    assert_eq!(
        titles,
        [
            "Go anywhere",
            "Vouchers",
            "Inside a voucher",
            "Reports",
            "Other actions"
        ]
    );
}

#[test]
fn the_classic_map_shows_the_mock_classic_keys() {
    // provenance: derived — design/mock-v2/index.html line 1836 (KEYMAP, Go anywhere): classic
    // Ctrl+K and Alt+G open the palette; line 1837 (Vouchers): F8 Sales; line 1838 (Inside a
    // voucher): Ctrl+A saves.
    let km = classic();
    assert_eq!(chord_texts(&km, Action::OpenPalette), ["Ctrl+K", "Alt+G"]);
    assert_eq!(chord_texts(&km, Action::NewSales), ["F8"]);
    assert_eq!(
        chord_texts(&km, Action::Save).first().map(String::as_str),
        Some("Ctrl+A")
    );
}

#[test]
fn an_action_without_a_key_shows_no_keys() {
    // provenance: derived — switch-side is pending (keymap-classic.toml), so the map has no key
    // to show for it on either preset.
    assert_eq!(chord_texts(&classic(), Action::SwitchSide).len(), 0);
    assert_eq!(chord_texts(&standard(), Action::SwitchSide).len(), 0);
}
