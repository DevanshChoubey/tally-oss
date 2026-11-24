//! The capture harness can fix the date the screens call "today": the sample company's own
//! 21 Nov 2026. Its own test binary, because the override lasts for the whole process.
//!
//! Provenance: `design/mock-v2/index.html:489` (`CO.today = '21-11-2026'`); the hour is the
//! mock's morning greeting, any fixed hour would do, 10 is used.

#![allow(clippy::unwrap_used, reason = "test code")]

use ledgerly_app::shell::navigate::{hour_now, set_clock_override, today};
use time::macros::date;

#[test]
fn the_override_fixes_today_and_the_hour() {
    set_clock_override(date!(2026 - 11 - 21), 10);
    assert_eq!(today(), date!(2026 - 11 - 21));
    assert_eq!(hour_now(), 10);
}
