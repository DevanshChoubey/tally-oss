# App against mock v2: parity report (2026-11-22)

The app screenshots are in `design/app-captures/` (1280×800, taken with
`crates/ledgerly-app/examples/ui_snapshots.rs` after commit 478ddf5). The mock captures are in
`design/mock-v2/captures/` (see `CATALOG.md` there). A read-only sub-agent wrote this report.
The coordinator checked the main claims against the source: theme.rs comes from mock v1, F8 is
routed to the entry screen, there is no sidebar, and the Home widths are fixed. Items marked
UNVERIFIED were not confirmed against the source.

## 1. Summary

theme.rs is built from mock v1, not v2: indigo accent, the `haldi` turmeric warn colour, and a
yellow high-contrast accent. None of the v2 tokens is in the app: plum accent, copper AI colour,
the 4/6/10/14 radius scale, and the 11–26 px type scale.

The app is a working keyboard-first prototype with the engine behind it. It has no shell in the
mock's sense:
- no sidebar;
- no top bar with company, date, lock and AI chips;
- no keycaps, badges or proposal cards;
- no review queue.

Coverage of the 89 mock states:
- 26 have no app screen: ask, purchase-bill, quick-entry, review, keyboard map, AI settings.
- The 8 setup states share one welcome screen.
- The rest map to an existing screen, but none matches its capture.

Several functional bugs are real. The worst are F8 for companies without inventory, and Home
clipping at 1280 px.

## 2. Shell and design system (every screen)

| Area | Mock | App now | Change |
|---|---|---|---|
| Layout | 48 px top bar, 208 px sidebar (60 px rail on Ctrl+\\), 32 px key bar | `shell/mod.rs:337-355`: tab strip and breadcrumbs, notice and key bar, central panel; no side panel | new `shell/topbar.rs`, `shell/sidebar.rs` (Overview, Enter, Books, Compliance, Settings) |
| Top bar | brand, company chip F3, date chip F2, lock chip, search Ctrl+K, AI tier chip, theme and keymap | `chrome.rs` `tab_strip`: "Ledgerly", tabs, "+", "Theme: Light" | AppCtx (`screen.rs:24-33`) gains company, FY, today, lock date, AI tier. The lock date is in `ledgerly-engine/src/locks.rs`. The AI tier store is new |
| Key bar | keycap chips per screen, screen title, "Hold Alt or Ctrl for more" layers | one hard-coded line in `chrome.rs` `key_bar` | `ui/keycap.rs`, `Screen::keys()`, modifier layers |
| Tabs | 2 px accent underline | `selectable_label` (masters.rs:810, bank.rs:857, gst.rs) | `ui/tabs.rs` |
| Page header | breadcrumb, 20 px title, sub-line, actions with keys | `ui.heading` and `ui.weak` | `ui/page_head.rs`, `Screen::page_title()` and `actions()` |
| Cards | 10 px radius, hairline, shadow-1 | 4 px, no shadow (home.rs:685-720) | `ui/card.rs` |
| Tables | header, 13 px body, right-aligned tabular numbers, hover, accent-soft selected row | Grid, TableBuilder or selectable rows, mixed | `ui/table.rs` on TableBuilder. Indian grouping moves to `ui/format.rs` |
| Badges, confidence, provenance | `.badge`, `.conf`, `.prov` | coloured text only | `ui/badge.rs`, `ui/conf.rs`, `ui/prov.rs` |
| AI proposal card | dashed copper border, sources, checks, Accept Ctrl+A / Edit Enter / Reject Alt+D | none reachable. `screens/assist.rs` exists but `AssistScreen` is never constructed | `ui/prop.rs` with `Shape::dashed_line`. Reuse the sound parts of assist.rs |
| Buttons | label plus keycap | "Save (Ctrl+A)" as text | `ui/button.rs` |

**Tokens** (theme.rs: LIGHT at 123, DARK at 142, HC at 161):

| Token | Mock light | App light |
|---|---|---|
| page bg | #F6F5F2 | #F2F4F7 |
| text | #1C1B19 | #172033 |
| accent | #6B2D73 plum | #3541A8 indigo |
| ai | #9A4211 copper | none |
| ok, warn, bad | #1E6E3F, #7E4C00, #B3261E | #286F45, #8F5C0C, #C23B35 |

The app's dark theme is navy `#0E1320`; the mock's is warm `#141312`. The app also lacks:
- a type scale;
- spacing tokens (raw floats are used instead);
- the radius scale: it uses 3 and 4, where the mock uses 4/6/10/14.

Renaming the token fields touches `theme.rs`, `shell/chrome.rs`, `shell/mod.rs`,
`tests/u1_theme.rs` and `tests/u1_shell.rs`.

**Other gaps:**
- **Fonts:** none are loaded, so Hindi names render as boxes. Add Inter and Noto Sans Devanagari
  (both OFL; check the licence files).
- **Motion:** not built. Defer until the screens exist.
- **Persistence:** not built. It is needed for the theme, the keyboard preset and first-visit
  tips.
