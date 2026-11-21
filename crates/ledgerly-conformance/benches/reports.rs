//! Benchmarks for the performance targets in spec section 8 (card H3).
//!
//! The company is the synthetic one from `synth`: 1,000,000 vouchers (2,000,000 postings) in
//! financial year 2025-26. The size comes from, in order:
//!
//! 1. the environment variable `LEDGERLY_BENCH_VOUCHERS`, when set;
//! 2. 1,000,000 when cargo runs the target as a benchmark (`cargo bench`, which passes `--bench`);
//! 3. 1,000 otherwise, so that `cargo test --all-targets` runs the file as a quick smoke test.
//!
//! What is measured, against the spec section 8 targets:
//!
//! | Benchmark | Target |
//! |---|---|
//! | `reports_full_year/trial_balance_detailed` and `_condensed` | < 300 ms |
//! | `reports_full_year/balance_sheet` | < 300 ms |
//! | `reports_full_year/profit_loss` | < 300 ms |
//! | `voucher_save/voucher_validate` (model invariants only) | part of the < 30 ms save |
//! | `voucher_save/voucher_post_unnumbered` and `_auto_numbered` | < 30 ms (p95) |
//!
//! Criterion reports a distribution; it does not fail a run. The CI benchmark job compares each
//! result with the previous one and fails on a regression over 10% (engineering rule D9).
//!
//! Not measured here, because the code does not exist yet: drill-down under 100 ms (needs the
//! group and ledger reports), type-ahead over 50k ledgers (card U-series), Tally import speed
//! (card X-series) and cold start (app). The memory target is only approximated: the resident
//! size of the process after the books are built is printed on Linux.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stderr,
    reason = "benchmark harness: rule B2 lets benches unwrap, and the run prints its setup"
)]

mod synth;

use std::hint::black_box;
use std::time::{Duration, Instant};

use criterion::{BatchSize, Criterion};
use ledgerly_core::Outcome;
use ledgerly_engine::vouchers::post_voucher;
use ledgerly_model::Books;
use ledgerly_model::validate::validate_voucher;
use ledgerly_reports::balance_sheet::balance_sheet;
use ledgerly_reports::profit_loss::profit_loss;
use ledgerly_reports::trial_balance::trial_balance;
use ledgerly_reports::{Detail, ReportKind};
use ledgerly_stock::StockValuation;
use synth::{Shape, fresh_voucher, report_params, synth_books};

/// Spec section 8: full-year reports at 1M vouchers.
const REPORT_TARGET: Duration = Duration::from_millis(300);
/// Vouchers in a full-size run.
const FULL_VOUCHERS: u64 = 1_000_000;
/// Vouchers in a smoke run.
const SMOKE_VOUCHERS: u64 = 1_000;

/// How many vouchers to generate (see the module doc).
fn voucher_count() -> u64 {
    if let Some(n) = std::env::var("LEDGERLY_BENCH_VOUCHERS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|n| *n > 0)
    {
        return n;
    }
    if std::env::args().any(|a| a == "--bench") {
        FULL_VOUCHERS
    } else {
        SMOKE_VOUCHERS
    }
}

/// The shape for `vouchers`: the spec company when full size, a small one otherwise.
fn shape_for(vouchers: u64) -> Shape {
    if vouchers >= FULL_VOUCHERS {
        Shape {
            vouchers,
            ..Shape::spec_target()
        }
    } else {
        Shape::small(vouchers)
    }
}

/// Resident set size of this process in MiB, on Linux. `None` elsewhere or when unreadable.
fn resident_mib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmRSS:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib / 1024)
}

/// Runs `f` once and prints how long it took, next to `target` when there is one.
fn timed_once<T>(name: &str, target: Option<Duration>, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let out = f();
    let took = start.elapsed();
    match target {
        Some(t) if took <= t => eprintln!("{name}: {took:?} (within the {t:?} target)"),
        Some(t) => eprintln!("{name}: {took:?} (OVER the {t:?} target)"),
        None => eprintln!("{name}: {took:?}"),
    }
    out
}

