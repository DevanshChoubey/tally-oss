//! Card P1: Print appears on the key map under Reports. Provenance: index.html:1839 lists
//! `['Ctrl+P','Ctrl+P','Print']` in the map's Reports group. The mock's report key bars
//! (index.html:1634) do not list Print, and `the_key_bar_reads_like_the_mocks` pins that bar, so
//! Ctrl+P is shown on the key map and not on the key bar.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_app::screens::keymap_map::layout;

#[test]
fn the_key_map_lists_print_under_reports() {
    let cards = layout();
    let reports = cards
        .iter()
        .find(|card| card.title == "Reports")
        .expect("the key map has a Reports card");
    assert!(
        reports.rows.iter().any(|row| row.label == "Print"),
        "Reports rows: {:?}",
        reports.rows.iter().map(|r| r.label).collect::<Vec<_>>()
    );
}
