//! Wave 2 card S-C: the command palette's prefixes, legend, grouping, recents, footer keys, the
//! inert assistant rows, the no-results state and the API search's prefix rules.
//!
//! Provenance: each expected value names the mock line it comes from (`design/mock-v2/index.html`,
//! `DESIGN.md`), or is worked out by hand in the test. Nothing is copied from this crate's output.
//! No test involves a money figure.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use egui::{Event, Key, Modifiers, RawInput};
use ledgerly_api::{MatchedOn, SearchHit};
use ledgerly_app::keymap::{Action, classic};
use ledgerly_app::palette::{
    LEGEND, PLACEHOLDER, Palette, PaletteChoice, PaletteEntry, PaletteMode, PaletteScope,
    PaletteSearch, PaletteTarget, command_entries, entries_from_hits,
};
use ledgerly_app::shell::palette_rows;
use ledgerly_core::{LedgerId, VoucherId};
use time::macros::date;

fn report(id: &str, title: &str) -> PaletteEntry {
    PaletteEntry::new(PaletteTarget::Report(id.to_owned()), title)
}

fn master(id: &str, title: &str) -> PaletteEntry {
    PaletteEntry::new(PaletteTarget::Master(id.to_owned()), title)
}

fn voucher(id: &str, title: &str) -> PaletteEntry {
    PaletteEntry::new(PaletteTarget::Voucher(id.to_owned()), title)
}

fn command(action: Action, title: &str) -> PaletteEntry {
    PaletteEntry::new(PaletteTarget::Command(action), title)
}

fn titles(palette: &Palette) -> Vec<String> {
    palette.results().iter().map(|e| e.title.clone()).collect()
}

fn open_with(entries: Vec<PaletteEntry>, mode: PaletteMode) -> Palette {
    let mut p = Palette::new();
    p.set_entries(entries);
    p.open(mode);
    p
}

/// A palette over one "Balance Sheet" report with "bal" typed, so that row is the selected one.
fn open_balance_sheet(mode: PaletteMode) -> Palette {
    let mut p = open_with(vec![report("bs", "Balance Sheet")], mode);
    p.set_query("bal");
    p
}

/// Draws the palette for one frame with `input` and returns the choice it made.
fn frame(palette: &mut Palette, input: RawInput) -> Option<PaletteChoice> {
    let ctx = egui::Context::default();
    let mut got = None;
    let out = ctx.run_ui(input, |ui| {
        got = palette.ui(ui);
    });
    out.drop_without_applying_deltas();
    got
}

fn key_press(key: Key, modifiers: Modifiers) -> RawInput {
    RawInput {
        events: vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..RawInput::default()
    }
}

// ---- prefixes and the legend ------------------------------------------------------------------

#[test]
fn each_prefix_selects_its_scope_and_the_rest_is_the_body() {
    // provenance: derived — DESIGN.md "Command palette" table: # voucher, @ party, > command,
    // ? ask, + entry. Leading spaces are ignored (`of_query` docs).
    assert_eq!(
        PaletteScope::of_query("#0412"),
        (PaletteScope::Vouchers, "0412")
    );
    assert_eq!(
        PaletteScope::of_query("@mehta"),
        (PaletteScope::Parties, "mehta")
    );
    assert_eq!(
        PaletteScope::of_query(">theme"),
        (PaletteScope::Commands, "theme")
    );
    assert_eq!(
        PaletteScope::of_query("?who owes"),
        (PaletteScope::Ask, "who owes")
    );
    assert_eq!(
        PaletteScope::of_query("+paid rent"),
        (PaletteScope::Entry, "paid rent")
    );
    assert_eq!(
        PaletteScope::of_query("  #0412"),
        (PaletteScope::Vouchers, "0412")
    );
    assert_eq!(PaletteScope::of_query("bal"), (PaletteScope::All, "bal"));
}

#[test]
fn the_legend_and_placeholder_are_the_mock_s() {
    // provenance: design/mock-v2/index.html:1136 (the input's placeholder) and :1138 (the legend
    // row, # voucher, @ party, > command, ? ask, + entry).
    assert_eq!(
        LEGEND,
        [
            ("#", "voucher"),
            ("@", "party"),
            (">", "command"),
            ("?", "ask"),
            ("+", "entry"),
        ]
    );
    assert_eq!(
        PLACEHOLDER,
        "Search reports, ledgers, vouchers, or type ? to ask, + to enter"
    );
}

