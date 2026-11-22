//! Card U2: command palette. Expected orders are derived by hand from the scoring rules written
//! in `palette.rs` (exact 1000, prefix 800, word prefix 600, substring 400, subsequence
//! 300 minus 10 per skipped character; an alias scores 50 less; detail text scores half) and from
//! spec section 7 ("Command palette searches reports, masters (name/alias/GSTIN), vouchers,
//! settings and AI intents"). The arithmetic is written next to each case; nothing is copied from
//! this crate's output. No test involves a money figure.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_app::keymap::{Action, classic};
use ledgerly_app::palette::{
    Palette, PaletteChoice, PaletteEntry, PaletteMode, PaletteTarget, command_entries, rank,
};

fn report(id: &str, title: &str) -> PaletteEntry {
    PaletteEntry::new(PaletteTarget::Report(id.to_owned()), title)
}

fn reports() -> Vec<PaletteEntry> {
    vec![
        report("tb", "Trial Balance"),
        report("bs", "Balance Sheet"),
        report("db", "Day Book"),
    ]
}

fn titles<'a>(entries: &'a [PaletteEntry], order: &[usize]) -> Vec<&'a str> {
    order
        .iter()
        .map(|i| entries.get(*i).unwrap().title.as_str())
        .collect()
}

fn key_frame(keys: &[Key]) -> RawInput {
    RawInput {
        events: keys
            .iter()
            .map(|key| Event::Key {
                key: *key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            })
            .collect(),
        ..RawInput::default()
    }
}

fn frame(palette: &mut Palette, input: RawInput) -> Option<PaletteChoice> {
    let ctx = egui::Context::default();
    let mut got = None;
    let out = ctx.run_ui(input, |ui| {
        got = palette.ui(ui);
    });
    out.drop_without_applying_deltas();
    got
}

// ---- ranking -----------------------------------------------------------------------------------

#[test]
fn an_empty_query_keeps_every_entry_in_the_given_order() {
    // provenance: derived — rank() docs: "An empty query keeps every entry", no recents.
    let entries = reports();
    assert_eq!(rank("", &entries, &[]), [0, 1, 2]);
    assert_eq!(rank("   ", &entries, &[]), [0, 1, 2]);
}

#[test]
fn a_prefix_beats_a_word_prefix() {
    // provenance: derived — query "balance": "Balance Sheet" starts with it (prefix, 800);
    // "Trial Balance" has it at the start of its second word (word prefix, 600); "Day Book" has
    // neither a match nor a subsequence (no 'n' after 'a','l','a' in "day book"). So the order
    // is Balance Sheet (800), Trial Balance (600).
    let entries = reports();
    let order = rank("balance", &entries, &[]);
    assert_eq!(titles(&entries, &order), ["Balance Sheet", "Trial Balance"]);
}

#[test]
fn an_exact_title_beats_a_prefix() {
    // provenance: derived — "cash" is exact for "Cash" (1000) and a prefix of "Cash Flow" (800).
    let entries = vec![report("cf", "Cash Flow"), report("c", "Cash")];
    let order = rank("cash", &entries, &[]);
    assert_eq!(titles(&entries, &order), ["Cash", "Cash Flow"]);
}

#[test]
fn matching_ignores_case() {
    // provenance: derived — both sides are lower-cased before scoring.
    let entries = reports();
    let order = rank("DAY", &entries, &[]);
    assert_eq!(titles(&entries, &order), ["Day Book"]);
}

#[test]
fn letters_in_order_find_a_title() {
    // provenance: derived — "trbl" is not a prefix, word prefix or substring of any title. In
    // "trial balance" (t0 r1 i2 a3 l4 space5 b6 a7 l8 n9 c10 e11) the letters match at 0, 1, 6, 8:
    // span 0..=8 is 9 characters, 4 matched, 5 skipped, score 300 - 50 = 250. "balance sheet"
    // has a 't' only at the end (after index 12) and no 'r', "day book" has no 't'.
    let entries = reports();
    let order = rank("trbl", &entries, &[]);
    assert_eq!(titles(&entries, &order), ["Trial Balance"]);
}

