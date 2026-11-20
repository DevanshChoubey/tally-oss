# Engineering rules

These rules apply to every crate in this repo. Where a rule can be checked by a tool, the tool is
the authority: `[workspace.lints]` in `Cargo.toml`, `clippy.toml`, `rustfmt.toml`, `deny.toml` and
`.github/workflows/ci.yml`. A change that weakens one of those files needs the reason in the commit
message and the owner's agreement.

The product's one promise is in the spec (`docs/specs/2026-11-20-ledgerly-design.md`):
**never show a wrong number.** Section A exists to keep it.

## A. Correctness: never a wrong number

1. Every computation (posting, a balance, a report figure, GST tax, a return table, a stock value,
   a reconciliation) returns exactly one of: a value, `Unsupported { reason }` naming what is not
   handled, or a typed error. No silent default, no "best effort", no auto-fix of user data.
2. No wildcard (`_ =>`) match arms over the closed domain enums: `BaseVoucherKind`,
   `ReservedGroup`, `Nature`, `VoucherStatus`, `BillRefKind`, `SupplyKind`, `PlaceOfSupply`,
   `RegistrationType`, `GstRateKind`, `ValuationMethod`, `ReturnSection`, `MatchKind`, Tally XML
   tag enums, and any enum marked `#[non_exhaustive]`-free in `ledgerly-core`/`ledgerly-model`.
   A new variant must fail to compile until every match handles it.
3. Money is `Paise` (`i64`). Quantities, rates and unit costs are `rust_decimal::Decimal`. `f32`
   and `f64` are banned in every crate except `ledgerly-app` layout code and `ledgerly-assist`
   model confidences (clippy `float_arithmetic` denied elsewhere). Every conversion that can lose
   precision names its rounding rule and returns an error on overflow (checked arithmetic only).
4. Every expected value in a test carries provenance, recorded next to it:
   - `observed`: captured from TallyPrime (edition, release, date) or from the GST portal / an
     official offline tool (tool version, date); or
   - `derived`: worked out by hand from the CGST/IGST Act, rules, a CBIC/GSTN notification or
     circular, an accounting standard, or a bank/Tally published format spec, with the reference
     and the arithmetic written in the test.
   Expected values never come from running our own engine and copying the output, and never from
   memory of how Tally or GST "usually" behaves. Research notes tagged `[K]` are not provenance.
5. A case without provenance is `pending`. The feature it covers counts as unverified in the
   conformance report, and the app reports it as unsupported to users.
6. The conformance pass-list (`crates/ledgerly-conformance/pass-list.txt`) is a ratchet: CI fails
   if a case that passed before now fails. Cases move off it only by an explicit, reviewed change.
7. Results are deterministic: same input, same output, regardless of thread count, hash seed,
   locale, time zone or platform. Collections that reach output are ordered (`BTreeMap`, sorted
   `Vec`), never `HashMap` iteration order.
8. Every GST rule is effective-dated and selected by time of supply. A rule pack entry without a
   legal reference is `pending` and evaluates to `Unsupported`.
9. Unknown fields from Tally XML, GST JSON and bank files are preserved (opaque) and never dropped
   silently; a dropped field is a reported import warning.
10. The AI (`ledgerly-assist`) only produces typed proposals. It never writes to the store, never
    posts, files or sends, and never supplies a derived number: the engine recomputes every total,
    tax and balance, and a mismatch with an extracted figure blocks acceptance (spec 6.8).

## B. Code quality

1. `unsafe` is forbidden (`unsafe_code = "forbid"`) in every crate except a named FFI crate
   (`ledgerly-ffi`, created only when needed). Every `unsafe` block there has a `// SAFETY:` comment.
2. Library crates do not panic on any input: no `unwrap`, `expect`, `panic!`, `unreachable!`,
   `todo!`, `unimplemented!`, or panicking indexing/slicing/arithmetic on outside data. Tests,
   benches and build scripts may use them.
3. Errors are typed per crate (`thiserror`). `anyhow` only in binaries. A user-facing error says
   what failed, where (company, voucher type + number, ledger, line, file and row) and what the user
   can do.
4. `cargo clippy --all-targets --all-features -- -D warnings` and `cargo fmt --check` pass. No
   stubs, `todo!` or commented-out code is merged.
