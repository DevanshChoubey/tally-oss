# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0] - 2026-11-26

The first release of Ledgerly for people to use. Early builds that were published under the
version numbers 1.0.0 and 1.0.1 on 24 November 2026 were withdrawn; this release replaces them.
Install this one. The full list of what is in it, and the known gaps, is in
[RELEASE-NOTES.md](RELEASE-NOTES.md).

### What Ledgerly does in this release

- **Companies.** A company is one file on your computer. A setup wizard (company, GST registration,
  features, keyboard, AI choice, backup) creates it; a recent-companies list opens it again; a sample
  company lets you try everything. Financial years, period locks, backups, and an append-only event
  log with a hash chain (`ledgerly verify`).
- **Masters and vouchers.** Ledgers, groups, cost centres, units, stock items, godowns; sales,
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

- The AI assistant. A new company starts with AI off, and the screens for Ask your books,
  plain-words entry, bill photos and AI bank suggestions show their AI-off state. No model, no
  reader and no network call for AI is part of this release. The MCP server is separate and works.
- Printing straight to a printer. Save as PDF, then print from your PDF viewer.
- GSTR-3B JSON export. The return's working and its set-off are on screen; no file is written.
- e-Invoice upload to the portal. Ledgerly writes the JSON; you upload it.

[Unreleased]: https://github.com/DevanshChoubey/tally-oss/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/DevanshChoubey/tally-oss/releases/tag/v1.0.0