#[test]
fn the_voucher_prefix_keeps_only_vouchers() {
    // provenance: derived — `#` finds vouchers only (DESIGN.md). Hand check: "0412" matches the
    // voucher title exactly; the report, ledger and command are not admitted by the scope.
    let mut p = open_with(
        vec![
            report("bs", "Balance Sheet"),
            master("ledger:1", "Sharma Traders"),
            voucher("voucher:4", "0412"),
            command(Action::Save, "Save"),
        ],
        PaletteMode::OnTop,
    );
    p.set_query("#0412");
    assert_eq!(titles(&p), ["0412"]);
}

#[test]
fn the_party_prefix_keeps_only_ledgers_and_parties() {
    // provenance: derived — `@` finds parties and ledgers (DESIGN.md). "sharma" is a prefix of
    // "Sharma Traders" only; "Sharma Report" is a report and is not admitted.
    let mut p = open_with(
        vec![
            report("r", "Sharma Report"),
            master("ledger:1", "Sharma Traders"),
            voucher("voucher:4", "Sharma voucher"),
        ],
        PaletteMode::OnTop,
    );
    p.set_query("@sharma");
    assert_eq!(titles(&p), ["Sharma Traders"]);
}

#[test]
fn the_command_prefix_keeps_only_commands() {
    // provenance: derived — `>` lists commands only (index.html:1125, the `>` branch of palResults).
    // "sales" is in the report title too; the report is not admitted.
    let mut p = open_with(
        vec![
            report("sr", "Sales Register"),
            command(Action::NewSales, "New sales voucher"),
        ],
        PaletteMode::OnTop,
    );
    p.set_query(">sales");
    assert_eq!(titles(&p), ["New sales voucher"]);
}

// ---- assistant rows: one inert row, chooses nothing -------------------------------------------

#[test]
fn the_ask_prefix_shows_one_row_that_says_it_is_not_available() {
    // provenance: index.html:1123 titles the row "Ask: <question>"; the detail is ours and says
    // the feature is not available (no answer is made).
    let mut p = open_with(vec![report("bs", "Balance Sheet")], PaletteMode::OnTop);
    p.set_query("?who owes us for more than 30 days");
    assert_eq!(titles(&p), ["Ask: who owes us for more than 30 days"]);
    let detail = p.results()[0].detail.clone();
    assert!(detail.starts_with("Not available yet"), "{detail}");
}

#[test]
fn the_ask_prefix_with_nothing_after_it_is_titled_ask_your_books() {
    // provenance: index.html:1121 heads the ? section "Ask your books".
    let mut p = open_with(Vec::new(), PaletteMode::OnTop);
    p.set_query("?");
    assert_eq!(titles(&p), ["Ask your books"]);
}

#[test]
fn choosing_an_assistant_row_does_nothing_and_keeps_the_palette_open() {
    // provenance: derived — the assistant is not connected, so the row is inert: accept() returns
    // None, the palette stays open and nothing enters the recents.
    let mut p = open_with(Vec::new(), PaletteMode::OnTop);
    p.set_query("+paid rent 25k to Sharma Properties");
    assert_eq!(titles(&p), ["Draft: paid rent 25k to Sharma Properties"]);
    assert!(p.accept().is_none());
    assert!(p.is_open());
    assert_eq!(p.recents(), []);
}

#[test]
fn enter_on_an_assistant_row_keeps_the_palette_open_in_the_drawn_frame() {
    // provenance: derived — the same inert rule as above, through the drawn palette's Enter key.
    let mut p = open_with(Vec::new(), PaletteMode::OnTop);
    p.set_query("?who owes");
    assert!(frame(&mut p, RawInput::default()).is_none());
    assert!(frame(&mut p, key_press(Key::Enter, Modifiers::NONE)).is_none());
    assert!(p.is_open());
}

// ---- modes and keys ---------------------------------------------------------------------------