#[test]
fn every_word_typed_must_match() {
    // provenance: derived — "sharma prop": "sharma" is a prefix (800) and "prop" starts the
    // second word (600) of "Sharma Properties" (total 1400). "Sharma Traders" matches "sharma"
    // but its text has no letter 'p' at all, so "prop" matches nothing and the entry is out.
    let entries = vec![
        PaletteEntry::new(PaletteTarget::Master("m1".to_owned()), "Sharma Properties"),
        PaletteEntry::new(PaletteTarget::Master("m2".to_owned()), "Sharma Traders"),
    ];
    let order = rank("sharma prop", &entries, &[]);
    assert_eq!(titles(&entries, &order), ["Sharma Properties"]);
}

#[test]
fn an_alias_finds_an_entry() {
    // provenance: derived — spec section 7 "masters (name/alias/GSTIN)": the alias "landlord" is
    // exact (1000) minus 50 = 950 for the first entry; the second has no such word.
    let entries = vec![
        PaletteEntry::new(PaletteTarget::Master("m1".to_owned()), "Sharma Properties")
            .with_aliases(["landlord"]),
        PaletteEntry::new(PaletteTarget::Master("m2".to_owned()), "Sharma Traders"),
    ];
    let order = rank("landlord", &entries, &[]);
    assert_eq!(titles(&entries, &order), ["Sharma Properties"]);
}

#[test]
fn a_gstin_style_alias_is_found_by_prefix() {
    // provenance: derived — a fictional identifier; prefix of an alias: 800 - 50 = 750.
    let entries = vec![
        PaletteEntry::new(PaletteTarget::Master("m1".to_owned()), "Sharma Properties")
            .with_aliases(["27ZZZZZ9999Z1Z0"]),
    ];
    assert_eq!(rank("27zzzzz", &entries, &[]), [0]);
    assert_eq!(rank("29zzzzz", &entries, &[]), [] as [usize; 0]);
}

#[test]
fn detail_text_counts_for_half() {
    // provenance: derived — "cash": title "Cash" is exact (1000). "Petty" has the detail
    // "Cash in hand", a prefix (800), halved to 400. Both match; 1000 comes first.
    let entries = vec![
        PaletteEntry::new(PaletteTarget::Master("p".to_owned()), "Petty")
            .with_detail("Cash in hand"),
        report("c", "Cash"),
    ];
    let order = rank("cash", &entries, &[]);
    assert_eq!(titles(&entries, &order), ["Cash", "Petty"]);
}

#[test]
fn equal_scores_keep_the_given_order_and_recents_win_ties() {
    // provenance: derived — "sales" is a prefix (800) of both titles; the tie goes to the recent
    // one, then to the given order.
    let entries = vec![
        report("reg", "Sales Register"),
        report("ret", "Sales Return"),
    ];
    let tie = rank("sales", &entries, &[]);
    assert_eq!(titles(&entries, &tie), ["Sales Register", "Sales Return"]);
    let recents = [PaletteTarget::Report("ret".to_owned())];
    let order = rank("sales", &entries, &recents);
    assert_eq!(titles(&entries, &order), ["Sales Return", "Sales Register"]);
}

#[test]
fn with_an_empty_query_recents_come_first_most_recent_first() {
    // provenance: derived — rank() docs.
    let entries = reports();
    let recents = [
        PaletteTarget::Report("db".to_owned()),
        PaletteTarget::Report("bs".to_owned()),
    ];
    assert_eq!(rank("", &entries, &recents), [2, 1, 0]);
}

#[test]
fn nothing_matches_gibberish() {
    // provenance: derived — no entry contains 'q', 'z', 'x' in order.
    assert_eq!(rank("qzx", &reports(), &[]), [] as [usize; 0]);
}

