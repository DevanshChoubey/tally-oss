//! `ledgerly` command line (card W2): import from TallyPrime, verify the audit chain, export a
//! GST return, rebuild check, and the golden conformance cases with the ratchet.
//!
//! Exit codes: `0` when the command ran and its check passed; `1` when a check failed (a broken
//! audit chain, projections that differ from the rebuild, a ratchet violation); `2` when the
//! command could not run (bad arguments, a missing file, a store or import error, or input this
//! build does not compute yet). A figure is printed only when the engine computed it.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "the command line prints its results and its errors"
)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, anyhow, bail};
use clap::{Args, Parser, Subcommand};
use ledgerly_api::{Api, RebuildOutcome, verify_file};
use ledgerly_conformance::runner::{CaseStatus, check_ratchet, run_all};
use ledgerly_core::{Date, Gstin, Outcome, Period};

/// Exit code of a check that ran and failed.
const EXIT_CHECK_FAILED: u8 = 1;
/// Exit code of a command that could not run.
const EXIT_COULD_NOT_RUN: u8 = 2;

/// The command line.
#[derive(Debug, Parser)]
#[command(
    name = "ledgerly",
    version,
    about = "Ledgerly command line: import, verify, export, conformance.",
    after_help = "Exit codes: 0 passed, 1 a check failed, 2 the command could not run."
)]
struct Cli {
    /// The command to run.
    #[command(subcommand)]
    command: Cmd,
}

/// The subcommands.
#[derive(Debug, Subcommand)]
enum Cmd {
    /// Import a company from a running TallyPrime (HTTP XML port) into a company file.
    ImportTally(ImportTallyArgs),
    /// Verify the audit chain and checkpoints of a company file. Exit 1 when the chain is broken.
    Verify {
        /// The company file.
        file: PathBuf,
    },
    /// Export a return from a company file.
    Export {
        /// What to export.
        #[command(subcommand)]
        what: ExportCmd,
    },
    /// Replay the event log and compare it with the stored projection tables.
    RebuildCheck {
        /// The company file.
        file: PathBuf,
    },
    /// Run the golden conformance cases and check the pass-list ratchet (rule A6).
    Conformance(ConformanceArgs),
    /// Serve the MCP server over stdio for an AI client (spec 6.8). Read queries and
    /// `propose_voucher` only; nothing is posted.
    Mcp(McpArgs),
}

/// Arguments of `mcp`.
#[derive(Debug, Args)]
struct McpArgs {
    /// The company file to serve. It must exist; the command never creates one.
    #[arg(long)]
    company: PathBuf,
}

/// Arguments of `import-tally`.
#[derive(Debug, Args)]
struct ImportTallyArgs {
    /// The company file to import into.
    file: PathBuf,
    /// Company name as TallyPrime shows it.
    #[arg(long)]
    company: String,
    /// Host of the running TallyPrime.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    /// HTTP XML port of the running TallyPrime.
    #[arg(long, default_value_t = 9000)]
    port: u16,
}

/// What `export` can write.
#[derive(Debug, Subcommand)]
enum ExportCmd {
    /// GSTR-1 for one period, as JSON.
    Gstr1(Gstr1Args),
}

/// Arguments of `export gstr1`.
#[derive(Debug, Args)]
struct Gstr1Args {
    /// The company file.
    file: PathBuf,
    /// GSTIN of the taxpayer.
    #[arg(long)]
    gstin: String,
    /// First day of the period (YYYY-MM-DD).
    #[arg(long)]
    from: String,
    /// Last day of the period (YYYY-MM-DD).
    #[arg(long)]
    to: String,
    /// Write the JSON to this file (atomically) instead of standard output.
    #[arg(long)]
    out: Option<PathBuf>,
}

/// Arguments of `conformance`.
#[derive(Debug, Args)]
struct ConformanceArgs {
    /// Directory of case files (`*.toml`).
    #[arg(long, default_value = "crates/ledgerly-conformance/cases")]
    cases: PathBuf,
    /// The pass-list (one passing case id per line).
    #[arg(long, default_value = "crates/ledgerly-conformance/pass-list.txt")]
    pass_list: PathBuf,
}

