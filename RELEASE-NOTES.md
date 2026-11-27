# Ledgerly 1.0.0

The first release of Ledgerly: a local-first, keyboard-driven desktop accounting app for Indian
small businesses, written in Rust. Your books stay in one company file on your computer. Nothing
is sent anywhere. The full list of changes is in [CHANGELOG.md](CHANGELOG.md).

Early builds published under the version numbers 1.0.0 and 1.0.1 on 24 November 2026 were
withdrawn. This release replaces them.

## What is in 1.0.0

- **Companies and books:** a setup wizard and a sample company; ledgers, groups, cost-centre
  masters, stock items and godown masters; sales, purchase, payment, receipt, journal, contra,
  credit note and debit note vouchers; numbering, period locks; an append-only event log with a
  hash chain (`ledgerly verify` checks it).
- **Item invoices:** sales and purchase invoices with items, HSN, quantities and rates, tax worked
  out by the engine, and stock valuation.
- **GST returns:** GSTR-1 and GSTR-1A by section with JSON export; GSTR-3B working with the set-off
  of input credit in the order the law gives; GSTR-2B and IMS actions with reconciliation; e-invoice
  JSON export for a sales invoice.
- **Banking:** statement import (CSV, XLSX, MT940), matching, bank reconciliation and cheques.
- **Reports:** balance sheet, profit and loss, trial balance, day book, ledger vouchers,
  outstanding bills, stock and the audit screen, with drill-down; CSV and XLSX export.
- **Print and PDF:** a print preview (Ctrl+P) with paper, copies and page range, then Save as PDF.
- **Import:** read an accounting file exported by another product, or pull a company from one on
  your network, with a reconciliation report.
- **Hindi and English** on every screen; light, dark and high-contrast themes; function keys, a
  command palette (Ctrl+K) and a keyboard map; zoom; screen-reader names.
- **Command line and MCP server:** `ledgerly verify`, `rebuild-check`, `export gstr1`, `conformance`
  and `ledgerly mcp` (read tools and draft proposals only; it cannot post).

## Not in this release

- **The AI assistant.** A new company starts with AI off, and the AI screens show their AI-off
  state. Nothing in this release contacts a network for AI.
- **Printing straight to a printer.** Save as PDF, then print from your PDF viewer.
- **Backups you can reach.** The store could write and restore backups, but no screen or command
  did in 1.0.0 (the setup wizard's backup step did not store its choice). Use a copy of the company
  file. Backup and restore arrive in the next release (see CHANGELOG.md, Unreleased).
- **E-way bill.** No screen or command writes e-way bill JSON.
- **Cost-centre allocation.** Cost-centre masters exist; a ledger that needs cost centres is
  refused on voucher entry, and there is no cost-centre report.
- **More than one godown on an item invoice.** A company with more than one godown cannot enter
  item invoices yet.
- **GSTR-3B JSON export.** The working and the set-off are on screen; no file is written.
- **e-Invoice upload.** Ledgerly writes the JSON; you upload it on the portal.

Smaller differences from the design are tracked in `design/PARITY-GAPS.md`. The list above was
corrected after the release: it first named backups, cost centres and godowns without these limits.

## Installing (the installers are not signed)

The Windows and macOS packages are **not signed** with a developer certificate, so each system
warns you the first time. The warnings are expected; the steps below get past them. First check
the download (see "Check your download").

- **Windows** (`ledgerly-1.0.0-windows-x86_64-setup.exe` or the `.zip`): when SmartScreen says
  "Windows protected your PC", choose **More info**, then **Run anyway**.
- **macOS** (`ledgerly-1.0.0-macos-arm64.dmg`, Apple silicon): drag Ledgerly to Applications. The
  first time, right-click (or Control-click) the app and choose **Open**, then **Open** again. On
  macOS 15 and later that route may not offer Open: try to open the app once, then go to
  **System Settings > Privacy & Security**, scroll to the message about Ledgerly and choose
  **Open Anyway**.
- **Linux** (x86_64): `sudo apt install ./ledgerly_1.0.0_amd64.deb` (it installs `ledgerly-app`
  and `ledgerly`), or unpack `ledgerly-1.0.0-linux-x86_64.tar.gz` anywhere and run
  `ledgerly-app`. No signing warning applies.

## Check your download

Every file is listed in `SHA256SUMS.txt` on this release, and every file has signed build
provenance.

```bash
# in the folder with the files you downloaded and SHA256SUMS.txt
sha256sum --ignore-missing -c SHA256SUMS.txt          # Linux
shasum -a 256 --ignore-missing -c SHA256SUMS.txt      # macOS
# Windows (PowerShell): compare with the line for your file in SHA256SUMS.txt
Get-FileHash .\ledgerly-1.0.0-windows-x86_64-setup.exe -Algorithm SHA256

# build provenance (GitHub CLI): proves the file was built by this repository's release workflow
gh attestation verify ledgerly_1.0.0_amd64.deb --repo DevanshChoubey/tally-oss
```
