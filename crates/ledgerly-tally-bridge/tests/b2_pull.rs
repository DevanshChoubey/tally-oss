//! Card B2 tests: batched pull and raw capture (spec 6.7 steps 2 and 3).
//!
//! Provenance. Every fixture is a fictional, hand-built Tally-shaped response served by a fake
//! transport; none was captured from TallyPrime, so nothing here is `observed`. Each expected
//! value is `derived`: hand arithmetic on the fixture's own numbers (days in a window, vouchers
//! per day) or a rule quoted from spec 6.7 ("at most 5,000 vouchers per request", "capturing raw
//! XML to disk first", masters in dependency order). The request envelope shape is R8 section
//! 2.2/2.3; the collection type names inside it are pending an observed capture, so tests only
//! use them to route the fake transport.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::Path;

use ledgerly_core::{CancelToken, Date, Month, Period};
use ledgerly_tally_bridge::pull::{MANIFEST_FILE, PullError, PullPlan, pull, pull_with_plan};
use ledgerly_tally_bridge::{TallyTransport, TransportError};

fn d(y: i32, m: Month, day: u8) -> Date {
    Date::from_calendar_date(y, m, day).unwrap()
}

fn period(from: Date, to: Date) -> Period {
    Period::new(from, to).unwrap()
}

/// (collection type sent in the request, element tag Tally answers with), in the order spec 6.7
/// step 2 requires.
const MASTER_ORDER: [(&str, &str); 10] = [
    ("Group", "GROUP"),
    ("Ledger", "LEDGER"),
    ("Unit", "UNIT"),
    ("StockGroup", "STOCKGROUP"),
    ("StockCategory", "STOCKCATEGORY"),
    ("StockItem", "STOCKITEM"),
    ("Godown", "GODOWN"),
    ("CostCategory", "COSTCATEGORY"),
    ("CostCentre", "COSTCENTRE"),
    ("VoucherType", "VOUCHERTYPE"),
];

fn wrap(inner: &str) -> String {
    format!(
        "<ENVELOPE><HEADER><VERSION>1</VERSION><STATUS>1</STATUS></HEADER><BODY><DATA>\
         <COLLECTION>{inner}</COLLECTION></DATA></BODY></ENVELOPE>"
    )
}

fn voucher_xml(date: Date, index: usize) -> String {
    format!(
        "<VOUCHER VCHTYPE=\"Journal\" ACTION=\"Create\"><GUID>g-{y}{m:02}{dd:02}-{index}</GUID>\
         <ALTERID>{index}</ALTERID><DATE>{y}{m:02}{dd:02}</DATE>\
         <VOUCHERTYPENAME>Journal</VOUCHERTYPENAME><VOUCHERNUMBER>J-{index}</VOUCHERNUMBER>\
         <ALLLEDGERENTRIES.LIST><LEDGERNAME>Cash</LEDGERNAME>\
         <ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE><AMOUNT>-100.00</AMOUNT></ALLLEDGERENTRIES.LIST>\
         <ALLLEDGERENTRIES.LIST><LEDGERNAME>Capital</LEDGERNAME>\
         <ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>100.00</AMOUNT></ALLLEDGERENTRIES.LIST>\
         </VOUCHER>",
        y = date.year(),
        m = u8::from(date.month()),
        dd = date.day(),
    )
}

fn tag_text<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let rest = text.split_once(format!("<{tag}>").as_str())?.1;
    Some(rest.split_once(format!("</{tag}>").as_str())?.0)
}

fn parse_ymd(text: &str) -> Date {
    let y: i32 = text[0..4].parse().unwrap();
    let m: u8 = text[4..6].parse().unwrap();
    let dd: u8 = text[6..8].parse().unwrap();
    d(y, Month::try_from(m).unwrap(), dd)
}

/// A fake Tally: answers master requests with one master each and voucher requests with
/// `per_day` vouchers for every day of the requested window.
struct FakeTally {
    per_day: usize,
    requests: RefCell<Vec<String>>,
    responses: RefCell<Vec<String>>,
    voucher_calls: Cell<usize>,
    /// Cancel this token once the nth voucher request has been answered.
    cancel_after: Option<(usize, CancelToken)>,
    /// Replaces the answer to every voucher request.
    voucher_override: Option<String>,
    /// Replaces the answer to every master request.
    master_override: Option<String>,
}

