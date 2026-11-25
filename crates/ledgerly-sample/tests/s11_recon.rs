//! The 2B reconciliation on the sample company. Expected values are the mock's own
//! (`design/mock-v2/index.html`: `G2B`, `OCT_2B`, `recon()`, evaluated by hand) or follow from the
//! rules written next to each test; none is copied from engine output. The 2B file shape is
//! `pending observed` (see `design/PARITY-GAPS.md`).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests stop on any failure"
)]

use ledgerly_api::{Api, ApiError, ImsAction, ReconState};
use ledgerly_core::{Clock, Gstin, Outcome, Paise, Period};
use time::OffsetDateTime;
use time::macros::date;

const DIR: &str = "/media/devansh/Experiment/playground/claude-tmp";

fn company(with_2b: bool) -> (tempfile::TempDir, std::path::PathBuf, Api) {
    let dir = tempfile::tempdir_in(DIR).unwrap();
    let path = dir.path().join("sample.ledgerly");
    let clock = Clock::Fixed(OffsetDateTime::UNIX_EPOCH);
    let api = if with_2b {
        ledgerly_sample::create(&path, clock).unwrap()
    } else {
        ledgerly_sample::create_without_2b(&path, clock).unwrap()
    };
    (dir, path, api)
}

fn gstin() -> Gstin {
    Gstin::parse("27ZZZZZ0000Z1ZQ").unwrap()
}

fn november() -> Period {
    Period::new(date!(2026 - 11 - 01), date!(2026 - 11 - 30)).unwrap()
}

fn october() -> Period {
    Period::new(date!(2026 - 10 - 01), date!(2026 - 10 - 31)).unwrap()
}

fn value<T>(o: Outcome<T>) -> T {
    match o {
        Outcome::Value(v) => v,
        Outcome::Unsupported(u) => panic!("unsupported: {u:?}"),
    }
}

fn rupees(n: i64) -> Paise {
    Paise::new(n * 100)
}

#[test]
fn november_rows_are_the_mocks_four() {
    let (_d, _p, api) = company(true);
    let rec = value(api.gstr2b_reconciliation(&gstin(), november()).unwrap());
    assert_eq!(rec.file_name.as_deref(), Some("gstr2b-nov-2026.json"));
    assert_eq!(rec.filed_on, None);
    let rows: Vec<_> = rec
        .rows
        .iter()
        .map(|r| (r.supplier_name.as_str(), r.doc_number.as_str(), r.state))
        .collect();
    // provenance: derived from the mock's recon(): G1 Kaveri KP/2231 matches PB/0141 (64,000 +
    // 11,520, 8 Nov); G2 Nilgiri NST/1145 differs; G3 Vardhaman VL/889 and G4 Rupa RP/5521 have
    // no purchase in the books.
    assert_eq!(
        rows,
        vec![
            ("Kaveri Packaging Pvt Ltd", "KP/2231", ReconState::Matched),
            (
                "Nilgiri Steel Traders",
                "NST/1145",
                ReconState::ValueDiffers
            ),
            ("Vardhaman Logistics", "VL/889", ReconState::NotInBooks),
            ("Rupa Polymers", "RP/5521", ReconState::NotInBooks),
        ]
    );
    // provenance: derived from the mock's G2B and the seed: Nilgiri 2B 1,02,500 + 18,450 =
    // 1,20,950; books 1,00,000 + 18,000 = 1,18,000; difference 2,950.
    let nilgiri = &rec.rows[1];
    assert_eq!(nilgiri.portal.total, Some(rupees(120_950)));
    assert_eq!(nilgiri.books.unwrap().total, Some(rupees(118_000)));
    assert_eq!(nilgiri.difference, Some(rupees(2_950)));
    // provenance: derived from the mock's G3, G4: 12,000 + 1,080 + 1,080 and 68,000 + 12,240.
    assert_eq!(rec.rows[2].portal.total, Some(rupees(14_160)));
    assert_eq!(rec.rows[3].portal.total, Some(rupees(80_240)));
    assert!(rec.rows[2].books.is_none() && rec.rows[3].difference.is_none());
    // provenance: the mock's G1 carries action Accept and the others none.
    assert_eq!(rec.rows[0].action, Some(ImsAction::Accept));
    assert_eq!(rec.rows[1].action, None);
}

#[test]
fn a_recorded_action_survives_closing_and_reopening_the_file() {
    let (_d, path, mut api) = company(true);
    let nilgiri = "29ZZZZZ0002Z1ZK";
    value(
        api.set_ims_action(
            &gstin(),
            november(),
            nilgiri,
            "NST/1145",
            ImsAction::Pending,
        )
        .unwrap(),
    );
    drop(api);
    let api = Api::open(&path).unwrap();
    let rec = value(api.gstr2b_reconciliation(&gstin(), november()).unwrap());
    assert_eq!(rec.rows[1].action, Some(ImsAction::Pending));
    // The last action in force wins.
    let mut api = api;
    value(
        api.set_ims_action(&gstin(), november(), nilgiri, "NST/1145", ImsAction::Reject)
            .unwrap(),
    );
    let rec = value(api.gstr2b_reconciliation(&gstin(), november()).unwrap());
    assert_eq!(rec.rows[1].action, Some(ImsAction::Reject));
}