fn no_stock() -> Outcome<Vec<StockValuation>> {
    Outcome::Value(Vec::new())
}

/// Benchmarks the trial balance and the profit and loss over one full year.
fn bench_reports(c: &mut Criterion, books: &Books) {
    let stock = no_stock();
    let tb_detailed = report_params(ReportKind::TrialBalance, Detail::Detailed).unwrap();
    let tb_condensed = report_params(ReportKind::TrialBalance, Detail::Condensed).unwrap();
    let bs = report_params(ReportKind::BalanceSheet, Detail::Detailed).unwrap();
    let pl = report_params(ReportKind::ProfitAndLoss, Detail::Detailed).unwrap();

    // One timed run each, so the job log shows the figures against the target.
    timed_once("trial balance (detailed)", Some(REPORT_TARGET), || {
        trial_balance(books, &tb_detailed).unwrap()
    });
    timed_once("balance sheet", Some(REPORT_TARGET), || {
        balance_sheet(books, &bs, &stock).unwrap()
    });
    timed_once("profit and loss", Some(REPORT_TARGET), || {
        profit_loss(books, &pl, &stock, &stock).unwrap()
    });

    let mut group = c.benchmark_group("reports_full_year");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(10));
    group.bench_function("trial_balance_detailed", |b| {
        b.iter(|| trial_balance(black_box(books), black_box(&tb_detailed)).unwrap());
    });
    group.bench_function("trial_balance_condensed", |b| {
        b.iter(|| trial_balance(black_box(books), black_box(&tb_condensed)).unwrap());
    });
    group.bench_function("balance_sheet", |b| {
        b.iter(|| balance_sheet(black_box(books), black_box(&bs), &stock).unwrap());
    });
    group.bench_function("profit_loss", |b| {
        b.iter(|| profit_loss(black_box(books), black_box(&pl), &stock, &stock).unwrap());
    });
    group.finish();
}

/// Benchmarks the voucher save path: model validation, and the engine's post command.
fn bench_voucher_save(c: &mut Criterion, books: &Books, shape: Shape) {
    let journal = fresh_voucher(shape, false).unwrap();
    let receipt = fresh_voucher(shape, true).unwrap();

    let mut group = c.benchmark_group("voucher_save");
    group.sample_size(20);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(10));
    group.bench_function("voucher_validate", |b| {
        b.iter(|| {
            validate_voucher(black_box(&receipt), &books.masters, &books.company).unwrap();
        });
    });
    group.bench_function("voucher_post_unnumbered", |b| {
        b.iter_batched(
            || journal.clone(),
            |v| post_voucher(v, black_box(books)).unwrap(),
            BatchSize::SmallInput,
        );
    });
    // Allocating the next number scans the type's vouchers; this shows what that costs.
    group.bench_function("voucher_post_auto_numbered", |b| {
        b.iter_batched(
            || receipt.clone(),
            |v| post_voucher(v, black_box(books)).unwrap(),
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn main() {
    let vouchers = voucher_count();
    let shape = shape_for(vouchers);
    let before = resident_mib();
    let books = timed_once("generate books", None, || synth_books(shape).unwrap());
    eprintln!(
        "books: {} vouchers, {} postings, {} ledgers",
        books.vouchers.len(),
        books.postings.len(),
        books.masters.ledgers.len()
    );
    if let (Some(before), Some(after)) = (before, resident_mib()) {
        eprintln!(
            "resident memory: {after} MiB after generating, {} MiB for the books (spec section 8: \
             under 500 MiB for the whole app)",
            after.saturating_sub(before)
        );
    }

    let mut c = Criterion::default().configure_from_args();
    bench_reports(&mut c, &books);
    bench_voucher_save(&mut c, &books, shape);
    c.final_summary();
}
