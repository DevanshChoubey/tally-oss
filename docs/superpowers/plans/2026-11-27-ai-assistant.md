# The AI assistant (later release) — plan, round s21

Scope: the AI NOT BUILT entries of `design/PARITY-GAPS.md` ("Review queue, AI and privacy, Ask,
quick entry, bill photo", bank `ai-suggestions`, the day book's Origin badges). Spec section 6.8
"AI (R10)", section 9, and research `docs/research/10-legal-ai.md`.

The release switch `release::AI_IN_THIS_RELEASE` stays `false` on this branch: 1.0.x ships with no
AI. Everything below is reachable with the switch on (`release::set_ai_enabled(true)` on the UI
thread) and is tested that way; the 1.0.0 tests keep running with it off and stay unchanged.

## Rules that shape every round

- The assistant only drafts typed proposals (`VoucherDraft`, `MatchSuggestion`, `ReportQuery`,
  `MappingSuggestion`). The engine recomputes every figure; a person accepts each draft. A screen
  never shows a number the assistant worked out, and never subtracts or adds one itself: "after"
  figures come from the API (engine), or the row says why there is none.
- The assistant never posts, files or sends. Accept is a user action that the API turns into an
  engine post.
- Every AI event (draft stored, accepted, rejected, question answered, read failed, cloud consent)
  is an audited event in the company file and a row of the AI activity log.
- Local by default. Cloud is bring-your-own-key, opt-in, with redaction and a per-document consent.
  The key lives in the OS keychain only (`keyring` 4, already in the workspace and passing
  `cargo deny`); the company file never holds it. Opening a company touches no network.
- Nothing is downloaded and no real provider is called. Providers sit behind the existing
  `Proposer` seam. The local runner points at a model file the user chooses and has a typed
  "Not installed" state. A deterministic fake provider ("Fixed answers, test provider") feeds the
  real pipeline (recorded model text → `nl_entry`; recorded OCR lines → `ocr::read_bill`;
  → `validate`) for tests and captures; it is reachable only from tests and the capture example.
- Mock first: each state exists in `design/mock-v2` before the app draws it. The mock's
  `AI_IN_RELEASE` stays `false`; a page shows the later-release AI design with the hash parameter
  `ai=on` (as the AI-only states already do).

## Data (round A, used by every later round)

- One new store event, appended to the enum: `Event::AiActivityRecorded(AiActivity)`
  (`ledgerly-model::records`). CBOR writes variants by name, so old files and their hashes are
  untouched (`s11_event_compat`).
- `AiActivity { feature, provider, model_version, prompt_template_version, input_label,
  input_hash, kind }`, `kind` one of `DraftStored { proposal_json }`, `DraftAccepted { draft,
  voucher, human_edits_json }`, `DraftRejected { draft, reason }`, `Answered { output_json }`,
  `ReadFailed { reason }`, `CloudConsent { granted }`. A draft's id is the sequence number of its
  `DraftStored` record. A draft is open until an accept or reject names it.
- `Api::ai_drafts()` lists open drafts; `Api::review_queue()` lists them, and drops the
  `BillDraft` and `PlainWordsDraft` sources from `unlisted` only when the assistant can be on
  (a provider is attached). With no provider the 1.0.0 queue is unchanged.

## Rounds

**A. Draft store and the review queue drafts.** The event, `ai_drafts`, `record_ai_draft`,
`reject_ai_draft`, `accept_ai_draft` (validates through `ledgerly_assist::validate`, posts through
the engine, records the accept with the posted voucher id). Review rows for bill and plain-words
drafts with source, confidence and the engine's amount (or "none"). Y/Ctrl+A accept, N/Alt+D
reject (stored, not session-only, for drafts). Mock: `review.default` already shows drafts.

**B. AI settings.** Mock first: restore `ai-settings.local` and `ai-settings.cloud` (with
`ai=on`), correcting claims the app cannot back (the local panel shows the chosen model file and
whether it is there, not a size and version it cannot know). App: key store seam
(`KeyVault`: status, save, delete; OS keychain in `main.rs` only, an in-memory double in tests and
captures), cloud panel (endpoint, provider format, model, key Set / Not set, ask before each
document), local model rows (model file path, Not installed / Found with its size on disk, runtime
"not in this build"), per-feature switches stored as audited settings, and AI event rows from the
activity log. `set_ai_tier(Cloud)` is allowed once a key store is attached and a key is saved;
otherwise it stays `Unsupported` as in 1.0.0.

**C. Plain-words entry (`quick-entry.*`).** Proposal, Hinglish, unknown ledger, loading, rejected.
Text → provider → `NlEntry` → draft → `validate` → engine figures; the "after" balance comes from
the API. Accept posts; reject records a reason.

**D. Bill photo (`purchase-bill.*`).** Reading, proposal, low confidence, mismatch, duplicate,
cloud consent, error, accepted. OCR through the seam (fake provider returns recorded OCR lines;
the local runner is Not installed). The reader reads no item rows today: the item table is an
open question settled in this round (extend `ocr.rs` test first, or correct the mock and list the
gap). Cloud documents: `cloud.rs` refuses images (pixels cannot be redacted); the consent sends the
locally read, redacted text, and the mock wording is corrected to say so.

**E. Ask (`ask.*`).** Question → `ReportQuery` → the report engine runs it; the answer shows the
query and the engine's table, never a typed-in number. Low confidence and unsupported states.

**F. Bank AI suggestions, `gst.recon-explain`, Origin badges.** `MatchSuggestion` rows marked
"AI suggested" with confidence; the 2B explanation as a draft explanation over the two records;
the day book Origin cell reads the activity log (AI draft, accepted) for posted vouchers.

Each round: test first and seen failing, provenance on expected values, all five gates green
(fmt, clippy `-D warnings`, nextest, doc `-D warnings`, deny), commit, recapture the touched
states in light, dark and hc, build mock|app sheets (`design/app-captures/s21-<round>-sheet-N.png`,
untracked), look at each, update `PARITY-GAPS.md`.

## Owner decisions (not made here)

- A local inference runtime (a llama.cpp binding) and an OCR engine (Tesseract or PaddleOCR
  binding): each is a new dependency and a model download. The seams and the Not installed state
  are built; no runtime is linked.
- The real cloud provider(s) and their default endpoints.

## Status (kept current each round)

- Round A: committed (draft store, provider seam, review queue drafts, `ai.review.default` mock
  state with the `ai=on` hash switch and its capture). Open: the bill-wise New Ref (round D).
- Round B: committed (key vault seam with `KeyringStore` in the real window and `MemoryVault` in
  tests and captures, cloud gating on vault + provider + saved key, local model file setting and
  Ctrl+O, per-feature switches, assistant log rows, mock states `ai.ai-settings.local` and
  `.cloud`). Open: a form to describe the provider and type the key (owner: which providers);
  no local runtime (owner: runtime and model download).
- Honesty notes (B): the Ctrl+O and vault hand-over tests were written after the code and checked
  by mutation; `w4_ai_settings` had its local-panel key list updated to the mock's new Ctrl+O entry
  (the mock changed in this round; the assertion is as strict as before).
- Honesty notes: round A's API tests (`s21_ai_drafts`) were written after the code and checked by
  mutation (ignoring decisions fails two); the app row test (`s21_review_drafts`) and the provider
  tests were checked failing by mutation too. Two 1.0.0 test files (`tally-bridge/tests/w3_import.rs`,
  `app/tests/i18n_ai.rs`) were touched for compile only (a new enum arm, a new struct field). The
  commit trailer names Claude Opus 5.5, the session's model, where the brief's template says Sonnet.