- **Keymap:** only the classic preset exists. F1, F2, F3, F11, F12 and Alt+F1 are listed in the
  palette but only raise "cannot run it yet".

## 3. Per screen

- **Home** (`screens/home.rs`). Mock states: home.default, loading, empty, ai-off, nudge-open.
  - Differences: no page actions, plain-words bar, review list or nudges. The tile labels differ.
  - B2 overflow: `TILE_WIDTH` and `BOX_WIDTH` are fixed (`home.rs:51-53`), so "Look up" and the
    fifth tile fall off the edge at 1280 px.
  - The GST payable tile is hard-coded "Not supported yet" (`home.rs:240-249`). It needs an
    engine net-payable value, and the UI must not subtract.
  - The sales-by-month chart needs a monthly series (UNVERIFIED).
- **Palette** (`palette.rs`). Mock states: palette.default, search, voucher, ask, entry, commands,
  no-results.
  - Missing: the prefixes # @ > ? +, the legend row, the "Go to" label, recents, footer key hints.
  - Commands with no handler raise a notice.
- **Sales invoice** (`screens/item_invoice/*`). Not reached because of B1. Its layout against the
  mock is UNVERIFIED until a screenshot exists.
- **Receipt and payment** (`voucher_entry.rs`, 2139 lines). The mock is single-entry and has:
  - a bank account header;
  - a "balance after" value from the engine;
  - a bill-wise popup (`voucher_entry.rs:777` returns Unsupported);
  - an overdraft warning.
- **Balance sheet** (`report_view`).
  - The mock is vertical; the app has two columns.
  - The as-at date: the mock uses today, the app uses the last voucher date (documented behaviour).
  - The footnote runs under the key bar because the body doesn't scroll (B7).
- **Day book**. The mock adds a period tab, a Particulars column with party and narration, plus
  entered-by and origin columns. There is no engine field for entered-by or origin. The empty
  state is missing.
- **GST returns** (`screens/gst.rs`).
  - The period is the financial year to date, not the month (B4, `navigate.rs:286`).
  - Missing: the status tiles, the "What happens next" card and Export JSON Ctrl+E.
  - 2B shows "Not loaded" with no import action, and no 2B or IMS engine API was found.
- **Bank reconciliation** (`screens/bank.rs`).
  - Missing: the mock's tabs and table, the proposal card, and an import action in the empty state.
    `import_bank_statement` and the CSV, MT940 and XLSX parsers do exist.
  - Shows "0 of 0 lines not matched" when no statement is loaded (B6).
  - `bank_suggestions` and `brs` exist, so the non-AI match list and the BRS tab can ship first.
- **Masters**. The app has ten kind tabs where the mock has two (Ledgers, Stock items).
  - Missing: the balance column (needs an engine value) and the ledger detail panel.
  - The GSTIN check in `masters/check.rs` can be reused.
- **Setup**. The welcome screen only. The 7-step flow is missing (company, GSTIN check, features,
  keyboard, AI, backup, done).

## 4. Coverage

**Mock states with no app screen (26):**
- ask: 5
- purchase-bill: 9
- quick-entry: 5
- review: 2
- keyboard map: 2
- AI settings: 3

The assist code exists (`ledgerly-assist`, `screens/assist.rs`) but nothing reaches it.

**App screens with no mock state:**
- the import wizard;
- audit views (never constructed);
- Trial Balance, P&L, Stock Summary, Outstanding, Integrity;
- Contra and Journal entry;
- six extra master kinds.

## 5. Functional bugs

| # | Bug | Where |
|---|---|---|
| B1 | F8 on a company without inventory shows "not supported yet". The Home list still says "Item invoice with GST" | `gateway.rs:198-201`, `voucher_entry.rs:489-514` |
| B2 | Home clips at 1280 px | `home.rs:51-53, 615-634, 745-760` |
| B3 | The palette lists commands that cannot run | `shell/mod.rs:207-212` |
| B4 | The GST period is the financial year to date | `navigate.rs:286, 351-366` |
| B5 | The GST tab chosen on Home is dropped | `gateway.rs:116` |
| B6 | "0 of 0 lines" shows with no statement | `bank.rs` |
| B7 | The balance sheet body doesn't scroll | `report_view` |
| B8 | Four date formats: ISO, dd-Mon-yyyy, dd-mm-yyyy and a range form | home, gst, report_view, voucher entry |
| B9 | Ctrl+A save may clash with select-all in text fields (UNVERIFIED) | `keymap-classic.toml` |
| B10 | No Devanagari font | `theme::apply` |
| B11 | The theme.rs header cites mock v1 | `theme.rs:1-27` |
| B12 | **Clean room: the UI says "Import from Tally"** | `screens/import/view.rs:188, 265, 323` |

## 6. Mock points not to copy as-is

- The greeting "Good afternoon, Ananya" needs a user name setting and the injected clock.
- The sidebar footer "Engine 0.1 mock · rule pack (illustrative)" is a development label.
  "Saved on this computer" implies autosave, but the app saves on command.
