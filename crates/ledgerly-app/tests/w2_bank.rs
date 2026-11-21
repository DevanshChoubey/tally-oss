//! Wave 2 item S-F: the bank reconciliation screen against the mock v2 states `bank.default`,
//! `bank.empty`, `bank.import-error` and `bank.brs`, and fix B6.
//!
//! Provenance:
//! * `derived`: the company below is a small fiction. Every expected figure is worked out by hand
//!   in the test that uses it.
//! * `mock`: the wording of the empty state and the import-error state is taken from
//!   `design/mock-v2/index.html` (the `SCREENS.bank` render function, around lines 1732-1746).
//! * B6 (`docs/status/2026-11-22-ui-parity-report.md`, section 5): with no statement loaded the
//!   screen must not say "0 of 0 lines are not matched".

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use egui::epaint::ClippedShape;
use egui::{RawInput, Shape};
use ledgerly_api::{Api, ApiError, Command};
use ledgerly_app::screens::bank::BankScreen;
use ledgerly_app::worker::ApiWorker;
use ledgerly_app::{AppCtx, Screen};
use ledgerly_core::{Clock, CompanyId, GroupId, LedgerId, Outcome, SignedAmount, StateCode};
use ledgerly_model::seed_groups::reserved_groups;
use ledgerly_model::{
    Address, BankDetails, Company, EntityKind, Features, Ledger, NegativeStockPolicy,
    ReservedGroup,
};
use time::OffsetDateTime;
use time::macros::date;

const BANK: LedgerId = LedgerId(10);

fn group(kind: ReservedGroup) -> GroupId {
    reserved_groups()
        .into_iter()
        .find(|g| g.reserved == Some(kind))
        .unwrap()
        .id
}

fn company() -> Company {
    Company {
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
    }
}

fn bank_ledger() -> Ledger {
    let details = BankDetails {
        account_number: "0123456789".to_owned(),
        ifsc: "TEST0000001".to_owned(),
        bank_name: "Test Bank".to_owned(),
        branch: "Main".to_owned(),
    };
    Ledger {
        ineligible_itc: None,
        id: BANK,
        name: "Bank Current Account".to_owned(),
        aliases: Vec::new(),
        group: group(ReservedGroup::BankAccounts),
        opening: SignedAmount::dr(ledgerly_core::Paise::new(100_000)),
        bill_wise: None,
        cost_centres_applicable: false,
        party: None,
        bank: Some(details),
        tax: None,
        gst: None,
        reserved: None,
        tally: None,
    }
}

/// A company file with one bank ledger and no imported statement. The file name is unique per
/// test.
fn setup_without_statement(name: &str) -> Api {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let clock = Clock::Fixed(OffsetDateTime::UNIX_EPOCH);
    let mut api = Api::create(&path, &company(), clock).unwrap();
    let out: Result<Outcome<ledgerly_api::CommandResult>, ApiError> =
        api.execute(Command::CreateLedger(bank_ledger()));
    match out {
        Ok(Outcome::Value(_)) => {}
        other => panic!("the bank ledger was not created: {other:?}"),
    }
    api
}

/// The text every shape of a frame draws, one galley per line.
fn drawn_text(shapes: &[ClippedShape]) -> String {
    let mut out = String::new();
    for clipped in shapes {
        collect_text(&clipped.shape, &mut out);
    }
    out
}

fn collect_text(shape: &Shape, out: &mut String) {
    match shape {
        Shape::Vec(children) => {
            for child in children {
                collect_text(child, out);
            }
        }
        Shape::Text(text) => {
            out.push_str(text.galley.text());
            out.push('\n');
        }
        _ => {}
    }
}

/// Draws one frame of `screen` and returns the text it drew. The frame is run through the
/// screen's own `ui`, which also advances the worker-mode state.
fn frame(ctx: &egui::Context, screen: &mut BankScreen, app: &mut AppCtx) -> String {
    let out = ctx.run_ui(RawInput::default(), |ui| {
        let _action = screen.ui(ui, app);
    });
    let text = drawn_text(&out.shapes);
    out.drop_without_applying_deltas();
    text
}

/// Draws frames until the screen has nothing pending, then returns the text of the last frame.
/// Bounded, with a short sleep between frames.
fn settle(ctx: &egui::Context, screen: &mut BankScreen, app: &mut AppCtx) -> String {
    for _ in 0..2000 {
        let text = frame(ctx, screen, app);
        if !screen.is_loading() {
            return text;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the bank screen did not settle within the bound");
}

/// Bug B6. With no statement loaded the matching tab must not report "0 of 0 lines".
/// Provenance: the parity report, section 5, row B6.
#[test]
fn with_no_statement_loaded_the_screen_does_not_report_zero_of_zero_lines() {
    let worker = ApiWorker::spawn(setup_without_statement("w2_b6_no_statement.ledgerly")).unwrap();
    let mut app = AppCtx {
        api: Some(worker),
        ..AppCtx::default()
    };
    let mut screen = BankScreen::worker("Bank Current Account", BANK, date!(2026 - 04 - 30));
    let ctx = egui::Context::default();
    let text = settle(&ctx, &mut screen, &mut app);
    assert!(!screen.is_loading());
    assert!(
        !text.contains("0 of 0"),
        "the screen drew a count with no statement behind it: {text}"
    );
}

/// The empty state of worker mode with no statement loaded: the wording is the mock's
/// `bank.empty` state (`design/mock-v2/index.html`, `SCREENS.bank`, line ~1737), and the import
/// action is offered.
/// Provenance: `mock`.
#[test]
fn with_no_statement_loaded_the_empty_state_offers_the_import() {
    let worker = ApiWorker::spawn(setup_without_statement("w2_empty_state.ledgerly")).unwrap();
    let mut app = AppCtx {
        api: Some(worker),
        ..AppCtx::default()
    };
    let mut screen = BankScreen::worker("Bank Current Account", BANK, date!(2026 - 04 - 30));
    let ctx = egui::Context::default();
    let text = settle(&ctx, &mut screen, &mut app);
    assert!(
        text.contains("No statement for this account yet"),
        "the empty heading is missing: {text}"
    );
    assert!(
        text.contains("Download a statement from your bank's website as CSV, XLSX or MT940"),
        "the empty sentence is missing: {text}"
    );
    assert!(
        text.contains("Import statement"),
        "the import action is missing: {text}"
    );
}
