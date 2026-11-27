//! S22 round 3: the Hindi mock keeps key names as they are printed on the keyboard.
//!
//! provenance: derived. The app's key bar under Hindi draws the keycap "Enter" (key names are not
//! in `hi.toml`; `KeyHint::keys` is drawn as given), and a key is named by what is printed on it.
//! The mock's Hindi pass (`translateDom` in `design/mock-v2/index.html`) replaces every whole text
//! node the app translates, and "Enter" is also the sidebar group "Enter" (`sidebar.group.enter`,
//! "प्रविष्टि"), so it must leave the text of a keycap (`<kbd>`) alone.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::path::PathBuf;

fn mock() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../design/mock-v2/index.html");
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn the_hindi_pass_of_the_mock_leaves_keycaps_alone() {
    let html = mock();
    let start = html.find("function translateDom(").unwrap();
    let end = start + html[start..].find("\nfunction ").unwrap();
    let body = &html[start..end];
    assert!(
        body.contains("KBD"),
        "translateDom must skip the text of <kbd> keycaps:\n{body}"
    );
}
