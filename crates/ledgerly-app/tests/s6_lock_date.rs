//! The shell keeps `AppCtx::lock_date` from the open company file, so the top bar's lock chip and
//! the invoice's period-lock note (mock state `sales-invoice.period-locked`, index.html:392:
//! "Date falls on or before the lock date 31 Oct 2026: saving is blocked") have a date to show.
//!
//! Expected values are `derived`: the date the test locks the file to, or `None` with no lock.

#![allow(
    clippy::unwrap_used,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::time::Duration;

use ledgerly_api::{Api, Command};
use ledgerly_app::screens::home::HomeScreen;
use ledgerly_app::shell::Shell;
use ledgerly_app::worker::ApiWorker;
use ledgerly_core::{Clock, CompanyId, Date, StateCode};
use ledgerly_model::{Address, Company, EntityKind, Features, NegativeStockPolicy};
use time::OffsetDateTime;
use time::macros::date;

fn company() -> Company {
    Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Lock Traders".to_owned(),
        mailing_name: None,
        address: Address::default(),
        state: StateCode::new(27).unwrap(),
        pan: None,
        registrations: Vec::new(),
        fy_start: date!(2026 - 04 - 01),
        books_begin: date!(2026 - 04 - 01),
        entity_kind: EntityKind::Company,
        features: Features::default(),
        negative_stock: NegativeStockPolicy::Block,
    }
}

/// A shell over a new company file, locked to `lock` when it is `Some`, after its books arrive.
fn shell_over(name: &str, lock: Option<Date>) -> Shell {
    let path = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _removed = std::fs::remove_file(&path);
    let mut api = Api::create(&path, &company(), Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap();
    if let Some(until) = lock {
        api.execute(Command::LockPeriod(until)).unwrap();
    }
    let mut shell = Shell::new(Box::new(HomeScreen::default()));
    shell.app_ctx_mut().api = Some(ApiWorker::spawn(api).unwrap());
    for _ in 0..2000 {
        shell.sync_books();
        if shell.app_ctx_mut().books.is_some() && shell.app_ctx_mut().lock_date == lock {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    shell
}

#[test]
fn the_shell_reads_the_lock_date_of_the_open_company() {
    let mut shell = shell_over("s6_locked.ledgerly", Some(date!(2026 - 10 - 31)));
    // provenance: derived, the date the file was locked to above.
    assert_eq!(shell.app_ctx_mut().lock_date, Some(date!(2026 - 10 - 31)));
}

#[test]
fn a_company_with_no_lock_has_no_lock_date() {
    let mut shell = shell_over("s6_open.ledgerly", None);
    assert!(shell.app_ctx_mut().books.is_some(), "the books arrived");
    // provenance: derived, no LockPeriod was sent.
    assert_eq!(shell.app_ctx_mut().lock_date, None);
}
