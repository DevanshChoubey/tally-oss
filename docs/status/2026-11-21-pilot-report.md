# Wave 1 pilot report (2026-11-21)

27 cards were coded on branches `w1/<card>` and merged into branch `integration`, which has no
code conflicts. **Most of this code has never been compiled.** On 2026-11-20 the owner parked
builds and tests. The "compiled" column says whether a card compiled and passed its own tests
before that and was not changed afterwards. Run fmt, clippy, nextest, doc and deny on
`integration` before trusting anything here.

Every card was reviewed once, read-only. A card the reviewer marked "changes" got one fix round,
and the fix itself has not been re-reviewed. Agents used: 42 in the final run and about 48 across
the earlier runs that were stopped. The final run used 4.8M subagent tokens.

**Integration compile (2026-11-21, later the same day).** The owner allowed one controlled build
of `integration`. After lint, rustdoc and one borrow fix (commits 2616a53, 33eedd7), card G4
gained purchase and debit-note GST from the supplier's state (b01d8ad), which the three A1
purchase tests needed. Now green on `integration`: `cargo fmt --check`, clippy `-D warnings`,
rustdoc `-D warnings`, `cargo deny check`, and nextest with 634 of 634 tests passing. No test
assertion was loosened. The "Compiled" column below describes the cards before this build.
Nothing is pushed or merged to `main`.

**Wave 1 complete (2026-11-21).** The other 66 Wave 1 cards were written code-only and merged
into `integration` with no conflicts. One controlled build then made all 93 cards green (commit
00dcbba): `cargo fmt --check`, clippy `-D warnings`, rustdoc `-D warnings`, doctests,
`cargo deny check`, and nextest with 2153 of 2153 tests passing. Four tests failed at first.
None was loosened:
- g5_rcm's expected value had a wrong hand derivation. It was corrected, and a case was added
  that tells per-line rounding from per-head rounding.
- u8_gst and u11_assist_panel kept the action from egui's second pass in a frame, not the first.
- s3_verify assumed SQLite's foreign keys are off by default, but bundled libsqlite3-sys turns
  them on.

Like the pilot cards, the 66 cards had one read-only review before anything was compiled: 8 lane
reviewers, followed by one fix round.

**Second review (2026-11-21).** 10 read-only reviewers checked all 66 cards on the compiled code.
They found 22 blocking, 68 should-fix and 38 nit findings; 87 findings from the first review were
still open. A fix round followed: 10 code-only agents running on Claude Haiku 5.5, each in its
own worktree. It fixed 74 findings and left 42, each with a reason. After the merge and one
controlled build, every gate passes with 2248 of 2248 tests (commit 0aed364). Tests that pinned
a silent default were replaced with stricter ones. None was deleted without a replacement, and
none was ignored. Open decisions and unfixed findings are in
`docs/status/2026-11-21-review2-decisions.md`, and the new interface requests are in
`docs/interface-requests.md`. Nothing is pushed or merged to `main`.

| Card | Status | Compiled | Review | Blocking | Fixed |
|---|---|---|---|---|---|
| C1 | done | yes | approve | 0 | - |
| M1 | done | yes | approve | 0 | - |
| M4 | done | yes | approve | 0 | - |
| S1 | done | no | approve | 0 | - |
| S2 | done | no | approve | 0 | - |
| S4 | done | no | changes | 1 | yes |
| E4 | done | yes | approve | 0 | - |
| E5 | partial | no | approve | 0 | - |
| E7 | done | yes | approve | 0 | - |
| E3 | done | no | changes | 1 | yes |
| RP1 | done | yes | approve | 0 | - |
| RP2 | done | no | approve | 0 | - |
| RP4 | done | no | approve | 0 | - |
| G1 | done | yes | approve | 0 | - |
| G2 | done | no | approve | 0 | - |
| G3 | done | no | approve | 0 | - |
| G4 | done | no | changes | 3 | yes |
| T3 | done | yes | approve | 0 | - |
| T4 | done | no | changes | 1 | yes |
| K1 | done | yes | approve | 0 | - |
| BF2 | partial | yes | approve | 0 | - |
| X2 | partial | no | changes | 1 | yes |
| X3 | partial | yes | approve | 0 | - |
| A1 | done | no | changes | 1 | yes |
| Q1 | done | no | changes | 2 | yes |
| U1 | done | no | changes | 1 | yes |
| U4 | done | no | changes | 1 | yes |

Partial cards:
- **X3, BF2, X2:** each needs a card outside the pilot: X1's `Envelope`, or BF1's `FormatSpec`.
- **E5:** `Books` has no lock date, so `lock_date(&Books)` is not built.

## Pending provenance (the owner must observe or source these)