fn main() -> ExitCode {
    init_tracing();
    let cli = Cli::parse();
    match run(&cli.command) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("ledgerly: {e:#}");
            ExitCode::from(EXIT_COULD_NOT_RUN)
        }
    }
}

/// Logs go to standard error, filtered by `RUST_LOG` (default: warnings only).
fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

/// Runs one command and returns its exit code.
///
/// # Errors
/// Any failure that stops the command from running (reported by `main` with exit code 2).
fn run(cmd: &Cmd) -> anyhow::Result<ExitCode> {
    match cmd {
        Cmd::ImportTally(args) => {
            let _span = tracing::info_span!("command", name = "import-tally").entered();
            import_tally(args)
        }
        Cmd::Verify { file } => {
            let _span = tracing::info_span!("command", name = "verify").entered();
            verify(file)
        }
        Cmd::Export { what } => {
            let _span = tracing::info_span!("command", name = "export").entered();
            match what {
                ExportCmd::Gstr1(args) => export_gstr1(args),
            }
        }
        Cmd::RebuildCheck { file } => {
            let _span = tracing::info_span!("command", name = "rebuild-check").entered();
            rebuild_check(file)
        }
        Cmd::Conformance(args) => {
            let _span = tracing::info_span!("command", name = "conformance").entered();
            conformance(args)
        }
        Cmd::Mcp(args) => {
            let _span = tracing::info_span!("command", name = "mcp").entered();
            mcp(args)
        }
    }
}

/// Opens an existing company file. A missing file is an error: opening would create one.
fn open_company(file: &Path) -> anyhow::Result<Api> {
    ensure_company_file(file)?;
    Api::open(file).with_context(|| format!("cannot open the company file {}", file.display()))
}

/// Checks that `file` exists as a file, so that no command creates a company file by accident.
fn ensure_company_file(file: &Path) -> anyhow::Result<()> {
    if file.is_file() {
        Ok(())
    } else {
        bail!(
            "there is no company file at {}; check the path",
            file.display()
        )
    }
}

fn import_tally(args: &ImportTallyArgs) -> anyhow::Result<ExitCode> {
    let mut api = open_company(&args.file)?;
    let outcome = api
        .import_tally_http(&args.host, args.port, &args.company)
        .with_context(|| {
            format!(
                "the import from TallyPrime at {}:{} failed; check that TallyPrime is running with its HTTP XML port open",
                args.host, args.port
            )
        })?;
    match outcome {
        Outcome::Value(summary) => {
            println!("{summary}");
            Ok(ExitCode::SUCCESS)
        }
        Outcome::Unsupported(u) => {
            eprintln!("unsupported: {}: {}", u.what, u.reason);
            Ok(ExitCode::from(EXIT_COULD_NOT_RUN))
        }
    }
}

fn verify(file: &Path) -> anyhow::Result<ExitCode> {
    ensure_company_file(file)?;
    let report = verify_file(file)
        .with_context(|| format!("cannot read the company file {}", file.display()))?;
    print!("{}", report.certificate());
    Ok(if report.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EXIT_CHECK_FAILED)
    })
}

fn export_gstr1(args: &Gstr1Args) -> anyhow::Result<ExitCode> {
    // Arguments are checked before the company file is opened.
    let gstin = Gstin::parse(&args.gstin)
        .map_err(|e| anyhow!("the GSTIN is not valid ({e}); check it against the GSTIN card"))?;
    let period = Period::new(
        parse_date("--from", &args.from)?,
        parse_date("--to", &args.to)?,
    )
    .map_err(|e| anyhow!("the period is not valid ({e}); --from must not be after --to"))?;
    let api = open_company(&args.file)?;
    let outcome = api
        .gstr1_json(&gstin, period)
        .context("the GSTR-1 export failed")?;
    match outcome {
        Outcome::Value(json) => {
            match &args.out {
                Some(path) => write_atomic(path, json.as_bytes())?,
                None => println!("{json}"),
            }
            Ok(ExitCode::SUCCESS)
        }
        Outcome::Unsupported(u) => {
            eprintln!("unsupported: {}: {}", u.what, u.reason);
            Ok(ExitCode::from(EXIT_COULD_NOT_RUN))
        }
    }
}

