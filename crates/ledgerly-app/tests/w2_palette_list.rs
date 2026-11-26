//! What the palette lists, to the mock's `palResults` (`design/mock-v2/index.html:1113-1129`):
//! the empty query's Recent and "Type a prefix" rows, the rows of a query with no match, the
//! highlighted part of a title, and the words under a report row.

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_app::AppCtx;
use ledgerly_app::keymap::classic;
use ledgerly_app::palette::{
    Palette, PaletteChoice, PaletteEntry, PaletteMode, PaletteTarget, match_span,
};
use ledgerly_app::shell::navigate::{new_ledger_key, new_ledger_name, open_target};
use ledgerly_app::shell::palette_rows;

fn report(id: &str, title: &str) -> PaletteEntry {
    PaletteEntry::new(PaletteTarget::Report(id.to_owned()), title)
}

fn titles(palette: &Palette) -> Vec<String> {
    palette.results().iter().map(|e| e.title.clone()).collect()
}

fn fill(text: &str) -> PaletteTarget {
    PaletteTarget::Fill(text.to_owned())
}

fn opened(entries: Vec<PaletteEntry>) -> Palette {
    let mut p = Palette::new();
    p.set_entries(entries);
    p.open(PaletteMode::OnTop);
    p
}

/// Draws the palette for one frame with `events` and returns the choice it made.
fn frame(palette: &mut Palette, ctx: &egui::Context, events: Vec<Event>) -> Option<PaletteChoice> {
    let mut got = None;
    let input = RawInput {
        events,
        ..RawInput::default()
    };
    let out = ctx.run_ui(input, |ui| got = palette.ui(ui));
    out.drop_without_applying_deltas();
    got
}

