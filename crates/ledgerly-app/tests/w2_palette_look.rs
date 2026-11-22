//! The palette drawn to the mock (`design/mock-v2/index.html`, `SCREENS.palette` and the
//! `.scrim`/`.palette` rules at lines 273-286): where the panel sits, the icon each row shows,
//! and the footer's keys.

use egui::{Rect, pos2, vec2};
use ledgerly_app::keymap::Action;
use ledgerly_app::palette::{Palette, PaletteMode, PaletteTarget, palette_rect};
use ledgerly_app::shell::chrome::palette_key_bar;
use ledgerly_app::shell::navigate::{
    AI_SETTINGS_KEY, ASK_KEY, BANK_RECON_KEY, GST_KEY, HOME_KEY, KEYS_KEY, MASTERS_KEY,
    QUICK_ENTRY_KEY, REVIEW_KEY, SETUP_KEY, bank_key, row_icon,
};
use ledgerly_app::ui::icon::{self, Icon};
use ledgerly_core::LedgerId;

#[test]
fn the_panel_is_640_wide_centred_72_below_the_top() {
    // provenance: derived from `.scrim{padding-top:72px;justify-content:center}` and
    // `.palette{width:640px;max-width:calc(100vw - 32px)}` (index.html lines 273-274). In a
    // 1280-point window the panel spans x 320..960 from y 72; in a 500-point window it is
    // 500 - 32 = 468 wide, from x 16. Catches the panel sized by its contents, or hung from the
    // top bar's edge (48) as in the 2026-11-23 capture.
    let wide = palette_rect(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0)));
    assert!((wide.min.x - 320.0).abs() < 0.01, "{wide:?}");
    assert!((wide.width() - 640.0).abs() < 0.01, "{wide:?}");
    assert!((wide.min.y - 72.0).abs() < 0.01, "{wide:?}");
    let narrow = palette_rect(Rect::from_min_size(pos2(0.0, 0.0), vec2(500.0, 800.0)));
    assert!((narrow.width() - 468.0).abs() < 0.01, "{narrow:?}");
    assert!((narrow.min.x - 16.0).abs() < 0.01, "{narrow:?}");
}

#[test]
fn each_row_shows_the_icon_the_mock_gives_its_kind() {
    // provenance: the mock's `PAL_SCREENS` icons (index.html:1109) for the screens it lists, and
    // `palResults` (index.html:1113-1128) for the kinds: commands `sliders`, vouchers `file`,
    // ledgers `users`, ask `spark`, entry `chat`. Catches rows drawn without an icon, or with
    // the wrong one.
    let report = |k: &str| PaletteTarget::Report(k.to_owned());
    let cases = [
        (report(HOME_KEY), Icon::Home),
        (report("balance-sheet"), Icon::Scale),
        (report("day-book"), Icon::Book),
        (report(GST_KEY), Icon::Gst),
        (report(BANK_RECON_KEY), Icon::Bank),
        (report(&bank_key(LedgerId(3))), Icon::Bank),
        (report(MASTERS_KEY), Icon::Users),
        (report(REVIEW_KEY), Icon::Inbox),
        (report(KEYS_KEY), Icon::Kbd),
        (report(AI_SETTINGS_KEY), Icon::Shield),
        (report(SETUP_KEY), Icon::Building),
        (report(ASK_KEY), Icon::Spark),
        (report(QUICK_ENTRY_KEY), Icon::Chat),
        (PaletteTarget::Master("ledger:4".to_owned()), Icon::Users),
        (PaletteTarget::Voucher("voucher:9".to_owned()), Icon::File),
        (PaletteTarget::Command(Action::OpenPalette), Icon::Sliders),
        (PaletteTarget::AiIntent("ask".to_owned()), Icon::Spark),
        // index.html:1116, the "Type a prefix" rows, and :1129, the no-match rows ('plus' for
        // the ledger create, 'search' for the voucher search).
        (PaletteTarget::Fill("#".to_owned()), Icon::File),
        (PaletteTarget::Fill("@".to_owned()), Icon::Users),
        (PaletteTarget::Fill(">".to_owned()), Icon::Sliders),
        (PaletteTarget::Fill("?".to_owned()), Icon::Spark),
        (PaletteTarget::Fill("+".to_owned()), Icon::Chat),
        (PaletteTarget::Fill("#zzqx".to_owned()), Icon::Search),
        (
            PaletteTarget::Master(ledgerly_app::shell::navigate::new_ledger_key("zzqx")),
            Icon::Plus,
        ),
    ];
    for (target, want) in cases {
        assert_eq!(row_icon(&target), want, "{target:?}");
    }
}

