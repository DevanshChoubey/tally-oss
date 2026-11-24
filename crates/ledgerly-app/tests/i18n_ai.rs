//! Hindi on the assistant screens: the AI gate notes, Ask your books and the rest of the AI
//! screens take their words from the translation files, not from English literals.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_app::Screen;
use ledgerly_app::i18n::Catalog;
use ledgerly_app::screens::ai_gate::{Feature, Gate, reason, reason_in};
use ledgerly_app::screens::ask::{self, AskReport, AskScreen};

const FEATURES: [Feature; 3] = [Feature::Ask, Feature::PlainWords, Feature::BillReading];

fn gates() -> Vec<Gate> {
    vec![
        Gate::NoCompany,
        Gate::Failed("the file is locked".to_owned()),
        Gate::Unset,
        Gate::Off,
        Gate::Local,
        Gate::Cloud,
    ]
}

#[test]
fn gate_reasons_come_from_the_hindi_file() {
    let hi = Catalog::builtin().unwrap().tr("hi");
    let en = Catalog::builtin().unwrap().tr("en");
    // provenance: the English sentences are the module's own words (ai_gate.rs `reason`, which
    // quotes index.html:1169 and 1430 for AI off and bill reading off).
    assert_eq!(
        reason_in(Feature::Ask, &Gate::Off, &en).as_deref(),
        Some(
            "AI is off for this company, so questions cannot be turned into queries. These reports answer most questions directly."
        )
    );
    assert_eq!(
        reason_in(Feature::BillReading, &Gate::Off, &en).as_deref(),
        Some("Bill reading is off for this company. Type the bill in the purchase form instead.")
    );
    for feature in FEATURES {
        for gate in gates() {
            let english = reason(feature, &gate);
            let hindi = reason_in(feature, &gate, &hi);
            assert_ne!(hindi, english, "{feature:?} {gate:?} is still English");
            let text = hindi.unwrap();
            assert!(!text.contains('\u{27E6}'), "missing key in {text}");
        }
    }
    assert_eq!(reason_in(Feature::Ask, &Gate::Reading, &hi), None);
}

#[test]
fn ask_title_key_bar_and_report_names_come_from_the_hindi_file() {
    let hi = Catalog::builtin().unwrap().tr("hi");
    let en = Catalog::builtin().unwrap().tr("en");
    let screen = AskScreen::default();
    assert_eq!(screen.title(&hi), hi.t("ask.title"));
    assert_ne!(screen.title(&hi), "Ask your books");
    assert_eq!(screen.title(&en), "Ask your books");
    let labels: Vec<String> = ask::key_hints_in(&hi)
        .iter()
        .map(|k| k.label.clone())
        .collect();
    assert_eq!(labels, vec![hi.t("ask.title")]);
    for report in AskReport::ALL {
        let text = report.label_in(&hi);
        assert_ne!(text, report.label(), "{report:?} is still English");
        assert!(!text.contains('\u{27E6}'), "missing key in {text}");
    }
}