- Dashed borders cost per-frame painting in egui. Keep them only on AI proposals.
- The review queue's "How decisions work" panel and the first-click tip need persistence: show
  them once.
- The AI tier chip must reflect a tier the engine path enforces, or the chip can lie.

## 7. Build order

Agents write code; the coordinator builds and runs the gates. Each file has one owner per wave.

**Wave 0, decisions.** See section 8.

**Wave 1, shell and design system (parallel, disjoint files).**

| Item | Size | Owns | Accept |
|---|---|---|---|
| U1 Tokens and fonts | M | `theme.rs`, `tests/u1_theme.rs`, `assets/fonts/` | tokens equal the mock hex values; contrast pairs from DESIGN.md; a font is installed |
| U2 Component kit | L | new `src/ui/*.rs`, `lib.rs` (`pub mod ui;`), `tests/w1_ui_kit.rs` | badge, chip, card, keycap, button, prov, conf, prop, table, tabs, page_head, empty, skeleton, format |
| U3 Shell chrome | L | `shell/mod.rs`, `shell/chrome.rs`, new `shell/topbar.rs`, `shell/sidebar.rs`, `screen.rs`, `tests/u1_shell.rs`, `tests/w1_shell_nav.rs` | sidebar keys, rail toggle; dead palette rows hidden or wired (B3) |
| U4 Routes and stubs | S | `navigate.rs`, `screens/mod.rs`, stub screens for review, ask, quick_entry, purchase_bill, keymap_map, ai_settings, setup | `tests/c5_navigation.rs` additions |
| B1 Sales routing, GST tab | S | `shell/gateway.rs`, `tests/b1_sales_routing.rs` | F8 opens the invoice; the Home GST tab is kept (B1, B5) |
| U5 Snapshot registry | S | `examples/ui_snapshots.rs` | per-screen shot registry, so later waves add entries without conflicts |

**Wave 2, screens (parallel).** Each item owns its screen files and tests only.

| Item | Size | Main mock states | Waits on |
|---|---|---|---|
| S-A Home | L | home.* | E-1 for the GST tile, E-8 for the chart |
| S-B Sales invoice | L | sales-invoice.* | stock on hand (UNVERIFIED) |
| S-C Palette | M | palette.* | wave 3 for ? and + |
| S-D Balance sheet, day book | L | balance-sheet.*, day-book.* | E-4 entered-by and origin |
| S-E GST returns | L | gst.gstr1, gstr1-correction, gstr3b | E-2; recon waits for E-3 |
| S-F Bank | L | bank.default, empty, import-error, brs | AI states wait for A6 |
| S-G Masters | M/L | masters.* | E-5 balances |
| S-H Receipt and payment | L | receipt.*, payment.* | E-6 bill-wise, E-7 overdraft. Also splits voucher_entry.rs into a module, with no behaviour change |
| S-I Setup | L | setup.* | the AI tier store from U3 |
| S-J Keyboard map, presets | M | keys.* | none |
| S-K AI settings | M | ai-settings.* | the tier store |

**Wave 3, engine and AI.**

| Item | Work |
|---|---|
| A0 | Assist wiring in `ledgerly-assist` and `ledgerly-api`; blocks A1–A4 and A6 |
| A1 | Review queue |
| A2 | Purchase from a bill: A2a reading, proposal, accepted; A2b the error and offline states |
| A3 | Plain-words entry |
| A4 | Ask your books |
| A5 | 2B and IMS reconciliation |
| A6 | Bank AI suggestions |
| A7 | Audit and assist panels: wire them or delete them |

**Engine items the UI needs.**

| Item | Value |
|---|---|
| E-1 | Net GST payable after set-off |
| E-2 | Monthly GST periods |
| E-3 | 2B and IMS import, reconciliation, action store |
| E-4 | Entered-by and origin on voucher rows |
| E-5 | Ledger balances and stock value for masters |
| E-6 | Bill-wise allocation and settlement |
| E-7 | Overdraft shortfall |
| E-8 | Monthly sales series |

## 8. Decisions

1. **Balance sheet as-at date:** today (mock) or the last voucher date (app's documented
   behaviour).
2. **Report labels:** Liabilities, Assets, Profit and Loss A/c, Capital Account, against the
   mock's "Equity and liabilities". Pick one rule.
3. **Import wording:** "Import from another app" in the UI and help text (clean room, B12).
4. **Save key default for new users:** Ctrl+A (classic) or Ctrl+S (standard).
5. **Reports outside the mock:** keep them behind "More reports", or drop them.
6. **Tab strip and Ctrl+T / Ctrl+W:** keep, hide or drop. The mock has none.
7. **GST default period:** monthly (mock) or financial year to date (code, marked pending).
8. **Extra master kinds:** move them under Settings, or drop them from the screen.
9. **Home "Enter" and "Look up" lists:** keep them, or leave that role to the sidebar.
10. **Ctrl+A save against select-all** in text fields: needs a test.