// ---- state -------------------------------------------------------------------------------------

fn open_palette(mode: PaletteMode) -> Palette {
    let mut p = Palette::new();
    p.set_entries(reports());
    p.open(mode);
    p
}

#[test]
fn it_starts_closed_and_opens_with_a_mode() {
    // provenance: derived — `is_open` and `mode` document it.
    let mut p = Palette::new();
    assert!(!p.is_open());
    assert_eq!(p.mode(), None);
    p.open(PaletteMode::OnTop);
    assert!(p.is_open());
    assert_eq!(p.mode(), Some(PaletteMode::OnTop));
    p.close();
    assert!(!p.is_open());
}

#[test]
fn the_two_palette_keys_open_the_two_modes() {
    // provenance: derived — docs/research/02-ux-keyboard.md section 3.1: Alt+G stacks on the
    // current screen, Ctrl+G replaces it.
    let mut p = Palette::new();
    assert!(p.open_for(Action::OpenPalette));
    assert_eq!(p.mode(), Some(PaletteMode::OnTop));
    assert!(p.open_for(Action::SwitchPalette));
    assert_eq!(p.mode(), Some(PaletteMode::Replace));
    p.close();
    assert!(!p.open_for(Action::Save));
    assert!(!p.is_open());
}

#[test]
fn accepting_returns_the_choice_closes_and_remembers_it() {
    // provenance: derived — accept() docs.
    let mut p = open_palette(PaletteMode::Replace);
    p.set_query("day");
    let choice = p.accept().unwrap();
    assert_eq!(choice.target, PaletteTarget::Report("db".to_owned()));
    assert_eq!(choice.title, "Day Book");
    assert_eq!(choice.mode, PaletteMode::Replace);
    assert!(!p.is_open());
    assert_eq!(p.recents(), [PaletteTarget::Report("db".to_owned())]);
    // The next time it opens with no query, the recent choice leads.
    p.open(PaletteMode::OnTop);
    assert_eq!(p.results().first().unwrap().title, "Day Book");
}

#[test]
fn accepting_with_no_match_or_when_closed_returns_nothing() {
    // provenance: derived — accept() docs: `None` when closed or when no row matches; a failed
    // accept leaves the palette open.
    let mut p = open_palette(PaletteMode::OnTop);
    p.set_query("qzx");
    assert!(p.accept().is_none());
    assert!(p.is_open());
    p.close();
    assert!(p.accept().is_none());
}

#[test]
fn the_selection_moves_and_stops_at_the_ends() {
    // provenance: derived — three rows (indices 0..=2); move_selection docs.
    // spec change 2026-11-23: the empty query now lists the prefix rows, not the entries
    // (index.html:1115-1116), so the test types "a": every title contains it (400 each, `rank`
    // docs), so the given order is kept.
    let mut p = open_palette(PaletteMode::OnTop);
    p.set_query("a");
    assert_eq!(p.selected(), 0);
    p.move_selection(1);
    assert_eq!(p.selected(), 1);
    p.move_selection(5);
    assert_eq!(p.selected(), 2);
    p.move_selection(-9);
    assert_eq!(p.selected(), 0);
    p.move_selection(1);
    let choice = p.accept().unwrap();
    assert_eq!(choice.title, "Balance Sheet");
}

#[test]
fn changing_the_query_selects_the_first_row() {
    // provenance: derived — set_query docs.
    let mut p = open_palette(PaletteMode::OnTop);
    p.move_selection(2);
    p.set_query("b");
    assert_eq!(p.selected(), 0);
}

