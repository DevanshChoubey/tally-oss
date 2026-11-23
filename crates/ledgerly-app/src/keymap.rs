//! Keymap with the classic preset; TOML round trip. **Card U2** implements this module.
//!
//! Frozen interface (plan card U2):
//!
//! ```text
//! pub enum Action { .. }   pub struct Keymap { .. }
//! pub fn classic() -> Keymap;   // loaded from assets/keymap-classic.toml
//! pub fn from_toml(s: &str) -> Result<Keymap, KeymapError>;  pub fn to_toml(k: &Keymap) -> String;
//! ```
//!
//! A [`Keymap`] maps each [`Action`] to zero or more [`Chord`]s. A chord belongs to at most one
//! action, so a key press never has two meanings. The keymap is fully rebindable (spec section 7)
//! and is exported as TOML by [`to_toml`]; [`from_toml`] reads it back and rejects anything it
//! does not understand (an unknown action, an unknown key, a chord bound twice) instead of
//! guessing.
//!
//! Key matching is exact on Ctrl/Cmd, Alt and Shift: with `F5` bound to a payment voucher, a
//! press of Alt+F5 does not open one. (egui's own `consume_key` ignores extra Alt and Shift, which
//! would make that mistake.)

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use egui::{Event, InputState, Key, Modifiers};
use serde::Deserialize;

/// The classic preset, embedded at build time.
const CLASSIC_TOML: &str = include_str!("../assets/keymap-classic.toml");

/// The standard desktop preset, embedded at build time.
const STANDARD_TOML: &str = include_str!("../assets/keymap-standard.toml");

/// Name of the classic preset.
const CLASSIC_NAME: &str = "classic";

/// Name of the standard desktop preset.
const STANDARD_NAME: &str = "standard";

/// Names of the presets that ship, in the order the keyboard map lists them.
pub const PRESET_NAMES: [&str; 2] = [CLASSIC_NAME, STANDARD_NAME];

/// The preset in use when nothing else is chosen. There is no persistence yet, so the app always
/// uses this one; a stored choice would replace it once settings are stored.
pub const DEFAULT_PRESET: &str = CLASSIC_NAME;

/// Keys a focused text field edits itself when Ctrl is held: select all, copy, paste, cut, undo
/// and redo.
const TEXT_EDIT_KEYS: [Key; 6] = [Key::A, Key::C, Key::V, Key::X, Key::Z, Key::Y];

/// Something the user can ask for with a key press. Each action has a stable id (used in the
/// TOML file) and a label in our own words (used in the palette and key bar).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    /// Create a contra voucher (cash and bank moves).
    NewContra,
    /// Create a payment voucher.
    NewPayment,
    /// Create a receipt voucher.
    NewReceipt,
    /// Create a journal voucher.
    NewJournal,
    /// Create a sales voucher.
    NewSales,
    /// Create a purchase voucher.
    NewPurchase,
    /// Create a credit note.
    NewCreditNote,
    /// Create a debit note.
    NewDebitNote,
    /// Change the date or the report period.
    ChangePeriod,
    /// Switch or select the company.
    SwitchCompany,
    /// Open the company features.
    CompanyFeatures,
    /// Configure the current screen.
    Configure,
    /// Show help.
    Help,
    /// Go back one screen (Esc).
    Back,
    /// Open the command palette on top of the current screen (resumable).
    OpenPalette,
    /// Open the command palette so the choice replaces the current screen.
    SwitchPalette,
    /// Open a new tab.
    NewTab,
    /// Close the current tab.
    CloseTab,
    /// Save the current form.
    Save,
    /// Create a master from a lookup field without leaving the voucher.
    CreateMaster,
    /// Alter the master under the cursor.
    AlterMaster,
    /// Open the optional sub-fields of the current line.
    MoreDetails,
    /// Delete the current voucher line.
    DeleteLine,
    /// Switch a voucher line between debit and credit.
    SwitchSide,
    /// Delete the voucher.
    DeleteVoucher,
    /// Cancel the voucher (keeps its number).
    CancelVoucher,
    /// Duplicate the selected voucher.
    DuplicateVoucher,
    /// Insert a voucher above the selection.
    InsertVoucher,
    /// Undo the last change.
    Undo,
    /// Toggle the detailed and condensed view of a report.
    ToggleDetail,
    /// Print the current screen.
    Print,
    /// Export the current screen.
    Export,
    /// Email the current screen.
    Email,
}

