# Interface requests

Builders may not change Wave 0 interfaces, `Cargo.toml`, `Cargo.lock`, `ledgerly-core` or
`ledgerly-model`. Append a section here instead; the wave reviewer decides.

Template:

```
## <card id>: <short title>
- Crate/file:
- Current interface:
- Requested change:
- Why (test or spec section that needs it):
```

## Q1: books_small() contains an expect
- Crate/file: crates/ledgerly-conformance/src/fixtures.rs
- Current interface: `pub fn books_small() -> Books`, frozen; implemented as `try_books_small().expect(..)` under a reasoned `#[expect(clippy::expect_used)]`. Data is constants; a test guards that it builds.
- Requested change: controller decides whether to accept this documented exception, or change the signature to `Result<Books, FixtureError>` in a coordinated change across consumers.
- Why (test or spec section that needs it): rule B2 (no panic paths in library crates). Not replaced with a silent default Books, which would risk a wrong number.
