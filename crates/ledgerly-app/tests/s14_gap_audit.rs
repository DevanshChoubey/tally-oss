//! Round s14 (gap audit): differences from the mock's states that `design/PARITY-GAPS.md` listed
//! and that the app could close. Each test names the mock state and the line it follows.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::time::Duration;

use egui::epaint::ClippedShape;
use egui::{Pos2, RawInput, Rect, Shape, Vec2};
use ledgerly_api::{Api, ReportKind};
use ledgerly_app::screens::report_view::ReportViewScreen;
use ledgerly_app::shell::navigate::report_screen;
use ledgerly_app::theme::{self, ThemeKind};
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{Clock, Outcome};
use time::OffsetDateTime;

fn texts(shapes: &[ClippedShape]) -> Vec<String> {
    fn walk(shape: &Shape, out: &mut Vec<String>) {
        match shape {
            Shape::Text(t) => out.push(t.galley.text().to_owned()),
            Shape::Vec(v) => {
                for s in v {
                    walk(s, out);
                }
            }
            Shape::Noop
            | Shape::Circle(_)
            | Shape::Ellipse(_)
            | Shape::LineSegment { .. }
            | Shape::Path(_)
            | Shape::Band(_)
            | Shape::Rect(_)
            | Shape::Mesh(_)
            | Shape::QuadraticBezier(_)
            | Shape::CubicBezier(_)
            | Shape::Callback(_) => {}
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, &mut out);
    }
    out
}

fn sample(name: &str) -> Api {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    ledgerly_sample::create_under(&dir, Clock::Fixed(OffsetDateTime::UNIX_EPOCH))
        .unwrap()
        .0
}

fn input() -> RawInput {
    RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 900.0))),
        ..RawInput::default()
    }
}