#[test]
fn only_the_last_eight_recents_are_kept() {
    // provenance: derived — the palette remembers 8 recent choices.
    let mut p = Palette::new();
    let entries: Vec<PaletteEntry> = (0..10)
        .map(|n| report(&format!("r{n}"), &format!("Report {n}")))
        .collect();
    p.set_entries(entries);
    for n in 0..10 {
        p.open(PaletteMode::OnTop);
        p.set_query(format!("report {n}"));
        assert!(p.accept().is_some());
    }
    assert_eq!(p.recents().len(), 8);
    assert_eq!(
        p.recents().first(),
        Some(&PaletteTarget::Report("r9".to_owned()))
    );
    assert_eq!(
        p.recents().last(),
        Some(&PaletteTarget::Report("r2".to_owned()))
    );
}

#[test]
fn choosing_the_same_entry_twice_keeps_one_recent() {
    // provenance: derived — a recent list has no duplicates.
    let mut p = open_palette(PaletteMode::OnTop);
    for _ in 0..2 {
        p.open(PaletteMode::OnTop);
        p.set_query("day");
        assert!(p.accept().is_some());
    }
    assert_eq!(p.recents().len(), 1);
}

// ---- commands from the keymap -------------------------------------------------------------------

#[test]
fn command_entries_show_the_keymap_shortcut() {
    // provenance: derived — spec section 7: Ctrl+A is the classic save key; entries come from
    // `Action::ALL`, one each.
    let km = classic();
    let entries = command_entries(&km);
    assert_eq!(entries.len(), Action::ALL.len());
    let mut p = Palette::new();
    p.set_entries(entries);
    p.open(PaletteMode::OnTop);
    p.set_query("save");
    let top = *p.results().first().unwrap();
    assert_eq!(top.target, PaletteTarget::Command(Action::Save));
    assert_eq!(top.detail, "Ctrl+A");
}

#[test]
fn a_command_is_found_by_its_id() {
    // provenance: derived — each command carries its id as an alias.
    let mut p = Palette::new();
    p.set_entries(command_entries(&classic()));
    p.open(PaletteMode::OnTop);
    p.set_query("new-contra");
    let top = *p.results().first().unwrap();
    assert_eq!(top.target, PaletteTarget::Command(Action::NewContra));
}

// ---- drawing and keys ---------------------------------------------------------------------------

#[test]
fn a_closed_palette_draws_nothing_and_chooses_nothing() {
    // provenance: derived — ui() docs.
    let mut p = Palette::new();
    p.set_entries(reports());
    assert!(frame(&mut p, key_frame(&[Key::Enter])).is_none());
}

#[test]
fn enter_chooses_the_selected_row() {
    // provenance: derived — ui() docs: "Enter chooses"; the first row of the query "a" is the
    // first entry (every title contains "a", 400 each, given order kept).
    // spec change 2026-11-23: the empty query now lists the prefix rows (index.html:1115-1116),
    // so the test types "a" instead of relying on the empty query.
    let mut p = open_palette(PaletteMode::OnTop);
    p.set_query("a");
    assert!(frame(&mut p, RawInput::default()).is_none());
    let choice = frame(&mut p, key_frame(&[Key::Enter])).unwrap();
    assert_eq!(choice.title, "Trial Balance");
    assert!(!p.is_open());
}

#[test]
fn down_then_enter_chooses_the_second_row() {
    // provenance: derived — ui() docs: "Up and Down move".
    // spec change 2026-11-23: typed "a" (all three rows, given order) since the empty query now
    // lists the prefix rows (index.html:1115-1116).
    let mut p = open_palette(PaletteMode::Replace);
    p.set_query("a");
    let choice = frame(&mut p, key_frame(&[Key::ArrowDown, Key::Enter])).unwrap();
    assert_eq!(choice.title, "Balance Sheet");
    assert_eq!(choice.mode, PaletteMode::Replace);
}

#[test]
fn escape_closes_without_choosing() {
    // provenance: derived — ui() docs: "Esc closes".
    let mut p = open_palette(PaletteMode::OnTop);
    assert!(frame(&mut p, key_frame(&[Key::Escape])).is_none());
    assert!(!p.is_open());
    assert_eq!(p.recents(), []);
}