impl Action {
    /// Every action, in the order the TOML export and the palette list them.
    pub const ALL: &'static [Action] = &[
        Action::NewContra,
        Action::NewPayment,
        Action::NewReceipt,
        Action::NewJournal,
        Action::NewSales,
        Action::NewPurchase,
        Action::NewCreditNote,
        Action::NewDebitNote,
        Action::ChangePeriod,
        Action::SwitchCompany,
        Action::CompanyFeatures,
        Action::Configure,
        Action::Help,
        Action::Back,
        Action::OpenPalette,
        Action::SwitchPalette,
        Action::NewTab,
        Action::CloseTab,
        Action::Save,
        Action::CreateMaster,
        Action::AlterMaster,
        Action::MoreDetails,
        Action::DeleteLine,
        Action::SwitchSide,
        Action::DeleteVoucher,
        Action::CancelVoucher,
        Action::DuplicateVoucher,
        Action::InsertVoucher,
        Action::Undo,
        Action::ToggleDetail,
        Action::Print,
        Action::Export,
        Action::Email,
    ];

    /// The stable id used as the key in the keymap file.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Action::NewContra => "new-contra",
            Action::NewPayment => "new-payment",
            Action::NewReceipt => "new-receipt",
            Action::NewJournal => "new-journal",
            Action::NewSales => "new-sales",
            Action::NewPurchase => "new-purchase",
            Action::NewCreditNote => "new-credit-note",
            Action::NewDebitNote => "new-debit-note",
            Action::ChangePeriod => "change-period",
            Action::SwitchCompany => "switch-company",
            Action::CompanyFeatures => "company-features",
            Action::Configure => "configure",
            Action::Help => "help",
            Action::Back => "back",
            Action::OpenPalette => "open-palette",
            Action::SwitchPalette => "switch-palette",
            Action::NewTab => "new-tab",
            Action::CloseTab => "close-tab",
            Action::Save => "save",
            Action::CreateMaster => "create-master",
            Action::AlterMaster => "alter-master",
            Action::MoreDetails => "more-details",
            Action::DeleteLine => "delete-line",
            Action::SwitchSide => "switch-side",
            Action::DeleteVoucher => "delete-voucher",
            Action::CancelVoucher => "cancel-voucher",
            Action::DuplicateVoucher => "duplicate-voucher",
            Action::InsertVoucher => "insert-voucher",
            Action::Undo => "undo",
            Action::ToggleDetail => "toggle-detail",
            Action::Print => "print",
            Action::Export => "export",
            Action::Email => "email",
        }
    }

    /// The action with this id, if there is one.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Action> {
        Action::ALL.iter().copied().find(|a| a.id() == id)
    }

    /// Human-readable name, in our own words.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Action::NewContra => "New contra voucher",
            Action::NewPayment => "New payment",
            Action::NewReceipt => "New receipt",
            Action::NewJournal => "New journal voucher",
            Action::NewSales => "New sales invoice",
            Action::NewPurchase => "New purchase",
            Action::NewCreditNote => "New credit note",
            Action::NewDebitNote => "New debit note",
            Action::ChangePeriod => "Change date or period",
            Action::SwitchCompany => "Switch company",
            Action::CompanyFeatures => "Company features",
            Action::Configure => "Configure this screen",
            Action::Help => "Help",
            Action::Back => "Go back",
            Action::OpenPalette => "Open command palette",
            Action::SwitchPalette => "Switch to (replace this screen)",
            Action::NewTab => "New tab",
            Action::CloseTab => "Close tab",
            Action::Save => "Save",
            Action::CreateMaster => "Create master here",
            Action::AlterMaster => "Alter master",
            Action::MoreDetails => "More details",
            Action::DeleteLine => "Delete line",
            Action::SwitchSide => "Switch debit or credit",
            Action::DeleteVoucher => "Delete voucher",
            Action::CancelVoucher => "Cancel voucher",
            Action::DuplicateVoucher => "Duplicate voucher",
            Action::InsertVoucher => "Insert voucher",
            Action::Undo => "Undo",
            Action::ToggleDetail => "Toggle detailed view",
            Action::Print => "Print",
            Action::Export => "Export",
            Action::Email => "Email",
        }
    }
}

