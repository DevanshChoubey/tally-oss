//! Card S-A (wave 2): the home screen's layout, body choice, AI panel, key bar and new-company
//! state. Expected values are derived by hand; each has its derivation or its mock reference next
//! to it. Nothing here copies what the screen prints.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_arithmetic,
    reason = "test code: rule B2 lets tests unwrap and panic; layout checks add points"
)]

use egui::{Pos2, RawInput, Rect, Vec2};
use ledgerly_app::screens::home::{
    AiPanel, AttentionItem, AttentionKind, HomeBody, HomeData, HomeScreen, HomeTarget, Kpi,
    ai_panel, body, date_line, greeting, panel_row_is_wide, sub_line, tile_columns,
};
use ledgerly_app::shell::AiTier;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{CompanyId, Outcome, Paise, StateCode};
use ledgerly_model::{Address, Books, Company, EntityKind, Features, Masters, NegativeStockPolicy};
use time::macros::date;

/// Height of the drawn window, in points. Only the width matters to the layout under test.
const WINDOW_HEIGHT: f32 = 800.0;
/// Content width in a 1280 px window: 1280 - 208 (sidebar) - 2 x 24 (margins) = 1024. The sidebar
/// and margins are read off `design/mock-v2/captures/states/light/home.default.webp`, where the
/// content runs from x = 232 to x = 1256.
const CONTENT_AT_1280: f32 = 1024.0;
/// Content width in a 1024 px window: 1024 - 208 - 2 x 24 = 768.
const CONTENT_AT_1024: f32 = 768.0;

fn kpi(label: &str, value: Outcome<Paise>, footnote: &str, target: HomeTarget) -> Kpi {
    Kpi {
        label: label.to_owned(),
        value,
        footnote: footnote.to_owned(),
        target,
    }
}

/// The four tiles of the mock, two of them unsupported with long reasons so that wrapping is
/// exercised. Figures are fictional: 45,000.00 and 8,000.00 rupees.
fn four_tiles() -> HomeData {
    HomeData {
        company_name: "Fictional Traders".to_owned(),
        subtitle: "Figures as of 30 Apr 2026".to_owned(),
        kpis: vec![
            kpi(
                "Cash and bank",
                Outcome::unsupported(
                    "Cash and bank",
                    "the engine gives cash in hand and bank accounts as two groups and has no combined total yet, and this screen does not add them",
                ),
                "Cash and bank accounts",
                HomeTarget::BalanceSheet {
                    node: "Cash-in-hand".to_owned(),
                },
            ),
            kpi(
                "Receivables",
                Outcome::Value(Paise::new(4_500_000)),
                "Closing balance as of 30 Apr 2026",
                HomeTarget::BalanceSheet {
                    node: "Sundry debtors".to_owned(),
                },
            ),
            kpi(
                "Payables",
                Outcome::Value(Paise::new(800_000)),
                "Closing balance as of 30 Apr 2026",
                HomeTarget::BalanceSheet {
                    node: "Sundry creditors".to_owned(),
                },
            ),
            kpi(
                "GST cash payable",
                Outcome::unsupported(
                    "GST cash payable",
                    "the engine gives the GSTR-3B working (tax and credit separately) but no payable after set-off, and this screen does not subtract",
                ),
                "after accepted ITC",
                HomeTarget::GstReturns {
                    tab: "gstr3b".to_owned(),
                },
            ),
        ],
        attention: Vec::new(),
    }
}

/// Books of a fictional company with no vouchers yet, starting on 1 April 2026.
fn books_without_vouchers() -> Books {
    Books {
        company: Company {
            ineligible_itc_default: None,
            id: CompanyId(1),
            name: "Fictional Traders".to_owned(),
            mailing_name: None,
            address: Address::default(),
            state: StateCode::new(29).unwrap(),
            pan: None,
            registrations: Vec::new(),
            fy_start: date!(2026 - 04 - 01),
            books_begin: date!(2026 - 04 - 01),
            entity_kind: EntityKind::Company,
            features: Features::default(),
            negative_stock: NegativeStockPolicy::Block,
        },
        masters: Masters::default(),
        vouchers: std::collections::BTreeMap::new(),
        postings: Vec::new(),
        stock_moves: Vec::new(),
    }
}

