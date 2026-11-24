# Contributing to Ledgerly

Thank you for helping. Ledgerly keeps people's books, so most of what this page asks is about one
promise: **never show a wrong number.** Please read this page and
[docs/engineering-rules.md](docs/engineering-rules.md) before you open a pull request.

Ledgerly is an independent project and is not affiliated with Tally Solutions. Report problems
with Ledgerly here.

## The rules that matter most

- **A value, `Unsupported { reason }`, or an error.** Every computation returns exactly one of
  these. No silent defaults, no "best effort", no `_ =>` arms over the domain enums.
- **Behaviour comes from public sources.** Accounting and GST behaviour is modelled on public
  documentation (the GST Acts, rules, CBIC/GSTN notifications, published file formats, the
  accounting product's public help pages), cited in the code or test with the URL and the date
  read. Never from memory, and never from that product's code or screens: this is a clean-room
  project. Do not add that product's name, logo or look to the app.
- **Expected values need provenance.** Each one in a test carries a `// provenance:` comment:
  `observed` (where and when it was seen) or `derived` (the hand working, written out). Never
  copy the engine's own output into an expected value.
- **Tests first.** A fix or feature starts with a failing test, seen to fail on an assertion.
- **Never delete, ignore or loosen a test** to get green.
- **The UI never computes a figure**, and the AI assistant never produces one: the engine
  computes every number shown.
- Library crates: no `unwrap`, `expect`, `panic!` or `unsafe`; typed errors with `thiserror`;
  documented public items.

## Getting set up

```bash
git clone https://github.com/DevanshChoubey/tally-oss
cd tally-oss
cargo build
cargo run -p ledgerly-app
```

The Rust version is pinned in `rust-toolchain.toml`.

## Before you open a pull request

Run the same gates CI runs, and paste the summary lines in the PR:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo deny check
```

- UI changes: compare against the design mock (`design/mock-v2/index.html`). Retake the captures
  for the states you changed (`design/app-captures/capture.sh --only <state>`), and record any
  difference you leave in `design/PARITY-GAPS.md`.
- Add an entry under `Unreleased` in `CHANGELOG.md` if users can see the change.
- One topic per PR.

## Never attach real books

Do not attach real company files, bank statements or GST data to issues or PRs. Make the
smallest example with made-up names and amounts.

## Licensing and sign-off

Contributions are licensed under the licence of the crates they touch (see `LICENSING.md`).
Sign off each commit (`git commit -s`) to certify the
[Developer Certificate of Origin](https://developercertificate.org/).

CI enforces this: the `dco` job in `.github/workflows/ci.yml` fails a pull request if any
non-merge commit in it lacks a `Signed-off-by: Your Name <you@example.com>` line. To fix it:

```bash
git commit -s --amend                  # the last commit
git rebase --signoff origin/main       # every commit on your branch, then push with --force-with-lease
```
