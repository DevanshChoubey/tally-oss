//! The API worker runs jobs off the UI thread, in order, and reports a stopped worker as `Lost`.
//!
//! Provenance: derived. These tests check ordering and delivery only; no figure is involved.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::time::{Duration, Instant};

use ledgerly_api::Api;
use ledgerly_app::worker::{ApiWorker, Poll, Ticket};
use ledgerly_core::{Clock, CompanyId, OffsetDateTime, StateCode};
use ledgerly_model::{Address, Company, EntityKind, Features, NegativeStockPolicy};
use time::macros::date;

fn wait<R>(ticket: &mut Ticket<R>) -> R {
    let start = Instant::now();
    loop {
        match ticket.poll() {
            Poll::Ready(r) => return r,
            Poll::Pending if start.elapsed() < Duration::from_secs(10) => {
                std::thread::sleep(Duration::from_millis(5));
            }
            other @ (Poll::Pending | Poll::Taken | Poll::Lost) => {
                panic!("expected a result, got {:?}", matches!(other, Poll::Lost))
            }
        }
    }
}

fn api(name: &str) -> Api {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("u1_worker");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.ledgerly"));
    if path.exists() {
        std::fs::remove_file(&path).unwrap();
    }
    let company = Company {
        ineligible_itc_default: None,
        id: CompanyId(1),
        name: "Worker Traders".to_owned(),
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
    };
    Api::create(&path, &company, Clock::Fixed(OffsetDateTime::UNIX_EPOCH)).unwrap()
}

#[test]
fn a_job_runs_on_the_worker_and_its_result_arrives_once() {
    let worker = ApiWorker::spawn(api("once")).unwrap();
    let mut ticket = worker.run(|_api: &mut Api| std::thread::current().name().map(str::to_owned));
    assert_eq!(wait(&mut ticket), Some("ledgerly-api".to_owned()));
    assert_eq!(ticket.poll(), Poll::Taken);
}

#[test]
fn jobs_run_in_submission_order() {
    let worker = ApiWorker::spawn(api("order")).unwrap();
    let (tx, rx) = std::sync::mpsc::channel::<u32>();
    let mut tickets = Vec::new();
    for n in 0..20_u32 {
        let tx = tx.clone();
        tickets.push(worker.run(move |_api: &mut Api| tx.send(n).unwrap()));
    }
    for t in &mut tickets {
        wait(t);
    }
    drop(tx);
    assert_eq!(rx.iter().collect::<Vec<_>>(), (0..20).collect::<Vec<_>>());
}

#[test]
fn a_ticket_from_a_dropped_worker_is_lost_or_ready_never_pending_forever() {
    let worker = ApiWorker::spawn(api("drop")).unwrap();
    let mut ticket = worker.run(|_api: &mut Api| 7_u8);
    // Drop runs the queued jobs before the thread ends, so the result is delivered.
    drop(worker);
    assert_eq!(ticket.poll(), Poll::Ready(7));
}

#[test]
fn a_held_worker_keeps_later_jobs_queued_until_the_hold_is_dropped() {
    // provenance: derived. The capture tool needs a screen to stay "computing"; this is the
    // contract it relies on: nothing queued behind a hold finishes while the hold lives, and
    // everything finishes, in order, once it is dropped.
    let worker = ApiWorker::spawn(api("held")).unwrap();
    let hold = worker.hold();
    let mut first = worker.run(|_api| 1_u8);
    let mut second = worker.run(|_api| 2_u8);
    std::thread::sleep(Duration::from_millis(150));
    assert!(matches!(first.poll(), Poll::Pending), "ran while held");
    assert!(matches!(second.poll(), Poll::Pending), "ran while held");
    drop(hold);
    assert_eq!(wait(&mut first), 1);
    assert_eq!(wait(&mut second), 2);
}
