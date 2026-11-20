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

## T4: StockError::Unsupported for inputs a valuation method cannot value
- Crate/file: `crates/ledgerly-stock/src/valuation.rs` (`StockError`)
- Current interface: `StockError` has `Money`, `Unit`, `NoStandardCost`, `NegativeStockBlocked` and the generic `Invalid { field, reason }`.
- Requested change: add `Unsupported { reason: String }` (engineering rule A1: a value, `Unsupported`, or an error). `fifo` returns `Invalid` today for: an item not set to FIFO, a voucher that both receives and issues the item (godown transfer, stock journal), and a value-only movement. These are unsupported inputs, not corrupt data, and a UI cannot tell them apart now. Once added, T4 switches those cases to `Unsupported`.
- Why (test or spec section that needs it): `malformed_moves_are_errors` and `other_valuation_method_is_an_error_not_a_fifo_number` in `crates/ledgerly-stock/tests/t4_fifo.rs`; spec section 6.4.