/// Why a chord string could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChordParseError {
    /// The text was empty.
    #[error("the chord is empty")]
    Empty,
    /// The text has modifiers but no key (for example `Ctrl+`).
    #[error("the chord has no key after its modifiers")]
    NoKey,
    /// A part before the last is not Ctrl, Alt or Shift.
    #[error("`{0}` is not a modifier (use Ctrl, Alt or Shift)")]
    UnknownModifier(String),
    /// The same modifier appears twice.
    #[error("the modifier {0} is given twice")]
    DuplicateModifier(&'static str),
    /// The last part is not a key name.
    #[error("`{0}` is not a key name")]
    UnknownKey(String),
}

/// Which modifier a word names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModifierName {
    Command,
    Alt,
    Shift,
}

impl ModifierName {
    fn parse(word: &str) -> Option<ModifierName> {
        match word.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "cmd" | "command" => Some(ModifierName::Command),
            "alt" | "option" => Some(ModifierName::Alt),
            "shift" => Some(ModifierName::Shift),
            _ => None,
        }
    }

    fn text(self) -> &'static str {
        match self {
            ModifierName::Command => "Ctrl",
            ModifierName::Alt => "Alt",
            ModifierName::Shift => "Shift",
        }
    }
}

/// One key press with its modifiers, for example `Ctrl+A` or `F4`. `Ctrl` means the platform's
/// command key (Cmd on macOS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Chord {
    /// Ctrl (Cmd on macOS) is held.
    pub command: bool,
    /// Alt is held.
    pub alt: bool,
    /// Shift is held.
    pub shift: bool,
    /// The key.
    pub key: Key,
}

impl Chord {
    /// A chord of a key with no modifiers.
    #[must_use]
    pub fn key(key: Key) -> Chord {
        Chord {
            command: false,
            alt: false,
            shift: false,
            key,
        }
    }

    /// The same key with Ctrl held.
    #[must_use]
    pub fn with_ctrl(self) -> Chord {
        Chord {
            command: true,
            ..self
        }
    }

    /// The same key with Alt held.
    #[must_use]
    pub fn with_alt(self) -> Chord {
        Chord { alt: true, ..self }
    }

    /// The same key with Shift held.
    #[must_use]
    pub fn with_shift(self) -> Chord {
        Chord {
            shift: true,
            ..self
        }
    }

    /// The egui modifier set this chord needs.
    #[must_use]
    pub fn modifiers(self) -> Modifiers {
        let mut m = Modifiers::NONE;
        if self.command {
            m |= Modifiers::COMMAND;
        }
        if self.alt {
            m |= Modifiers::ALT;
        }
        if self.shift {
            m |= Modifiers::SHIFT;
        }
        m
    }

    /// Does this key event (key plus the modifiers held) press exactly this chord?
    fn matches(self, key: Key, held: Modifiers) -> bool {
        self.key == key && held.matches_exact(self.modifiers())
    }
}

impl fmt::Display for Chord {
    /// Writes the canonical text: modifiers in the order Ctrl, Alt, Shift, then the key name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.command {
            f.write_str(ModifierName::Command.text())?;
            f.write_str("+")?;
        }
        if self.alt {
            f.write_str(ModifierName::Alt.text())?;
            f.write_str("+")?;
        }
        if self.shift {
            f.write_str(ModifierName::Shift.text())?;
            f.write_str("+")?;
        }
        f.write_str(self.key.name())
    }
}

impl FromStr for Chord {
    type Err = ChordParseError;

