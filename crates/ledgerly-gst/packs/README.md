# GST rule packs

Card G1 defines the format; card G2 writes the rate packs. Every entry carries `legal_ref`
(Act section, rule, or notification number and date) **or** `pending = true`. A pending entry
evaluates to `Unsupported` (engineering rule A8). Facts tagged `[K]` in `docs/research/04-gst.md`
stay pending until a person checks the notification.