impl FakeTally {
    fn new(per_day: usize) -> Self {
        FakeTally {
            per_day,
            requests: RefCell::new(Vec::new()),
            responses: RefCell::new(Vec::new()),
            voucher_calls: Cell::new(0),
            cancel_after: None,
            voucher_override: None,
            master_override: None,
        }
    }

    fn voucher_requests(&self) -> Vec<(Date, Date)> {
        self.requests
            .borrow()
            .iter()
            .filter_map(|r| {
                let from = tag_text(r, "SVFROMDATE")?;
                let to = tag_text(r, "SVTODATE")?;
                Some((parse_ymd(from), parse_ymd(to)))
            })
            .collect()
    }

    fn answer_vouchers(&self, from: Date, to: Date) -> String {
        let calls = self.voucher_calls.get() + 1;
        self.voucher_calls.set(calls);
        if let Some((n, token)) = &self.cancel_after
            && calls == *n
        {
            token.cancel();
        }
        if let Some(text) = &self.voucher_override {
            return text.clone();
        }
        let mut body = String::new();
        let mut day = from;
        loop {
            for i in 0..self.per_day {
                body.push_str(&voucher_xml(day, i));
            }
            if day == to {
                break;
            }
            day = day.next_day().unwrap();
        }
        wrap(&body)
    }

    fn answer_masters(&self, xml_request: &str) -> String {
        if let Some(text) = &self.master_override {
            return text.clone();
        }
        let (ty, tag) = MASTER_ORDER
            .iter()
            .find(|(ty, _)| xml_request.contains(&format!("<TYPE>{ty}</TYPE>")))
            .expect("known collection type");
        wrap(&format!(
            "<{tag} NAME=\"n-{ty}\"><GUID>guid-{ty}</GUID><ALTERID>1</ALTERID>\
             <NAME>n-{ty}</NAME></{tag}>"
        ))
    }
}

