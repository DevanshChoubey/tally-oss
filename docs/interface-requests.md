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

## G4: RuleSet lookup by id prefix
- Crate/file: `crates/ledgerly-gst/src/pack.rs` (`RuleSet`)
- Current interface: `RuleSet::get(id)` only; there is no way to list the entries in force.
- Requested change: an iterator over the verified (non-pending) entries in force, or a prefix lookup
  such as `entries_with_prefix("slab.")`, returning entries in id order.
- Why (test or spec section that needs it): `compute.rs` finds a rate slab by probing a fixed list of
  ids (`SLAB_IDS`); a slab added to a pack under a new id is silently never found (the voucher is
  `Unsupported`, the safe direction). A test guards the list meanwhile.
