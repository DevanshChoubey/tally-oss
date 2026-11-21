# Wave 3 (finish the wiring): status, 2026-11-21

Eight code-only cards (C1 to C8) worked in their own worktrees, all starting from main `5bca530`.
The coordinator merged them into `integration` and fixed what failed.

**Gates at `c6e2cd9`:** fmt, clippy (`-D warnings`), doc (`-D warnings`), doctest and deny are
clean. nextest: 2473 of 2473 passed. After the shell wiring, 2483 of 2483.

## What is in

| Card | What it added |
|---|---|
| C1 | `Api::books`; `CreatedId` and `CommandResult::created`; a Create* command with id 0 gets the largest id + 1. The shell fetches the books after open, create and every booked command. Masters Create and voucher-entry Alt+C go through the worker. |
| C2 | `Api::voucher_gst` / `invoice_gst`. PostVoucher and AlterVoucher recompute GST and refuse typed tax lines that disagree. The item invoice screen gets its tax block from the worker. `RulePack::merge`, plus `gstr-rules.toml` with the B2CL threshold (Rule 59(4), Notification 12/2024-CT). |
| C3 | `Event::BankStatementStored`: statements survive reopening, and duplicate detection works across sessions. CSV balances come from the balance column. The MT940 `:62F:` end date is stored, and the BRS covers it. The bank screen runs on the worker. |
| C4 | GST screen (GSTR-1, GSTR-3B), the direct import path, and home KPIs, all on the worker. |
| C5 | `shell/navigate.rs` (report menu, open targets), palette search on the worker, `VoucherViewScreen`. |
| C6 | Projection tables maintained by commit; `enable_projections`; the rebuild check can report a match. |
| C7 | The Tally import takes opening balances and warns about vouchers outside the period. |
| C8 | `c8_e2e.rs`: masters, GST invoices, a bank statement, backup and the chain check, end to end. |

## Owner decisions this wave (2026-11-21)

- **Projections on by default.** `Api::create` switches maintenance on. With approval, eight tests
  that pinned the old gaps now assert the closed ones, and each carries a comment naming the old
  gap:
  - the w2 rebuild check, in the API and the CLI;
  - the c6 opt-in test;
  - three w5_bank tests: CSV balances, the end date, and a statement kept across a reopen.

  The store's own default stays off, so the s6 tests are unchanged.
- **"Everything should match Tally": Main Location.** TallyPrime keeps all stock in its default
  godown, Main Location, while multiple godowns is off, and offers no godown list during entry
  (<https://help.tallysolutions.com/tally-prime/godowns-locations/godowns-locations-faq-tally/>
  and
  <https://help.tallysolutions.com/docs/te9rel65/Creating_Masters/Inventory_Info/faq-godowns.htm>,
  accessed 2026-11-21).
  - Every company file is now seeded with godown 1, "Main Location" (`ledgerly_model::seed_godowns`).
  - The item invoice puts each line there when it is the only godown.
  - With more than one godown the invoice is still Unsupported, because there is no picker yet.
  - The w8, c2 and c8 tests had been creating Main Location by hand; they now use the seeded one.
  - u5's one-godown check now expects the Tally allocation and gained a two-godown refusal check.
  - Before this change, an item invoice could not be saved in a new company.

## Fixed during integration

- **Lint and format:** `clone_into`, merged match arms, `map_or_else`, doc headers holding
  numbers, too-similar names, private field access, and a double mutable borrow in voucher entry.
- **C3:** handled the new event in the store, the loader and the Tally import, and repaired its
  test file.
- **C4:** a new `egui::Grid` in the import screen re-ran the frame and lost the Esc key, so a u10
  test failed. It now draws plain rows. **Latent risk:** other Grids in key-handling screens can
  lose keys the same way in the running app.
- **C8:** the expected MT940 closing was an arithmetic slip, now corrected to 446,950. Purchases
  from an unregistered supplier now expect Unsupported GST.

## Pending (cited, or no source yet)

- **Opening balance sign on import:** negative is Dr and positive is Cr. This was derived from
  Tally's case-study material and needs an observed capture.
- **Masters in a locked period:** masters can be created in a locked period. The citation is
  weak.
- **Reuse of ids:** an id can be reused after the newest master of its kind is deleted.
- **Alt+C:** the inline ledger is not tested.
- **Rule pack:**
  - `itc.blocked.*` and `gstr1.hsn_credit_notes` are still `pending`.
  - The merged pack is not yet the bundled one, because switching needs approval to change the
    w4 tests.
  - Round-off is Unsupported, since there is no Invoice Rounding ledger type.
- **Voucher drill** shows the voucher read-only. The default report period for reports other than
  the Trial Balance has no source.
- **Silent default:** `Api::bank_statements` turns a store read error into an empty list. It
  should return the error.

## Shell wiring (coordinator, after the cards)

Now in the running app:

- **Palette:** Ctrl+K, or Alt+G as in TallyPrime's Go To, opens the palette over any screen.
  Ctrl+G opens it to switch.
  - The palette lists the commands, the report menu, Home, GST Returns, Import from Tally, and
    one "Bank Reconciliation" row per bank ledger. Ledgers and vouchers come from the API search.
  - The palette is drawn before the screen. While it is open, the screen under it gets no keys,
    so Esc closes the palette and does not go back.
  - A pick that opens nothing leaves a notice in the bottom bar.
- **Home** (`shell::GatewayScreen`) turns each link into a screen:
  - F8 opens the item invoice when the company keeps inventory, otherwise voucher entry.
  - F6 and F5 open voucher entry; B and D open the worker-mode reports.
  - G opens the GST screen. It needs exactly one registration; otherwise a notice says why.
  - M opens the masters.
  - The unmatched bank lines open the bank reconciliation when there is one bank ledger. With
    more than one, a notice says to choose one from the palette.
  - Purchase from a bill, describing a transaction and pending IMS say they are not there yet.
  - The KPI tiles are asked for again whenever the loaded books change.
- **Welcome:** once a file opens or is created, the Welcome screen hands over to Home. Ctrl+T
  opens Home while a company is open.
- The import and bank-confirm screens now mark the books changed, so the shell fetches them
  again.
- Tests: `crates/ledgerly-app/tests/c9_shell.rs` has 10 tests that drive `Shell::show` against
  real company files. Each was checked to fail with its wiring disconnected.

Still open:

- **First screen after open:** Home, as the design mock shows it. No Tally page read on
  2026-11-21 says which screen comes first, so this is `pending`.
- **Default GST and bank period:** the financial year up to the last voucher dated in it. No
  Tally source, so `pending`.
- **Commands from the palette:** a palette command other than new tab, close tab, back or the
  palette itself only shows a notice. Assistant intents say the assistant is not connected.
- **Several GSTINs or bank ledgers:** there is no picker on the GST screen or on the Home bank
  link yet.
