//! UI strings from translation files: one TOML file per language, English as the fallback.
//!
//! A file is `<code>.toml` with a `[meta]` table holding the language's own name (`name`) and a
//! `[strings]` table of stable key ids to text. `en.toml` and `hi.toml` are built into the binary;
//! any other `<code>.toml` in an `i18n` folder next to the executable is loaded at start-up, so a
//! community file needs no code change. A key missing from a language shows the English text; a
//! key missing from English too shows `⟦key⟧`, which makes the gap visible rather than blank.
//!
//! Strings with values put them in `{name}` placeholders, filled by [`Tr::tf`]. Numbers, dates
//! and amounts are never translated here: they come from the formatters and the engine.
//!
//! Frozen interface:
//!
//! ```text
//! pub struct Catalog { .. }      // every loaded language
//! pub struct Tr { .. }           // one language, cheap to clone; screens take `&Tr`
//! impl Tr { pub fn t(&self, key: &str) -> String; pub fn tf(&self, key: &str, args: &[(&str, &str)]) -> String; }
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The language every key must exist in, and the fallback for every other language.
pub const FALLBACK: &str = "en";

/// Translation files built into the binary, as `(code, text)`.
const BUILTIN: [(&str, &str); 2] = [
    ("en", include_str!("../assets/i18n/en.toml")),
    ("hi", include_str!("../assets/i18n/hi.toml")),
];

/// Folder name, next to the executable, that holds community translation files.
pub const EXTERNAL_DIR: &str = "i18n";

/// A translation file that could not be used.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum I18nError {
    /// The file does not parse, has no `[meta] name`, or a string is not text.
    #[error("translation file for `{code}` is not usable: {reason}")]
    Parse {
        /// The language code the file was offered under.
        code: String,
        /// What is wrong with it.
        reason: String,
    },
    /// The file name is not a language code (lowercase letters, digits and `-`).
    #[error("`{code}` is not a language code")]
    BadCode {
        /// The rejected code.
        code: String,
    },
}

/// One loaded language: its code and its own name (`हिन्दी` for `hi`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LangInfo {
    /// The code, such as `en` or `hi`.
    pub code: String,
    /// The name the language gives itself, for the language switcher.
    pub name: String,
}

#[derive(Debug, Clone, Default)]
struct Language {
    name: String,
    strings: BTreeMap<String, String>,
}

/// Every loaded language. Build it once at start-up and hand [`Tr`] values out of it.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    langs: BTreeMap<String, Language>,
}

impl Catalog {
    /// The catalog of the built-in files (English and Hindi).
    ///
    /// # Errors
    ///
    /// [`I18nError`] if a built-in file is broken; a test keeps this from happening.
    pub fn builtin() -> Result<Self, I18nError> {
        let mut catalog = Self::default();
        for (code, text) in BUILTIN {
            catalog.add_source(code, text)?;
        }
        Ok(catalog)
    }

    /// Adds (or replaces) the language `code` from TOML `text`.
    ///
    /// # Errors
    ///
    /// [`I18nError::BadCode`] for a code that is not a file-name-safe language code, and
    /// [`I18nError::Parse`] for a file that is not a usable translation file.
    pub fn add_source(&mut self, code: &str, text: &str) -> Result<(), I18nError> {
        if !valid_code(code) {
            return Err(I18nError::BadCode {
                code: code.to_owned(),
            });
        }
        let parse_err = |reason: String| I18nError::Parse {
            code: code.to_owned(),
            reason,
        };
        let table: toml::Table = text
            .parse()
            .map_err(|e: toml::de::Error| parse_err(e.to_string()))?;
        let name = table
            .get("meta")
            .and_then(|meta| meta.get("name"))
            .and_then(toml::Value::as_str)
            .ok_or_else(|| parse_err("no [meta] name".to_owned()))?
            .to_owned();
        let strings_table = table
            .get("strings")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| parse_err("no [strings] table".to_owned()))?;
        let mut strings = BTreeMap::new();
        for (key, value) in strings_table {
            let text = value
                .as_str()
                .ok_or_else(|| parse_err(format!("key `{key}` is not text")))?;
            strings.insert(key.clone(), text.to_owned());
        }
        self.langs
            .insert(code.to_owned(), Language { name, strings });
        Ok(())
    }

    /// Loads every `<code>.toml` in `dir` that is usable, and returns the problems with the rest.
    /// A missing folder is not a problem: there are simply no community files.
    pub fn load_dir(&mut self, dir: &Path) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
            .collect();
        paths.sort();
        let mut problems = Vec::new();
        for path in paths {
            let Some(code) = path.file_stem().and_then(|stem| stem.to_str()) else {
                problems.push(format!("{}: file name is not text", path.display()));
                continue;
            };
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    if let Err(e) = self.add_source(code, &text) {
                        problems.push(format!("{}: {e}", path.display()));
                    }
                }
                Err(e) => problems.push(format!("{}: {e}", path.display())),
            }
        }
        problems
    }

    /// The languages loaded, in code order.
    #[must_use]
    pub fn languages(&self) -> Vec<LangInfo> {
        self.langs
            .iter()
            .map(|(code, lang)| LangInfo {
                code: code.clone(),
                name: lang.name.clone(),
            })
            .collect()
    }

    /// Whether `code` is a loaded language.
    #[must_use]
    pub fn has(&self, code: &str) -> bool {
        self.langs.contains_key(code)
    }

    /// Keys that the English file has and language `code` lacks, in order. A language that is
    /// not loaded lacks every key.
    #[must_use]
    pub fn missing(&self, code: &str) -> Vec<String> {
        let Some(english) = self.langs.get(FALLBACK) else {
            return Vec::new();
        };
        let lang = self.langs.get(code);
        english
            .strings
            .keys()
            .filter(|key| lang.is_none_or(|lang| !lang.strings.contains_key(*key)))
            .cloned()
            .collect()
    }

    /// The strings of language `code`, for a screen to draw with. An unknown code draws English.
    #[must_use]
    pub fn tr(&self, code: &str) -> Tr {
        let code = if self.has(code) { code } else { FALLBACK };
        Tr {
            code: code.to_owned(),
            catalog: Arc::new(self.clone()),
        }
    }
}

