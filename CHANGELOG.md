# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Backups and updates** (Settings). Back up now (Alt+B), a backup folder, a scheduled backup
  (Off, Daily, Weekly or Monthly) that keeps 1 to 366 versions (7 by default) and is taken when you
  open the company and one is due, the backups listed newest first, and restore to a new file after
  the audit chain is checked; a backup with a broken chain is refused. Backup file names never
  collide, even within one second.
- `ledgerly backup <file> [--to] [--if-due] [--list]` and `ledgerly restore <backup> <target>`.
- **Upgrade on open.** A company file made before Cash and Profit & Loss A/c were predefined is
  backed up, then given them, as one change in the audit trail. A hand-made Cash in Cash-in-Hand
  takes the role and keeps its balance. If the backup fails, the file opens as it was.
- **Opt-in update check.** Off by default. It asks only the GitHub releases API of this project,
  offers a release only when its `SHA256SUMS.txt` agrees with GitHub's digest of each file, and
  never downloads or installs anything. `ledgerly check-update [--compare <file>]` does the same.
- Release packaging: a Windows code-signing step (SignPath), skipped until its secrets exist, and
  winget and Chocolatey manifests filled from `SHA256SUMS.txt` (`docs/signing.md`).

### Changed

- The setup wizard's backup step now stores its folder with a Daily schedule and 7 versions.

### Fixed

- The 1.0.0 notes named backups, cost centres and godowns, and the README named e-way bill JSON,
  ahead of the code. They now say what works and what does not.

## [1.0.0] - 2026-11-26

The first release of Ledgerly for people to use. Early builds that were published under the
version numbers 1.0.0 and 1.0.1 on 24 November 2026 were withdrawn; this release replaces them.
Install this one. The full list of what is in it, and the known gaps, is in
[RELEASE-NOTES.md](RELEASE-NOTES.md).

### What Ledgerly does in this release

- **Companies.** A company is one file on your computer. A setup wizard (company, GST registration,
  features, keyboard, AI choice, backup) creates it; a recent-companies list opens it again; a sample
  company lets you try everything. Financial years, period locks, and an append-only event
  log with a hash chain (`ledgerly verify`). The backup step did not store its choice, and no screen
  or command took a backup.
- **Masters and vouchers.** Ledgers, groups, cost-centre masters (no allocation on vouchers), units,
  stock items, godown masters (item invoices need a company with one godown); sales,
  purchase, payment, receipt, journal, contra, credit note and debit note vouchers, with numbering
  and "as voucher" mode.
- **Item invoices.** Sales and purchase invoices with items, HSN, quantities, rates and the tax the
  engine works out; stock valuation follows.
- **GST returns.** Place of supply and tax on every voucher. GSTR-1 and GSTR-1A as a table per
  section of the return, with correction side cards and a JSON export. GSTR-3B working with the
  set-off of input credit in the order the law gives. GSTR-2B reading and IMS actions, with
  reconciliation against your purchases. e-Invoice JSON export for a sales invoice (Ctrl+E on the
  saved invoice), ready to upload on the portal yourself.
- **Banking.** Bank statement import (CSV, XLSX, MT940), matching, bank reconciliation with a
  Reconciled or Not yet reconciled card, a statement of the lines not yet cleared, and cheques.
- **Reports.** Balance sheet, profit and loss, trial balance, day book, ledger vouchers,
  outstanding bills (overdue days in red), stock and the audit screen, with drill-down to the
  voucher; export to CSV and XLSX.
- **Print and PDF.** Ctrl+P opens a print preview of the pages the print engine laid out (paper,
  copies, page range, first page number), then Save as PDF. Invoices, notes, payment, receipt,
  journal, contra and purchase vouchers, and the reports print with Indian digit grouping.
- **Import.** Read an accounting file exported by another product, or pull a company from one that
  runs on your network, with a reconciliation report.
- **Hindi and English.** Every screen speaks Hindi as well as English; switch the language
  from the command palette (Ctrl+K, Language).
- **Look and keyboard.** Light, dark and high-contrast themes; a keyboard-first design with
  function keys, Alt and Ctrl shortcuts, a command palette (Ctrl+K) and a keyboard map; zoom;
  screen-reader names and Tab order.
- **Command line and MCP server.** `ledgerly verify`, `rebuild-check`, `export gstr1`,
  `conformance`, and `ledgerly mcp` (read tools and draft proposals only).
- **Packages.** Linux (`.tar.gz`, `.deb`), Windows (`.zip`, installer) and macOS (`.dmg`) with a
  `SHA256SUMS.txt` and build provenance you can check with `gh attestation verify`. The Windows
  and macOS packages are not signed with a developer certificate; the README says how to open them.

### Not in this release

- The AI assistant. Every company runs with AI off. The AI and privacy screen and the setup step
  show "On this computer" and "Cloud, with your own key" as disabled, tagged "Not in this release".
  There is no Plain-words entry, "Ask" or bill-photo route in the sidebar, the search box, the
  command palette or the purchase form. A company file saved with an AI tier by an older build
  opens and shows AI off. No model, no reader and no network call for AI is part of this release.
  The MCP server is separate and works.
- Printing straight to a printer. Save as PDF, then print from your PDF viewer.
- GSTR-3B JSON export. The return's working and its set-off are on screen; no file is written.
- e-Invoice upload to the portal. Ledgerly writes the JSON; you upload it.

[Unreleased]: https://github.com/DevanshChoubey/tally-oss/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/DevanshChoubey/tally-oss/releases/tag/v1.0.0