fn press(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

/// The mock's "Type a prefix" rows, title and the words under it (index.html:1116).
const PREFIX_ROWS: [(&str, &str); 5] = [
    ("# voucher number", "e.g. #0412"),
    ("@ party or ledger", "name, alias or GSTIN"),
    ("> command", "e.g. > export"),
    ("? ask your books", "answer is a report the engine runs"),
    (
        "+ entry in plain words",
        "drafts a voucher for you to review",
    ),
];

// ---- the empty query -------------------------------------------------------------------------

#[test]
fn the_empty_query_lists_the_prefix_rows_and_no_reports() {
    // Provenance: owner decision of 2026-11-26. Release 1.0.0 ships without the AI assistant, so
    // `release::AI_IN_THIS_RELEASE` is false. This test exercises the AI UI itself, so it turns the
    // switch on for this test's thread only; the 1.0.0 screens are pinned in `s19b_ai_not_in_release.rs`.
    ledgerly_app::release::set_ai_enabled(true);
    // provenance: index.html:1115-1116: with nothing typed the mock lists "Recent" and then
    // "Type a prefix" with these five rows, each filling its prefix (`it[4]`), and nothing else.
    // A fresh palette has no recent choice (session only), so only the prefix rows show. Catches
    // the old empty query, which listed every report.
    let p = opened(vec![
        report("bs", "Balance Sheet"),
        report("db", "Day Book"),
    ]);
    let got: Vec<(String, String, PaletteTarget)> = p
        .results()
        .iter()
        .map(|e| (e.title.clone(), e.detail.clone(), e.target.clone()))
        .collect();
    let want: Vec<(String, String, PaletteTarget)> = PREFIX_ROWS
        .iter()
        .zip(["#", "@", ">", "?", "+"])
        .map(|((t, d), f)| ((*t).to_owned(), (*d).to_owned(), fill(f)))
        .collect();
    assert_eq!(got, want);
}

#[test]
fn a_recent_choice_leads_the_empty_query_then_the_prefix_rows() {
    // Provenance: owner decision of 2026-11-26. Release 1.0.0 ships without the AI assistant, so
    // `release::AI_IN_THIS_RELEASE` is false. This test exercises the AI UI itself, so it turns the
    // switch on for this test's thread only; the 1.0.0 screens are pinned in `s19b_ai_not_in_release.rs`.
    ledgerly_app::release::set_ai_enabled(true);
    // provenance: index.html:1115: "Recent" comes before "Type a prefix". Catches recents
    // dropped from the empty query, or shown after the prefix rows.
    let mut p = opened(vec![
        report("bs", "Balance Sheet"),
        report("db", "Day Book"),
    ]);
    p.set_query("day");
    assert!(p.accept().is_some());
    p.open(PaletteMode::OnTop);
    let mut want = vec!["Day Book".to_owned()];
    want.extend(PREFIX_ROWS.iter().map(|(t, _)| (*t).to_owned()));
    assert_eq!(titles(&p), want);
}

#[test]
fn choosing_a_prefix_row_fills_the_query_and_keeps_the_palette_open() {
    // provenance: index.html:1150 (`open`): a row with a fill puts it in the query box and stays
    // in the palette. Shift+Enter does the same. A fill is never returned as a choice, and it is
    // not remembered. Catches a prefix row that closes the palette or opens something.
    let mut p = opened(vec![report("bs", "Balance Sheet")]);
    assert_eq!(p.accept(), None);
    assert!(p.is_open());
    assert_eq!(p.query(), "#");
    assert_eq!(p.recents(), []);
}

#[test]
fn typing_after_a_filled_prefix_goes_after_it() {
    // provenance: derived from the same rule: after Enter on "# voucher number" the cursor is at
    // the end of "#", so typing 0412 gives "#0412". Catches the cursor left before the prefix.
    let ctx = egui::Context::default();
    let mut p = opened(vec![report("bs", "Balance Sheet")]);
    frame(&mut p, &ctx, Vec::new());
    assert_eq!(frame(&mut p, &ctx, vec![press(Key::Enter)]), None);
    frame(&mut p, &ctx, Vec::new());
    frame(&mut p, &ctx, vec![Event::Text("0412".to_owned())]);
    assert_eq!(p.query(), "#0412");
}

// ---- a query with no match -------------------------------------------------------------------

#[test]
fn no_match_rows_wait_for_the_search_of_that_text() {
    // provenance: derived. The API search for the typed text may still find a ledger; until it
    // has answered, offering to create one would open a form for a party that exists. So nothing
    // is listed and Enter chooses nothing while the search is out. Catches a quick Enter that
    // creates a duplicate ledger.
    let mut p = opened(vec![report("bs", "Balance Sheet")]);
    p.set_query("zzqx");
    assert_eq!(titles(&p), [] as [String; 0]);
    assert_eq!(p.accept(), None);
    assert!(p.is_open());
}

#[test]
fn a_settled_search_with_no_match_offers_to_create_the_ledger_or_search_vouchers() {
    // provenance: index.html:1129: with no match the mock lists 'Create ledger "<q>"' (opens the
    // ledger form with this name) and 'Search vouchers for "<q>"' (fills `#<q>`), the first one
    // selected. The mock's third row, "Ask", is dropped when AI is off (`tierOff`); the
    // assistant is not available here, so it is not listed. Our voucher search matches number,
    // party, amount or date (`MatchedOn`), and its row says so. Catches the old no-match state,
    // which listed nothing and chose nothing on Enter.
    let mut p = opened(vec![report("bs", "Balance Sheet")]);
    p.set_query("zzqx");
    p.search_done("zzqx");
    let got: Vec<(String, String, PaletteTarget)> = p
        .results()
        .iter()
        .map(|e| (e.title.clone(), e.detail.clone(), e.target.clone()))
        .collect();
    assert_eq!(
        got,
        [
            (
                "Create ledger \"zzqx\"".to_owned(),
                "Opens the ledger form with this name".to_owned(),
                PaletteTarget::Master(new_ledger_key("zzqx")),
            ),
            (
                "Search vouchers for \"zzqx\"".to_owned(),
                "Number, party, amount or date".to_owned(),
                fill("#zzqx"),
            ),
        ]
    );
    assert_eq!(p.selected(), 0);
    let choice = p.accept().expect("Enter creates the ledger");
    assert_eq!(choice.target, PaletteTarget::Master(new_ledger_key("zzqx")));
    assert_eq!(p.recents(), [], "a create is not offered again as recent");
}

#[test]
fn the_search_vouchers_row_fills_the_voucher_prefix() {
    // provenance: index.html:1129, the second no-match row's fill `'#'+t`.
    let mut p = opened(Vec::new());
    p.set_query("zzqx");
    p.search_done("zzqx");
    p.move_selection(1);
    assert_eq!(p.accept(), None);
    assert_eq!(p.query(), "#zzqx");
}

#[test]
fn a_new_ledger_key_carries_any_name_unchanged() {
    // provenance: derived. The name typed in the palette reaches the ledger form as typed: a
    // colon, quotes and Devanagari survive the key. Catches a name cut at the first colon.
    for name in ["zzqx", "A: B \"C\"", "\u{0930}\u{093E}\u{092E} Traders"] {
        assert_eq!(new_ledger_name(&new_ledger_key(name)), Some(name));
    }
    assert_eq!(new_ledger_name("ledger:4"), None);
}

#[test]
fn a_fill_is_not_something_the_host_opens() {
    // provenance: derived. The palette handles a fill itself; were one to reach the host it is
    // refused with a typed error, not opened as a screen.
    let ctx = AppCtx {
        tr: ledgerly_app::i18n::Tr::default(),
        books: None,
        api: None,
        lock_date: None,
        ai_tier: None,
    };
    assert!(open_target(&fill("#"), &ctx).is_err());
}

// ---- the highlight and the words under a row -------------------------------------------------

#[test]
fn the_highlight_is_the_first_place_the_query_appears_in_the_title() {
    // provenance: the mock's `hl` (index.html:1112): the first case-insensitive occurrence of the
    // query (prefix removed, trimmed) is marked; a fuzzy match marks nothing. Byte offsets for
    // "राम Traders": three 3-byte letters and a space, so "Traders" spans 10..17.
    assert_eq!(match_span("Balance Sheet", "bal"), Some((0, 3)));
    assert_eq!(match_span("Bank reconciliation", "RECON"), Some((5, 10)));
    assert_eq!(
        match_span("\u{0930}\u{093E}\u{092E} Traders", "traders"),
        Some((10, 17))
    );
    assert_eq!(match_span("Balance", "bln"), None);
    assert_eq!(match_span("Balance", ""), None);
}

#[test]
fn report_and_screen_rows_say_report_or_screen() {
    // provenance: index.html:1126: every screen the palette finds has the words "Report or
    // screen" under its title. Catches "Report", "Screen" and "Banking" under such rows.
    for entry in palette_rows(&classic()) {
        if matches!(entry.target, PaletteTarget::Report(_)) {
            assert_eq!(entry.detail, "Report or screen", "{}", entry.title);
        }
    }
}