5. Every `#[allow(...)]` has a `reason = "..."`. Prefer `#[expect(...)]`.
6. Public items in library crates are documented (`missing_docs`).
7. Dependencies are checked by `cargo-deny`: permissive licences only (MIT, Apache-2.0, BSD-2/3,
   ISC, Zlib, Unicode-3.0, MPL-2.0 for the app only), no known advisories, crates.io sources only
   unless listed. The MIT/Apache format crates may depend only on MIT/Apache/BSD/ISC crates.
8. C/C++ dependencies (SQLite, llama.cpp, OCR) must build on Windows, macOS and Linux in CI with no
   manual install step (vendored/bundled features).

## C. Complexity and structure

1. Headless crates (everything except `ledgerly-app`) never depend on egui/eframe/wgpu or any UI
   crate. CI checks this with `cargo tree`.
2. One trait per seam: `Store` (engine ↔ storage), `TallyTransport` (bridge ↔ HTTP), `Proposer`
   (assist ↔ model). No other traits or generics until a second implementation exists.
3. Limits: cognitive complexity 25, function length 80 lines, 7 arguments, type complexity 250.
   Exceeding one needs an `#[expect]` with a reason.
4. One query path for figures: every number on screen or in an export comes from
   `ledgerly-api` → engine/reports. The UI never sums anything.
5. No global mutable state. Configuration and the clock (`Clock` value) are passed in.
6. Modules over 800 lines get split, unless generated or one coherent table of cases.
7. Crate boundaries in Wave 0 are frozen: a task may not change another crate's public interface;
   it files the need in `docs/interface-requests.md` and the wave reviewer decides.

## D. Production grade

1. The UI thread never blocks on I/O, a query, an import or the AI.
2. Every import, report over a full year, reconciliation and AI call can be cancelled, and
   cancellation is tested (stops promptly, leaves no partial state).
3. Voucher save is one SQLite transaction: validate, post, project, audit event — all or nothing.
   File exports are atomic (temp file in the same directory, flush, rename).
4. Tally XML, GST JSON/Excel, bank statements, images and PDFs are untrusted input. Parsers are
   fuzzed (`cargo-fuzz`) and never panic, hang or allocate without bound.
5. Opening a company touches no network. Network only on an explicit user action (Tally fetch,
   cloud AI call).
6. Credentials (IRP/GSP, AI keys) live in the OS keychain only. Logs never contain amounts, party
   names, GSTINs or account numbers by default.
7. Imports stream with bounded memory.
8. Logging and tracing use `tracing`, with spans around every command, report, import step and file
   operation.
9. CI runs on Windows, macOS and Linux. Benchmarks for spec section 8 run in CI; a regression over
   10% fails the benchmark job.
10. The audit chain is never rewritten. Migrations add events; they never edit or delete them.

## E. Tests

1. Layers, each with a home: unit tests next to the code; `insta` snapshots for parser output and
   exported files; `proptest` for invariants and round-trips; conformance cases in
   `ledgerly-conformance`; headless end-to-end tests (create company, post vouchers, run reports,
   export returns, verify chain).
2. A bug fix starts with a test that fails without the fix.
3. Never delete, `#[ignore]`, loosen or rewrite a test's expected value to get CI green. If an
   expected value is wrong, fix it with new provenance (A4) in a separate, explained change.
4. Tests are hermetic: no network, no wall clock (inject `Clock`), no machine-specific paths. Tally
   bridge tests use recorded fixtures behind `TallyTransport`.
5. Coverage floor: 85% line coverage on core, model, store, engine, gst, stock, reports (checked
   with `cargo-llvm-cov`). Periodic `cargo-mutants` on gst and engine.
6. Snapshot changes need a reason in the commit.
7. Test data is fictional: no real GSTINs, PANs, names or account numbers. Generated GSTINs must
   carry a valid checksum and use state codes from the official list.

## F. Process

1. Small changes: one concern per change, ideally under 400 changed lines excluding snapshots.
2. Review: a fresh reviewer reads the diff itself, not the author's summary.
3. Commits explain why. Breaking a rule above requires saying so in the commit message.
4. CI must be green before merge.
5. Clean room: no Tally binaries, TDL, decompiled code or screenshots of Tally screens are used as
   design input; only public documentation, the user's own exported data, and observed outputs.
   No "Tally" in product names, icons or UI copy except factual interop labels ("Import from Tally").