    fn from_str(s: &str) -> Result<Chord, ChordParseError> {
        let text = s.trim();
        if text.is_empty() {
            return Err(ChordParseError::Empty);
        }
        let mut parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let key_text = parts.pop().ok_or(ChordParseError::Empty)?;
        if key_text.is_empty() {
            return Err(ChordParseError::NoKey);
        }
        let mut seen_command = false;
        let mut seen_alt = false;
        let mut seen_shift = false;
        for part in parts {
            let modifier = ModifierName::parse(part)
                .ok_or_else(|| ChordParseError::UnknownModifier(part.to_owned()))?;
            let slot = match modifier {
                ModifierName::Command => &mut seen_command,
                ModifierName::Alt => &mut seen_alt,
                ModifierName::Shift => &mut seen_shift,
            };
            if *slot {
                return Err(ChordParseError::DuplicateModifier(modifier.text()));
            }
            *slot = true;
        }
        if ModifierName::parse(key_text).is_some() {
            return Err(ChordParseError::NoKey);
        }
        let key = Key::from_name(key_text)
            .ok_or_else(|| ChordParseError::UnknownKey(key_text.to_owned()))?;
        Ok(Chord {
            command: seen_command,
            alt: seen_alt,
            shift: seen_shift,
            key,
        })
    }
}

/// Why a keymap could not be built, read or changed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeymapError {
    /// The text is not valid TOML, or has the wrong shape.
    #[error("the keymap file is not valid: {0}")]
    Toml(String),
    /// A binding names an action that does not exist.
    #[error("the keymap binds unknown action `{id}`")]
    UnknownAction {
        /// The id as written in the file.
        id: String,
    },
    /// A chord in the file could not be read.
    #[error("action `{action}`: `{text}` is not a key chord: {source}")]
    BadChord {
        /// The action's id.
        action: String,
        /// The chord text as written.
        text: String,
        /// What is wrong with it.
        source: ChordParseError,
    },
    /// One chord is bound to two actions.
    #[error("{chord} is bound to both `{first}` and `{second}`; a key may do one thing")]
    Conflict {
        /// The chord, in canonical text.
        chord: String,
        /// The first action's id.
        first: &'static str,
        /// The second action's id.
        second: &'static str,
    },
    /// One chord is listed twice for the same action.
    #[error("{chord} is listed twice for `{action}`")]
    DuplicateChord {
        /// The action's id.
        action: &'static str,
        /// The chord, in canonical text.
        chord: String,
    },
    /// No preset of this name ships (see [`PRESET_NAMES`]).
    #[error("there is no keyboard preset named `{name}`")]
    UnknownPreset {
        /// The name asked for.
        name: String,
    },
}

/// The shape of the keymap file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawKeymap {
    preset: String,
    bindings: BTreeMap<String, Vec<String>>,
}

/// A set of key bindings. An action with no chords is unbound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    name: String,
    bindings: BTreeMap<Action, Vec<Chord>>,
}

impl Keymap {
    /// A keymap with this name and no bindings.
    #[must_use]
    pub fn unbound(name: impl Into<String>) -> Keymap {
        Keymap {
            name: name.into(),
            bindings: BTreeMap::new(),
        }
    }

    /// Builds a keymap, rejecting a chord that is bound twice. An action with an empty list is
    /// stored as unbound.
    ///
    /// # Errors
    /// [`KeymapError::Conflict`] when two actions share a chord, [`KeymapError::DuplicateChord`]
    /// when one action lists a chord twice.
    pub fn new(
        name: impl Into<String>,
        bindings: BTreeMap<Action, Vec<Chord>>,
    ) -> Result<Keymap, KeymapError> {
        // An action with no chords is the same as an action that is absent.
        let bindings: BTreeMap<Action, Vec<Chord>> = bindings
            .into_iter()
            .filter(|(_, chords)| !chords.is_empty())
            .collect();
        let mut seen: BTreeMap<Chord, Action> = BTreeMap::new();
        for (action, chords) in &bindings {
            for chord in chords {
                if let Some(previous) = seen.insert(*chord, *action) {
                    return Err(conflict(*chord, previous, *action));
                }
            }
        }
        Ok(Keymap {
            name: name.into(),
            bindings,
        })
    }

    /// The preset's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The chords bound to an action (empty when unbound).
    #[must_use]
    pub fn chords(&self, action: Action) -> &[Chord] {
        self.bindings.get(&action).map_or_default(Vec::as_slice)
    }

    /// The action a chord is bound to.
    #[must_use]
    pub fn action_for(&self, chord: Chord) -> Option<Action> {
        self.bindings
            .iter()
            .find(|(_, chords)| chords.contains(&chord))
            .map(|(action, _)| *action)
    }