/// Draws the home screen twice in a window `window_width` points wide, with its content
/// `content_width` points wide (the window less the sidebar and margins), and returns the tile
/// rectangles of the second frame. `app` supplies the rest of the context.
fn drawn_tiles(
    window_width: f32,
    content_width: f32,
    data: HomeData,
    app: &mut AppCtx,
) -> Vec<Rect> {
    let ctx = egui::Context::default();
    let mut screen = HomeScreen::new(data);
    for _ in 0..2 {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(window_width, WINDOW_HEIGHT),
            )),
            ..RawInput::default()
        };
        let output = ctx.run_ui(input, |ui| {
            ui.set_max_width(content_width);
            let _action = screen.ui(ui, app);
        });
        output.drop_without_applying_deltas();
    }
    screen.tile_rects().to_vec()
}

/// The tiles that share the first tile's row (its top edge within one point of it).
fn first_row_count(rects: &[Rect]) -> usize {
    let Some(first) = rects.first() else {
        panic!("no tiles were drawn");
    };
    rects.iter().filter(|r| r.min.y < first.min.y + 1.0).count()
}

// ---- layout: no tile clips (B2) ---------------------------------------------------------------

#[test]
fn every_tile_is_inside_the_content_at_1280_and_1024() {
    // provenance: derived. The tile rectangles are measured in the drawn frame. Each must start at
    // or after the left edge, have a width, and end at or before the content width (1024 at 1280,
    // 768 at 1024). A tile that overflowed would end past it, which is the B2 bug.
    // Four tiles are drawn at both widths, one per tile.
    for (window, content) in [(1280.0, CONTENT_AT_1280), (1024.0, CONTENT_AT_1024)] {
        let mut app = AppCtx::default();
        let rects = drawn_tiles(window, content, four_tiles(), &mut app);
        assert_eq!(rects.len(), 4, "one rectangle per tile at window {window}");
        for rect in &rects {
            assert!(
                rect.min.x >= 0.0 && rect.width() > 0.0 && rect.max.x <= content,
                "tile {rect:?} is not inside the {content} point content at window {window}"
            );
        }
    }
}

#[test]
fn four_tiles_share_one_row_at_1280_and_two_rows_at_1024() {
    // provenance: derived. Four tiles need 4 x 200 + 3 x 12 = 836 points (tile_columns). At 1024
    // points they fit in one row of four. At 768 they do not, and two fit (2 x 200 + 12 = 412),
    // so the first row holds two tiles.
    let mut app = AppCtx::default();
    let wide = drawn_tiles(1280.0, CONTENT_AT_1280, four_tiles(), &mut app);
    assert_eq!(first_row_count(&wide), 4);

    let mut app = AppCtx::default();
    let narrow = drawn_tiles(1024.0, CONTENT_AT_1024, four_tiles(), &mut app);
    assert_eq!(first_row_count(&narrow), 2);
}

#[test]
fn tile_columns_switch_at_the_derived_widths() {
    // provenance: derived. Four tiles: 4 x 200 + 3 x 12 = 836. Two tiles: 2 x 200 + 12 = 412.
    assert_eq!(tile_columns(1024.0), 4);
    assert_eq!(tile_columns(836.0), 4);
    assert_eq!(tile_columns(835.0), 2);
    assert_eq!(tile_columns(768.0), 2);
    assert_eq!(tile_columns(412.0), 2);
    assert_eq!(tile_columns(411.0), 1);
    assert_eq!(tile_columns(0.0), 1);
}

#[test]
fn the_two_panels_sit_side_by_side_from_720_points() {
    // provenance: derived. 720 is the constant WIDE_MIN_WIDTH in home.rs: below it the lower
    // panels stack, so each keeps a usable width. The test pins the switch point so a change is
    // deliberate.
    assert!(panel_row_is_wide(768.0));
    assert!(panel_row_is_wide(720.0));
    assert!(!panel_row_is_wide(719.0));
}

// ---- bodies: loading, empty, figures ----------------------------------------------------------

#[test]
fn a_company_with_no_vouchers_shows_first_steps_even_while_loading() {
    // provenance: derived. The mock's empty state (design/mock-v2/index.html line 1049) shows no
    // figure and the first-step card, because totals stay hidden until there is data. A refresh in
    // progress does not change that, so the choice ignores `loading`.
    let begins = date!(2026 - 04 - 01);
    assert_eq!(
        body(Some(begins), true, true),
        HomeBody::NewCompany { begins }
    );
    assert_eq!(
        body(Some(begins), false, false),
        HomeBody::NewCompany { begins }
    );
}