impl TallyTransport for FakeTally {
    fn post(&self, xml_request: &str) -> Result<String, TransportError> {
        self.requests.borrow_mut().push(xml_request.to_owned());
        let response = match (
            tag_text(xml_request, "SVFROMDATE"),
            tag_text(xml_request, "SVTODATE"),
        ) {
            (Some(from), Some(to)) => self.answer_vouchers(parse_ymd(from), parse_ymd(to)),
            _ => self.answer_masters(xml_request),
        };
        self.responses.borrow_mut().push(response.clone());
        Ok(response)
    }
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn one_day() -> Period {
    let day = d(2026, Month::April, 1);
    period(day, day)
}

#[test]
fn year_is_split_into_windows_of_the_planned_days() {
    // Range 1 April 2026 to 31 March 2027, 1 voucher a day, windows of 100 days.
    // provenance: derived — 365 days (2026-27 has no 29 February) = 3 * 100 + 65. Window ends by
    // calendar count: 1 Apr + 99 days = 9 Jul (30 + 31 + 30 = 91 days to 30 Jun, plus 9);
    // 10 Jul + 99 = 17 Oct (22 + 31 + 30 = 83 to 30 Sep, plus 17); 18 Oct + 99 = 25 Jan
    // (14 + 30 + 31 = 75 to 31 Dec, plus 25); 26 Jan to 31 Mar = 6 + 28 + 31 = 65 days.
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeTally::new(1);
    let range = period(d(2026, Month::April, 1), d(2027, Month::March, 31));
    let plan = PullPlan::new(100, 5_000).unwrap();
    let result =
        pull_with_plan(&fake, "Acme", range, dir.path(), &CancelToken::new(), plan).unwrap();
    let windows: Vec<(Date, Date)> = result
        .batches
        .iter()
        .map(|b| (b.window.from(), b.window.to()))
        .collect();
    assert_eq!(
        windows,
        vec![
            (d(2026, Month::April, 1), d(2026, Month::July, 9)),
            (d(2026, Month::July, 10), d(2026, Month::October, 17)),
            (d(2026, Month::October, 18), d(2027, Month::January, 25)),
            (d(2027, Month::January, 26), d(2027, Month::March, 31)),
        ]
    );
    // derived: 1 voucher a day for 365 days.
    assert_eq!(result.vouchers.len(), 365);
    assert_eq!(
        result.batches.iter().map(|b| b.count).collect::<Vec<_>>(),
        vec![100, 100, 100, 65]
    );
    // One request per window, nothing re-requested.
    assert_eq!(fake.voucher_requests().len(), 4);
}

#[test]
fn default_plan_requests_31_day_windows() {
    // provenance: derived — the default first window is 31 days, so 1 April to 30 June 2026
    // (30 + 31 + 30 = 91 days) is 1 Apr..1 May (1 Apr + 30 days), 2 May..1 Jun (2 May + 30 days),
    // and the remaining 2 Jun..30 Jun (29 days, clamped to the range end).
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeTally::new(0);
    let range = period(d(2026, Month::April, 1), d(2026, Month::June, 30));
    let result = pull(&fake, "Acme", range, dir.path(), &CancelToken::new()).unwrap();
    let windows: Vec<(Date, Date)> = result
        .batches
        .iter()
        .map(|b| (b.window.from(), b.window.to()))
        .collect();
    assert_eq!(
        windows,
        vec![
            (d(2026, Month::April, 1), d(2026, Month::May, 1)),
            (d(2026, Month::May, 2), d(2026, Month::June, 1)),
            (d(2026, Month::June, 2), d(2026, Month::June, 30)),
        ]
    );
    assert_eq!(result.vouchers.len(), 0);
}

#[test]
fn oversized_window_is_split_until_each_accepted_window_is_within_the_limit() {
    // 8 days, 20 vouchers a day, limit 50, first window 4 days.
    // provenance: derived — 1..4 Apr holds 4 * 20 = 80 > 50, so it is split to 2 days:
    // 1..2 Apr = 40 <= 50 accepted; 3..4 Apr = 40; 5..6 Apr = 40; 7..8 Apr = 40 (the window size
    // stays at 2 after a split). Requests: 1 rejected + 4 accepted = 5. Vouchers kept: 8 * 20 =
    // 160, not 160 + 80 (the rejected response contributes none).
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeTally::new(20);
    let range = period(d(2026, Month::April, 1), d(2026, Month::April, 8));
    let plan = PullPlan::new(4, 50).unwrap();
    let result =
        pull_with_plan(&fake, "Acme", range, dir.path(), &CancelToken::new(), plan).unwrap();
    assert_eq!(
        fake.voucher_requests(),
        vec![
            (d(2026, Month::April, 1), d(2026, Month::April, 4)),
            (d(2026, Month::April, 1), d(2026, Month::April, 2)),
            (d(2026, Month::April, 3), d(2026, Month::April, 4)),
            (d(2026, Month::April, 5), d(2026, Month::April, 6)),
            (d(2026, Month::April, 7), d(2026, Month::April, 8)),
        ]
    );
    assert_eq!(result.batches.len(), 4);
    assert!(result.batches.iter().all(|b| b.count <= 50));
    assert_eq!(result.vouchers.len(), 160);
    // The accepted windows tile the range with no gap and no overlap.
    let mut next = range.from();
    for b in &result.batches {
        assert_eq!(b.window.from(), next);
        next = b.window.to().next_day().unwrap_or(b.window.to());
    }
    assert_eq!(result.batches.last().unwrap().window.to(), range.to());
    // The rejected response is kept on disk and marked oversize in the manifest.
    let manifest = fs::read_to_string(dir.path().join(MANIFEST_FILE)).unwrap();
    assert!(
        manifest.contains("oversize\tvouchers-20260401-20260404.xml\t20260401\t20260404\t80\n")
    );
    assert!(dir.path().join("vouchers-20260401-20260404.xml").exists());
}

#[test]
fn a_single_day_over_the_limit_is_an_error_not_a_silent_overrun() {
    // provenance: derived — 60 vouchers on one day with a limit of 50 cannot be split by date.
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeTally::new(60);
    let plan = PullPlan::new(1, 50).unwrap();
    let err = pull_with_plan(
        &fake,
        "Acme",
        one_day(),
        dir.path(),
        &CancelToken::new(),
        plan,
    )
    .unwrap_err();
    assert_eq!(
        err,
        PullError::BatchTooLarge {
            day: d(2026, Month::April, 1),
            count: 60,
            limit: 50,
            file: "vouchers-20260401-20260401.xml".to_owned()
        }
    );
}

#[test]
fn masters_are_requested_in_dependency_order_before_vouchers() {
    // provenance: derived — spec 6.7 step 2: groups, ledgers, units, stock groups/categories,
    // items, godowns, cost categories/centres, voucher types, then vouchers.
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeTally::new(1);
    let result = pull(&fake, "Acme", one_day(), dir.path(), &CancelToken::new()).unwrap();
    let sent: Vec<String> = fake.requests.borrow().clone();
    assert_eq!(sent.len(), 11);
    for (request, (ty, _)) in sent.iter().zip(MASTER_ORDER.iter()) {
        assert!(
            request.contains(&format!("<TYPE>{ty}</TYPE>")),
            "expected {ty} in {request}"
        );
    }
    assert!(sent[10].contains("<TYPE>Voucher</TYPE>"));
    assert!(sent[10].contains("<SVFROMDATE>20260401</SVFROMDATE>"));
    assert_eq!(result.master_files.len(), 10);
    // Each of the 9 modelled collections yields the one master the fixture sends; the stock
    // category element has no master kind in the parser, so it is skipped with a warning that
    // the pull reports (rule A9) instead of dropping it silently.
    assert_eq!(result.masters.len(), 9);
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].file, "masters-05-stock-categories.xml");
    assert_eq!(result.vouchers.len(), 1);
}

