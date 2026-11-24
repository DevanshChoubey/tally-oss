# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
