//! Round S12: the bank chooser shows each account's statement status, as the mock's
//! `bank-chooser.default` does (design/mock-v2/index.html, `SCREENS.bank-chooser`: a Statement
//! column with an "Imported" or "Not imported" badge and the period).
//!
//! Provenance (derived): the sample company stores one statement, for Saptagiri Commercial Bank
//! A/c (crates/ledgerly-sample/assets/saptagiri-nov-2026.mt940). Its first transaction line is
//! `:61:261105...` (5 Nov 2026) and its last is `:61:261120...` (20 Nov 2026), and a statement's
//! period is the dates of its first and last line (`ledgerly-bank` `check_statement`). Mula Valley
//! Co-op Bank has no statement. The mock writes "1 Nov" (the opening balance date); the engine's
//! period starts at the first line, so the app shows 5 Nov and the gap is in PARITY-GAPS.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use egui::RawInput;
use ledgerly_app::screens::bank::{BankChooserScreen, Statement};
use ledgerly_app::shell::{Shell, submit};
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen, ScreenAction};
use ledgerly_core::{Clock, Date, LedgerId, Outcome};
use time::OffsetDateTime;
use time::macros::date;

struct Shared(Rc<RefCell<BankChooserScreen>>);

impl Screen for Shared {
    fn title(&self) -> String {
        self.0.borrow().title()
    }
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut AppCtx) -> ScreenAction {
        self.0.borrow_mut().ui(ui, ctx)
    }
}

fn frame(ctx: &egui::Context, shell: &mut Shell) {
    let out = ctx.run_ui(RawInput::default(), |ui| shell.show(ui));
    out.drop_without_applying_deltas();
}

#[test]
fn the_chooser_says_which_accounts_have_a_statement_and_its_period() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("s12_bank_chooser_look");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (mut api, _path) =
        ledgerly_sample::create_under(&dir, Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    let books = match api.books() {
        Ok(Outcome::Value(b)) => b.clone(),
        other => panic!("no books: {other:?}"),
    };
    let _ = &mut api;
    let id_of = |name: &str| -> LedgerId {
        books
            .masters
            .ledgers
            .values()
            .find(|l| l.name == name)
            .unwrap()
            .id
    };
    let saptagiri = id_of("Saptagiri Commercial Bank A/c");
    let mula = id_of("Mula Valley Co-op Bank, current A/c");
    let choices = vec![
        ledgerly_app::screens::bank::BankChoice::new(
            saptagiri,
            "Saptagiri Commercial Bank A/c",
            "XXXX 4471",
        ),
        ledgerly_app::screens::bank::BankChoice::new(
            mula,
            "Mula Valley Co-op Bank, current A/c",
            "XXXX 0932",
        ),
    ];
    let ctx = egui::Context::default();
    let chooser = Rc::new(RefCell::new(BankChooserScreen::new(
        choices,
        date!(2026 - 11 - 20),
        None,
    )));
    let mut shell = Shell::new(Box::new(Shared(Rc::clone(&chooser))));
    shell.app_ctx_mut().api = Some(ApiWorker::spawn(api).unwrap());
    submit::note_changed();
    let start = Instant::now();
    while chooser.borrow().is_loading() {
        frame(&ctx, &mut shell);
        assert!(start.elapsed() < Duration::from_secs(10), "timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
    let statements: Vec<Statement> = chooser
        .borrow()
        .rows()
        .iter()
        .map(|r| r.statement().clone())
        .collect();
    let d =
        |day: u8| -> Date { Date::from_calendar_date(2026, time::Month::November, day).unwrap() };
    assert_eq!(
        statements,
        vec![Statement::Imported(Some((d(5), d(20)))), Statement::None]
    );
}