    /// The first chord of an action as text, for hints (for example `Ctrl+A`).
    #[must_use]
    pub fn shortcut_label(&self, action: Action) -> Option<String> {
        self.chords(action).first().map(ToString::to_string)
    }

    /// Replaces the chords of one action. Nothing changes when this fails.
    ///
    /// # Errors
    /// [`KeymapError::Conflict`] when a chord is already bound to another action,
    /// [`KeymapError::DuplicateChord`] when `chords` lists one twice.
    pub fn rebind(&mut self, action: Action, chords: Vec<Chord>) -> Result<(), KeymapError> {
        let mut candidate = self.bindings.clone();
        candidate.insert(action, chords);
        let checked = Keymap::new(self.name.clone(), candidate)?;
        self.bindings = checked.bindings;
        Ok(())
    }

    /// Removes every chord of an action.
    pub fn unbind(&mut self, action: Action) {
        self.bindings.remove(&action);
    }

    /// Finds the action pressed this frame and consumes its key event so no widget also sees it.
    /// Returns `None` when no bound chord was pressed. Modifiers must match exactly.
    pub fn consume(&self, input: &mut InputState) -> Option<Action> {
        for (action, chords) in &self.bindings {
            for chord in chords {
                if consume_chord(input, *chord) {
                    return Some(*action);
                }
            }
        }
        None
    }

    /// Consumes the key event of one action, if one of its chords was pressed this frame. Screens
    /// call this each frame for the keys they own.
    pub fn consume_action(&self, input: &mut InputState, action: Action) -> bool {
        let mut fired = false;
        for chord in self.chords(action) {
            if consume_chord(input, *chord) {
                fired = true;
            }
        }
        fired
    }

    /// Like [`Keymap::consume_action`], for a screen that has text fields. While a text field has
    /// focus (`text_focused`, from `egui::Context::text_edit_focused`), a text-editing chord (see
    /// [`is_text_edit_chord`]) is left in the frame, so the field can select all, copy or paste
    /// with it. Every other chord of `action` still fires. Returns true when a chord was consumed.
    ///
    /// Call this before the widgets draw: a key that is consumed never reaches a text field.
    pub fn consume_action_outside_text(
        &self,
        input: &mut InputState,
        action: Action,
        text_focused: bool,
    ) -> bool {
        let mut fired = false;
        for chord in self.chords(action) {
            if text_focused && is_text_edit_chord(*chord) {
                continue;
            }
            if consume_chord(input, *chord) {
                fired = true;
            }
        }
        fired
    }
}

/// Whether a chord is one a focused text field takes for itself: Ctrl with A, C, V, X, Z or Y,
/// and no Alt or Shift. With a text field focused, such a chord is never consumed by an action
/// (see [`Keymap::consume_action_outside_text`]), so Ctrl+A selects all and never saves.
#[must_use]
pub fn is_text_edit_chord(chord: Chord) -> bool {
    chord.command && !chord.alt && !chord.shift && TEXT_EDIT_KEYS.contains(&chord.key)
}

/// Removes every press of exactly this chord from the frame's events; true if there was one.
fn consume_chord(input: &mut InputState, chord: Chord) -> bool {
    let before = input.events.len();
    input.events.retain(|event| {
        !matches!(
            event,
            Event::Key { key, modifiers, pressed: true, .. } if chord.matches(*key, *modifiers)
        )
    });
    input.events.len() != before
}

fn conflict(chord: Chord, first: Action, second: Action) -> KeymapError {
    if first == second {
        KeymapError::DuplicateChord {
            action: first.id(),
            chord: chord.to_string(),
        }
    } else {
        KeymapError::Conflict {
            chord: chord.to_string(),
            first: first.id(),
            second: second.id(),
        }
    }
}

/// The classic preset (spec section 7), read from `assets/keymap-classic.toml`.
///
/// The file is embedded at build time and a test checks that it parses ([`try_classic`] returns
/// the error). Should it ever fail anyway, this returns a keymap with no bindings and logs the
/// error, so keys do nothing rather than do the wrong thing.
#[must_use]
pub fn classic() -> Keymap {
    match try_classic() {
        Ok(keymap) => keymap,
        Err(error) => {
            tracing::error!("the embedded classic keymap is invalid: {error}");
            Keymap::unbound(CLASSIC_NAME)
        }
    }
}

