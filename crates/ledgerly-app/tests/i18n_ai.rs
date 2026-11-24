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

mod assist_panel {
    use ledgerly_app::i18n::Catalog;
    use ledgerly_app::screens::assist::{
        AssistScreen, DraftView, EngineCheck, OFF_TEXT, ProposalView, ProposedField, Tier,
    };

    fn field(label: &str, value: &str) -> ProposedField {
        ProposedField {
            label: label.to_owned(),
            value: Some(value.to_owned()),
            confidence_permille: 925,
            source: "Typed text".to_owned(),
        }
    }

    fn draft(check: EngineCheck, create: Vec<String>) -> ProposalView {
        ProposalView::Voucher(DraftView {
            kind: "Payment".to_owned(),
            fields: vec![field("Party", "Acme")],
            lines: Vec::new(),
            create_ledgers: create,
            check,
        })
    }

    #[test]
    fn assist_tier_names_come_from_the_hindi_file() {
        let hi = Catalog::builtin().unwrap().tr("hi");
        let en = Catalog::builtin().unwrap().tr("en");
        // provenance: the English words are the panel's own (assist.rs `Tier::describe`).
        assert_eq!(Tier::Local.describe(), "Assistant: local model");
        assert_eq!(Tier::Local.describe_in(&en), "Assistant: local model");
        assert_eq!(Tier::Local.describe_in(&hi), hi.t("assist.tier.local"));
        assert_ne!(Tier::Local.describe_in(&hi), Tier::Local.describe());
    }

    #[test]
    fn assist_off_message_comes_from_the_hindi_file() {
        let hi = Catalog::builtin().unwrap().tr("hi");
        let mut screen = AssistScreen::new(Tier::Off);
        screen.set_input("Payment to Acme 500");
        screen.submit_input_in(&hi);
        // provenance: OFF_TEXT is the panel's English sentence for the off tier.
        assert_eq!(screen.error_text(), Some(hi.t("assist.off").as_str()));
        assert_ne!(screen.error_text(), Some(OFF_TEXT));
    }

    #[test]
    fn assist_accept_reasons_come_from_the_hindi_file() {
        let hi = Catalog::builtin().unwrap().tr("hi");
        let en = Catalog::builtin().unwrap().tr("en");
        let mut screen = AssistScreen::new(Tier::Local);
        assert_eq!(
            screen.accept_blocked_reason_in(&hi).as_deref(),
            Some(hi.t("assist.nothing_has_been_proposed").as_str())
        );
        assert_eq!(
            screen.accept_blocked_reason_in(&en).as_deref(),
            Some("Nothing has been proposed.")
        );

        screen.set_proposal(draft(EngineCheck::NotChecked, Vec::new()));
        let noun = hi.t("assist.noun.draft");
        assert_eq!(
            screen.accept_blocked_reason_in(&hi),
            Some(hi.tf("assist.check.not_checked", &[("noun", noun.as_str())]))
        );
        assert_ne!(
            screen.accept_blocked_reason_in(&hi),
            screen.accept_blocked_reason()
        );

        screen.set_proposal(draft(
            EngineCheck::Ok {
                figures: Vec::new(),
            },
            vec!["Acme Traders".to_owned()],
        ));
        assert_eq!(
            screen.accept_blocked_reason_in(&hi),
            Some(hi.tf("assist.create_ledger_first", &[("name", "Acme Traders")]))
        );
    }
}