#[test]
fn enter_chooses_in_the_opened_mode_and_shift_enter_in_the_other() {
    // provenance: index.html:1139 footer "Enter open on top", "Shift+Enter replace screen"; the
    // mock opens Ctrl+K on top, so Enter keeps the mode and Shift+Enter flips it.
    // spec change 2026-11-23: `open_balance_sheet` types "bal", since the empty query now lists
    // the prefix rows, which fill rather than choose (index.html:1115-1116).
    let mut p = open_balance_sheet(PaletteMode::OnTop);
    let choice = frame(&mut p, key_press(Key::Enter, Modifiers::SHIFT)).unwrap();
    assert_eq!(choice.mode, PaletteMode::Replace);

    let mut p = open_balance_sheet(PaletteMode::OnTop);
    let choice = frame(&mut p, key_press(Key::Enter, Modifiers::NONE)).unwrap();
    assert_eq!(choice.mode, PaletteMode::OnTop);
}

#[test]
fn a_palette_opened_to_replace_flips_the_same_way() {
    // provenance: derived — Ctrl+G opens in Replace mode; Enter keeps it and Shift+Enter gives
    // OnTop, as the footer says (see `key_hints_follow_the_mode_they_were_opened_in`).
    // spec change 2026-11-23: typed "bal" as in the test above.
    let mut p = open_balance_sheet(PaletteMode::Replace);
    let choice = frame(&mut p, key_press(Key::Enter, Modifiers::NONE)).unwrap();
    assert_eq!(choice.mode, PaletteMode::Replace);

    let mut p = open_balance_sheet(PaletteMode::Replace);
    let choice = frame(&mut p, key_press(Key::Enter, Modifiers::SHIFT)).unwrap();
    assert_eq!(choice.mode, PaletteMode::OnTop);
}

#[test]
fn the_footer_keys_follow_the_mode_they_were_opened_in() {
    // provenance: index.html:1159, the palette's key bar: a first hint with no key, "Go to", then
    // "↑ ↓" Move, Enter "Open on top", Shift+Enter "Replace", Esc "Close". Spec change
    // 2026-11-23: this test pinned "Up / Down" and "Replace screen" (the footer's words, index.html
    // :1139); the key bar now reads as the mock's does, and the footer's own words are pinned in
    // `w2_palette_look.rs`.
    let on_top = open_with(Vec::new(), PaletteMode::OnTop);
    let hints: Vec<(String, String)> = on_top
        .key_hints()
        .into_iter()
        .map(|h| (h.keys, h.label))
        .collect();
    assert_eq!(
        hints,
        [
            (String::new(), "Go to".to_owned()),
            ("\u{2191} \u{2193}".to_owned(), "Move".to_owned()),
            ("Enter".to_owned(), "Open on top".to_owned()),
            ("Shift+Enter".to_owned(), "Replace".to_owned()),
            ("Esc".to_owned(), "Close".to_owned()),
        ]
    );

    let replace = open_with(Vec::new(), PaletteMode::Replace);
    let enter: Vec<String> = replace
        .key_hints()
        .into_iter()
        .filter(|h| h.keys == "Enter" || h.keys == "Shift+Enter")
        .map(|h| h.label)
        .collect();
    assert_eq!(enter, ["Replace", "Open on top"]);
}

// ---- grouping, recents -----------------------------------------------------------------------

#[test]
fn rows_are_grouped_by_heading_in_the_mock_s_order() {
    // provenance: index.html:1127-1129 push the headings Reports and screens, Ledgers, Commands in
    // that order. Equal scores keep the given order inside a heading (rank docs).
    // spec change 2026-11-23: the empty query now lists the prefix rows (index.html:1115-1116),
    // so the test types "a": each title contains it, not at a word start (400 each).
    let mut p = open_with(
        vec![
            command(Action::Save, "Save"),
            report("db", "Day Book"),
            report("bs", "Balance Sheet"),
        ],
        PaletteMode::OnTop,
    );
    p.set_query("a");
    assert_eq!(titles(&p), ["Day Book", "Balance Sheet", "Save"]);
}

#[test]
fn ranked_rows_stay_in_rank_order_inside_each_heading() {
    // provenance: derived. "cash": "Cash Book" starts with it (prefix, 800); "New cash payment"
    // and "Petty Cash" have it at a word start (600 each, so the given order decides between
    // them). Grouped: the two reports first (in rank order), then the command.
    let p = {
        let mut p = open_with(
            vec![
                report("cb", "Cash Book"),
                command(Action::NewPayment, "New cash payment"),
                report("pc", "Petty Cash"),
            ],
            PaletteMode::OnTop,
        );
        p.set_query("cash");
        p
    };
    assert_eq!(titles(&p), ["Cash Book", "Petty Cash", "New cash payment"]);
}

