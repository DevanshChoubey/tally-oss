//! The mock's other palette commands (`design/mock-v2/index.html:1110`, `PAL_CMDS`): the ones the
//! app can run are rows of the host's palette list, after the four voucher commands.
//!
//! Provenance. `derived` from the mock's list: Read a bill photo, Import bank statement, Import
//! 2B / IMS JSON, Export GSTR-1 JSON, Switch theme, Collapse side bar, Back up now. Two of them
//! have no feature behind them (a 2B reader, a backup folder), and the export needs a file chosen
//! on the GST screen, so only the four that route to something real are listed
//! (`design/PARITY-GAPS.md`, "Palette").

use ledgerly_app::keymap::classic;
use ledgerly_app::palette::{Palette, PaletteMode, PaletteTarget, RunCommand};
use ledgerly_app::shell::navigate::host_entries;

fn open_commands() -> Palette {
    let mut p = Palette::new();
    p.set_entries(host_entries(&classic()));
    p.open(PaletteMode::OnTop);
    p.set_query(">");
    p
}

#[test]
fn the_commands_scope_lists_the_runnable_mock_commands_after_the_voucher_commands() {
    // provenance: derived, the mock's order: New sales invoice, New purchase, New payment, New
    // receipt, then Read a bill photo, Import bank statement, then (not built) 2B import and the
    // GSTR-1 export, then Switch theme and Collapse side bar.
    let p = open_commands();
    let titles: Vec<&str> = p.results().iter().map(|e| e.title.as_str()).collect();
    let at = |t: &str| titles.iter().position(|x| *x == t);
    let photo = at("Read a bill photo");
    let bank = at("Import bank statement");
    let theme = at("Switch theme");
    let rail = at("Collapse side bar");
    assert!(
        photo.is_some() && bank.is_some() && theme.is_some() && rail.is_some(),
        "{titles:?}"
    );
    assert!(photo < bank && bank < theme && theme < rail, "{titles:?}");
    let receipt = at("New receipt");
    assert!(receipt < photo, "{titles:?}");
}

#[test]
fn choosing_a_run_command_returns_it() {
    // provenance: derived, the palette returns the chosen row's target unchanged.
    let mut p = Palette::new();
    p.set_entries(host_entries(&classic()));
    p.open(PaletteMode::OnTop);
    p.set_query("switch theme");
    let choice = p.accept().unwrap();
    assert_eq!(choice.target, PaletteTarget::Run(RunCommand::SwitchTheme));
}

#[test]
fn the_side_bar_command_shows_the_shortcut_the_shell_binds() {
    // provenance: derived, the shell folds the side bar on Ctrl+\ (mock: "Ctrl+\").
    let p = open_commands();
    let rail = p
        .results()
        .into_iter()
        .find(|e| e.target == PaletteTarget::Run(RunCommand::CollapseSideBar))
        .unwrap();
    assert_eq!(rail.detail, "Ctrl+\\");
}

#[test]
fn alt_c_creates_the_ledger_even_when_another_row_is_selected() {
    // provenance: derived from the mock (index.html:1129): the no-match row 'Create ledger "<q>"'
    // carries the Alt+C key. With the second row selected, Enter would not create the ledger, so
    // a choice of the create row after Alt+C can only come from the key.
    let mut p = Palette::new();
    p.set_entries(host_entries(&classic()));
    p.open(PaletteMode::OnTop);
    p.set_query("zzqx");
    p.search_done("zzqx");
    p.move_selection(1);
    let input = egui::RawInput {
        events: vec![egui::Event::Key {
            key: egui::Key::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::ALT,
        }],
        ..egui::RawInput::default()
    };
    let ctx = egui::Context::default();
    let mut got = None;
    let out = ctx.run_ui(input, |ui| {
        got = p.ui(ui);
    });
    out.drop_without_applying_deltas();
    let choice = got.expect("Alt+C chooses a row");
    assert_eq!(
        choice.target,
        PaletteTarget::Master(ledgerly_app::shell::navigate::new_ledger_key("zzqx"))
    );
}