#[test]
fn company_name_is_escaped_in_the_request() {
    // provenance: derived — XML 1.0 predefined entities; an unescaped `&` would break the envelope.
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeTally::new(0);
    pull(
        &fake,
        "A&B <Traders>",
        one_day(),
        dir.path(),
        &CancelToken::new(),
    )
    .unwrap();
    let sent: Vec<String> = fake.requests.borrow().clone();
    for request in &sent {
        assert!(
            request.contains("<SVCURRENTCOMPANY>A&amp;B &lt;Traders&gt;</SVCURRENTCOMPANY>"),
            "{request}"
        );
    }
}

#[test]
fn bad_company_names_are_refused_before_any_request() {
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeTally::new(0);
    for name in ["", "   ", "Acme\u{0004}"] {
        let err = pull(&fake, name, one_day(), dir.path(), &CancelToken::new()).unwrap_err();
        assert!(matches!(err, PullError::InvalidCompany { .. }), "{name:?}");
    }
    assert_eq!(fake.requests.borrow().len(), 0);
}

#[test]
fn every_response_is_on_disk_exactly_as_received() {
    // provenance: derived — spec 6.7 step 2: "capturing raw XML to disk first".
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeTally::new(2);
    let range = period(d(2026, Month::April, 1), d(2026, Month::April, 3));
    let plan = PullPlan::new(2, 5_000).unwrap();
    let result =
        pull_with_plan(&fake, "Acme", range, dir.path(), &CancelToken::new(), plan).unwrap();
    let files: Vec<String> = result
        .master_files
        .iter()
        .cloned()
        .chain(result.batches.iter().map(|b| b.file.clone()))
        .collect();
    let responses = fake.responses.borrow();
    assert_eq!(files.len(), responses.len());
    for (file, response) in files.iter().zip(responses.iter()) {
        assert_eq!(
            &fs::read_to_string(dir.path().join(file)).unwrap(),
            response
        );
    }
    // derived: 10 master files + 2 voucher windows (1..2 Apr, 3 Apr) + the manifest; no leftovers.
    let names = names_in(dir.path());
    assert_eq!(names.len(), 13);
    assert!(
        names
            .iter()
            .all(|n| std::path::Path::new(n).extension() != Some(std::ffi::OsStr::new("part")))
    );
    // derived: 2 vouchers a day for 3 days.
    assert_eq!(result.vouchers.len(), 6);
}

