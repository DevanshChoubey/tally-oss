//! Hindi on the GST screen: its key bar, the exception buckets, the IMS actions and the match
//! words come from the translation files, not from English literals in the screen code.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_app::i18n::Catalog;
use ledgerly_app::screens::gst::{Bucket, GstTab, ImsAction, MatchState, key_hints};

/// The key bar's label words. `KeyHint` is not re-exported, so the labels are read in place.
macro_rules! labels {
    ($hints:expr) => {
        $hints
            .iter()
            .map(|k| k.label.clone())
            .collect::<Vec<String>>()
    };
}

#[test]
fn gst_key_bar_comes_from_the_hindi_file() {
    let hi = Catalog::builtin().unwrap().tr("hi");
    let en = Catalog::builtin().unwrap().tr("en");
    let gstr1 = labels!(key_hints(GstTab::Gstr1, false, &hi));
    assert!(gstr1.contains(&hi.t("gst.export_json")), "{gstr1:?}");
    assert!(!gstr1.contains(&"Export JSON".to_owned()), "{gstr1:?}");
    // The reconciliation tab with its file loaded has the IMS actions in Hindi.
    let recon = labels!(key_hints(GstTab::Reconciliation, true, &hi));
    assert!(recon.contains(&hi.t("gst.ims.accept")), "{recon:?}");
    // The English key bar keeps its English words.
    let english = labels!(key_hints(GstTab::Reconciliation, true, &en));
    assert!(english.contains(&"Accept".to_owned()), "{english:?}");
}

#[test]
fn gst_bucket_action_and_match_words_come_from_the_hindi_file() {
    let hi = Catalog::builtin().unwrap().tr("hi");
    // provenance: the English words are the mock's bucket, action and match labels (index.html GST).
    assert_eq!(
        Bucket::NeedsCorrection.label_in(&hi),
        hi.t("gst.bucket.needs_correction")
    );
    assert_ne!(Bucket::Ready.label_in(&hi), "Ready");
    assert_eq!(ImsAction::Accept.label_in(&hi), hi.t("gst.ims.accept"));
    assert_ne!(ImsAction::Accept.label_in(&hi), "Accept");
    assert_eq!(
        MatchState::NotIn2b.label_in(&hi),
        hi.t("gst.match.not_in_2b")
    );
}