#[test]
fn the_new_icons_parse_to_their_paths() {
    // provenance: derived by hand from the mock's `ICONS.file` ("M6 3h9l4 4v14H6z M14 3v5h5"):
    // the page outline starts (6,3) then (15,3), (19,7), (19,21), (6,21). Catches the new icons
    // left blank.
    let file = icon::parse(Icon::File.path());
    let outline = file.first().cloned().unwrap_or_default();
    let want = [
        pos2(6.0, 3.0),
        pos2(15.0, 3.0),
        pos2(19.0, 7.0),
        pos2(19.0, 21.0),
        pos2(6.0, 21.0),
    ];
    for (got, want) in outline.iter().zip(want) {
        assert!((*got - want).length() < 1e-3, "got {got:?}, want {want:?}");
    }
    for which in [Icon::File, Icon::Sliders, Icon::Plus] {
        let paths = icon::parse(which.path());
        assert!(
            !paths.is_empty() && paths.iter().all(|p| p.len() >= 2),
            "{which:?}"
        );
    }
}

fn opened(mode: PaletteMode) -> Palette {
    let mut p = Palette::new();
    p.open(mode);
    p
}

#[test]
fn the_footer_reads_like_the_mocks() {
    // provenance: index.html:1139, the `.pfoot` line: two keycaps "↑" "↓" then "move", "Enter"
    // "open on top", "Shift+Enter" "replace screen", "Esc" "close, back to where you were". A
    // palette opened to replace swaps the two Enter labels (the mode rule of
    // `the_footer_keys_follow_the_mode_they_were_opened_in`). Catches "Up / Down" in one cap and
    // the shorter labels of the key bar.
    let on_top = opened(PaletteMode::OnTop).footer_hints();
    assert_eq!(
        on_top,
        vec![
            (vec!["\u{2191}", "\u{2193}"], "move"),
            (vec!["Enter"], "open on top"),
            (vec!["Shift+Enter"], "replace screen"),
            (vec!["Esc"], "close, back to where you were"),
        ]
    );
    let replace = opened(PaletteMode::Replace).footer_hints();
    let enter: Vec<&str> = replace.iter().skip(1).take(2).map(|h| h.1).collect();
    assert_eq!(enter, ["replace screen", "open on top"]);
}

#[test]
fn the_key_bar_shows_the_palettes_keys_while_it_is_open() {
    // provenance: index.html:1159, the palette's `keys()`: the key bar names "Go to" (the hint
    // with no key, drawn bold as the bar's context) and then "↑ ↓" Move, Enter Open on top,
    // Shift+Enter Replace, Esc Close, with no "Esc Back" of the screen under it. Catches the
    // 2026-11-23 capture, where the bar kept the Home screen's keys under the open palette.
    let (context, hints) = palette_key_bar(&opened(PaletteMode::OnTop).key_hints());
    assert_eq!(context, "Go to");
    let keys: Vec<(&str, &str)> = hints
        .iter()
        .map(|(k, l)| (k.as_str(), l.as_str()))
        .collect();
    assert_eq!(
        keys,
        [
            ("\u{2191} \u{2193}", "Move"),
            ("Enter", "Open on top"),
            ("Shift+Enter", "Replace"),
            ("Esc", "Close"),
        ]
    );
}

/// A root screen that draws nothing.
struct Leaf;

impl ledgerly_app::Screen for Leaf {
    fn title(&self) -> String {
        "Home".to_owned()
    }
    fn ui(
        &mut self,
        _ui: &mut egui::Ui,
        _ctx: &mut ledgerly_app::AppCtx,
    ) -> ledgerly_app::ScreenAction {
        ledgerly_app::ScreenAction::Stay
    }
}

/// Runs one 1280 x 800 frame of `shell` with `events`.
fn shell_frame(
    ctx: &egui::Context,
    shell: &mut ledgerly_app::shell::Shell,
    events: Vec<egui::Event>,
) {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
        events,
        ..egui::RawInput::default()
    };
    let out = ctx.run_ui(input, |ui| shell.show(ui));
    out.drop_without_applying_deltas();
}

/// Opens the palette in a fresh shell, clicks at `at`, and says whether it is still open.
fn open_after_click(at: egui::Pos2) -> bool {
    use egui::{Event, Modifiers, PointerButton};
    let ctx = egui::Context::default();
    let mut shell = ledgerly_app::shell::Shell::new(Box::new(Leaf));
    shell.palette_mut().open(PaletteMode::OnTop);
    shell_frame(&ctx, &mut shell, Vec::new());
    shell_frame(&ctx, &mut shell, Vec::new());
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    shell_frame(
        &ctx,
        &mut shell,
        vec![Event::PointerMoved(at), button(true)],
    );
    shell_frame(&ctx, &mut shell, vec![button(false)]);
    shell.palette().is_open()
}

#[test]
fn a_click_on_the_scrim_closes_the_palette_and_a_click_in_it_does_not() {
    // provenance: the mock's scrim handler (index.html:1143): a click whose target is the scrim
    // itself goes back, and the panel stops its clicks from reaching the scrim. At 1280 x 800 the
    // panel spans x 320..960 from y 72 (`the_panel_is_640_wide_centred_72_below_the_top`), so
    // (40, 700) is on the scrim, and (900, 140) is the empty right end of the prefix legend
    // inside the panel. Catches a scrim that does not close, or one that eats the panel's clicks.
    assert!(
        !open_after_click(pos2(40.0, 700.0)),
        "the scrim click left it open"
    );
    assert!(
        open_after_click(pos2(900.0, 140.0)),
        "a click in the panel closed it"
    );
}