#[test]
fn unparseable_voucher_response_is_kept_on_disk_before_the_error() {
    // The truncated document is invalid; the raw text must already be saved when the parser fails.
    let dir = tempfile::tempdir().unwrap();
    let truncated = "<ENVELOPE><BODY><DATA><COLLECTION><VOUCHER><DATE>20260401</DATE>";
    let mut fake = FakeTally::new(0);
    fake.voucher_override = Some(truncated.to_owned());
    let err = pull(&fake, "Acme", one_day(), dir.path(), &CancelToken::new()).unwrap_err();
    assert!(
        matches!(&err, PullError::Vouchers { file, .. } if file == "vouchers-20260401-20260401.xml"),
        "{err:?}"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("vouchers-20260401-20260401.xml")).unwrap(),
        truncated
    );
    // The failed window is not part of the capture: the manifest lists the masters only.
    let manifest = fs::read_to_string(dir.path().join(MANIFEST_FILE)).unwrap();
    assert!(!manifest.contains("vouchers\t"));
}

#[test]
fn unparseable_master_response_is_kept_on_disk_before_the_error() {
    let dir = tempfile::tempdir().unwrap();
    let truncated = "<ENVELOPE><BODY><DATA><COLLECTION><GROUP NAME=\"x\"><GUID>g";
    let mut fake = FakeTally::new(0);
    fake.master_override = Some(truncated.to_owned());
    let err = pull(&fake, "Acme", one_day(), dir.path(), &CancelToken::new()).unwrap_err();
    assert!(
        matches!(&err, PullError::Masters { file, .. } if file == "masters-01-groups.xml"),
        "{err:?}"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("masters-01-groups.xml")).unwrap(),
        truncated
    );
    // Only the failing request was made.
    assert_eq!(fake.requests.borrow().len(), 1);
}

#[test]
fn plain_text_answer_is_not_mistaken_for_an_empty_company() {
    // An empty voucher list parses to zero vouchers, so a non-XML answer must be caught first.
    let dir = tempfile::tempdir().unwrap();
    let mut fake = FakeTally::new(0);
    fake.voucher_override = Some("Unknown Request, cannot be processed".to_owned());
    let err = pull(&fake, "Acme", one_day(), dir.path(), &CancelToken::new()).unwrap_err();
    assert_eq!(
        err,
        PullError::NotAnEnvelope {
            file: "vouchers-20260401-20260401.xml".to_owned()
        }
    );
}

#[test]
fn status_zero_is_a_refusal_with_tallys_message() {
    // provenance: derived — R8 section 2.2: the response header has STATUS, 1 success, 0 failure.
    let dir = tempfile::tempdir().unwrap();
    let mut fake = FakeTally::new(0);
    fake.master_override = Some(
        "<ENVELOPE><HEADER><VERSION>1</VERSION><STATUS>0</STATUS></HEADER><BODY>\
         <LINEERROR>Company 'Acme' is not loaded</LINEERROR></BODY></ENVELOPE>"
            .to_owned(),
    );
    let err = pull(&fake, "Acme", one_day(), dir.path(), &CancelToken::new()).unwrap_err();
    assert_eq!(
        err,
        PullError::Refused {
            file: "masters-01-groups.xml".to_owned(),
            message: "Company 'Acme' is not loaded".to_owned()
        }
    );
}

#[test]
fn voucher_outside_the_requested_window_is_rejected() {
    // provenance: derived — a voucher dated 15 May in a response for 1..1 April cannot be trusted.
    let dir = tempfile::tempdir().unwrap();
    let mut fake = FakeTally::new(0);
    fake.voucher_override = Some(wrap(&voucher_xml(d(2026, Month::May, 15), 7)));
    let err = pull(&fake, "Acme", one_day(), dir.path(), &CancelToken::new()).unwrap_err();
    assert_eq!(
        err,
        PullError::VoucherOutsideWindow {
            file: "vouchers-20260401-20260401.xml".to_owned(),
            number: "J-7".to_owned(),
            date: d(2026, Month::May, 15),
            from: d(2026, Month::April, 1),
            to: d(2026, Month::April, 1),
        }
    );
}