/// Parses `YYYY-MM-DD`. `flag` names the option in the error.
fn parse_date(flag: &str, input: &str) -> anyhow::Result<Date> {
    let format = time::macros::format_description!("[year]-[month]-[day]");
    Date::parse(input, &format).map_err(|e| anyhow!("{flag} must be a date as YYYY-MM-DD ({e})"))
}

/// Writes `bytes` to `path` through a temporary file in the same directory, flushed to disk and
/// then renamed over `path`, so a reader never sees a half-written export (rule D3).
fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| anyhow!("{} is not a file name", path.display()))?;
    let temp = dir.join(format!(".{}.partial", name.to_string_lossy()));
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temp, path)
    })();
    if let Err(e) = result {
        if let Err(remove) = std::fs::remove_file(&temp) {
            tracing::debug!(error = %remove, "could not remove the temporary export");
        }
        return Err(e).with_context(|| format!("cannot write {}", path.display()));
    }
    Ok(())
}

fn rebuild_check(file: &Path) -> anyhow::Result<ExitCode> {
    let api = open_company(file)?;
    let report = api
        .rebuild_check()
        .context("the rebuild check could not read the company file")?;
    println!("Events replayed: {}", report.events_replayed);
    for skipped in &report.not_compared {
        println!("Not compared: {} ({})", skipped.table, skipped.reason);
    }
    match &report.outcome {
        RebuildOutcome::Match => {
            println!("Projections match the rebuild from the event log.");
            Ok(ExitCode::SUCCESS)
        }
        RebuildOutcome::NotMaintained => {
            println!(
                "Not checked: the projection tables are not maintained, so nothing was compared. This is not a match."
            );
            Ok(ExitCode::from(EXIT_COULD_NOT_RUN))
        }
        RebuildOutcome::Mismatch(differences) => {
            println!(
                "Projections differ from the rebuild: {} difference(s).",
                differences.len()
            );
            for d in differences {
                println!("{} {}: {}", d.table, d.key, d.detail);
            }
            Ok(ExitCode::from(EXIT_CHECK_FAILED))
        }
    }
}

/// Serves the MCP protocol over this process's standard input and output until the client closes
/// its input. Standard output carries protocol messages only; logs go to standard error.
fn mcp(args: &McpArgs) -> anyhow::Result<ExitCode> {
    let api = open_company(&args.company)?;
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    ledgerly_mcp::server::serve(&api, stdin.lock(), stdout.lock())
        .context("the MCP server stopped with an error")?;
    Ok(ExitCode::SUCCESS)
}

fn conformance(args: &ConformanceArgs) -> anyhow::Result<ExitCode> {
    let report = run_all(&args.cases)
        .with_context(|| format!("cannot run the cases in {}", args.cases.display()))?;
    let pass_list = std::fs::read_to_string(&args.pass_list)
        .with_context(|| format!("cannot read the pass-list {}", args.pass_list.display()))?;
    println!(
        "Passed {}, failed {}, pending {}.",
        report.passed_count(),
        report.failed_count(),
        report.pending_count()
    );
    for result in &report.results {
        if let CaseStatus::Failed { detail } = &result.status {
            println!("FAILED {}: {detail}", result.id);
        }
    }
    match check_ratchet(&report, &pass_list) {
        Ok(()) => {
            println!("Ratchet: every listed case still passes.");
            Ok(ExitCode::SUCCESS)
        }
        Err(violations) => {
            for violation in violations {
                eprintln!("ratchet: {violation}");
            }
            Ok(ExitCode::from(EXIT_CHECK_FAILED))
        }
    }
}
