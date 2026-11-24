## What this changes

<!-- One topic per PR. Link the related issue, for example "Fixes #123". -->

## Behaviour source

<!-- Accounting and GST behaviour follows public documentation. Say where yours comes from. -->

- [ ] The behaviour follows a public source, cited in the code or test (URL and the date read).
- [ ] Or: it is a hand derivation, written out in the test.

## Tests

- [ ] The change starts with a failing test, and I saw it fail on an assertion before the fix.
- [ ] Every new expected value has a `// provenance:` comment (observed, derived, or a mock line).
- [ ] No existing test was deleted, ignored or loosened.

## Gates

<!-- Paste the summary lines. -->

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo nextest run --workspace`
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
- [ ] `cargo deny check`
- [ ] I added an entry under `Unreleased` in `CHANGELOG.md`, if users can see the change.

## Licensing

- [ ] I agree to license this contribution under the licence of the crates it touches (see
      `LICENSING.md`), and I sign off my commits (Developer Certificate of Origin).