#[test]
fn transport_errors_pass_through_typed() {
    struct Down;
    impl TallyTransport for Down {
        fn post(&self, _xml_request: &str) -> Result<String, TransportError> {
            Err(TransportError::TallyNotRunning { port: 9000 })
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let err = pull(&Down, "Acme", one_day(), dir.path(), &CancelToken::new()).unwrap_err();
    assert_eq!(
        err,
        PullError::Transport(TransportError::TallyNotRunning { port: 9000 })
    );
}

#[test]
fn cancel_between_batches_leaves_a_consistent_capture() {
    // Cancel is raised while the 2nd voucher request is answered. 10 days, 1 voucher a day,
    // windows of 3 days: the 3rd window must never be requested.
    // provenance: derived — window 1 = 1..3 Apr, window 2 = 4..6 Apr; cancel is seen at the
    // checkpoint before window 3, so 2 windows and 10 master collections are complete.
    let dir = tempfile::tempdir().unwrap();
    let cancel = CancelToken::new();
    let mut fake = FakeTally::new(1);
    fake.cancel_after = Some((2, cancel.clone()));
    let range = period(d(2026, Month::April, 1), d(2026, Month::April, 10));
    let plan = PullPlan::new(3, 5_000).unwrap();
    let err = pull_with_plan(&fake, "Acme", range, dir.path(), &cancel, plan).unwrap_err();
    assert_eq!(
        err,
        PullError::Cancelled {
            master_files_done: 10,
            batches_done: 2
        }
    );
    assert_eq!(fake.voucher_requests().len(), 2);
    // Manifest: header, 10 master lines (1 master each; the stock-category element is skipped so
    // it counts 0), 2 voucher lines (3 days * 1 voucher).
    let manifest = fs::read_to_string(dir.path().join(MANIFEST_FILE)).unwrap();
    let lines: Vec<&str> = manifest.lines().collect();
    assert_eq!(lines.len(), 13);
    assert_eq!(lines[0], "ledgerly-capture-manifest-v1");
    assert_eq!(lines[1], "masters\tmasters-01-groups.xml\t1");
    assert_eq!(lines[5], "masters\tmasters-05-stock-categories.xml\t0");
    assert_eq!(
        lines[11],
        "vouchers\tvouchers-20260401-20260403.xml\t20260401\t20260403\t3"
    );
    assert_eq!(
        lines[12],
        "vouchers\tvouchers-20260404-20260406.xml\t20260404\t20260406\t3"
    );
    // Every manifest file is on disk and complete; nothing else is, and no .part is left.
    let mut expected: Vec<String> = lines[1..]
        .iter()
        .map(|l| l.split('\t').nth(1).unwrap().to_owned())
        .collect();
    expected.push(MANIFEST_FILE.to_owned());
    expected.sort();
    assert_eq!(names_in(dir.path()), expected);
    let responses = fake.responses.borrow();
    assert_eq!(responses.len(), 12);
    for (line, response) in lines[1..].iter().zip(responses.iter()) {
        let file = line.split('\t').nth(1).unwrap();
        assert_eq!(
            &fs::read_to_string(dir.path().join(file)).unwrap(),
            response
        );
    }
}

#[test]
fn cancel_before_the_first_request_sends_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let cancel = CancelToken::new();
    cancel.cancel();
    let fake = FakeTally::new(1);
    let err = pull(&fake, "Acme", one_day(), dir.path(), &cancel).unwrap_err();
    assert_eq!(
        err,
        PullError::Cancelled {
            master_files_done: 0,
            batches_done: 0
        }
    );
    assert_eq!(fake.requests.borrow().len(), 0);
    assert_eq!(
        fs::read_to_string(dir.path().join(MANIFEST_FILE)).unwrap(),
        "ledgerly-capture-manifest-v1\n"
    );
}

#[test]
fn a_new_pull_replaces_an_old_manifest() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(MANIFEST_FILE), "stale\n").unwrap();
    let cancel = CancelToken::new();
    cancel.cancel();
    let fake = FakeTally::new(0);
    let err = pull(&fake, "Acme", one_day(), dir.path(), &cancel).unwrap_err();
    assert!(matches!(err, PullError::Cancelled { .. }));
    let manifest = fs::read_to_string(dir.path().join(MANIFEST_FILE)).unwrap();
    assert!(!manifest.contains("stale"));
}
