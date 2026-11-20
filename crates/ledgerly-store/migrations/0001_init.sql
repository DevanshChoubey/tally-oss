-- Ledgerly company file, schema version 1 (card S1).
--
-- Contract with the other store cards (they may not edit this file):
--   meta         S1 writes 'company' (CBOR of ledgerly_model::Company) and 'created_at'.
--   events       S2 appends; the log is the source of truth. `meta` and `event` are CBOR.
--                hash = SHA-256 over canonical CBOR of (seq, prev_hash, meta, event) (spec 6.6).
--   checkpoints  S3 writes one row per day: the chain head at the end of that day.
--   masters, vouchers, postings, stock_moves
--                S4/S5 projections, rebuilt from `events` (S6 compares a rebuild with them).
--                Snapshot columns are CBOR; the other columns exist for filtering and ordering.
-- Ids are unsigned 64-bit in Rust and stored as INTEGER; money is integer paise; quantities are
-- decimal text (never floating point); dates are ISO 8601 text (YYYY-MM-DD).
-- The version and application id live in PRAGMA user_version / application_id, set by the runner
-- in schema.rs, not here.

CREATE TABLE meta (
    key   TEXT PRIMARY KEY NOT NULL,
    value BLOB NOT NULL
) STRICT;

CREATE TABLE events (
    seq       INTEGER PRIMARY KEY NOT NULL CHECK (seq >= 1),
    prev_hash BLOB NOT NULL CHECK (length(prev_hash) = 32),
    hash      BLOB NOT NULL CHECK (length(hash) = 32),
    meta      BLOB NOT NULL,
    event     BLOB NOT NULL
) STRICT;

CREATE TABLE checkpoints (
    day  TEXT PRIMARY KEY NOT NULL,
    seq  INTEGER NOT NULL REFERENCES events(seq),
    hash BLOB NOT NULL CHECK (length(hash) = 32)
) STRICT;

CREATE TABLE masters (
    kind     TEXT NOT NULL,
    id       INTEGER NOT NULL,
    snapshot BLOB NOT NULL,
    PRIMARY KEY (kind, id)
) STRICT;

CREATE TABLE vouchers (
    id       INTEGER PRIMARY KEY NOT NULL,
    date     TEXT NOT NULL,
    status   TEXT NOT NULL,
    snapshot BLOB NOT NULL
) STRICT;
CREATE INDEX vouchers_by_date ON vouchers(date, id);

CREATE TABLE postings (
    voucher_id INTEGER NOT NULL REFERENCES vouchers(id) ON DELETE CASCADE,
    line_no    INTEGER NOT NULL,
    date       TEXT NOT NULL,
    ledger_id  INTEGER NOT NULL,
    paise      INTEGER NOT NULL CHECK (paise >= 0),
    side       TEXT NOT NULL CHECK (side IN ('Dr', 'Cr')),
    snapshot   BLOB NOT NULL,
    PRIMARY KEY (voucher_id, line_no)
) STRICT;
CREATE INDEX postings_by_date ON postings(date, voucher_id, line_no);
CREATE INDEX postings_by_ledger ON postings(ledger_id, date);

CREATE TABLE stock_moves (
    voucher_id INTEGER NOT NULL REFERENCES vouchers(id) ON DELETE CASCADE,
    line_no    INTEGER NOT NULL,
    date       TEXT NOT NULL,
    item_id    INTEGER NOT NULL,
    godown_id  INTEGER NOT NULL,
    batch_id   INTEGER,
    qty_in     TEXT NOT NULL,
    qty_out    TEXT NOT NULL,
    paise      INTEGER NOT NULL,
    PRIMARY KEY (voucher_id, line_no)
) STRICT;
CREATE INDEX stock_moves_by_item ON stock_moves(item_id, date);
