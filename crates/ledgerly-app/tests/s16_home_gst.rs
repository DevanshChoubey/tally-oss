//! Round s16: the Home GST tile and the IMS nudge carry the engine's answers. Expected values
//! are the mock's own text (`design/mock-v2/index.html`, `NUDGES()` n2 and `SCREENS.home`) or are
//! derived by hand as written beside each; none is the output of the code under test.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_api::HomeGst;
use ledgerly_app::screens::home::HomeTarget;
use ledgerly_app::screens::home::kpi::gst_tile;
use ledgerly_app::screens::home::nudge::{NudgeAction, NudgeTone, ims_nudge};
use ledgerly_core::{Outcome, Paise};
use time::macros::date;

#[test]
fn the_gst_tile_shows_the_cash_the_engine_worked_out() {
    // provenance: derived: the tile is titled with the month of the as-at date and carries the
    // engine's cash payable as it came (39,498.00 for the sample, derived in
    // crates/ledgerly-sample/tests/s16_payment.rs).
    let tile = gst_tile(
        &Outcome::Value(Paise::new(3_949_800)),
        date!(2026 - 11 - 20),
    );
    assert_eq!(tile.label, "GST cash payable, Nov");
    assert_eq!(tile.value, Outcome::Value(Paise::new(3_949_800)));
    assert_eq!(tile.footnote, "GSTR-3B working, after accepted ITC");
}

#[test]
fn the_gst_tile_keeps_the_engines_reason_when_there_is_no_figure() {
    // provenance: derived: a refusal is shown as the refusal, never as a figure.
    let refused = Outcome::unsupported("Needs GSTR-2B", "no 2B file is imported for the month");
    let tile = gst_tile(&refused, date!(2026 - 11 - 20));
    assert_eq!(tile.value, refused);
}

#[test]
fn the_ims_nudge_reads_like_the_mock() {
    // provenance: the mock's n2 card: title "N 2B records have no IMS action", body "With no
    // action they are deemed accepted when 2B is generated. Review them before GSTR-3B.", source
    // "IMS rule", action "Review 2B", amber.
    let n = ims_nudge(&Outcome::Value(Some(3))).unwrap();
    assert_eq!(n.title, "3 2B records have no IMS action");
    assert_eq!(
        n.body,
        "With no action they are deemed accepted when 2B is generated. Review them before GSTR-3B."
    );
    assert_eq!(n.source, "IMS rule");
    assert_eq!(n.action_label, "Review 2B");
    assert_eq!(n.tone, NudgeTone::Warn);
    assert_eq!(
        n.action,
        NudgeAction::Open(HomeTarget::GstReturns {
            tab: "gstr2b-ims".to_owned()
        })
    );
}

#[test]
fn the_ims_nudge_says_record_for_one() {
    // provenance: derived: singular for a count of one.
    let n = ims_nudge(&Outcome::Value(Some(1))).unwrap();
    assert_eq!(n.title, "1 2B record has no IMS action");
}

#[test]
fn no_records_without_action_and_no_file_raise_no_nudge() {
    // provenance: derived: nothing to review is not a nudge; with no 2B file there is nothing
    // to count (the GST screen says the file is missing).
    assert!(ims_nudge(&Outcome::Value(Some(0))).is_none());
    assert!(ims_nudge(&Outcome::Value(None)).is_none());
}

#[test]
fn an_ims_count_the_engine_cannot_give_is_said_not_hidden() {
    // provenance: derived: silence must not read as all clear, so a refusal is a neutral card.
    let n = ims_nudge(&Outcome::unsupported(
        "IMS",
        "the 2B file could not be read",
    ))
    .unwrap();
    assert_eq!(n.tone, NudgeTone::Neutral);
    assert_eq!(n.body, "the 2B file could not be read");
}

#[test]
fn home_gst_is_what_the_api_returns() {
    // provenance: derived: the type carries both answers.
    let g = HomeGst {
        cash_payable: Outcome::Value(Paise::new(1)),
        ims_no_action: Outcome::Value(Some(2)),
    };
    assert_eq!(g.ims_no_action, Outcome::Value(Some(2)));
}