#[test]
fn october_is_final_after_the_3b_was_filed_on_20_nov() {
    let (_d, _p, mut api) = company(true);
    let rec = value(api.gstr2b_reconciliation(&gstin(), october()).unwrap());
    // provenance: the mock's OCT_2B and its recon-locked state: KP/2205 47,200 Accept and
    // VL/858 14,160 Reject, 3B filed on 20 Nov.
    assert_eq!(rec.filed_on, Some(date!(2026 - 11 - 20)));
    assert_eq!(rec.rows.len(), 2);
    assert_eq!(rec.rows[0].doc_number, "KP/2205");
    assert_eq!(rec.rows[0].portal.total, Some(rupees(47_200)));
    assert_eq!(rec.rows[0].action, Some(ImsAction::Accept));
    assert_eq!(rec.rows[1].doc_number, "VL/858");
    assert_eq!(rec.rows[1].portal.total, Some(rupees(14_160)));
    assert_eq!(rec.rows[1].action, Some(ImsAction::Reject));
    // The October bills are in the books, so both match.
    assert_eq!(rec.rows[0].state, ReconState::Matched);
    assert_eq!(rec.rows[1].state, ReconState::Matched);
    // Final: no action and no new file once filed.
    let kaveri = "29ZZZZZ0001Z1ZL";
    let refused = api.set_ims_action(&gstin(), october(), kaveri, "KP/2205", ImsAction::Reject);
    assert!(matches!(refused, Err(ApiError::GstrPeriodFiled { .. })));
    let refused = api.import_gstr2b(
        &gstin(),
        october(),
        "x.json",
        ledgerly_sample::GSTR2B_OCTOBER,
    );
    assert!(matches!(refused, Err(ApiError::GstrPeriodFiled { .. })));
    let again = api.mark_gstr3b_filed(&gstin(), october(), date!(2026 - 11 - 21));
    assert!(matches!(again, Err(ApiError::GstrPeriodFiled { .. })));
}

#[test]
fn without_a_file_there_are_no_rows() {
    let (_d, _p, api) = company(false);
    let rec = value(api.gstr2b_reconciliation(&gstin(), november()).unwrap());
    assert_eq!(rec.file_name, None);
    assert!(rec.rows.is_empty());
    assert_eq!(rec.filed_on, None);
}

#[test]
fn a_file_for_another_gstin_is_refused_and_nothing_is_kept() {
    let (_d, _p, mut api) = company(false);
    // The mock's error state: this 2B file is for GSTIN X, not Y.
    let other = ledgerly_sample::GSTR2B_NOVEMBER.replace("27ZZZZZ0000Z1ZQ", "27ZZZZZ0005Z1ZL");
    let refused = api.import_gstr2b(&gstin(), november(), "other.json", &other);
    match refused {
        Err(ApiError::Gstr2bWrongGstin { file, company }) => {
            assert_eq!(file, "27ZZZZZ0005Z1ZL");
            assert_eq!(company, "27ZZZZZ0000Z1ZQ");
        }
        other => panic!("expected the wrong-GSTIN refusal, got {other:?}"),
    }
    let rec = value(api.gstr2b_reconciliation(&gstin(), november()).unwrap());
    assert_eq!(rec.file_name, None);
}

#[test]
fn a_file_for_another_month_is_refused() {
    let (_d, _p, mut api) = company(false);
    let refused = api.import_gstr2b(
        &gstin(),
        november(),
        "oct.json",
        ledgerly_sample::GSTR2B_OCTOBER,
    );
    assert!(matches!(refused, Err(ApiError::Gstr2bWrongPeriod { .. })));
}

#[test]
fn an_action_needs_a_document_of_the_file() {
    let (_d, _p, mut api) = company(true);
    let refused = api.set_ims_action(
        &gstin(),
        november(),
        "29ZZZZZ0001Z1ZL",
        "KP/9999",
        ImsAction::Accept,
    );
    assert!(matches!(
        refused,
        Err(ApiError::ImsDocumentNotInFile { .. })
    ));
    let (_d2, _p2, mut bare) = company(false);
    let refused = bare.set_ims_action(
        &gstin(),
        november(),
        "29ZZZZZ0001Z1ZL",
        "KP/2231",
        ImsAction::Accept,
    );
    assert!(matches!(refused, Err(ApiError::Gstr2bNotImported { .. })));
}

#[test]
fn filing_a_period_locks_its_actions_and_survives_reopening() {
    let (_d, path, mut api) = company(true);
    value(
        api.mark_gstr3b_filed(&gstin(), november(), date!(2026 - 11 - 21))
            .unwrap(),
    );
    let refused = api.set_ims_action(
        &gstin(),
        november(),
        "29ZZZZZ0002Z1ZK",
        "NST/1145",
        ImsAction::Accept,
    );
    assert!(matches!(refused, Err(ApiError::GstrPeriodFiled { .. })));
    drop(api);
    let api = Api::open(&path).unwrap();
    let rec = value(api.gstr2b_reconciliation(&gstin(), november()).unwrap());
    assert_eq!(rec.filed_on, Some(date!(2026 - 11 - 21)));
    assert!(api.verify().unwrap().is_ok());
}
