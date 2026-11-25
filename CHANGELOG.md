# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- The redactor also removes email addresses before a document goes to a cloud model.
- Bills receivable and payable show overdue days in red, and a bill due on the as-at date is due
  today, not overdue.
- Stock items with no HSN show a "Missing: add HSN" badge with an alert icon; a GSTIN with a wrong
  check character says which character it should be.
- Bank reconciliation shows a Reconciled or Not yet reconciled card from the statement's difference.
- The balance sheet exports the view that is on screen (groups and sub-groups, or ledgers).
- The audit screen's Integrity certificate tab lists the engine's facts as a certificate.
- High contrast: egui's own disabled widgets keep 7:1 text instead of fading to 5.3:1.
- Ctrl+P on a report or an invoice opens a print preview of the pages the print engine laid out;
  Save as PDF there writes the same PDF as before, and Esc closes the preview.
- Printed reports use Indian digit grouping and Dr/Cr instead of a minus; the profit and loss prints
  as the two-sided account and the balance sheet as one vertical statement.
- Balance sheet: the default view lists each group with its sub-groups, and Alt+F1 adds ledgers.
- Day book: Entered by, Origin and the narration are shown.
- GSTR-1: a table per section of the return (records, taxable value, tax, status); the tiles count
  sections; the period button is drawn.
- The sample company's figures match the design mock (capital, profit, opening stock).
- A provisional stock figure names the item that is negative.
- GSTR-1: a missing HSN keeps the invoice in its tables and only Table 12 needs correction; sections
  are selectable with a correction side card; Alt+F2 picks the month; a line says whether taxable
  value agrees with GSTR-3B.
- GSTR-3B computes on the sample company.

## [1.0.1] - 2026-11-24

### Fixed

- macOS: the app bundle is now signed ad hoc. In 1.0.0 it had no signature, so macOS called the
  downloaded app "damaged" and would not open it.

### Changed

- The README and release notes say how to get past the first-run warning on macOS and Windows
  (the packages are not signed with a developer certificate).

## [1.0.0] - 2026-11-24

First release. The full list of what is in it, and its known gaps, is in
[RELEASE-NOTES.md](RELEASE-NOTES.md).

### Added

- Books: masters, all v1 voucher types, numbering, period locks, an append-only event log with a
  hash chain (`ledgerly verify`), backups.
- Reports with drill-down: balance sheet, profit and loss, trial balance, day book, ledger
  vouchers, outstanding bills; CSV and XLSX export.
- GST: place of supply, voucher tax, GSTR-1 / GSTR-1A and GSTR-3B working with JSON export, IMS
  actions, e-invoice and e-way bill JSON.
- Import from Tally XML, with a reconciliation report.
- Bank statement import (CSV, XLSX, MT940), matching and reconciliation; basic inventory.
- PDF printing of invoices, credit and debit notes and five reports (Ctrl+P).
- An MCP server with read and propose-only tools (`ledgerly mcp`). The in-app AI assistant
  screens are present but do not run yet.
- Light, dark and high-contrast themes; screen-reader names, Tab order and zoom.
- Release packages for Linux, Windows and macOS; reproducible Linux builds.

[Unreleased]: https://github.com/DevanshChoubey/tally-oss/compare/v1.0.1...HEAD
[1.0.1]: https://github.com/DevanshChoubey/tally-oss/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/DevanshChoubey/tally-oss/releases/tag/v1.0.0