/// The texts one report screen draws once the engine has answered.
fn report_texts(name: &str, kind: ReportKind) -> (ReportViewScreen, Vec<String>) {
    let api = sample(name);
    let books = match api.books() {
        Ok(Outcome::Value(b)) => b,
        other => panic!("no books: {other:?}"),
    };
    let worker = ApiWorker::spawn(api).unwrap();
    let mut screen = report_screen(kind, &books).unwrap();
    let mut ctx = AppCtx {
        books: Some(books),
        api: Some(worker),
        ..AppCtx::default()
    };
    let egui_ctx = egui::Context::default();
    theme::apply(&egui_ctx, ThemeKind::Light);
    let mut drawn = Vec::new();
    for _ in 0..4000 {
        let out = egui_ctx.run_ui(input(), |ui| {
            let _stay = screen.ui(ui, &mut ctx);
        });
        drawn = texts(&out.shapes);
        out.drop_without_applying_deltas();
        if !screen.is_loading() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let out = egui_ctx.run_ui(input(), |ui| {
        let _stay = screen.ui(ui, &mut ctx);
    });
    drawn = if screen.is_loading() {
        drawn
    } else {
        texts(&out.shapes)
    };
    out.drop_without_applying_deltas();
    (screen, drawn)
}

#[test]
fn the_profit_and_loss_head_has_the_detailed_button() {
    // provenance: index.html:2234 `plScreen`: the head's actions are "Detailed Alt+F1" (or
    // "Condensed" when detailed), Print preview, Export.
    let (_screen, drawn) = report_texts("s14_pl_head", ReportKind::ProfitAndLoss);
    assert!(
        drawn.iter().any(|t| t == "Detailed"),
        "texts drawn: {drawn:?}"
    );
    assert!(
        drawn.iter().any(|t| t == "Alt+F1"),
        "texts drawn: {drawn:?}"
    );
}

#[test]
fn the_audit_edit_log_has_no_particulars_column() {
    // provenance: index.html `audit.default`: columns Seq, Date, Time, User, Action, Object,
    // Reason, Field, Before, After; there is no "Particulars" column.
    let (_screen, drawn) = report_texts(
        "s14_audit",
        ReportKind::Audit(ledgerly_api::AuditReportKind::EditLog),
    );
    assert!(drawn.iter().any(|t| t == "Seq"), "texts drawn: {drawn:?}");
    assert!(
        !drawn.iter().any(|t| t == "Particulars"),
        "texts drawn: {drawn:?}"
    );
}

mod masters_forms {
    //! `masters.create-item` and `masters.gstin-error`.

    use super::*;
    use ledgerly_app::screens::masters::{MasterDraft, MasterKind, MastersScreen, PartyDraft};
    use ledgerly_model::Books;

    fn books(name: &str) -> Books {
        match sample(name).books() {
            Ok(Outcome::Value(b)) => b,
            other => panic!("no books: {other:?}"),
        }
    }

    fn draw(screen: &mut MastersScreen, books: &Books) -> Vec<ClippedShape> {
        let mut ctx = AppCtx {
            books: Some(books.clone()),
            ..AppCtx::default()
        };
        let egui_ctx = egui::Context::default();
        theme::apply(&egui_ctx, ThemeKind::Light);
        let mut last = Vec::new();
        for _ in 0..3 {
            let out = egui_ctx.run_ui(input(), |ui| {
                let _stay = screen.ui(ui, &mut ctx);
            });
            last.clone_from(&out.shapes);
            out.drop_without_applying_deltas();
        }
        last
    }

    #[test]
    fn the_stock_item_form_is_its_own_page_titled_new_stock_item() {
        // provenance: index.html `masters.create-item`: the page is titled "New stock item",
        // its subtitle says creating it posts nothing, it has no tabs and no Create button, and a
        // note under the fields says "A unit is needed before the item can be saved."
        let books = books("s14_item_form");
        let mut screen = MastersScreen::with_kind(MasterKind::StockItem);
        screen.begin_create(&books).unwrap();
        assert_eq!(
            screen.page_title(&ledgerly_app::i18n::Tr::default()),
            "New stock item"
        );
        let drawn = texts(&draw(&mut screen, &books));
        for want in [
            "New stock item",
            "Creating a stock item posts nothing. Esc goes back to the list.",
            "A unit is needed before the item can be saved.",
        ] {
            assert!(drawn.iter().any(|t| t == want), "{want:?} in {drawn:?}");
        }
        for not in ["Create stock item", "Ledgers", "Stock items"] {
            assert!(!drawn.iter().any(|t| t == not), "{not:?} in {drawn:?}");
        }
    }

    fn ledger_with_gstin(books: &Books, gstin: &str) -> MastersScreen {
        let mut screen = MastersScreen::with_kind(MasterKind::Ledger);
        screen.begin_create(books).unwrap();
        if let Some(MasterDraft::Ledger(d)) = screen.draft_mut() {
            "Jadhav Irrigation Services".clone_into(&mut d.name);
            d.party = Some(PartyDraft {
                gstin: gstin.to_owned(),
                ..PartyDraft::default()
            });
        }
        screen
    }

    fn red_outlines(shapes: &[ClippedShape], bad: egui::Color32) -> usize {
        fn walk(shape: &Shape, bad: egui::Color32, n: &mut usize) {
            match shape {
                Shape::Rect(r) if r.stroke.width > 0.0 && r.stroke.color == bad => *n += 1,
                Shape::Vec(v) => v.iter().for_each(|s| walk(s, bad, n)),
                Shape::Noop
                | Shape::Circle(_)
                | Shape::Ellipse(_)
                | Shape::LineSegment { .. }
                | Shape::Path(_)
                | Shape::Band(_)
                | Shape::Rect(_)
                | Shape::Text(_)
                | Shape::Mesh(_)
                | Shape::QuadraticBezier(_)
                | Shape::CubicBezier(_)
                | Shape::Callback(_) => {}
            }
        }
        let mut n = 0;
        for c in shapes {
            walk(&c.shape, bad, &mut n);
        }
        n
    }

    #[test]
    fn a_wrong_gstin_has_a_red_outline_and_blocks_the_save_button() {
        // provenance: index.html `masters.gstin-error`: the GSTIN field is outlined in red and
        // Save is disabled while the GSTIN fails its check; a right GSTIN does neither.
        let books = books("s14_gstin");
        let bad = theme::tokens(ThemeKind::Light).bad;
        let mut wrong = ledger_with_gstin(&books, "27ZZZZZ0100Z1Z7");
        let drawn = draw(&mut wrong, &books);
        assert!(red_outlines(&drawn, bad) > 0);
        assert!(wrong.save_is_blocked());
        let mut right = ledger_with_gstin(&books, "27ZZZZZ0000Z1ZQ");
        let drawn = draw(&mut right, &books);
        assert_eq!(red_outlines(&drawn, bad), 0);
        assert!(!right.save_is_blocked());
        let none = ledger_with_gstin(&books, "");
        assert!(!none.save_is_blocked());
    }
}
