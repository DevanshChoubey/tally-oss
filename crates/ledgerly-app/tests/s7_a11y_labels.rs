#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "tests assert with expect"
)]

//! Card S7-A11Y: screen reader names and roles. The tree is read from egui's AccessKit output,
//! so each assertion checks what a screen reader would be told, by role and accessible name.
//!
//! Expected names come from the labels the mock draws (`design/mock-v2/index.html`, cited on each
//! test) or from the text passed in, so a name is never copied from this crate's own output.

use egui::accesskit::{Node, Role, TreeUpdate};
use egui::{Context, RawInput, Rect, Ui, Vec2};
use ledgerly_app::theme::{self, ThemeKind};
use ledgerly_app::ui::button::{self, ButtonKind};
use ledgerly_app::ui::chip::{self, Chip};
use ledgerly_app::ui::icon::Icon;
use ledgerly_app::ui::option_card::{OptionCard, option_grid};
use ledgerly_app::ui::switch::switch;
use ledgerly_app::ui::tabs;

/// Runs one frame of `body` with AccessKit enabled and returns the tree update it produced.
fn frame(ctx: &Context, body: impl FnMut(&mut Ui)) -> TreeUpdate {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(
            egui::Pos2::ZERO,
            Vec2::new(900.0, 700.0),
        )),
        ..RawInput::default()
    };
    let mut output = ctx.run_ui(input, body);
    let update = output
        .platform_output
        .accesskit_update
        .take()
        .expect("AccessKit is enabled");
    output.drop_without_applying_deltas();
    update
}

/// A light-themed context with AccessKit on and one warm-up frame.
fn context() -> Context {
    let ctx = Context::default();
    ctx.enable_accesskit();
    theme::apply(&ctx, ThemeKind::Light);
    frame(&ctx, |_| {});
    ctx
}

/// The node with `role` and accessible name `label`, if the tree has one.
fn find<'a>(update: &'a TreeUpdate, role: Role, label: &str) -> Option<&'a Node> {
    update
        .nodes
        .iter()
        .map(|(_, node)| node)
        .find(|node| node.role() == role && node.label() == Some(label))
}

#[test]
fn a_button_is_named_by_its_label_and_the_key_is_not_part_of_the_name() {
    // provenance: index.html:1059 `New sale` button with the F8 keycap; the name is the words.
    let ctx = context();
    let update = frame(&ctx, |ui| {
        button::button(ui, "New sale", Some("F8"), ButtonKind::Primary);
    });
    let node = find(&update, Role::Button, "New sale").expect("a button named New sale");
    assert!(!node.is_disabled());
}

#[test]
fn a_disabled_button_is_reported_disabled() {
    // provenance: index.html `.btn:disabled` (a disabled button is still named).
    let ctx = context();
    let update = frame(&ctx, |ui| {
        ui.add_enabled_ui(false, |ui| {
            button::button(ui, "Save", Some("Ctrl+A"), ButtonKind::Primary);
        });
    });
    let node = find(&update, Role::Button, "Save").expect("a button named Save");
    assert!(node.is_disabled());
}

#[test]
fn an_icon_button_is_named_by_the_same_text_as_its_hover_text() {
    // provenance: index.html:902 `.iconbtn` with `aria-label="Switch theme"`.
    let ctx = context();
    let update = frame(&ctx, |ui| {
        chip::icon_button(ui, Icon::Sun, "Theme: light (switch)");
    });
    assert!(find(&update, Role::Button, "Theme: light (switch)").is_some());
}

#[test]
fn a_chip_is_a_button_named_by_its_bold_part_and_its_words() {
    // provenance: index.html:900 `.chip.search` names its action in words.
    let ctx = context();
    let update = frame(&ctx, |ui| {
        chip::chip_parts(
            ui,
            Chip {
                bold: Some("Saptagiri"),
                text: "FY 2026-27",
                ..Chip::default()
            },
        );
    });
    assert!(find(&update, Role::Button, "Saptagiri, FY 2026-27").is_some());
}

#[test]
fn tabs_are_tab_roles_and_the_selected_one_is_reported_selected() {
    // provenance: index.html `.tabs button` with `GSTR-1` and `GSTR-3B` labels (SCREENS.gst).
    let ctx = context();
    let update = frame(&ctx, |ui| {
        tabs::tabs(ui, &["GSTR-1", "GSTR-3B"], 1);
    });
    let gstr3b = update
        .nodes
        .iter()
        .map(|(_, n)| n)
        .find(|n| n.role() == Role::Tab && n.label() == Some("GSTR-3B"))
        .expect("a tab named GSTR-3B");
    assert_eq!(gstr3b.toggled(), Some(egui::accesskit::Toggled::True));
    let gstr1 = find(&update, Role::Tab, "GSTR-1").expect("a tab named GSTR-1");
    assert_eq!(gstr1.toggled(), Some(egui::accesskit::Toggled::False));
}

#[test]
fn an_option_card_is_a_radio_named_by_its_title_and_text() {
    // provenance: index.html:319-323 `.opt-card` with `aria-checked`, drawn as a radio.
    let ctx = context();
    let update = frame(&ctx, |ui| {
        let cards = [
            OptionCard::new(Icon::Book, "Keep books here", "Stored on this computer")
                .selected(true),
        ];
        option_grid(ui, egui::Id::unique("a11y-cards"), 1, &cards);
    });
    let node = find(
        &update,
        Role::RadioButton,
        "Keep books here, Stored on this computer",
    )
    .expect("a radio named by title and text");
    assert_eq!(node.toggled(), Some(egui::accesskit::Toggled::True));
}

#[test]
fn a_switch_is_named_and_reports_its_state() {
    // provenance: index.html `.switch` with its row label (SCREENS.setup features).
    let ctx = context();
    let update = frame(&ctx, |ui| {
        switch(ui, true, egui::Sense::click(), "Use GST");
    });
    let node = find(&update, Role::Switch, "Use GST").expect("a switch named Use GST");
    assert_eq!(node.toggled(), Some(egui::accesskit::Toggled::True));
}
