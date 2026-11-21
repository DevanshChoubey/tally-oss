//! Card U2: keymap. Expected bindings come from the spec and the keyboard research note, quoted
//! next to each case (provenance: derived). Nothing is copied from this crate's output: every
//! expected chord is built from key constants here, not parsed by the code under test. The text
//! format cases (canonical chord text, the TOML layout) are derived from the format documented
//! in `keymap.rs` and `assets/keymap-classic.toml`.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeSet;

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_app::keymap::{
    Action, Chord, ChordParseError, Keymap, KeymapError, classic, from_toml, to_toml, try_classic,
};

fn f(key: Key) -> Chord {
    Chord::key(key)
}

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

/// Runs one frame with `input` and returns what `Keymap::consume` found.
fn consumed(km: &Keymap, input: RawInput) -> Option<Action> {
    let ctx = egui::Context::default();
    let mut got = None;
    let out = ctx.run_ui(input, |ui| {
        got = ui.input_mut(|i| km.consume(i));
    });
    out.drop_without_applying_deltas();
    got
}

// ---- the classic preset ----------------------------------------------------------------------

#[test]
fn classic_binds_the_voucher_keys_f4_to_f9() {
    // provenance: derived — spec section 7 "F4–F9 voucher types"; the order is
    // docs/research/02-ux-keyboard.md section 3.2: Contra F4, Payment F5, Receipt F6,
    // Journal F7, Sales F8, Purchase F9.
    let km = classic();
    assert_eq!(km.action_for(f(Key::F4)), Some(Action::NewContra));
    assert_eq!(km.action_for(f(Key::F5)), Some(Action::NewPayment));
    assert_eq!(km.action_for(f(Key::F6)), Some(Action::NewReceipt));
    assert_eq!(km.action_for(f(Key::F7)), Some(Action::NewJournal));
    assert_eq!(km.action_for(f(Key::F8)), Some(Action::NewSales));
    assert_eq!(km.action_for(f(Key::F9)), Some(Action::NewPurchase));
}

#[test]
fn classic_binds_the_context_keys() {
    // provenance: derived — spec section 7: "F2 date/period, F3 company, F11 features,
    // F12 configure"; "Esc back"; "Alt+F1 detail toggle".
    let km = classic();
    assert_eq!(km.action_for(f(Key::F2)), Some(Action::ChangePeriod));
    assert_eq!(km.action_for(f(Key::F3)), Some(Action::SwitchCompany));
    assert_eq!(km.action_for(f(Key::F11)), Some(Action::CompanyFeatures));
    assert_eq!(km.action_for(f(Key::F12)), Some(Action::Configure));
    assert_eq!(km.action_for(f(Key::Escape)), Some(Action::Back));
    assert_eq!(
        km.action_for(f(Key::F1).with_alt()),
        Some(Action::ToggleDetail)
    );
}

#[test]
fn classic_binds_ctrl_a_to_save() {
    // provenance: derived — spec section 7 "Ctrl+A save", and the plan's key note "Save is
    // Ctrl+A (and Ctrl+S)".
    let km = classic();
    assert_eq!(km.action_for(f(Key::A).with_ctrl()), Some(Action::Save));
    assert_eq!(km.action_for(f(Key::S).with_ctrl()), Some(Action::Save));
}

#[test]
fn classic_binds_the_master_and_voucher_editing_keys() {
    // provenance: derived — spec section 7: "Alt+C create master inline, Ctrl+Enter alter master,
    // Alt+D delete, Alt+X cancel, Alt+2 duplicate".
    let km = classic();
    assert_eq!(
        km.action_for(f(Key::C).with_alt()),
        Some(Action::CreateMaster)
    );
    assert_eq!(
        km.action_for(f(Key::Enter).with_ctrl()),
        Some(Action::AlterMaster)
    );
    assert_eq!(
        km.action_for(f(Key::D).with_alt()),
        Some(Action::DeleteVoucher)
    );
    assert_eq!(
        km.action_for(f(Key::X).with_alt()),
        Some(Action::CancelVoucher)
    );
    assert_eq!(
        km.action_for(f(Key::Num2).with_alt()),
        Some(Action::DuplicateVoucher)
    );
}