#[test]
fn a_pending_refresh_shows_skeletons_before_any_figure() {
    // provenance: derived. The mock's loading state (index.html line 1047) draws skeleton rows
    // while the figures are fetched, so with a refresh pending the body is Loading whether or not
    // old figures exist.
    assert_eq!(body(None, true, false), HomeBody::Loading);
    assert_eq!(body(None, true, true), HomeBody::Loading);
}

#[test]
fn settled_figures_and_no_company_bodies() {
    // provenance: derived. With no refresh pending, figures show when there are some, and with no
    // company open nothing does.
    assert_eq!(body(None, false, true), HomeBody::Figures);
    assert_eq!(body(None, false, false), HomeBody::NoFigures);
}

#[test]
fn a_new_company_draws_no_tile() {
    // provenance: derived. A company with no vouchers draws no tile, so the engine's zero figures
    // never appear (index.html line 1049: "Totals stay hidden until there is data").
    let mut app = AppCtx {
        books: Some(books_without_vouchers()),
        ..AppCtx::default()
    };
    let rects = drawn_tiles(1280.0, CONTENT_AT_1280, four_tiles(), &mut app);
    assert!(rects.is_empty(), "tiles drawn for a new company: {rects:?}");
}

#[test]
fn no_company_draws_no_tile() {
    // provenance: derived. With no company and no figures, the screen draws no tile.
    let mut app = AppCtx::default();
    let rects = drawn_tiles(1280.0, CONTENT_AT_1280, HomeData::default(), &mut app);
    assert_eq!(rects, [] as [egui::Rect; 0]);
}

// ---- AI panel ---------------------------------------------------------------------------------

#[test]
fn only_ai_off_hides_the_ai_area() {
    // provenance: derived from the mock's state description (design/mock-v2/index.html line 373:
    // "AI tier Off: no AI cards or entry bar"). Local, Cloud and an unset tier show the labelled
    // placeholder, because the assistant does not run yet.
    assert_eq!(ai_panel(Some(AiTier::Off)), AiPanel::Off);
    assert_eq!(ai_panel(Some(AiTier::Local)), AiPanel::Placeholder);
    assert_eq!(ai_panel(Some(AiTier::Cloud)), AiPanel::Placeholder);
    assert_eq!(ai_panel(None), AiPanel::Placeholder);
}

#[test]
fn an_ai_off_company_still_draws_its_figures() {
    // provenance: derived. The mock's AI-off state keeps the KPI row (index.html line 1054 hides
    // only the entry bar and the review list). With AI off, all four tiles still draw.
    let mut app = AppCtx {
        ai_tier: Some(AiTier::Off),
        ..AppCtx::default()
    };
    let rects = drawn_tiles(1280.0, CONTENT_AT_1280, four_tiles(), &mut app);
    assert_eq!(rects.len(), 4);
}

// ---- titles and keys --------------------------------------------------------------------------

#[test]
fn the_page_title_is_home_and_the_company_line_joins_name_and_subtitle() {
    // provenance: derived. Before its first frame (no clock or books read yet) the page title is
    // "Home" and the company line joins the name and the subtitle with a middle dot, as the mock's
    // sub-line does ("Saturday 21 Nov 2026 · FY 2026-27", index.html line 1054). After a frame
    // the title is the greeting (see `after_a_frame_home_greets_and_the_key_bar_still_says_home`).
    let screen = HomeScreen::new(four_tiles());
    assert_eq!(screen.page_title(), "Home");
    assert_eq!(
        screen.subtitle().as_deref(),
        Some("Fictional Traders \u{b7} Figures as of 30 Apr 2026")
    );
    assert_eq!(HomeScreen::default().subtitle(), None);
}

#[test]
fn sub_line_handles_each_empty_part() {
    // provenance: derived. Name only, subtitle only, both, and neither.
    assert_eq!(sub_line("Fictional Traders", ""), "Fictional Traders");
    assert_eq!(sub_line("", "Books are open."), "Books are open.");
    assert_eq!(
        sub_line("Fictional Traders", "Books are open."),
        "Fictional Traders \u{b7} Books are open."
    );
    assert_eq!(sub_line("", ""), "");
}

