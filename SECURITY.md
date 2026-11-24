# Security policy

## Supported versions

Security fixes go to the `main` branch and to the latest release.

## What counts as a security issue

Ledgerly keeps a business's books in a company file on the user's computer, and it reads files
that come from other people: Tally XML exports, bank statements (CSV, XLSX, MT940), GST portal
JSON and bill images. We treat these as security issues:

- An imported file that makes Ledgerly run code, write or delete files outside what the user
  chose, hang, or use unbounded memory.
- Any way that books, party data or keys leave the computer without the user turning that on.
  Ledgerly is local-only by default; the cloud AI tier and the Tally bridge only talk to the
  places the user configures.
- A way to change posted vouchers without the change showing in the event log, or to break the
  hash chain without `ledgerly verify` reporting it.
- The MCP server (`ledgerly mcp`) or the AI assistant posting a voucher, or reading outside the
  company file it was given. They may only read and propose drafts.
- A vulnerability in a release artifact or in the release process.

A wrong figure in a report is a serious bug but not a security issue: report it as a normal
issue, without your real company data.

## Reporting a vulnerability

Please **do not open a public issue.**

- Preferred: use GitHub's private vulnerability reporting (repository **Security** tab, then
  **Report a vulnerability**).
- Or email: devanshc13@gmail.com

Please include the smallest input that triggers the problem (with private data removed), the
steps, the version and the platform.

We aim to acknowledge reports within 7 days and to agree on a disclosure date with the reporter.