/// Like [`classic`], but returns the error instead of an empty keymap.
///
/// # Errors
/// Any [`KeymapError`] from reading the embedded file.
pub fn try_classic() -> Result<Keymap, KeymapError> {
    from_toml(CLASSIC_TOML)
}

/// The standard desktop preset (decision 4, `docs/status/2026-11-22-ui-parity-report.md`), read
/// from `assets/keymap-standard.toml`. Saves with Ctrl+S, so Ctrl+A stays select-all in text
/// fields. Like [`classic`], this returns an empty keymap and logs the error if the file is bad.
#[must_use]
pub fn standard() -> Keymap {
    match try_standard() {
        Ok(keymap) => keymap,
        Err(error) => {
            tracing::error!("the embedded standard keymap is invalid: {error}");
            Keymap::unbound(STANDARD_NAME)
        }
    }
}

/// Like [`standard`], but returns the error instead of an empty keymap.
///
/// # Errors
/// Any [`KeymapError`] from reading the embedded file.
pub fn try_standard() -> Result<Keymap, KeymapError> {
    from_toml(STANDARD_TOML)
}

/// The preset called `name` (`classic` or `standard`, see [`PRESET_NAMES`]). Loading by name
/// does not change the keymap in use: the app keeps [`DEFAULT_PRESET`] until a choice can be
/// stored and applied.
///
/// # Errors
/// [`KeymapError::UnknownPreset`] for any other name; any [`KeymapError`] from reading the file.
pub fn preset(name: &str) -> Result<Keymap, KeymapError> {
    match name {
        CLASSIC_NAME => try_classic(),
        STANDARD_NAME => try_standard(),
        other => Err(KeymapError::UnknownPreset {
            name: other.to_owned(),
        }),
    }
}

/// Reads a keymap from TOML (the format [`to_toml`] writes).
///
/// An action missing from the file is unbound; nothing is filled in from the classic preset.
///
/// # Errors
/// [`KeymapError::Toml`] for bad TOML or an unknown top-level field, [`KeymapError::UnknownAction`],
/// [`KeymapError::BadChord`], [`KeymapError::Conflict`] and [`KeymapError::DuplicateChord`].
pub fn from_toml(s: &str) -> Result<Keymap, KeymapError> {
    let raw: RawKeymap = toml::from_str(s).map_err(|e| KeymapError::Toml(e.to_string()))?;
    let mut bindings: BTreeMap<Action, Vec<Chord>> = BTreeMap::new();
    for (id, texts) in raw.bindings {
        let action =
            Action::from_id(&id).ok_or_else(|| KeymapError::UnknownAction { id: id.clone() })?;
        let mut chords = Vec::with_capacity(texts.len());
        for text in texts {
            let chord = text
                .parse::<Chord>()
                .map_err(|source| KeymapError::BadChord {
                    action: id.clone(),
                    text: text.clone(),
                    source,
                })?;
            chords.push(chord);
        }
        bindings.insert(action, chords);
    }
    Keymap::new(raw.preset, bindings)
}

/// Writes a keymap as TOML. Every action is listed, in [`Action::ALL`] order, so an unbound
/// action shows as an empty list. [`from_toml`] reads the result back to an equal keymap.
#[must_use]
pub fn to_toml(k: &Keymap) -> String {
    let mut out = String::new();
    out.push_str("preset = ");
    out.push_str(&toml_string(&k.name));
    out.push_str("\n\n[bindings]\n");
    for action in Action::ALL {
        let chords: Vec<String> = k
            .chords(*action)
            .iter()
            .map(|c| toml_string(&c.to_string()))
            .collect();
        out.push_str(action.id());
        out.push_str(" = [");
        out.push_str(&chords.join(", "));
        out.push_str("]\n");
    }
    out
}

/// A TOML basic string for `s`, with the characters TOML requires escaped.
#[expect(
    clippy::format_push_string,
    reason = "a \\u escape is rare; format! keeps the four-digit padding readable"
)]
fn toml_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                out.push_str("\\u");
                out.push_str(&format!("{:04X}", u32::from(c)));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