#[test]
fn the_key_bar_lists_the_working_home_keys_and_not_the_removed_lists() {
    // provenance: derived. The mock's home key bar (index.html line 1078) lists F8 Sales, F9, F5,
    // F6, Ctrl+K, J/K for the review list and Enter. The review list and the Enter list are not
    // on this screen (the review list needs the assistant, and the sidebar replaces the Enter
    // list), and F9 opens nothing yet, so the bar lists F8 and not J/K or Enter.
    let keys = HomeScreen::default().keys();
    assert!(keys.iter().any(|k| k.keys == "F8" && k.label == "Sales"));
    assert!(keys.iter().any(|k| k.keys == "Ctrl+K"));
    assert!(!keys.iter().any(|k| k.keys == "Enter"));
    assert!(!keys.iter().any(|k| k.keys == "J" || k.keys == "K"));
    assert!(!keys.iter().any(|k| k.keys == "F9"));
}

#[test]
fn attention_items_keep_their_words_and_targets() {
    // provenance: derived. Each nudge line carries its word (Pending, Due, Done) and its target
    // unchanged; the screen does not change them.
    let item = AttentionItem {
        kind: AttentionKind::Due,
        headline: "GSTR-1 due".to_owned(),
        detail: String::new(),
        target: Some(HomeTarget::GstReturns {
            tab: "gstr1".to_owned(),
        }),
    };
    assert_eq!(item.kind.word(), "Due");
    assert_eq!(
        item.target,
        Some(HomeTarget::GstReturns {
            tab: "gstr1".to_owned()
        })
    );
}

// ---- greeting and date line (user decision 2026-11-23: the mock's greeting, no name) ---------

#[test]
fn the_greeting_follows_the_hour() {
    // provenance: derived. The mock greets "Good afternoon" (index.html line 1059) and names the
    // user; the user chose the greeting without a name on 2026-11-23. The hour bands are ours:
    // morning from 05:00, afternoon from 12:00, evening from 17:00 until 05:00. Catches a
    // greeting that ignores the clock or has a gap between bands.
    assert_eq!(greeting(5), "Good morning");
    assert_eq!(greeting(11), "Good morning");
    assert_eq!(greeting(12), "Good afternoon");
    assert_eq!(greeting(16), "Good afternoon");
    assert_eq!(greeting(17), "Good evening");
    assert_eq!(greeting(23), "Good evening");
    assert_eq!(greeting(0), "Good evening");
    assert_eq!(greeting(4), "Good evening");
}

#[test]
fn the_date_line_reads_like_the_mocks() {
    // provenance: derived from the mock's line, "Saturday 21 Nov 2026 · FY 2026-27 · books
    // locked to 31 Oct 2026" (index.html line 1059); 21 Nov 2026 is a Saturday by the calendar.
    // A part that is not known is left out, never invented.
    assert_eq!(
        date_line(
            date!(2026 - 11 - 21),
            Some("FY 2026-27"),
            Some(date!(2026 - 10 - 31))
        ),
        "Saturday 21 Nov 2026 \u{b7} FY 2026-27 \u{b7} books locked to 31 Oct 2026"
    );
    assert_eq!(
        date_line(date!(2026 - 11 - 23), Some("FY 2026-27"), None),
        "Monday 23 Nov 2026 \u{b7} FY 2026-27"
    );
    assert_eq!(
        date_line(date!(2026 - 04 - 10), None, None),
        "Friday 10 Apr 2026"
    );
}

#[test]
fn after_a_frame_home_greets_and_the_key_bar_still_says_home() {
    // provenance: derived. Once drawn with the clock and books, the page title is a greeting and
    // the line under it names today, the financial year and the lock; the key bar's name stays
    // "Home", as the mock's key bar does (`keys` of `SCREENS.home`, index.html line 1081).
    let ctx = egui::Context::default();
    let mut screen = HomeScreen::new(four_tiles());
    let mut app = AppCtx {
        books: Some(books_without_vouchers()),
        lock_date: Some(date!(2026 - 03 - 31)),
        ..AppCtx::default()
    };
    let out = ctx.run_ui(RawInput::default(), |ui| {
        screen.ui(ui, &mut app);
    });
    out.drop_without_applying_deltas();
    assert!(
        screen.page_title().starts_with("Good "),
        "title {}",
        screen.page_title()
    );
    assert_eq!(screen.key_bar_name(), "Home");
    let line = screen.subtitle().unwrap_or_default();
    assert!(
        line.ends_with("\u{b7} FY 2026-27 \u{b7} books locked to 31 Mar 2026"),
        "line {line}"
    );
}