#[test]
fn classic_binds_output_keys_and_modern_aliases() {
    // provenance: derived — spec section 7: "Ctrl+P/E/M output", "modern aliases (Ctrl+S,
    // Ctrl+K palette, Ctrl+Z undo)".
    let km = classic();
    assert_eq!(km.action_for(f(Key::P).with_ctrl()), Some(Action::Print));
    assert_eq!(km.action_for(f(Key::E).with_ctrl()), Some(Action::Export));
    assert_eq!(km.action_for(f(Key::M).with_ctrl()), Some(Action::Email));
    assert_eq!(
        km.action_for(f(Key::K).with_ctrl()),
        Some(Action::OpenPalette)
    );
    assert_eq!(km.action_for(f(Key::Z).with_ctrl()), Some(Action::Undo));
}

#[test]
fn classic_has_the_tab_and_palette_keys_the_shell_uses() {
    // provenance: derived — docs/interface-requests.md (U1 note) says the shell hard-codes
    // Ctrl+T and Ctrl+W; docs/research/02-ux-keyboard.md section 3.1: Alt+G palette that stacks,
    // Ctrl+G palette that replaces.
    let km = classic();
    assert_eq!(km.action_for(f(Key::T).with_ctrl()), Some(Action::NewTab));
    assert_eq!(km.action_for(f(Key::W).with_ctrl()), Some(Action::CloseTab));
    assert_eq!(
        km.action_for(f(Key::G).with_alt()),
        Some(Action::OpenPalette)
    );
    assert_eq!(
        km.action_for(f(Key::G).with_ctrl()),
        Some(Action::SwitchPalette)
    );
}

#[test]
fn the_embedded_classic_file_parses_and_names_itself() {
    // provenance: derived — the asset's first key is `preset = "classic"`.
    let km = try_classic().unwrap();
    assert_eq!(km.name(), "classic");
    assert_eq!(km, classic());
}

#[test]
fn classic_binds_every_action_and_no_chord_twice() {
    // provenance: derived — every action in `Action::ALL` is part of the preset, and a key press
    // may mean one thing (`Keymap::new` rejects a chord bound twice, so a built keymap is clean).
    let km = classic();
    let mut seen = BTreeSet::new();
    for action in Action::ALL {
        assert!(
            !km.chords(*action).is_empty(),
            "{} is unbound in classic",
            action.id()
        );
        for chord in km.chords(*action) {
            assert!(seen.insert(*chord), "{chord} appears twice");
        }
    }
}

// ---- actions -----------------------------------------------------------------------------------

#[test]
fn action_ids_are_unique_and_round_trip() {
    // provenance: derived — an id names exactly one action.
    let mut ids = BTreeSet::new();
    for action in Action::ALL {
        assert!(ids.insert(action.id()), "id {} repeated", action.id());
        assert_eq!(Action::from_id(action.id()), Some(*action));
    }
    assert_eq!(Action::from_id("no-such-action"), None);
}

#[test]
fn action_labels_are_not_empty() {
    // provenance: derived — a label is shown in the palette, so it must exist.
    for action in Action::ALL {
        assert_ne!(action.label(), "");
    }
}

// ---- chord text --------------------------------------------------------------------------------

#[test]
fn chord_text_is_modifiers_in_fixed_order_then_the_key() {
    // provenance: derived — the canonical format in `keymap.rs`: Ctrl, Alt, Shift, then the key
    // name, joined with `+`; input is case-insensitive for modifiers and accepts either order.
    let c: Chord = "Shift+alt+CTRL+F4".parse().unwrap();
    assert_eq!(c, f(Key::F4).with_ctrl().with_alt().with_shift());
    assert_eq!(c.to_string(), "Ctrl+Alt+Shift+F4");
    assert_eq!("F4".parse::<Chord>().unwrap(), f(Key::F4));
    assert_eq!("Esc".parse::<Chord>().unwrap(), f(Key::Escape));
    assert_eq!(f(Key::Escape).to_string(), "Escape");
    assert_eq!(f(Key::Num2).with_alt().to_string(), "Alt+2");
}