#[test]
fn recent_choices_come_first_and_survive_the_search_that_found_them() {
    // provenance: derived — a voucher the API found is chosen, then the palette is reopened: the
    // search's rows are cleared by `open`, but the recent choice is still shown first (its row is
    // kept with the target).
    let mut p = open_with(Vec::new(), PaletteMode::OnTop);
    p.set_remote_entries(vec![voucher("voucher:4", "0412")]);
    p.set_query("0412");
    let choice = p.accept().unwrap();
    assert_eq!(
        choice.target,
        PaletteTarget::Voucher("voucher:4".to_owned())
    );
    assert_eq!(
        p.recents(),
        [PaletteTarget::Voucher("voucher:4".to_owned())]
    );

    p.open(PaletteMode::OnTop);
    // spec change 2026-11-23: the empty query lists Recent and then the five "Type a prefix"
    // rows (index.html:1115-1116); the recent row is still first.
    let mut want = vec!["0412".to_owned()];
    want.extend(
        [
            "# voucher number",
            "@ party or ledger",
            "> command",
            "? ask your books",
            "+ entry in plain words",
        ]
        .map(str::to_owned),
    );
    assert_eq!(titles(&p), want);
}

#[test]
fn a_fresh_palette_remembers_nothing() {
    // provenance: derived — recents are session only (module docs): a new palette has none.
    assert_eq!(Palette::new().recents(), []);
}

// ---- no results ------------------------------------------------------------------------------

#[test]
fn a_query_with_no_match_is_drawn_and_leaves_the_palette_open() {
    // provenance: index.html:1129 — with no match the mock shows 'No match for "<query>"' and a
    // suggestion. The suggestion's text is ours ("Search vouchers for ..."); it is clickable only,
    // so Enter keeps the palette open and chooses nothing.
    let mut p = open_with(vec![report("bs", "Balance Sheet")], PaletteMode::OnTop);
    p.set_query("zzqx");
    assert_eq!(titles(&p), [] as [std::string::String; 0]);
    assert!(frame(&mut p, RawInput::default()).is_none());
    assert!(frame(&mut p, key_press(Key::Enter, Modifiers::NONE)).is_none());
    assert!(p.is_open());
}

// ---- the API search and the prefixes ---------------------------------------------------------

#[test]
fn only_the_voucher_and_party_prefixes_and_plain_text_are_searched() {
    // provenance: derived — `#` and `@` send the text after the prefix to the API; `>`, `?` and
    // `+` send nothing (their rows are commands or inert rows). Nothing typed means nothing sent.
    let mut p = open_with(Vec::new(), PaletteMode::OnTop);
    let cases = [
        ("#0412", Some("0412")),
        ("@mehta", Some("mehta")),
        ("  bal  ", Some("bal")),
        (">theme", None),
        ("?who", None),
        ("+paid", None),
        ("#", None),
        ("", None),
    ];
    for (query, expected) in cases {
        p.set_query(query);
        assert_eq!(p.search_text(), expected, "query {query:?}");
    }
}

#[test]
fn an_assistant_query_makes_no_api_call_and_no_note() {
    // provenance: derived — with no worker, a query that is searched gets the no-company note; a
    // `?` query is not searched, so it gets none and nothing is pending.
    let mut p = open_with(Vec::new(), PaletteMode::OnTop);
    p.set_query("?who owes");
    let mut search = PaletteSearch::new();
    search.tick(&mut p, None);
    assert!(p.note().is_none());
    assert!(!search.is_pending());
}

#[test]
fn a_voucher_found_by_the_api_shows_its_date_in_the_mock_s_format() {
    // provenance: design/mock-v2/index.html:853 (fmtDate): "21 Nov 2026", the day without a leading
    // zero. The date is the one the API returned; no amount or party is shown (not in the hit).
    let hits = vec![SearchHit::Voucher {
        id: VoucherId(9),
        number: Some("0412".to_owned()),
        date: date!(2026 - 04 - 10),
        matched: MatchedOn::Number,
    }];
    let rows = entries_from_hits("0412", &hits);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].detail, "Voucher, 10 Apr 2026, found by number");
}

