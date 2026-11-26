//! Every English phrase the screens hand to `lit(...)` is a value in `en.toml`, and every key the
//! screens name through the thread-local `t`/`tf` exists. A phrase without an entry would draw in
//! English under Hindi with no warning, so this fails the build instead.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests may unwrap; a failure is a test failure"
)]

use ledgerly_app::i18n::Catalog;
use std::collections::BTreeSet;

fn sources() -> Vec<(std::path::PathBuf, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![root];
    let mut out = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                out.push((path, text));
            }
        }
    }
    out
}

/// The string literal that starts right after `at` (a `"`), with `\"` and `\u{..}` decoded, or
/// `None` when it holds another escape or does not close on this line.
fn literal(rest: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\n' => return None,
            '\\' => match chars.next()? {
                '"' => out.push('"'),
                'u' => {
                    let mut hex = String::new();
                    if chars.next()? != '{' {
                        return None;
                    }
                    for h in chars.by_ref() {
                        if h == '}' {
                            break;
                        }
                        hex.push(h);
                    }
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                _ => return None,
            },
            other => out.push(other),
        }
    }
    None
}

#[test]
fn every_lit_phrase_has_an_english_entry() {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/i18n/en.toml");
    let table: toml::Table = std::fs::read_to_string(file).unwrap().parse().unwrap();
    let strings = table["strings"].as_table().unwrap();
    let known: BTreeSet<String> = strings
        .values()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    let mut missing = Vec::new();
    for (path, text) in sources() {
        // The unit tests at the end of a file call `lit` with made-up phrases.
        let cut = text.find("#[cfg(test)]").unwrap_or(text.len());
        let text = &text[..cut];
        for marker in ["lit(\""] {
            for (at, _) in text.match_indices(marker) {
                if let Some(phrase) = literal(&text[at + marker.len()..])
                    && !known.contains(&phrase)
                {
                    missing.push(format!("{}: {phrase}", path.display()));
                }
            }
        }
    }
    assert_eq!(missing, Vec::<String>::new());
}

#[test]
fn every_thread_local_key_exists() {
    let english = Catalog::builtin().unwrap();
    let en = english.tr("en");
    let mut unknown = Vec::new();
    for (path, text) in sources() {
        let cut = text.find("#[cfg(test)]").unwrap_or(text.len());
        let text = &text[..cut];
        for marker in ["i18n::t(\"", "i18n::tf(\""] {
            for (at, _) in text.match_indices(marker) {
                if let Some(key) = literal(&text[at + marker.len()..])
                    && en.t(&key).starts_with('\u{27E6}')
                {
                    unknown.push(format!("{}: {key}", path.display()));
                }
            }
        }
    }
    assert_eq!(unknown, Vec::<String>::new());
}