#[test]
fn chord_text_errors_say_what_is_wrong() {
    // provenance: derived — each malformed form named in `ChordParseError`.
    assert_eq!("".parse::<Chord>(), Err(ChordParseError::Empty));
    assert_eq!("Ctrl+".parse::<Chord>(), Err(ChordParseError::NoKey));
    assert_eq!("Ctrl+Shift".parse::<Chord>(), Err(ChordParseError::NoKey));
    assert_eq!(
        "Hyper+A".parse::<Chord>(),
        Err(ChordParseError::UnknownModifier("Hyper".to_owned()))
    );
    assert_eq!(
        "Ctrl+Ctrl+A".parse::<Chord>(),
        Err(ChordParseError::DuplicateModifier("Ctrl"))
    );
    assert_eq!(
        "Ctrl+Banana".parse::<Chord>(),
        Err(ChordParseError::UnknownKey("Banana".to_owned()))
    );
}

#[test]
fn chord_text_round_trips_for_every_key_the_presets_use() {
    // provenance: derived — display then parse returns the same chord.
    let keys = [
        Key::F1,
        Key::F12,
        Key::A,
        Key::Z,
        Key::Num0,
        Key::Num9,
        Key::Enter,
        Key::Escape,
        Key::Tab,
        Key::ArrowDown,
        Key::Plus,
        Key::Minus,
    ];
    for key in keys {
        for chord in [
            f(key),
            f(key).with_ctrl(),
            f(key).with_alt().with_shift(),
            f(key).with_ctrl().with_alt().with_shift(),
        ] {
            assert_eq!(chord.to_string().parse::<Chord>(), Ok(chord));
        }
    }
}

// ---- TOML --------------------------------------------------------------------------------------

#[test]
fn classic_round_trips_through_toml() {
    // provenance: derived — export then import returns an equal keymap (spec section 7:
    // "Fully rebindable; exported as TOML").
    let km = classic();
    let text = to_toml(&km);
    assert_eq!(from_toml(&text).unwrap(), km);
}

#[test]
fn rebinding_round_trips_through_toml() {
    // provenance: derived — spec section 7 "Fully rebindable; exported as TOML": a changed
    // binding survives export and import.
    let mut km = classic();
    km.rebind(
        Action::Save,
        vec![f(Key::S).with_ctrl().with_shift(), f(Key::F10)],
    )
    .unwrap();
    km.rebind(Action::NewPayment, vec![]).unwrap();
    let back = from_toml(&to_toml(&km)).unwrap();
    assert_eq!(back, km);
    assert_eq!(
        back.action_for(f(Key::S).with_ctrl().with_shift()),
        Some(Action::Save)
    );
    assert_eq!(back.action_for(f(Key::F10)), Some(Action::Save));
    // The old Ctrl+A is free again and F5 is unbound.
    assert_eq!(back.action_for(f(Key::A).with_ctrl()), None);
    assert_eq!(back.chords(Action::NewPayment), []);
    assert_eq!(back.action_for(f(Key::F5)), None);
}

#[test]
fn the_exported_file_has_the_documented_layout() {
    // provenance: derived — layout documented in assets/keymap-classic.toml: a `preset` key, a
    // `[bindings]` table, one `id = [..]` line per action in `Action::ALL` order.
    let mut km = Keymap::unbound("mine");
    km.rebind(
        Action::Save,
        vec![f(Key::A).with_ctrl(), f(Key::S).with_ctrl()],
    )
    .unwrap();
    let text = to_toml(&km);
    assert!(text.starts_with("preset = \"mine\"\n\n[bindings]\n"));
    assert!(text.contains("save = [\"Ctrl+A\", \"Ctrl+S\"]\n"));
    assert!(text.contains("new-contra = []\n"));
    assert_eq!(text.lines().count(), 3 + Action::ALL.len());
}