#[test]
fn a_ledger_hit_is_found_by_the_query_it_was_searched_for() {
    // provenance: derived — the alias is the searched text, so the row is found again by it.
    let hits = vec![SearchHit::Ledger {
        id: LedgerId(7),
        name: "Ghee".to_owned(),
        matched: MatchedOn::Alias,
    }];
    let rows = entries_from_hits("ghee", &hits);
    assert_eq!(rows[0].aliases, ["ghee"]);
    assert_eq!(rows[0].detail, "Ledger, found by alias");
}

#[test]
fn the_shell_s_palette_rows_never_list_a_command_with_no_route() {
    // provenance: derived — `sidebar::command_route` makes Save `Hidden` (no screen runs it from the
    // palette, rule B3) and NewSales a `Screen`. The shell's rows must keep NewSales, drop Save.
    let keymap = classic();
    let mut p = Palette::new();
    p.set_entries(palette_rows(&keymap));
    p.open(PaletteMode::OnTop);
    p.set_query(">");
    let targets: Vec<PaletteTarget> = p.results().iter().map(|e| e.target.clone()).collect();
    assert!(!targets.contains(&PaletteTarget::Command(Action::Save)));
    assert!(targets.contains(&PaletteTarget::Command(Action::NewSales)));
}

#[test]
fn the_command_list_starts_with_sales_purchase_payment_and_receipt() {
    // provenance: design/mock-v2/index.html:1110 (`PAL_CMDS`): the `>` list starts New sales
    // invoice, New purchase, New payment, New receipt. Derived: the shell's `>` rows start with
    // those four commands, in that order; the rest follow.
    let mut p = Palette::new();
    p.set_entries(palette_rows(&classic()));
    p.open(PaletteMode::OnTop);
    p.set_query(">");
    let first: Vec<PaletteTarget> = p
        .results()
        .iter()
        .take(4)
        .map(|e| e.target.clone())
        .collect();
    assert_eq!(
        first,
        [
            PaletteTarget::Command(Action::NewSales),
            PaletteTarget::Command(Action::NewPurchase),
            PaletteTarget::Command(Action::NewPayment),
            PaletteTarget::Command(Action::NewReceipt),
        ]
    );
}

#[test]
fn the_four_voucher_commands_carry_the_mock_s_titles() {
    // provenance: design/mock-v2/index.html:1110 (`PAL_CMDS`): "New sales invoice", "New
    // purchase", "New payment", "New receipt".
    assert_eq!(Action::NewSales.label(), "New sales invoice");
    assert_eq!(Action::NewPurchase.label(), "New purchase");
    assert_eq!(Action::NewPayment.label(), "New payment");
    assert_eq!(Action::NewReceipt.label(), "New receipt");
}

#[test]
fn the_host_s_command_list_has_one_row_per_keymap_action() {
    // provenance: derived — `command_entries` documents one entry per keymap action, so the count
    // is `Action::ALL.len()` (keymap.rs:104). No key-chord text is asserted here.
    let entries = command_entries(&classic());
    assert_eq!(entries.len(), Action::ALL.len());
}

// ---- drawing ----------------------------------------------------------------------------------

#[test]
fn every_mock_state_draws_without_choosing_anything() {
    // provenance: the seven states of design/mock-v2/states.json (palette.default, search,
    // voucher, ask, entry, commands, no-results). Drawing each must not panic or choose a row
    // when no key is pressed.
    let queries = [
        "",
        "bal",
        "#0412",
        "?who owes us",
        "+paid rent 25k",
        ">",
        "zzqx",
    ];
    for query in queries {
        let mut p = open_with(
            vec![
                report("bs", "Balance Sheet"),
                master("ledger:1", "Sharma Traders"),
                command(Action::NewSales, "New sales voucher"),
            ],
            PaletteMode::OnTop,
        );
        p.set_query(query);
        assert!(
            frame(&mut p, RawInput::default()).is_none(),
            "query {query:?}"
        );
        assert!(p.is_open(), "query {query:?}");
    }
}

#[test]
fn a_closed_palette_has_no_rows_to_draw() {
    // provenance: derived — the shell draws the palette only while it is open; closed, it lists
    // nothing (close clears the query).
    let mut p = open_with(vec![report("bs", "Balance Sheet")], PaletteMode::OnTop);
    p.close();
    assert_eq!(p.results(), [] as [&ledgerly_app::palette::PaletteEntry; 0]);
}
