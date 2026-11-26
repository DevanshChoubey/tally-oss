//! s11: the 2B and IMS tab reads the engine's reconciliation, keeps IMS actions in the company
//! file, and shows October final. Expected values are the mock's own (`design/mock-v2/index.html`:
//! `G2B`, `OCT_2B`, `recon()`), worked by hand; none is copied from engine output. The 2B file
//! shape is `pending observed` (design/PARITY-GAPS.md).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::time::{Duration, Instant};

use ledgerly_api::Api;
use ledgerly_app::screens::gst::{ActionError, GstCmd, GstScreen, GstTab, ImsAction, MatchState};
use ledgerly_app::ui::dialogs::script_open_file;
use ledgerly_app::worker::ApiWorker;
use ledgerly_core::{Clock, Gstin, Paise, Period};
use time::OffsetDateTime;
use time::macros::date;

fn gstin() -> Gstin {
    Gstin::parse("27ZZZZZ0000Z1ZQ").unwrap()
}

fn november() -> Period {
    Period::new(date!(2026 - 11 - 01), date!(2026 - 11 - 30)).unwrap()
}

fn october() -> Period {
    Period::new(date!(2026 - 10 - 01), date!(2026 - 10 - 31)).unwrap()
}

fn rupees(n: i64) -> Paise {
    Paise::new(n * 100)
}

struct Fixture {
    dir: tempfile::TempDir,
    path: std::path::PathBuf,
    worker: ApiWorker,
}

fn open(with_2b: bool) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sample.ledgerly");
    let clock = Clock::Fixed(OffsetDateTime::UNIX_EPOCH);
    let api = if with_2b {
        ledgerly_sample::create(&path, clock).unwrap()
    } else {
        ledgerly_sample::create_without_2b(&path, clock).unwrap()
    };
    Fixture {
        dir,
        path,
        worker: ApiWorker::spawn(api).unwrap(),
    }
}

fn settle(s: &mut GstScreen, f: &Fixture) {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        s.tick(Some(&f.worker));
        if !s.is_loading() && !s.has_pending_changes() {
            return;
        }
        assert!(Instant::now() < until, "the worker did not finish");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn screen(f: &Fixture, label: &str, period: Period) -> GstScreen {
    let mut s = GstScreen::new(label, Some(gstin()));
    s.switch_tab(GstTab::Reconciliation);
    s.request_load(gstin(), period);
    settle(&mut s, f);
    s
}

#[test]
fn november_shows_the_mocks_four_rows_from_the_engine() {
    let f = open(true);
    let s = screen(&f, "November 2026", november());
    let rows = s.recon_rows();
    // provenance: the mock's recon(): G1 matches PB/0141, G2 value differs, G3 and G4 are not in
    // the books. G2: 2B 1,02,500 + 18,450 = 1,20,950; books 1,00,000 + 18,000 = 1,18,000; the
    // difference is 2,950. G1 carries the action Accept in the mock.
    let seen: Vec<_> = rows
        .iter()
        .map(|r| (r.invoice_no.as_str(), r.state))
        .collect();
    assert_eq!(
        seen,
        vec![
            ("KP/2231", MatchState::Matched),
            ("NST/1145", MatchState::ValueDiffers),
            ("VL/889", MatchState::NotInBooks),
            ("RP/5521", MatchState::NotInBooks),
        ]
    );
    assert_eq!(rows[1].total_2b, Some(rupees(120_950)));
    assert_eq!(rows[1].total_books, Some(rupees(118_000)));
    assert_eq!(rows[1].difference, Some(rupees(2_950)));
    assert_eq!(rows[2].total_books, None);
    assert_eq!(s.decision(0), Some(ImsAction::Accept));
    assert_eq!(s.decision(1), None);
    assert_eq!(s.recon_file(), Some("gstr2b-nov-2026.json"));
    assert_eq!(s.filed_on(), None);
}

#[test]
fn a_chosen_action_is_kept_in_the_company_file() {
    let f = open(true);
    let mut s = screen(&f, "November 2026", november());
    s.apply(GstCmd::Next);
    s.apply(GstCmd::Decide(ImsAction::Reject));
    settle(&mut s, &f);
    assert!(
        s.notice().unwrap().contains("company file"),
        "{:?}",
        s.notice()
    );
    // A second connection to the file sees the action: it is in the log, not only on screen.
    let other = Api::open(&f.path).unwrap();
    let rec = other.gstr2b_reconciliation(&gstin(), november()).unwrap();
    let ledgerly_core::Outcome::Value(rec) = rec else {
        panic!("unsupported");
    };
    assert_eq!(rec.rows[1].action, Some(ledgerly_api::ImsAction::Reject));
    drop(f.dir);
}

#[test]
fn october_is_final_and_takes_no_action() {
    let f = open(true);
    let mut s = screen(&f, "October 2026", october());
    // provenance: the mock's recon-locked: October's two records, 3B filed on 20 Nov 2026.
    assert_eq!(s.recon_rows().len(), 2);
    assert_eq!(s.recon_rows()[0].total_2b, Some(rupees(47_200)));
    assert_eq!(s.recon_rows()[1].total_2b, Some(rupees(14_160)));
    assert_eq!(s.decision(0), Some(ImsAction::Accept));
    assert_eq!(s.decision(1), Some(ImsAction::Reject));
    assert_eq!(s.filed_on(), Some(date!(2026 - 11 - 20)));
    let refused = s.decide(0, ImsAction::Pending);
    assert!(
        matches!(refused, Err(ActionError::Final { .. })),
        "{refused:?}"
    );
    assert_eq!(s.decision(0), Some(ImsAction::Accept));
}

#[test]
fn a_file_for_another_gstin_shows_the_refusal_and_keeps_nothing() {
    let f = open(false);
    let mut s = screen(&f, "November 2026", november());
    assert_eq!(s.recon_file(), None);
    assert_eq!(s.recon_rows().len(), 0);
    // provenance: the mock's recon-error: "Not imported: this 2B file is for GSTIN X, not Y."
    let wrong = ledgerly_sample::GSTR2B_NOVEMBER.replace("27ZZZZZ0000Z1ZQ", "27ZZZZZ0005Z1ZL");
    let file = f.dir.path().join("other.json");
    std::fs::write(&file, wrong).unwrap();
    script_open_file(file);
    s.apply(GstCmd::ImportFile);
    settle(&mut s, &f);
    assert_eq!(
        s.import_error(),
        Some("this 2B file is for GSTIN 27ZZZZZ0005Z1ZL, not 27ZZZZZ0000Z1ZQ")
    );
    assert_eq!(s.recon_file(), None);
}

#[test]
fn a_good_file_is_imported_and_its_rows_appear() {
    let f = open(false);
    let mut s = screen(&f, "November 2026", november());
    let file = f.dir.path().join("gstr2b.json");
    std::fs::write(&file, ledgerly_sample::GSTR2B_NOVEMBER).unwrap();
    script_open_file(file);
    s.apply(GstCmd::ImportFile);
    settle(&mut s, &f);
    assert_eq!(s.import_error(), None);
    assert_eq!(s.recon_file(), Some("gstr2b.json"));
    assert_eq!(s.recon_rows().len(), 4);
}
