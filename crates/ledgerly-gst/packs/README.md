# GST rule packs

Card G1 defines the format; card G2 writes the rate packs. Every entry carries `legal_ref`
(Act section, rule, or notification number and date) **or** `pending = true`. A pending entry
evaluates to `Unsupported` (engineering rule A8). Facts tagged `[K]` in `docs/research/04-gst.md`
stay pending until a person checks the notification.

## File format (card G1)

```toml
version = "2026.1"                 # required, non-blank

[[entry]]
id = "rate.example"                # required; entries with one id are successive versions of a rule
effective_from = "2025-09-22"      # required, YYYY-MM-DD, inclusive
effective_to = "2026-03-31"        # optional, inclusive; omit for no end
legal_ref = "Notification No. ..." # required unless pending = true
pending = false                    # true: unverified, evaluates to Unsupported
[entry.params]
percent = "18"                     # strings only; callers parse to Decimal/Paise (no floats)
```

Load rejects: bad TOML or unknown fields, blank version or id, bad dates, `effective_to` before
`effective_from`, an entry with neither a non-blank `legal_ref` nor `pending = true`, and two
entries with the same id whose inclusive ranges share a day. A pack is selected by the time of
supply, never the entry date. A date with no entry in force gives `Unsupported`; so does a pending
entry or an id with no entry on that date.