/// Whether `code` is a usable language code: lowercase ASCII letters, digits and `-`, not empty.
fn valid_code(code: &str) -> bool {
    !code.is_empty()
        && code
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// One language's strings. Cheap to clone; every screen that shows text takes `&Tr`.
#[derive(Debug, Clone)]
pub struct Tr {
    code: String,
    catalog: Arc<Catalog>,
}

impl Default for Tr {
    /// English from the built-in files. A broken built-in file gives an empty catalog, so every
    /// text then shows its key in `⟦⟧` rather than blank.
    fn default() -> Self {
        Catalog::builtin().unwrap_or_default().tr(FALLBACK)
    }
}

impl Tr {
    /// The language code of these strings.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// The text of `key`: this language, else English, else `⟦key⟧`.
    #[must_use]
    pub fn t(&self, key: &str) -> String {
        self.lookup(key)
            .map_or_else(|| format!("\u{27E6}{key}\u{27E7}"), str::to_owned)
    }

    /// The text of `key` with each `{name}` replaced by the value paired with it in `args`.
    #[must_use]
    pub fn tf(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut text = self.t(key);
        for (name, value) in args {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }

    /// The text for a fixed English phrase that lives in a static table (a keyboard-map row, an
    /// action's name): the phrase is looked up among the English strings and its translation in
    /// this language is returned. A phrase the English file does not hold comes back unchanged;
    /// the tests list every such table, so a missing phrase fails there rather than on screen.
    #[must_use]
    pub fn lit(&self, english: &str) -> String {
        match phrase_key(english) {
            Some(key) => self.t(key),
            None => english.to_owned(),
        }
    }

    fn lookup(&self, key: &str) -> Option<&str> {
        let own = self
            .catalog
            .langs
            .get(&self.code)
            .and_then(|lang| lang.strings.get(key));
        own.or_else(|| {
            self.catalog
                .langs
                .get(FALLBACK)
                .and_then(|lang| lang.strings.get(key))
        })
        .map(String::as_str)
    }
}

/// The key of the built-in English string whose text is `english`, if any.
#[must_use]
pub fn phrase_key(english: &str) -> Option<&'static str> {
    static REVERSE: std::sync::OnceLock<std::collections::HashMap<String, String>> =
        std::sync::OnceLock::new();
    let map = REVERSE.get_or_init(|| {
        let mut map = std::collections::HashMap::new();
        if let Some(lang) = Catalog::builtin()
            .ok()
            .and_then(|catalog| catalog.langs.get(FALLBACK).cloned())
        {
            for (key, text) in &lang.strings {
                map.entry(text.clone()).or_insert_with(|| key.clone());
            }
        }
        map
    });
    map.get(english).map(String::as_str)
}

/// The folder of community translation files: `i18n` next to the executable.
#[must_use]
pub fn external_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.join(EXTERNAL_DIR);
    dir.is_dir().then_some(dir)
}

thread_local! {
    /// The language of the frame being drawn on this thread. egui draws on one thread, so the
    /// shell sets it once per frame and drawing helpers that have no `&Tr` of their own read it.
    static CURRENT: std::cell::RefCell<Option<Tr>> = const { std::cell::RefCell::new(None) };
}

/// Makes `tr` the language of the frame being drawn on this thread. The shell calls it at the
/// start of every frame; a test that draws a screen directly calls it with the context's `tr`.
pub fn set_current(tr: &Tr) {
    CURRENT.with(|cell| *cell.borrow_mut() = Some(tr.clone()));
}

/// The language of the frame being drawn on this thread, English until [`set_current`] is called.
#[must_use]
pub fn current() -> Tr {
    static ENGLISH: std::sync::OnceLock<Tr> = std::sync::OnceLock::new();
    CURRENT
        .with(|cell| cell.borrow().clone())
        .unwrap_or_else(|| ENGLISH.get_or_init(Tr::default).clone())
}

/// The text of `key` in the current language ([`current`]).
#[must_use]
pub fn t(key: &str) -> String {
    current().t(key)
}

/// The text of `key` in the current language with `{name}` placeholders filled from `args`.
#[must_use]
pub fn tf(key: &str, args: &[(&str, &str)]) -> String {
    current().tf(key, args)
}

/// The translation of a fixed English phrase in the current language ([`Tr::lit`]).
#[must_use]
pub fn lit(english: &str) -> String {
    current().lit(english)
}
