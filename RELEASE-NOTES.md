# Ledgerly 1.0.0

First release. A local-first desktop accounting app for Indian small businesses, written in Rust.
Your books stay in one company file on your computer; nothing is sent anywhere unless you turn a
cloud feature on.

## What is in 1.0.0

- **Books:** ledgers, groups, cost centres, all the usual voucher types (sales, purchase, payment,
  receipt, journal, contra, credit and debit notes), numbering, period locks, an append-only event
  log with a hash chain (`ledgerly verify` checks it), backups.
- **Reports:** balance sheet, profit and loss, trial balance, day book, ledger vouchers and
  outstanding bills, with drill-down to the voucher, and export to CSV / XLSX.
- **GST:** place of supply, tax on vouchers, GSTR-1 / GSTR-1A and GSTR-3B working with offline
  JSON export, IMS actions, e-invoice and e-way bill JSON (no portal API calls in 1.0).
- **Interop:** import from Tally XML (an exported file, or pulled from a running Tally over its
  XML port) with a reconciliation report.
- **Banking and stock:** bank statement import (CSV, XLSX, MT940), matching and bank
  reconciliation, cheques; stock items, units, godowns, item invoices and stock valuation.
- **Printing:** sales invoices, credit and debit notes and five reports (balance sheet, profit and
  loss, trial balance, day book, ledger vouchers) save as PDF with Ctrl+P, then open in your PDF
  viewer.
- **AI assistant:** an MCP server (`ledgerly mcp --company <file>`) gives AI clients read tools
  and draft proposals only; it cannot post. The in-app assistant screens (plain-words entry, bill
  photos, "ask your books") are present but do not run yet: no local model or OCR engine ships,
  and the cloud tier is refused because no keychain store is wired in. The assistant never writes
  a number you see: the engine computes them.
- **Look and access:** light, dark and high-contrast themes; screen-reader names and roles on
  every control; Tab order in reading order; Ctrl+= / Ctrl+- / Ctrl+0 zoom; no signal carried by
  colour alone.
- **Builds:** Linux (tar.gz, .deb), Windows (zip, installer) and macOS (.dmg); Linux release
  builds are reproducible (`scripts/repro-check.sh`); `SHA256SUMS.txt` lists every file.

## Known gaps

- English only. Hindi is planned for 1.1.
- The in-app AI assistant does not run yet (see above); the MCP server does.
- GSTR-3B working refuses any period with a purchase in it: the bundled rule pack does not yet
  list the blocked-credit items of CGST Act s.17(5).
- No export to Tally XML from the app yet, and no Excel import of masters or vouchers.
- No print preview inside the app, no printer output (save as PDF, then print), no print
  configuration, no UPI QR code on invoices yet; other reports do not print yet.
- The Windows and macOS packages are built by CI and were not tried by hand before release; the
  Windows installer has no icon.
- The theme choice and zoom level are not remembered between runs.
- The divider line colour is below the 3:1 contrast guideline in light and dark.
- Payment bill-wise allocation and the overdraft warning are not built; the 2B reconciliation
  reader is not built.
- Smaller differences from the design are tracked in `design/PARITY-GAPS.md`.

Ledgerly is an independent project and is not affiliated with or endorsed by Tally Solutions.