#[test]
fn a_preset_name_with_quotes_and_newlines_round_trips() {
    // provenance: derived — TOML basic strings escape `"`, `\` and control characters.
    let km = Keymap::unbound("my \"keys\" \\ one\ntwo");
    assert_eq!(from_toml(&to_toml(&km)).unwrap(), km);
}

#[test]
fn an_action_missing_from_the_file_is_unbound_not_defaulted() {
    // provenance: derived — rule A1: no silent defaults; `from_toml` documents it.
    let km = from_toml("preset = \"p\"\n[bindings]\nsave = [\"Ctrl+A\"]\n").unwrap();
    assert_eq!(km.action_for(f(Key::A).with_ctrl()), Some(Action::Save));
    assert_eq!(km.action_for(f(Key::F4)), None);
    assert_eq!(km.chords(Action::NewContra), []);
}

#[test]
fn from_toml_rejects_what_it_does_not_understand() {
    // provenance: derived — each error variant's documented trigger.
    assert!(matches!(
        from_toml("this is not toml ["),
        Err(KeymapError::Toml(_))
    ));
    assert!(matches!(
        from_toml("preset = \"p\"\nextra = 1\n[bindings]\n"),
        Err(KeymapError::Toml(_))
    ));
    assert_eq!(
        from_toml("preset = \"p\"\n[bindings]\nfly = [\"F1\"]\n"),
        Err(KeymapError::UnknownAction {
            id: "fly".to_owned()
        })
    );
    assert_eq!(
        from_toml("preset = \"p\"\n[bindings]\nsave = [\"Ctrl+\"]\n"),
        Err(KeymapError::BadChord {
            action: "save".to_owned(),
            text: "Ctrl+".to_owned(),
            source: ChordParseError::NoKey,
        })
    );
}

#[test]
fn from_toml_rejects_a_chord_bound_twice() {
    // provenance: derived — a key press may mean one thing.
    let two_actions = "preset = \"p\"\n[bindings]\nnew-contra = [\"F4\"]\nnew-payment = [\"F4\"]\n";
    assert_eq!(
        from_toml(two_actions),
        Err(KeymapError::Conflict {
            chord: "F4".to_owned(),
            first: "new-contra",
            second: "new-payment",
        })
    );
    let one_action = "preset = \"p\"\n[bindings]\nsave = [\"Ctrl+A\", \"ctrl+a\"]\n";
    assert_eq!(
        from_toml(one_action),
        Err(KeymapError::DuplicateChord {
            action: "save",
            chord: "Ctrl+A".to_owned(),
        })
    );
}

// ---- rebinding ---------------------------------------------------------------------------------

#[test]
fn rebinding_to_a_taken_chord_fails_and_changes_nothing() {
    // provenance: derived — `rebind` documents "Nothing changes when this fails".
    let mut km = classic();
    let before = km.clone();
    let err = km.rebind(Action::NewPayment, vec![f(Key::F4)]).unwrap_err();
    assert!(matches!(err, KeymapError::Conflict { .. }));
    assert_eq!(km, before);
}

#[test]
fn rebinding_an_action_to_its_own_chord_is_fine() {
    // provenance: derived — an action's own old chords are replaced, not a conflict.
    let mut km = classic();
    km.rebind(Action::NewPayment, vec![f(Key::F5), f(Key::F10)])
        .unwrap();
    assert_eq!(km.action_for(f(Key::F10)), Some(Action::NewPayment));
}

#[test]
fn unbind_frees_the_chords() {
    // provenance: derived — `unbind` removes every chord of an action.
    let mut km = classic();
    km.unbind(Action::Undo);
    assert_eq!(km.action_for(f(Key::Z).with_ctrl()), None);
    assert_eq!(km.chords(Action::Undo), []);
}

