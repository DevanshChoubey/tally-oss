# Ledgerly

**A local-first, keyboard-driven desktop accounting app for Indian small businesses. Your books stay in one company file on your own computer.**

[![CI](https://github.com/DevanshChoubey/tally-oss/actions/workflows/ci.yml/badge.svg)](https://github.com/DevanshChoubey/tally-oss/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/DevanshChoubey/tally-oss)](https://github.com/DevanshChoubey/tally-oss/releases/latest)
[![Licence: AGPL-3.0-or-later](https://img.shields.io/badge/licence-AGPL--3.0--or--later-blue.svg)](LICENSING.md)

Ledgerly covers books, GST returns, bank reconciliation, basic inventory and printing, in English and Hindi. It is written
in Rust with [egui](https://github.com/emilk/egui).

![Sales invoice entry in Ledgerly](design/app-captures/light/sales-invoice.default.webp)

## Contents

- [Key features](#key-features)
- [Install](#install)
- [Quick start](#quick-start)
- [Command line](#command-line)
- [AI and the MCP server](#ai-and-the-mcp-server)
- [Data and privacy](#data-and-privacy)
- [Correctness](#correctness)
- [Project status and known gaps](#project-status-and-known-gaps)
- [Repository layout](#repository-layout)
- [Contributing](#contributing)
- [Security](#security)
- [Licence](#licence)
- [Independence and trademarks](#independence-and-trademarks)

## Key features

- **Books.** Ledgers, groups and cost-centre masters; sales, purchase, payment, receipt, journal,
  contra, credit note and debit note vouchers; voucher numbering; period locks. Allocating a voucher
  line to cost centres is not built yet: a ledger that needs cost centres is refused on entry.
- **Backup and restore** (in the source tree after 1.0.0; not in the 1.0.0 packages). Settings >
  Backups and updates backs up the company file now (Alt+B) or on
  a schedule (Daily, Weekly or Monthly, keeping 1 to 366 scheduled versions; 7 by default), and
  restores a backup to a new file after checking its audit chain. `ledgerly backup` and
  `ledgerly restore` do the same from the command line. A file made by an older build is backed up
  before it is upgraded on open.
- **Audit trail.** Every change is an event in an append-only log. Each event is hash-chained to the
  one before it, and `ledgerly verify` checks the chain.
- **Reports.** Balance sheet, profit and loss, trial balance, day book, ledger vouchers and
  outstanding bills, with drill-down to the voucher. Reports can be exported to CSV and XLSX.
- **GST.** Place of supply and tax on vouchers; GSTR-1 and GSTR-1A by section with JSON export;
  GSTR-3B working with the set-off of input credit; GSTR-2B and IMS actions with reconciliation;
  e-invoice JSON for a sales invoice. Ledgerly does not call any GST portal API: you upload the
  files yourself. There is no e-way bill yet (see the known gaps).
- **Bank.** Import statements in CSV, XLSX and MT940; match entries and reconcile; record cheques.
- **Stock.** Stock items, units and godown masters; invoices for items; stock valuation. Item
  invoices work in a company with one godown; with more than one, the invoice is refused because
  the screen cannot pick a godown yet.
- **Import from Tally.** Read a Tally XML file that you exported, or pull the company from a running
  Tally over its HTTP XML port, with a reconciliation report. Ledgerly does not export Tally XML.
- **Printing.** Ctrl+P opens a print preview of invoices, notes, other vouchers and the reports, with
  paper, copies and page range; Save as PDF writes the file, which you print from your PDF viewer.
  Ledgerly does not print straight to a printer.
- **Hindi and English.** Every screen is in both; switch from the command palette (Ctrl+K, Language).
- **Keyboard first.** Function keys and a command palette (Ctrl+K). Two keymaps are included:
  `keymap-standard.toml` and `keymap-classic.toml`.
- **Themes and access.** Light, dark and high-contrast themes; screen-reader names and Tab order;
  Ctrl+= / Ctrl+- / Ctrl+0 zoom.

## Install

Download a release from the [releases page](https://github.com/DevanshChoubey/tally-oss/releases).
Each release has a `SHA256SUMS.txt` file with the SHA-256 of every artifact.

| Platform | Artifacts |
| --- | --- |
| Linux (x86_64) | `ledgerly-<version>-linux-x86_64.tar.gz`, `ledgerly_<version>_amd64.deb` |
| Windows (x86_64) | `ledgerly-<version>-windows-x86_64.zip`, `ledgerly-<version>-windows-x86_64-setup.exe` |
| macOS (arm64, Apple silicon) | `ledgerly-<version>-macos-arm64.dmg` |

The Windows and macOS packages are **not signed** with a developer certificate (Microsoft or Apple),
so each system warns you the first time you run Ledgerly. The warning is expected; here is how to
get past it.

- **Windows.** SmartScreen shows "Windows protected your PC". Choose **More info**, then **Run
  anyway**. This applies to the installer (`-setup.exe`) and to `ledgerly-app.exe` from the zip.
- **macOS.** Open the `.dmg` and drag Ledgerly to Applications. The first time, right-click (or
  Control-click) Ledgerly and choose **Open**, then **Open** again. On macOS 15 and later that may
  not offer Open: try to open the app once, then go to **System Settings > Privacy & Security**,
  scroll to the message about Ledgerly and choose **Open Anyway**.
- **Linux.** No signing warning applies. Install the `.deb` with
  `sudo apt install ./ledgerly_<version>_amd64.deb` (it installs `ledgerly-app` and `ledgerly`), or
  unpack the `.tar.gz` anywhere and run `ledgerly-app`.

### Check your download

Check a download before you run it. Every release lists the SHA-256 of each file in
`SHA256SUMS.txt`, and every file has signed build provenance from the release workflow.

```bash
# in the folder with your download and SHA256SUMS.txt
sha256sum --ignore-missing -c SHA256SUMS.txt        # Linux
shasum -a 256 --ignore-missing -c SHA256SUMS.txt    # macOS
Get-FileHash .\ledgerly-<version>-windows-x86_64-setup.exe -Algorithm SHA256   # Windows PowerShell; compare by eye

# build provenance, with the GitHub CLI
gh attestation verify ledgerly_<version>_amd64.deb --repo DevanshChoubey/tally-oss
```

`gh attestation verify` proves the file was built from this repository by its release workflow.
The check does not replace the operating system's warning; it tells you the file is the one the
project built.

### Build from source

The Rust toolchain is pinned in `rust-toolchain.toml` (channel `1.100`). With
[rustup](https://rustup.rs) installed, it is selected automatically.

```bash
git clone https://github.com/DevanshChoubey/tally-oss
cd tally-oss
cargo build --release --locked -p ledgerly-app -p ledgerly-cli
```

This produces:

- `target/release/ledgerly-app`, the desktop app;
- `target/release/ledgerly`, the command line tool.

To run the app straight from the source tree, use `cargo run --release -p ledgerly-app`.

## Quick start

1. Start `ledgerly-app`. The first-run screens cover the company, the features you want, the backup
   folder and the keyboard. After 1.0.0 the backup step stores a Daily schedule that keeps 7
   versions; in 1.0.0 it stores nothing.
2. Create the company. Ledgerly stores it as a single file on your computer.
3. Enter vouchers from the sidebar or with the function keys shown in the status bar. Press Ctrl+K
   to find any screen or master by name.
4. Open **Balance sheet**, **Day book** or **GST returns** to see the engine's figures.
5. Keep backups of the company file. In 1.0.0, copy the file. After 1.0.0, Settings > Backups and
   updates shows the backups, takes one
   now (Alt+B) and restores one to a new file. A scheduled backup is taken when you open the company
   and one is due; with Ledgerly closed, run `ledgerly backup <file> --if-due` from your system's
   scheduler.

## Command line

The `ledgerly` binary has these subcommands. Run `ledgerly --help` or `ledgerly <command> --help` for
all options.

| Command | What it does |
| --- | --- |
| `ledgerly verify <file>` | Checks the audit chain and checkpoints of a company file. |
| `ledgerly rebuild-check <file>` | Replays the event log and compares the result with the stored projection tables. |
| `ledgerly export gstr1 <file> --gstin <GSTIN> --from <YYYY-MM-DD> --to <YYYY-MM-DD> [--out <file>]` | Writes GSTR-1 for one period as JSON, to standard output or to `--out`. |
| `ledgerly import-tally <file> --company "<name>" [--host <host>] [--port <port>]` | Imports a company from a running Tally's HTTP XML port (default `127.0.0.1:9000`) into a company file. |
| `ledgerly backup <file> [--to <folder>] [--if-due] [--list]` | After 1.0.0. Writes a checked backup of a company file into its backup folder (or `--to`); `--if-due` backs up only when the stored schedule is due, and rotates scheduled backups; `--list` lists the backups. |
| `ledgerly restore <backup> <target>` | After 1.0.0. Restores a backup to a new file after checking its audit chain. It never replaces an existing file; a broken chain exits with `1`. |
| `ledgerly check-update [--compare <file>]` | After 1.0.0. Asks the GitHub releases API for the latest release and checks that its `SHA256SUMS.txt` agrees with GitHub's record of each file; `--compare` checks a downloaded file against it. It never downloads or installs a release. |
| `ledgerly conformance [--cases <dir>] [--pass-list <file>]` | Runs the golden conformance cases and checks the pass-list ratchet. |
| `ledgerly mcp --company <file>` | Serves the MCP server over stdio. See the next section. |

Exit codes:

- `0`: the command ran and its check passed.
- `1`: a check failed, for example a broken audit chain or projections that differ from the rebuild.
- `2`: the command could not run, for example bad arguments, a missing file, or input this build does
  not compute yet.

`verify`, `rebuild-check` and `export` need an existing company file. None of them creates one.

## AI and the MCP server

**Release 1.0.0 ships without the AI assistant.** Every company runs with AI off, and the AI and
privacy screen shows the other choices as "Not in this release". The sidebar, the search box and the
command palette have no route to plain-words entry, Ask your books or bill photos. Nothing in the app sends anything to a model. The assistant
never produces a figure that you see; when it comes, it will only propose drafts, and the engine
computes every number.

**The MCP server works in this release.** `ledgerly mcp --company <file>` runs a Model Context Protocol
server over stdio for an AI client you run yourself. It exposes three tools:

- `search` finds ledgers, groups, stock items and vouchers by name, alias, GSTIN, voucher number,
  party or amount. Read only.
- `report` runs a report through the engine and returns the engine's result. Read only.
- `propose_voucher` checks a voucher draft against the books and reports what would be written. It
  posts nothing. The server has no tool that posts a voucher.

## Data and privacy

- **Local by default.** The company is one file on your computer. Ledgerly does not upload books,
  party data or bank data.
- **Network use only when you ask for it.** Ledgerly makes a network connection in two cases: when
  you import from a running Tally, it connects to the HTTP XML port you give it (default
  `127.0.0.1:9000`); and, in the source tree after 1.0.0 (1.0.0 has no update check), when you
  turn on the update check (Settings > Backups and updates > Updates, off by default) and press
  Check now, or run `ledgerly check-update`. The check asks `api.github.com` for the latest release
  of this project and reads its `SHA256SUMS.txt` from GitHub's download hosts, with the user agent
  `Ledgerly-update-check` and nothing about you, your company or your files. It never downloads or
  installs a release. There is no AI or telemetry connection.
- **Backups** (after 1.0.0). Backups go to the folder you choose (or `Backups` beside the company
  file). Each is a separate file whose audit chain is checked when it is written and again before
  a restore; a restore always writes a new file. Manual backups are never removed; scheduled ones
  are rotated.

## Correctness

Ledgerly's main promise is that it never shows a wrong number. Every computation returns one of three
things:

- a **value**, computed by the engine;
- an **`Unsupported`** result with a reason, when the case is not built yet;
- an **error**.

There are no silent defaults and no "best effort" figures. The screens and the MCP server show the
engine's result as it is, or the reason it is not available.

The books can also check themselves:

- Every change is a hash-chained event. `ledgerly verify <file>` recomputes the chain and reports a
  break. A broken chain exits with code `1`.
- `ledgerly rebuild-check <file>` replays the event log and compares it with the stored tables.
- The golden conformance cases carry provenance, either a recorded observation or a derivation written
  out by hand. `ledgerly conformance` runs them and fails if a listed case stops passing.

The engineering rules that enforce this are in [docs/engineering-rules.md](docs/engineering-rules.md).

## Project status and known gaps

The latest release is Ledgerly 1.0.0 (2026-11-26), the first release. The full list of what is in
it and what is not is in [RELEASE-NOTES.md](RELEASE-NOTES.md). The changes are in
[CHANGELOG.md](CHANGELOG.md).

Not in 1.0.0:

- The AI assistant (see above).
- Printing straight to a printer (save as PDF, then print).
- GSTR-3B JSON export, and e-invoice upload (Ledgerly writes the e-invoice JSON; you upload it).
- E-way bill JSON. Format code exists in `ledgerly-gst-formats`, but no screen or command uses it.
- Cost-centre allocation on vouchers, and a cost-centre report (the masters exist).
- Item invoices in a company with more than one godown.
- Backup and restore, and the update check (both arrive after 1.0.0; see CHANGELOG.md, Unreleased).
  Even then, scheduled backups run only when the company is opened, or from a system scheduler with
  `ledgerly backup --if-due`.
- Export to Tally XML, and Excel import of masters or vouchers.
- The Windows and macOS installers are not signed (see [Install](#install)). The release workflow
  has a code-signing step for the Windows installer that runs once the project's signing account
  exists; see [docs/signing.md](docs/signing.md).

Smaller differences from the design are tracked in [design/PARITY-GAPS.md](design/PARITY-GAPS.md).
The design spec is [docs/specs/2026-11-20-ledgerly-design.md](docs/specs/2026-11-20-ledgerly-design.md).

## Repository layout

| Crate | Licence | Purpose |
| --- | --- | --- |
| `ledgerly-core` | MIT OR Apache-2.0 | Shared types: paise, quantities, rates, dates, financial years, GSTIN, PAN, state codes, `Outcome` and `Unsupported`. |
| `ledgerly-model` | AGPL-3.0-or-later | Masters, vouchers and the books read model, with invariant checks. |
| `ledgerly-store` | AGPL-3.0-or-later | SQLite event store: the hash-chained log, projections, verification and backup. |
| `ledgerly-engine` | AGPL-3.0-or-later | Command handlers that validate a command against the books and produce events. |
| `ledgerly-gst` | AGPL-3.0-or-later | GST rule packs, place of supply, voucher tax, return builders and reconciliation. |
| `ledgerly-gst-formats` | MIT OR Apache-2.0 | GST return data shapes and their JSON formats. |
| `ledgerly-stock` | AGPL-3.0-or-later | Units, stock positions and inventory valuation. |
| `ledgerly-reports` | AGPL-3.0-or-later | Report parameters, report builders, and CSV and XLSX export. |
| `ledgerly-bank` | AGPL-3.0-or-later | Bank statements, matching, bank reconciliation, cheques and UPI payload building. |
| `ledgerly-bankfmt` | MIT OR Apache-2.0 | Bank statement format registry and the CSV, XLSX and MT940 parsers. |
| `ledgerly-tally-xml` | MIT OR Apache-2.0 | Tally XML types, parsers and request builders. No network. |
| `ledgerly-tally-bridge` | AGPL-3.0-or-later | Import from a running Tally or from exported XML files, with mapping and reconciliation. |
| `ledgerly-assist` | AGPL-3.0-or-later | The assistant's typed proposals, OCR and model seams, and the validator that recomputes through the engine. |
| `ledgerly-print` | AGPL-3.0-or-later | Print templates and PDF rendering for invoices and vouchers. |
| `ledgerly-api` | AGPL-3.0-or-later | The command and query facade used by the app, the CLI and the MCP server. |
| `ledgerly-mcp` | AGPL-3.0-or-later | The MCP server (JSON-RPC over stdio) with read tools and a propose tool. No posting tool. |
| `ledgerly-conformance` | AGPL-3.0-or-later | Golden cases with provenance, fixtures, the pass-list ratchet and benchmarks. |
| `ledgerly-cli` | AGPL-3.0-or-later | The `ledgerly` command line. |
| `ledgerly-app` | AGPL-3.0-or-later | The desktop app (egui). |

The headless crates do not depend on any UI crate. CI checks this.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) and [docs/engineering-rules.md](docs/engineering-rules.md)
before you open a pull request. The main rule is that a wrong number is a bug. Tests come first, and
expected values need provenance. Ledgerly is a clean-room project, so contributions must not use code,
artwork or screens from any other product.

Before you open a pull request, run the same checks as CI:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo deny check
```

Sign off each commit with `git commit -s` (Developer Certificate of Origin). Do not attach real company
files, bank statements or GST data to issues or pull requests. Make a small example with made-up names
and amounts instead.

## Security

Please do not report security problems in a public issue. Use GitHub's private vulnerability reporting
(the **Security** tab, then **Report a vulnerability**). [SECURITY.md](SECURITY.md) describes what
counts as a security issue and what to expect.

## Licence

- The app and most crates are licensed under the GNU Affero General Public License v3.0 or later
  ([LICENSE-AGPL](LICENSE-AGPL)).
- `ledgerly-core`, `ledgerly-tally-xml`, `ledgerly-gst-formats` and `ledgerly-bankfmt` are dual-licensed
  under MIT OR Apache-2.0 ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)), so other
  projects can reuse the format code.
- The bundled fonts are under the SIL Open Font License 1.1. See [NOTICE](NOTICE) and
  [LICENSING.md](LICENSING.md).

## Independence and trademarks

Ledgerly is an independent project. It is not affiliated with, endorsed by or supported by Tally
Solutions Pvt. Ltd. "Tally" and "TallyPrime" are trademarks of their owner. Ledgerly uses those names
only to say which file formats and ports it reads for import.
