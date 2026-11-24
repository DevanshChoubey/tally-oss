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

mod ai_settings_screen {
    use ledgerly_api::AiTier;
    use ledgerly_app::i18n::Catalog;
    use ledgerly_app::screens::ai_settings::{
        NEVER, OFF_NOTE, OPTION_CLOUD, OPTION_LOCAL, OPTION_OFF, UNSET_NOTE, key_hints,
        key_hints_in, never_in, off_note_in, option_in, unset_note_in,
    };

    #[test]
    fn ai_settings_options_and_notes_come_from_the_hindi_file() {
        let hi = Catalog::builtin().unwrap().tr("hi");
        let en = Catalog::builtin().unwrap().tr("en");
        // provenance: the English words are the mock's (index.html:1905-1907, 1911, 1916).
        assert_eq!(OPTION_OFF.0, "Off");
        assert_eq!(
            option_in(AiTier::Off, &en),
            (OPTION_OFF.0.to_owned(), OPTION_OFF.1.to_owned())
        );
        let (title, body) = option_in(AiTier::Local, &hi);
        assert_eq!(title, hi.t("ai.settings.option.local.title"));
        assert_ne!(title, OPTION_LOCAL.0);
        assert_ne!(body, OPTION_LOCAL.1);
        let (cloud_title, _) = option_in(AiTier::Cloud, &hi);
        assert_ne!(cloud_title, OPTION_CLOUD.0);
        assert_ne!(off_note_in(&hi), OFF_NOTE);
        assert_ne!(unset_note_in(&hi), UNSET_NOTE);
        assert_eq!(unset_note_in(&en), UNSET_NOTE);
        assert_eq!(off_note_in(&en), OFF_NOTE);
    }

    #[test]
    fn ai_settings_never_list_and_key_bar_come_from_the_hindi_file() {
        let hi = Catalog::builtin().unwrap().tr("hi");
        let en = Catalog::builtin().unwrap().tr("en");
        let hindi = never_in(&hi);
        assert_eq!(hindi.len(), NEVER.len());
        for (line, english) in hindi.iter().zip(NEVER) {
            assert_ne!(line, english, "never line is still English");
            assert!(!line.contains('\u{27E6}'), "missing key in {line}");
        }
        assert_eq!(never_in(&en).to_vec(), NEVER.map(str::to_owned).to_vec());
        let labels: Vec<String> = key_hints_in(true, &hi)
            .iter()
            .map(|k| k.label.clone())
            .collect();
        assert!(
            labels.contains(&hi.t("ai.settings.key.export")),
            "{labels:?}"
        );
        assert!(!labels.contains(&"Export log".to_owned()), "{labels:?}");
        assert_eq!(
            key_hints_in(true, &en)
                .iter()
                .map(|k| k.label.clone())
                .collect::<Vec<_>>(),
            key_hints(true)
                .iter()
                .map(|k| k.label.clone())
                .collect::<Vec<_>>()
        );
    }
}

mod review_screen {
    use ledgerly_api::{ReviewQueue, ReviewSource, UnlistedSource};
    use ledgerly_app::Screen;
    use ledgerly_app::i18n::Catalog;
    use ledgerly_app::screens::review::{
        EmptyView, ReviewScreen, empty_view_in, not_connected_in, source_label, source_label_in,
    };

    const SOURCES: [ReviewSource; 6] = [
        ReviewSource::BillDraft,
        ReviewSource::PlainWordsDraft,
        ReviewSource::BankSuggestion,
        ReviewSource::NewLedgerSuggestion,
        ReviewSource::DuplicateCheck,
        ReviewSource::Gstr2bExplanation,
    ];

    #[test]
    fn review_source_names_come_from_the_hindi_file() {
        let hi = Catalog::builtin().unwrap().tr("hi");
        let en = Catalog::builtin().unwrap().tr("en");
        for source in SOURCES {
            // provenance: the English names are the screen's own (review.rs `source_label`).
            assert_eq!(source_label_in(source, &en), source_label(source));
            assert_ne!(
                source_label_in(source, &hi),
                source_label(source),
                "{source:?}"
            );
            assert!(!source_label_in(source, &hi).contains('\u{27E6}'));
        }
    }

    #[test]
    fn review_title_key_bar_and_unlisted_lines_come_from_the_hindi_file() {
        let hi = Catalog::builtin().unwrap().tr("hi");
        let screen = ReviewScreen::default();
        assert_eq!(screen.title(&hi), hi.t("review.title"));
        assert_ne!(screen.title(&hi), "Review queue");
        let labels: Vec<String> = screen.keys(&hi).iter().map(|k| k.label.clone()).collect();
        assert!(labels.contains(&hi.t("review.key.back")), "{labels:?}");
        assert!(!labels.contains(&"Back".to_owned()), "{labels:?}");

        let queue = ReviewQueue {
            items: Vec::new(),
            unlisted: vec![UnlistedSource {
                source: ReviewSource::BillDraft,
                reason: "the engine's own reason".to_owned(),
            }],
        };
        let lines = not_connected_in(&queue, &hi);
        let label = hi.t("review.source.bill_draft");
        assert_eq!(
            lines,
            vec![hi.tf(
                "review.unlisted",
                &[
                    ("label", label.as_str()),
                    ("reason", "the engine's own reason")
                ]
            )]
        );
        assert!(!lines[0].starts_with("Bill photos"), "{lines:?}");
        assert_eq!(empty_view_in(&queue, &hi), EmptyView::NotConnected(lines));
    }
}

mod entry_screens_gate_notes {
    use ledgerly_app::i18n::Catalog;
    use ledgerly_app::screens::ai_gate::{self, Feature, Gate};
    use ledgerly_app::screens::{purchase_bill, quick_entry};

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
    fn quick_entry_gate_notes_come_from_the_hindi_file() {
        let hi = Catalog::builtin().unwrap().tr("hi");
        let en = Catalog::builtin().unwrap().tr("en");
        for gate in gates() {
            let english = ai_gate::reason(Feature::PlainWords, &gate);
            // The English file gives the same words as the module's English wrapper.
            assert_eq!(quick_entry::gate_line_in(&gate, &en), english);
            let text = quick_entry::gate_line_in(&gate, &hi);
            assert_ne!(text, english, "{gate:?} is still English");
            assert!(!text.unwrap().contains('\u{27E6}'), "missing key");
        }
    }

    #[test]
    fn purchase_bill_gate_notes_come_from_the_hindi_file() {
        let hi = Catalog::builtin().unwrap().tr("hi");
        let en = Catalog::builtin().unwrap().tr("en");
        for gate in gates() {
            let english = ai_gate::reason(Feature::BillReading, &gate);
            assert_eq!(purchase_bill::gate_line_in(&gate, &en), english);
            let text = purchase_bill::gate_line_in(&gate, &hi);
            assert_ne!(text, english, "{gate:?} is still English");
            assert!(!text.unwrap().contains('\u{27E6}'), "missing key");
        }
    }
}