- **C1:** amount_in_words for whole-rupee amounts (e.g. 'Rupees One Only'): the card gives no example of the no-paise wording
- **C1:** amount_in_words for sub-rupee amounts ('Fifty Paise Only') and zero ('Rupees Zero Only'): wording is an extrapolation
- **C1:** amount_in_words for negative amounts: implemented as a 'Minus ' prefix, no source
- **C1:** amount_in_words for amounts of one lakh crore rupees or more: implemented as '... Lakh Crore', no source for the wording
- **M1:** Nature of Suspense A/c: R1 marks it 'unconfirmed' (inferred), so no provenance; seeded as nature None.
- **M1:** Nature of Branch / Divisions: R1 marks it 'unconfirmed', so no provenance; seeded as nature None.
- **M1:** Exact display names (e.g. 'Misc. Expenses (ASSET)', 'Cash-in-Hand') are not covered by tests; R1 gives names only as help-page text and the card does not pin spelling.
- **S2:** Byte-exact known-answer hash vector from an independent CBOR implementation. Until then the encoding is pinned only by the test's own ciborium/sha2 recomputation of the tuple, which is stated in the test header.
- **S4:** First clippy run should confirm that the file-level unwrap_used allow is enough, and that no other lint (for example indexing_slicing in helpers) fires.
- **S4:** s1_schema.rs:139 and :189 still use the catch-all `other => panic!` arm; that is S1's to fix.
- **E5:** lock_date(&Books) -> Option<Date> from the frozen module doc is not implemented. Books has no lock-date field, so any value computed here would be a guess. The lock date must be supplied by the W1 integration layer.
- **E5:** The card's check_unlocked(date, &Books) is implemented as check_unlocked(date, lock: Option<Date>), matching the frozen module doc.
- **E5:** The lock reason and actor are not in the Event, because PeriodLocked only has until. They go in the store's AuditMeta at commit time. The 'lock event recorded with reason' test therefore checks only the event, not a reason.
- **E7:** Credit-period start date: counted from the voucher date. Whether TallyPrime counts from the voucher date or the supplier reference date has no provenance, so this is an assumption and an observed fixture is needed. Only the due date is affected, never the open amount.
- **E7:** Order, note and stock voucher kinds (ReceiptNote, DeliveryNote, RejectionIn/Out, StockJournal, PhysicalStock, SalesOrder, PurchaseOrder) that carry bill allocations return Invalid. Whether any of them legitimately carries bill refs in TallyPrime is unobserved.
- **E7:** Reuse of a bill name after the bill closed: treated as Duplicate, with no observed Tally behaviour behind it.
- **E3:** Controller to run cargo fmt --check, clippy -D warnings and the E3 tests once centrally.
- **E3:** Controller to record the interface requests in docs/interface-requests.md (not owned by this card): lock date in Books, pub(crate) check_gst_number in numbering.rs, and the W1 rule to call only *_with_lock functions until Books carries the lock.
- **G2:** Notification text for N.9/2025, N.15/2025 and N.14/2025 has not been read; the 22 Sep 2025 slab and special-rate entries rest on research 04-gst.md 2.2 [V] and need a person to check them.
- **G2:** Special rates 1.5%, 0.125% and 0.75% from the research are not encoded; no item mapping is known.
- **G2:** No HSN-to-rate mappings are encoded; the three HSN entries (7102, 7108, 2402) only reserve keys and are pending.
- **G2:** Pre-22-Sep-2025 slabs (0/5/12/18/28) are pending ([K] in research).
- **G2:** Cess entries (tobacco, non-tobacco goods) are pending.
- **G3:** The s.10(1)(ca) rule (unregistered recipient, address on the invoice) is unconfirmed in the research, so it is only used as a cross-check. If the address disagrees with the delivery State, the result is Unsupported.
- **G3:** The state-code table and the UTGST territory list in ledgerly-core are marked 'recalled, unverified'. The Chandigarh UTGST test depends on that list.
- **G4:** The exactly-50-paise round-up case (exactly_fifty_paise_rounds_up) is marked `pending` in its provenance. It pins the half-up product policy, not a statutory fact. It needs the CGST s.170 text or an observed invoice from an accounting tool.
- **G4:** Purchase and debit-note GST is Unsupported until a supplier-aware computation exists. That would use the party's state or GSTIN prefix, handle composition and unregistered suppliers, and cover reverse charge and ITC.
- **T3:** Valuation of a negative closing quantity has no provenance (R3 open question 3), so average_cost returns StockError::Invalid instead of a number.
- **T3:** Moves dated before period.from (carrying a prior period's closing in as opening) are not derived, so they are refused with StockError::Invalid.
- **T3:** Whether TallyPrime's Average Cost equals this periodic definition is not observed (spec open question 1). The tests pin our documented definition only.
- **T4:** Run fmt, clippy and test once CPU use is allowed again.
- **T4:** Wave reviewer to decide on StockError::Unsupported; T4 then switches the unsupported cases from Invalid to Unsupported.
- **X2:** parse_masters(&Envelope) wrapper once X1 defines Envelope
- **X2:** Capture real TallyPrime master exports to add observed fixtures
- **X3:** Inner tag names for inventory, batch and bill allocations (STOCKITEMNAME, BILLEDQTY, ACTUALQTY, RATE, GODOWNNAME, BATCHNAME, BILLTYPE, NAME) are not in R8 (it names only the list tags). Fixtures are hand-built shapes, so these are pending an observed Tally capture.
- **X3:** Sign convention for BILLALLOCATIONS, inventory and batch amounts: R8 documents it only for ledger lines. Those amounts keep the sign as written and are not mapped to DrCr.
- **X3:** Quantities and rates are tested only in the simple 'N Unit' and 'R/Unit' forms. Compound quantities (containing '=') return an error and are unsupported.
- **X3:** Invalid control characters and &#4; inside a voucher are not tolerated here. That belongs to X1's decode. quick-xml errors on them, and the parser returns a typed error rather than a guess.
- **A1:** Integrator: run cargo fmt/clippy/test; switch the DraftCheck #[allow(clippy::large_enum_variant)] to #[expect] or remove it once clippy has run.
- **A1:** Rupee rounding policy remains G4's own and unverified; the round-off tests cite it as such.
- **U1:** Run clippy --all-targets -D warnings and rustdoc -D warnings once the CPU freeze lifts.
- **U4:** Ctrl+D (delete line) and Alt+S (switch side) have no spec source. They are kept as provisional bindings in one const table, with the need logged for U2.
- **U4:** The UI-side sums in VoucherDraft::balance and build_voucher remain until ledgerly_core::money_ops::sum and validate_voucher exist. Both are stubs today.
- **U4:** Private copies of format_amount/group_indian and parse_entry_date remain until C1 and C3 merge.
- **U4:** The split of voucher_entry.rs (about 1900 lines) is requested but not done.
- **U4:** Several test expectations are still design choices with no observed provenance. They are now labelled as pending: type-ahead ranking order, the first side per kind, and the 50-item cap.

## Interface requests (for the wave reviewer)

- **C1:** amount_in_words returns String, so it cannot return Unsupported for the pending cases (negative, zero, sub-rupee, whole-rupee, 1 lakh crore and above). Request: change it to return Outcome<String> or Result, or have the owner confirm the house wording for those cases.
- **M1:** Group.nature is Option<Nature>, so Suspense and Branch/Divisions seed with None. Downstream consumers (balance sheet, reports) must treat a primary group with nature None as Unsupported, not default it. Needs the owner to confirm both natures with provenance (observed from the incumbent app, or a cited source) before they are filled in.
- **M4:** None filed. validate_voucher cannot know which ledger an existing bill belongs to, so BillOnWrongLedger only fires on a bill opened (New/Advance) on one ledger and settled (Against) on another within the same voucher. The engine needs a store-backed check for the rest.
- **S4:** docs/interface-requests.md: added section 'S4: replay failures need their own StoreError variant' asking for a StoreError::Replay or InconsistentLog variant, and noting the projection-table question for S6.
- **E4:** Rule 46(b) length and character check is applied only to BaseVoucherKind::Sales when features.gst is on. CreditNote and DebitNote (CGST rule 53) are not checked because I could not source from the repo whether a debit note is supplier-issued; the owner should decide whether to extend it.
- **E4:** Voucher types with a GST-document flag would let the check be explicit instead of inferred from Sales plus features.gst.
- **E4:** Monthly restart is read as a calendar month; the spec says only 'each month'. The owner should confirm.
- **E5:** E5: ledgerly-model Books (crates/ledgerly-model/src/books.rs). Books has no lock date, so lock_date(&Books) -> Option<Date> cannot be implemented. Request one of: add `lock_until: Option<Date>` to Books (folded from PeriodLocked events), or confirm that callers pass the lock date to check_unlocked as implemented. Needed for spec section 5 invariant 8 so E3 can call the guard on post, alter and delete. I could not append this to docs/interface-requests.md because that file is off-limits.
- **E7:** Ledger needs an opening bill breakup (for example opening_bills: Vec<OpeningBill> with name, date, due and a SignedAmount summing to the ledger opening) so open_bills can include opening bills. Not filed in docs/interface-requests.md because the brief forbids editing that file; file it there.
- **E7:** Optional: open_bills and validate_bill_refs return Result<_, EngineError> as declared, not Outcome. No Unsupported path exists; every unsupported construct is a typed error. Confirm this is acceptable.
- **E3:** Blocking decision before Wave 2: add a period lock date to Books (E5 also asks), or instruct W1 to call only post/alter/cancel/delete_voucher_with_lock; the frozen four functions enforce no lock.
- **E3:** Make numbering.rs check_gst_number pub(crate) so vouchers.rs can drop its copy.
- **E3:** Controller to append these to docs/interface-requests.md (reviewer and builder do not own it).
- **RP1:** ReportKind has no variant for an item's vouchers or a bill's originating voucher, so drill_down returns None for DrillKey::Item and DrillKey::Bill. If those should drill, add a kind (for example ItemVouchers(StockItemId)) and tell RP1.
- **RP1:** drill_down(report,row) has no column argument, so a click on a Comparative or Monthly cell drills with the report's whole period, not that column's period. A column-aware variant would need a new signature.
- **RP4:** Engine card E6 `effective_postings` is not built yet, so profit_loss.rs applies the Scenario filter itself (regular always, post-dated if on or before as_of or named, optional/cancelled only if named). Switch it to the engine function once E6 lands.
- **RP4:** The frozen signature takes stock as `&Outcome<Vec<StockValuation>>` dated as_of = day before period start (opening) and period end (closing). A wrong date is a ReportError::Invalid. Confirm the caller (the orchestration layer) follows that convention.
- **G1:** The plan card says RulePack::load and RuleSet::for_time_of_supply, but the frozen module doc says load_pack and RulePack::for_time_of_supply. I implemented the module doc and added RulePack::load as a thin alias. Please reconcile the card text with the module doc.
- **G4:** Appended to docs/interface-requests.md: G4 RuleSet lookup by id prefix, or an iterator over verified entries in force, to replace the hard-coded SLAB_IDS probe.
- **T3:** StockError has no Unsupported variant, so refused-by-design cases use StockError::Invalid { field, reason }. Consider adding StockError::Unsupported { reason } so callers can tell 'not supported yet' from 'bad input'.
- **T3:** The frozen avg.rs lists only average_cost. I added pub fn average_consumption(item, moves, period) -> Result<Paise, StockError> (opening + inwards - closing, R3 consumption). It is additive; the reviewer should confirm it belongs in this module.
- **T3:** Same-day ordering for the negative-stock check follows slice order (sorted by date, voucher). T5's first_negative may choose end-of-day instead; the reviewer should align the two.
- **T4:** T4: StockError::Unsupported { reason }. Now recorded in docs/interface-requests.md. It was missing before.
- **BF2:** BF2: the frozen signature parse_csv(bytes, &FormatSpec) cannot be implemented. FormatSpec is declared only in the registry.rs doc comment (BF1) and is not defined anywhere in wave-0, and there is no w1/BF1 branch. csv.rs therefore defines CsvLayout (with AmountColumns and DateFormat) and provides parse_csv_with_layout(bytes, &CsvLayout). Once BF1 lands, either BF1 exposes a conversion FormatSpec -> CsvLayout (needs the same fields: delimiter, skip_rows, column indexes, Split/Signed/WithIndicator amounts, date format and separator, century base, format id) or the wave reviewer adds a thin parse_csv wrapper. I did not touch docs/interface-requests.md, as instructed.
- **X2:** Resolve whether RawElement is replaced by X1's tree type and add parse_masters(&Envelope) as a thin wrapper
- **X2:** Decide whether MasterRecord aliases stay or become distinct newtypes
- **X3:** X3 depends on X1: the frozen parse_vouchers(&Envelope) signature needs Envelope, Header and RawMessage from tally-xml/src/envelope.rs, which X1 has not built. Once X1 lands, add a wrapper in vouchers.rs. Its shape depends on whether RawMessage keeps the TALLYMESSAGE subtree as text (re-parse each with parse_vouchers_xml) or as a tree. Request: X1 should expose the decoded text, or each message's text, so X3 can wrap parse_vouchers_xml without a second tree walk.
- **X3:** docs/interface-requests.md is not mine to edit, so this request is only recorded here for the wave reviewer.
- **X3:** parse_vouchers_xml is a public addition to the frozen X3 interface, not a replacement for it.
- **A1:** place_of_supply on VoucherDraft (appended to docs/interface-requests.md)
- **A1:** AssistError variant for engine and GST errors (appended)
- **A1:** Acceptance of validate_draft_with_place and Transcribed::is_confident (appended)
- **Q1:** books_small() expect: appended to docs/interface-requests.md for the controller to accept or convert to Result.
- **U1:** Appended to docs/interface-requests.md: update design/mock/index.html to the corrected light tokens ink_3 #5D6A82, haldi #8F5C0C and leaf #286F45. The mock's values fail WCAG 1.4.3 at 3.82, 4.41 and 4.42 to 1. Also listed Wave 2 open items: keymap routing for Esc, Ctrl+T and Ctrl+W, split view, scalable fonts.
- **U4:** Appended four sections to docs/interface-requests.md: the voucher_entry module split (rule C6), the AppCtx outbox/inbox for save and ledger-create requests, the keymap bindings for U2, and the swap to C1/C2/C3 shared helpers. The last one also covers dropping the duplicated Unbalanced check once the engine owns it.

## Compile risks and unfixed should-fix findings

- **C1 (review, should-fix, not yet fixed):** amount_in_words returns guessed wording for cases with no source: zero ("Rupees Zero Only"), sub-rupee ("Fifty Paise Only"), whole-rupee ("Rupees One Only"), negative ("Minus " prefix) and 1 lakh crore or more ("Lakh Crore"). The rules prefer Unsupported to a guess, but the frozen signature returns String so it cannot. The builder flagged all of these as pending and left them untested, which is the right handling.
- **C1 (review, should-fix, not yet fixed):** The builder did not capture a failing run of the assertions before implementing (report: only the compile error was seen). Weak TDD evidence, though the expected values are hand-derived.
- **M1 (review, should-fix, not yet fixed):** The module doc and the `reserved_groups` doc use intra-doc links to the private const [`ALL`]. A public item linking to a private one trips rustdoc::private_intra_doc_links. Wave 0 ran rustdoc with -D warnings, so `cargo doc` would likely fail.
- **M1 (review, should-fix, not yet fixed):** `unconfirmed_natures_are_not_guessed` is labelled `provenance: derived`. R1 marks both natures "unconfirmed", and the test asserts a pending decision (None), not a derived value. The primary-natures test is also labelled derived, but R1 says the nature column is "our inference" (standard accounting plus group names), not an observed fact.
- **M1 (review, should-fix, not yet fixed):** `reserved_groups` uses `filter_map` with `?`. If `id_of` ever failed, groups would be dropped silently and the seed would be short. It cannot fail today, because the ids come from ALL, and the 28-count test would catch a short result. This is a small departure from "never silent".
- **M4 (review, should-fix, not yet fixed):** MissingParty is enforced for Sales, Purchase, CreditNote and DebitNote in every VoucherMode. The card names only Sales, and the model defines VoucherMode::AsVoucher as 'Plain Dr/Cr lines'. A Sales voucher entered as plain Dr/Cr, including one imported that way, is rejected with MissingParty. That is a likely false rejection, and the rule goes beyond the card and spec invariants 1, 2 and 6.
- **M4 (review, should-fix, not yet fixed):** Purchase, CreditNote and DebitNote are enforced in the code but have no test. The builder's own report says only Sales and Journal are tested. The MissingParty provenance comment cites an error text ('this voucher kind needs a party') that differs from the real one ('this voucher needs a party'). The file header says a Journal has no party field, but Voucher.party exists.
- **M4 (review, should-fix, not yet fixed):** No test covers a credit-side bill-wise line mismatch, so the signed reporting of BillAllocationMismatch (Cr as a negative Paise) is unverified. The field docs in error.rs say 'Line amount' and 'Sum of allocations', which does not say they are signed.
- **M4 (review, should-fix, not yet fixed):** A bill-wise ledger line with no allocations is rejected as BillAllocationMismatch (allocated 0). Invariant 2 can be read that way, but Tally normally allows plain 'On Account' postings. Valid imported vouchers could be rejected. This is a judgment call recorded only in the builder report.
- **S1:** Not compiled by order. I checked rusqlite 0.43 pragma_update, pragma_update_and_check, pragma_query_value and Transaction Deref against source.
- **S1:** Text file open yields NotACompanyFile only if SQLite reports SQLITE_NOTADB on the first PRAGMA read. I reasoned this from SQLite page-count rounding, not from source. The test file is 54 bytes.
- **S1:** STRICT tables need SQLite 3.37 or later. The bundled build is 3.54.1, so this is fine.
- **S1:** Ok(self.conn.query_row(..)?) in journal_mode might draw a clippy needless_question_mark warning. The error types differ, so I expect it won't.
- **S1:** The #[allow(dead_code, reason)] on SqliteStore.conn in sqlite.rs is not my file. It stays harmless because it is an allow, not an expect.
- **S1:** The test GSTIN 29ABCDE1234F1ZW is hand-derived in ledgerly-core's gstin.rs tests.
- **S1 (review, should-fix, not yet fixed):** The helper fns date(), company() and new_store() call unwrap() but have no #[test] or #[cfg(test)] attribute. clippy.toml's allow-unwrap-in-tests may not cover helpers in an integration-test crate, so `cargo clippy --all-targets -- -D warnings` could fail on clippy::unwrap_used. I could not compile to confirm.
- **S2:** Nothing was compiled or run. This session reviewed the existing commit by reading source and ran only rustfmt --check, which passed.
- **S2:** Confirmed by reading source: rusqlite 0.43 has ToSql and FromSql for [u8; N]; hybrid-array 0.4.14 has From<Array<T,U>> for [T;N], which the sha2 0.11 digest().into() relies on; EventSeq is a core id newtype with a pub u64 field (defined by macro in ids.rs, derives not re-read); Clock::Fixed and OffsetDateTime::UNIX_EPOCH exist.
- **S2:** Not confirmed: that ciborium serialises &[u8;32] and OffsetDateTime deterministically through serde. The time serde feature is assumed to be enabled in the workspace, because Event and AuditMeta already derive Serialize.
- **S2:** Not confirmed: clippy cleanliness under the workspace lints, such as pedantic lints on casts or doc backticks.
- **S2:** The failure-injection test needs a second SQLite connection on the same file while the store is open. That should work in WAL mode, but it is unverified.
- **S2 (review, should-fix, not yet fixed):** The module doc has bare identifiers containing underscores: 'sqlite.org/lang_createtrigger.html' and 'sqlite.org/lang_transaction.html'. clippy::doc_markdown (pedantic, warn, so -D warnings) flags words with underscores that are not in backticks. S1's header only cites underscore-free URLs. Nothing was compiled, so this is a likely clippy failure and not a confirmed one.
- **S2 (review, should-fix, not yet fixed):** `Sha256::digest(&bytes)` borrows a local Vec that is never used again. clippy::needless_borrows_for_generic_args (style, warn) will probably fire under -D warnings. The test helper `expected_hash` has the same pattern.
- **S2 (review, should-fix, not yet fixed):** The helpers `date`, `company`, `new_store`, `expected_hash` and `row_count` call unwrap outside any #[test] fn. clippy's allow-unwrap-in-tests only covers #[test] fns and #[cfg(test)] modules, so clippy::unwrap_used (deny) will probably fire on these helpers in the integration test crate. S1's test file has the same pattern, so the problem is shared and not new in S2. Unverified because nothing was compiled.
- **S2 (review, should-fix, not yet fixed):** Provenance for the hash is weak. The expected hash is recomputed with the same ciborium and sha2 path that production uses, so the test cannot catch a wrong CBOR encoding. The header discloses this and the report lists the known-answer vector as pending, which is allowed by rule A4/A5. Nothing weakens or hides it. `hash_depends_on_every_tuple_member` only exercises the test's own helper, although its comment says it also checks the public helper.
- **S4:** `#![allow(clippy::unwrap_used, reason = ...)]` placed after the `//!` docs and before the `use` items: inner attribute position should be valid, but is unverified by compiler.
- **S4:** assert_encoding_error takes `Result<Books, StoreError>` and uses `matches!(&result, Err(StoreError::Encoding(m)) if m.contains(needle))` then `{result:?}`; relies on Books and StoreError both implementing Debug (Books derives Debug; StoreError Debug not re-read, but the previous code formatted it with {other:?}).
- **S4:** `Books` is imported from `ledgerly_model` in the test; assumed to be exported at crate root (load.rs imports it from there, so it is).
- **S4:** negative_ledger_line_amount_fails asserts the message contains 'negative amount', which is the wording in load.rs postings_of; wording-coupled.
- **S4:** rustfmt was run (from ~/.cargo/bin) on s4_load.rs only; no cargo or compile was run.
- **E4 (review, should-fix, not yet fixed):** The rule 46(b) length and character check runs only for BaseVoucherKind::Sales when features.gst is on. Credit and debit notes (CGST rule 53) are also issued with a 16-character serial number and skip the check. The builder flagged this and gave a reason, but the spec has no decision on it.
- **E4 (review, should-fix, not yet fixed):** The GST tests (16-character limit, 16 vs 17 boundary, and the allowed character set of letters, digits, hyphen and slash) are labelled derived, but the rule text comes from the builder's recall. No source is quoted in the repo, and the builder's own notes say so. This is weak provenance under the expected-values rule.
- **E4 (review, should-fix, not yet fixed):** counter_of returns Invalid when an existing prefix+digits+suffix number has more than 19 digits and overflows u64. That one odd voucher then blocks all allocation in its scope. Refusing is safe, but the error message does not say which voucher is at fault.
- **E4 (review, should-fix, not yet fixed):** Monthly restart is read as a calendar month, while spec section 5 says only 'each month'. The builder asked for the owner to confirm. The test monthly_restart_starts_again_each_calendar_month encodes this reading, and its provenance is a reading of the spec, not an observation.
- **E5:** Nothing was compiled (CPU rule). I checked by reading source: EngineError::{PeriodLocked{until}, Invalid{field,reason}}; Event::PeriodLocked{until} (Debug, PartialEq); Outcome::Value (PartialEq); Books and Company field lists; Masters and Features and Address derive Default; EntityKind::Proprietorship; NegativeStockPolicy::Block; CompanyId(pub u64); StateCode::new(u8)->Result; time with macros feature in the workspace.
- **E5:** Test asserts Result<Outcome<Vec<Event>>, EngineError> with assert_eq!, which needs PartialEq and Debug on all of them. I confirmed EngineError, Outcome and Event derive both.
- **E5:** Test uses time::macros::date! and names time::Date as the same type as ledgerly_core::Date (a re-export of time::Date); engine Cargo.toml lists time, so this should resolve.
- **E5:** Not confirmed: whether clippy pedantic flags the Some(_) | None arm in check_unlocked; Option is not our enum, so wildcard_enum_match_arm should not apply.
- **E5:** rustfmt (edition 2024) ran clean on both files via the rustup toolchain binary, since rustfmt is not on PATH.
- **E5 (review, should-fix, not yet fixed):** The frozen interface lists lock_date(&Books) -> Option<Date>, but it is not implemented, and check_unlocked takes Option<Date> instead of the card's &Books. Books has no lock field, so omitting it is the honest choice and it is documented in the module doc, but E3 integration still depends on an interface decision.
- **E5 (review, should-fix, not yet fixed):** The card says 'lock event recorded with reason', but the test only checks Event::PeriodLocked { until }. Event has no reason field, so the reason is untested here. Also, post_alter_delete_on_lock_date_error is a single assertion on check_unlocked and does not exercise post, alter and delete separately.
- **E5 (review, should-fix, not yet fixed):** lock_period rejects until < books_begin with EngineError::Invalid. This is the builder's reading of invariant 6 and not in the card or spec invariant 8. It is reasonable, but it is unrequested behaviour, and nothing stops a caller from moving or lifting the lock.
- **E7 (review, should-fix, not yet fixed):** validate_bill_refs discards the result of effect_of(books, v). A Memorandum or ReversingJournal voucher carrying bill allocations is therefore validated, and its origins and settlements applied, as if it counted. The module doc says such vouchers never open or settle bills, and open_bills ignores them. This is inconsistent, not a wrong number.
- **E7 (review, should-fix, not yet fixed):** When validate_bill_refs alters or deletes an invoice that later vouchers settle, project() fails inside the stored later receipt. The user gets NotFound or OverSettled naming a bill and voucher they are not editing, which is a confusing message. Also, missing_bill's 'elsewhere' search ignores voucher status, so an Optional or Cancelled voucher can produce BillOnWrongLedger instead of NotFound.
- **E7 (review, should-fix, not yet fixed):** Settlements are applied after all origins regardless of date, so an Against dated before its bill's New date is accepted. Unobserved and not tested. The open amount is unaffected.
- **E7 (review, should-fix, not yet fixed):** Provenance is thin. Every test is 'derived' by comment, but the cited basis is an internal 'R1 BillAlloc' note and the spec invariant, not an observed Tally release or published documentation. The behaviour choices are unobserved and the builder correctly lists them as pending: credit days counted from the voucher date, Duplicate on bill-name reuse, and order or note vouchers carrying bills being rejected. The arithmetic is hand-checkable: 10,000-4,000=6,000, 8,000+5,000>10,000 with 2,000 open, 10 Apr+30 days=10 May.
- **E7 (review, should-fix, not yet fixed):** Not compiled: rustfmt is unavailable here, so formatting was not checked. By hand, I found no compile errors: Paise, SignedAmount, MoneyError, Date::checked_add, Option::is_none_or (1.82+), the VoucherError variants and fields, the EngineError variants, and the ledgerly_model re-exports all exist with matching signatures. Matches over BaseVoucherKind (all 16 variants), VoucherStatus, and BillRefKind are exhaustive with no wildcard arms. No unwrap, expect, panic or indexing in library code, no floats, and all money goes through checked methods.
- **E3:** rustfmt is not installed here, so it was not run; the new test code may need cargo fmt (e.g. wrapping of long let e = alter_voucher(...).unwrap_err() lines).
- **E3:** check_renumbering uses a let-chain (if a && let Some(..) = ..), valid in edition 2024 on the workspace rust-version 1.100; clippy collapsible_if style is satisfied.
- **E3:** New tests assume validate_voucher accepts the fixture Journal (NumberingMethod::None) voucher and 2027 dates without a books-end or books_begin complaint; not verifiable without compiling.
- **RP1 (review, should-fix, not yet fixed):** Provenance is tagged 'derived' but several expected values are design choices, not derivations. The column label formats ('01-Apr-2025 to 31-Mar-2026', 'Apr 2025') come from the builder's own convention. The drill target mapping (group to GroupSummary with Detailed, ledger to LedgerVouchers with Single columns and exceptions_only off) rests on one spec line, 'group -> ledger -> voucher'. Calendar and FY values (month ends, 29 Feb 2024, 365 days) are genuinely derived, and I rechecked the 365-day sum and the part-year clip by hand.
- **RP1 (review, should-fix, not yet fixed):** Comparative periods are not validated: reversed or overlapping periods pass through, and Monthly is only checked against the FY, not Comparative. Period::new already prevents reversed periods. Overlap is arguably legitimate for comparison, so this is not wrong behaviour.
- **RP2:** Nothing was compiled or linted, per the CPU rule. All types, fields and signatures were checked by reading ledgerly-core and ledgerly-model source.
- **RP2:** rustfmt was run on both files and exited cleanly.
- **RP2:** clippy::wildcard_enum_match_arm was avoided by listing Cell variants explicitly in the test helper dr_cr. Other clippy pedantic lints (for example doc_markdown or too_many_lines near the 80-line limit) are unconfirmed. build_report is about 64 non-blank lines after the refactor.
- **RP2:** time::macros::date! and proptest are used in the integration test. The workspace enables time's macros feature and proptest is a dev-dependency, but neither was confirmed by building.
- **RP2:** The helper two_line has 8 arguments with #[expect(clippy::too_many_arguments)] in test code. If the lint does not fire there, the expect becomes an unfulfilled-expectation warning.
- **RP2:** Borrow shapes in Builder::group, which clones Vec<&'a Group> and Vec<&'a Ledger> before calling self.group recursively, were reasoned about but not compiler-checked.
- **RP2 (review, should-fix, not yet fixed):** Helper fns outside any #[test] (run, dr_cr, row, totals, unsupported, books, params) use unwrap, panic! and slice indexing (cells[0], LEDGERS[*idx]). The workspace denies unwrap_used, panic and indexing_slicing. clippy.toml allow-*-in-tests may not cover plain helper functions in an integration-test crate, since it keys on #[test] or #[cfg(test)] items. Nothing was compiled, so this is unconfirmed, but it could fail the gate cargo clippy --all-targets -D warnings.
- **RP2 (review, should-fix, not yet fixed):** No test puts Dr and Cr ledgers with different sides in one group, so no test pins the rule that a group row shows the net of its ledgers. In the group-rows test both Current Assets ledgers are Dr and both Duties & Taxes ledgers are Cr. A bug that summed absolute values would still pass. The proptest only checks that the totals are equal.
- **RP2 (review, should-fix, not yet fixed):** In inclusion(), Scenario::Including(statuses) other than Optional (Regular, PostDated, Cancelled) is silently ignored. Also, `let detailed = match self.detail { Detailed => true, Condensed => false }` in Builder::group may trip clippy::match_like_matches_macro. The clippy trigger is unconfirmed.
- **RP4:** Nothing was compiled by the owner's rule. I checked by reading source the imports, field names and methods of Paise, SignedAmount, Period, Outcome, Group, Ledger, Posting, VoucherStatus, StockValuation, Books and Masters, plus the model re-exports and the exhaustive ReportKind match.
- **RP4:** Clippy lints not confirmed: vec_init_then_push on `let mut rows = vec![text_row(..)]` followed by pushes (I think it only fires on an empty vec), and pedantic lints I could not run.
- **RP4:** Borrow split in accumulate(): ledger_chain is read while group_net is mutated, through direct field access on `&mut Tree`. This should compile, since the borrows are of disjoint fields.
- **RP4:** Test file uses `#![allow(.., reason = ..)]` with the lint list copied from the style of other crates; not checked against another test file.
- **RP4:** StateCode::new(27) is assumed to be a valid code; the table was not read.
- **RP4 (review, should-fix, not yet fixed):** The proptest body binds five single-character names in one scope (`p`, the closure params `n`, `l`, `a`, and `r`). clippy::pedantic `many_single_char_names` (default threshold 4) will probably fire. The workspace sets pedantic to warn and CI uses `-D warnings`. Nothing was compiled, so this is unconfirmed. The test file's `#![allow]` does not list the lint.
- **RP4 (review, should-fix, not yet fixed):** The provenance comment in `closing_stock_comes_from_the_injected_valuation` says the two items are "20,000 + 10,000 = 30,000 gives the base case". The test's items are 25,000 and 12,500, which sum to 37,500. The expected value (45,000 + 7,500 = 52,500 = 5,250,000 paise) is correct, but the written derivation does not match the inputs.
- **RP4 (review, should-fix, not yet fixed):** Provisional profit cells use a different convention from final ones. A final profit is `Cell::Signed(profit)`, positive for profit. A provisional profit is `Cell::Provisional{amount: SignedAmount}` showing profit as a credit balance, so a gross loss appears as Dr. A UI or exporter that handles both cells the same way could show a loss as a profit. The behaviour is tested and documented in the helper doc comment. The report notes say only "positive for a profit", which does not describe the provisional cells.
- **RP4 (review, should-fix, not yet fixed):** Three implemented behaviours have no test. First, a primary group with no nature that holds a ledger returns Unsupported. Second, the same item valued twice in one stock input returns ReportError::Invalid. Third, a post-dated posting is excluded when Scenario::Including names another status and the posting is after as_of. The builder's report mentions the first two only as behaviours. The card's listed tests (GP formula on a hand set, injected closing stock) are all present, and no test was weakened.
- **G1 (review, should-fix, not yet fixed):** The test pack_selected_by_time_of_supply_not_entry_date binds `_entry_date` (2025-09-25) and never uses it. The API has no entry-date input, so the 'not entry date' claim is only a comment and nothing is checked against it. The values themselves are right: the supply date 2025-09-21 is the inclusive end of the 12 entry and 2025-09-22 starts the 18 entry.
- **G1 (review, should-fix, not yet fixed):** pending_entry_may_omit_legal_ref only asserts load_pack(..).is_ok(). It does not check that the entry loads as pending, has legal_ref() == None, and that RuleSet::get on it is Unsupported. Another test covers the get part, but only for an entry that also has params.
- **G1 (review, should-fix, not yet fixed):** RulePack::load is a public alias added beside the frozen load_pack, and the report does not say it was added to docs/interface-requests.md. The plan card says RulePack::load and RuleSet::for_time_of_supply, while the frozen module doc says load_pack and RulePack::for_time_of_supply. This duplicates public surface.
- **G2:** Nothing compiled, per the CPU rule. Only rustfmt --check was run (passes).
- **G2:** Tests rely on G1 API as read in pack.rs: load_pack, RulePack::{entries,version,for_time_of_supply}, PackEntry accessors, RuleSet::get returning Outcome<&PackEntry>.
- **G2:** Uses Option::is_none_or, so needs Rust 1.82 or later; G1 code uses it too.
- **G2:** Test helpers outside #[test] fns use unwrap/expect/panic under a crate-level #![allow] with reason; not compiled, so the clippy config was not exercised.
- **G2 (review, should-fix, not yet fixed):** cgst_plus_sgst_equals_igst_in_every_verified_slab has five single-char bindings in one scope (e, p, c, s, i). clippy::many_single_char_names is pedantic and warns above 4, and -D warnings turns that into an error. I could not confirm clippy counts them as one scope, because nothing is compiled. Separately, rustfmt was not on PATH here (exit 127), so I could not re-check the builder's claim that formatting passes.
- **G2 (review, should-fix, not yet fixed):** The 22 Sep 2025 slab entries (slab.zero/five/eighteen/forty, special.rough_diamonds, special.gold_jewellery) are not pending and carry a legal_ref, but the notification text for N.9/2025 and N.15/2025 was never read. They rest on research 04-gst.md 2.2 [V], which is secondary. The tests label this provenance 'derived (research [V])', which is weaker than a hand derivation from the notification. The builder and the file header both disclose this. The research also says corrigenda were issued.
- **G3:** Nothing was compiled, per the CPU rule. Types and signatures were checked by reading source only: Outcome::unsupported, StateCode::new and is_utgst_territory, RegistrationType variants, and the pub mod pos line in lib.rs.
- **G3:** Unused-import and dead-code lints were not checked by a compiler.
- **G3:** The result_large_err clippy lint should be fine, because Outcome<PlaceOfSupply> is about 48 bytes, but I did not confirm it with clippy.
- **G3 (review, should-fix, not yet fixed):** For an unregistered recipient and goods, the code cross-checks against an unconfirmed s.10(1)(ca) reading (address State, else supplier State). A common case is goods moved to another State for an unregistered buyer with no address on record. By s.10(1)(a) this is Inter, but the cross-check compares the supplier State with the destination, disagrees, and returns Unsupported. This is safe because it never gives a wrong number, but it is over-conservative and relies on a rule the builder says is unconfirmed.
- **G3 (review, should-fix, not yet fixed):** The Chandigarh UTGST test depends on the UTGST territory list and state table in ledgerly-core, which are marked 'recalled, unverified'. The expected value is derived from statute (UTGST Act, Chandigarh has no legislature), so provenance holds. The test would pass or fail on the strength of an unverified table.
- **G4:** Nothing was compiled; only rustfmt --check was run. item_history returns Result<Outcome<&GstHistory>, GstError> with elided lifetime. The `[one] => Ok(Outcome::Value(*one))` slice arm and the `[..]` arm rely on Vec<&GstHistory>::as_slice() binding `one` as &&GstHistory.
- **G4:** The new test uses Option::is_none_or (needs Rust 1.82 or later), and PackEntry::{effective_from, effective_to, is_pending, params, id} plus RulePack::entries, all of which I confirmed exist in pack.rs.
- **G4:** In the new tests, StateCode::new(99) is valid (confirmed in the core STATES table), and the Decimal comparisons rely on Decimal's numeric equality across scales.
- **G4:** `checked >= 6` in every_verified_slab_in_the_pack_is_found assumes six verified igst_percent entries are in force on 2025-09-22 (zero, five, eighteen, forty, rough_diamonds, gold_jewellery). I counted these from rates-slabs.toml by hand.
- **G4:** similar_names was avoided by renaming the find_slab locals to total_pct, central_pct and state_pct. It was not run through clippy.
- **T3 (review, should-fix, not yet fixed):** Test `rounds_down_below_half` does not round down. Its own comment shows 2 x 1.00 / 3 = 0.6666 Rs, which rounds UP to 0.67. The name misleads, and no test covers a remainder below half that rounds down.
- **T3 (review, should-fix, not yet fixed):** A move for this item with zero qty_in and zero qty_out but a non-zero `value` (a value-only adjustment) is silently ignored in `apply`. Rule A1 forbids silent defaults, and the pool value would be wrong without any signal.
- **T3 (review, should-fix, not yet fixed):** Refused-by-design cases (negative closing quantity, moves before the period start) use `StockError::Invalid`, which callers cannot tell apart from bad input. The spec (6.4) says valuation over negative stock is returned as Provisional; the builder returns an error for a negative closing quantity. This is defensible because it is a typed error and not a number. It follows the frozen error enum, and the builder already filed it as an interface request.
- **T3 (review, should-fix, not yet fixed):** `average_consumption` is an extra public function that is not in the frozen interface. It is additive and stays inside the owned file, so it is acceptable. It builds the pool twice, because `average_cost` rebuilds it. Also, the proptest property `consumption + closing == pool` is close to a tautology, since consumption is defined as pool minus closing.
- **T4:** Nothing was compiled, by owner's CPU rule. Only rustfmt (edition 2024) ran.
- **T4:** FinancialYear::containing returns Result<_, PeriodError>; I map the error with format!("{e}"), which relies on thiserror's Display impl.
- **T4:** Book::apply(m) is called with m: &&StockMove from `for m in live`. This relies on deref coercion to &StockMove.
- **T4:** Paise::checked_add / checked_sub are used with `?` into StockError. The existing code already did the same, via From<MoneyError>.
- **T4:** self.year.is_some_and(|prev| fy > prev) relies on FinancialYear being Copy and PartialOrd (derived in dates.rs).
- **T4:** `Date` is imported from ledgerly_core, where it is a re-export of time::Date, the same type as StockMove.date.
- **T4:** clippy too_many_lines: fifo() is now about 50 lines and apply() is about 22. I could not confirm clippy passes.
- **K1 (review, should-fix, not yet fixed):** Non-blocking: the 'all-or-nothing' test only checks that Err is returned. That is the real guarantee here, because the BankStatement fields are private and check_statement is the only constructor. The test does not assert it, though, and the report claims 'nothing stored' without a store being involved.
- **BF2 (review, should-fix, not yet fixed):** Frozen entry point parse_csv(bytes, &FormatSpec) is not provided because FormatSpec is not defined anywhere (BF1 not landed). The card is only partly delivered. This is a legitimate blocker outside BF2, but the wave integrator must add the FormatSpec to CsvLayout conversion plus a parse_csv wrapper, and the interface request must be recorded in docs/interface-requests.md (the builder was told not to write it).
- **BF2 (review, should-fix, not yet fixed):** The module doc says exactly one of debit and credit is non-zero on every line, but this is not enforced. A split row with debit '0.00' and an empty credit, or a signed '0.00' or '-0', yields debit=0 and credit=0 and is accepted. The invariant is stated but not guaranteed, and no test covers zero amounts.
- **BF2 (review, should-fix, not yet fixed):** The comment in structural_errors_and_edge_cases says invalid UTF-8 is a format error, but the assertion (correctly) expects BankFmtError::Row. Also no test covers a negative balance cell (balance goes through the same parse_amount, so a leading '-' is accepted).
- **X2:** Truncation test now passes a &str (raw literal) to parse_masters_text(&str) and uses full.len() on it; no wrap() so no closing tags. Not compiled; relies on the reviewer's reading that the parser yields Truncated{offset: len}, one master and one warning for this input.
- **X3 (review, should-fix, not yet fixed):** The module doc links to [`crate::envelope::decode`], but envelope.rs on this branch holds only contract text and has no `decode` item. rustdoc raises a broken intra-doc link warning, and the Wave 0 gates include rustdoc -D warnings. The link only resolves once X1 merges.
- **X3 (review, should-fix, not yet fixed):** The frozen entry point `parse_vouchers(&Envelope)` is not implemented, because Envelope, Header and RawMessage belong to X1 and do not exist yet. The card is therefore only partial. `parse_vouchers_xml(&str)` is a public addition to the frozen interface that is not in the plan. The builder reported the interface request but did not file it, because docs/interface-requests.md is not the builder's file.
- **X3 (review, should-fix, not yet fixed):** Provenance is thin. The sign convention comes from R8 2.3(e), which the research note itself says is an illustrative example written by the note's author, citing a vendor sample page. The inner tag names for inventory, batch and bill allocations (STOCKITEMNAME, BILLEDQTY, BATCHNAME, GODOWNNAME, BILLTYPE, NAME) are not in R8. The tests declare this honestly as hand-built fixtures and pending an observed capture. There is no test for an invalid ALTERID and none for a bill allocation missing NAME or AMOUNT. The assertions derived from the fixtures are sound, and none were copied from engine output.
- **A1:** Nothing was compiled (CPU rule); only rustfmt ran.
- **A1:** validate.rs: `details.and_then(|p| p.gstin.as_ref()).and_then(Gstin::state)` relies on Gstin::state(&self) -> Option<StateCode> (checked in ledgerly-core/src/gstin.rs); StateCode is Copy + Eq (checked).
- **A1:** validate.rs: or-pattern `(Some(a), _) | (None, Some(a)) => a` after a guarded arm; should compile, but the guard arm is the only one using `b`.
- **A1:** New test fixture adds ledgers 13/14 and voucher type 5; the engine path for a DebitNote voucher with party on the credit side was read in compute.rs but not run.
- **A1:** The reworked tests rely on G4 rounding 9,005.4 paise to 9,005 and the invoice value 118,070 paise to 118,100 (rupee rounding, half-up of .70); both are hand-derived and not ties.
- **Q1:** Crate-level #![allow(..., reason = ...)] placed after the //! inner docs in tests/q1_*.rs; inner attributes after inner doc comments are valid, but an unused allow is not flagged (allow, not expect).
- **Q1:** StateCode::get is `pub const fn get(self) -> u8`, so the path form `map(StateCode::get)` should type-check; StateCode is re-exported from ledgerly_core (lib.rs line 37).
- **Q1:** runner.rs fold closure uses `r.id` as a String (push_str(&r.id)); not compiled.
- **U1:** Nothing compiled or linted, per the CPU rule. The only tool run was rustfmt on tests/u1_theme.rs, which was clean.
- **U1:** The changed hex literals are 0x00RR_GGGG style (for example 0x00F2_F4F7), the same form as theme.rs. They are compared against hex(), which returns u32, so the types still match.
- **U1:** The rewrite was a regex over every hex literal in u1_theme.rs. A grep afterwards found no remaining ungrouped literal of 5 or more digits, but clippy has not confirmed it.
- **U4:** Nothing was compiled, per the CPU rule. Only rustfmt was run.
- **U4:** ctrl_enter_does_not_save asserts only that no save request exists and the error is not 'no lines'. With Ctrl+Enter held, a focused singleline TextEdit may still lose focus on Enter, and text_field then submits it as a plain Enter. I could not confirm this by reading egui 0.37, so the test avoids depending on it.
- **U4:** The new search_ledgers calls slice::select_nth_unstable_by and binds its tuple result to `_partitioned`. I could not read the std source to check for #[must_use], so the binding stays.
- **U4:** Clippy pedantic could flag a lint I did not foresee. Candidates are items_after_statements (the SortKey alias is now first in the fn), too_many_lines and cognitive_complexity on `draw` and `search_ledgers`.
- **U4:** The bindings module uses `pub(super) const` arrays and tuples of egui Modifiers and Key. Both are Copy and the Modifiers consts exist in egui 0.37 src/data/input/modifiers.rs.
- **U4:** The a_lone_percent test was rewritten, not just relabelled. Its new expected value is a design decision with no observed provenance, as the finding asked, and I labelled it that way.