#[test]
fn shortcut_label_is_the_first_chord() {
    // provenance: derived — the label is the canonical text of the first bound chord.
    let km = classic();
    assert_eq!(km.shortcut_label(Action::Save).as_deref(), Some("Ctrl+A"));
    assert_eq!(
        km.shortcut_label(Action::DuplicateVoucher).as_deref(),
        Some("Alt+2")
    );
    assert_eq!(Keymap::unbound("x").shortcut_label(Action::Save), None);
}

// ---- reading key presses ----------------------------------------------------------------------

#[test]
fn consume_finds_the_pressed_action() {
    // provenance: derived — a press of a bound chord returns its action.
    let km = classic();
    assert_eq!(
        consumed(&km, press(Key::F5, Modifiers::NONE)),
        Some(Action::NewPayment)
    );
    assert_eq!(
        consumed(&km, press(Key::A, Modifiers::COMMAND)),
        Some(Action::Save)
    );
    assert_eq!(
        consumed(&km, press(Key::Enter, Modifiers::COMMAND)),
        Some(Action::AlterMaster)
    );
    assert_eq!(
        consumed(&km, press(Key::Num2, Modifiers::ALT)),
        Some(Action::DuplicateVoucher)
    );
    assert_eq!(
        consumed(&km, press(Key::Escape, Modifiers::NONE)),
        Some(Action::Back)
    );
}

#[test]
fn consume_needs_the_modifiers_to_match_exactly() {
    // provenance: derived — keymap.rs docs: with F5 bound to Payment, Alt+F5 must not open one;
    // Alt+F1 is the detail toggle, not Help (F1).
    let km = classic();
    assert_eq!(consumed(&km, press(Key::F5, Modifiers::ALT)), None);
    assert_eq!(consumed(&km, press(Key::A, Modifiers::NONE)), None);
    assert_eq!(
        consumed(&km, press(Key::A, Modifiers::COMMAND | Modifiers::SHIFT)),
        None
    );
    assert_eq!(
        consumed(&km, press(Key::F1, Modifiers::ALT)),
        Some(Action::ToggleDetail)
    );
    assert_eq!(
        consumed(&km, press(Key::F1, Modifiers::NONE)),
        Some(Action::Help)
    );
}

#[test]
fn consume_removes_the_event_so_it_fires_once() {
    // provenance: derived — `consume` documents that it consumes the key event.
    let km = classic();
    let ctx = egui::Context::default();
    let mut first = None;
    let mut second = None;
    let out = ctx.run_ui(press(Key::F9, Modifiers::NONE), |ui| {
        first = ui.input_mut(|i| km.consume(i));
        second = ui.input_mut(|i| km.consume(i));
    });
    out.drop_without_applying_deltas();
    assert_eq!(first, Some(Action::NewPurchase));
    assert_eq!(second, None);
}

#[test]
fn consume_action_checks_only_that_action() {
    // provenance: derived — a screen asks for the keys it owns.
    let km = classic();
    let ctx = egui::Context::default();
    let mut save = false;
    let mut payment = false;
    let out = ctx.run_ui(press(Key::S, Modifiers::COMMAND), |ui| {
        payment = ui.input_mut(|i| km.consume_action(i, Action::NewPayment));
        save = ui.input_mut(|i| km.consume_action(i, Action::Save));
    });
    out.drop_without_applying_deltas();
    assert!(!payment);
    assert!(save);
}

#[test]
fn a_rebound_key_is_what_consume_reads() {
    // provenance: derived — rebinding changes what a press means.
    let mut km = classic();
    km.rebind(Action::Save, vec![f(Key::F10)]).unwrap();
    assert_eq!(
        consumed(&km, press(Key::F10, Modifiers::NONE)),
        Some(Action::Save)
    );
    assert_eq!(consumed(&km, press(Key::A, Modifiers::COMMAND)), None);
}
