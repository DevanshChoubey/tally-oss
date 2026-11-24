# Real-path audit (2026-11-24)

Every screen and state reachable from the real cold start (`Shell::cold_start()`, no company
open), captured in a real window through the same construction path as `main`. Captures live in
`design/app-captures/audit/cold.<id>.webp` (made with `capture.sh --audit`; `compare.mjs` skips
this folder because these shots have no mock state). Only `setup.welcome` has a mock pair; its
SSIMs are light 0.8395, dark 0.8452, hc 0.8121 (see `design/COMPARE.md`). All 24 images were looked at.

## Still open

1. **The import screen's step-by-step path is not connected.** It is drawn in the app's kit, but
   its two start buttons are disabled with the reason; only the one-step import works, and only
   with a company open (with none, the screen says so and offers "Back to setup").
2. **First-launch chrome differs from the mock** in text only: AI chip ("AI: not set up" vs "AI on
   this computer"), sidebar footer, no "Hold Alt or Ctrl for more" hint (PARITY-GAPS, setup).
3. Bank, Contra and Journal refuse with the generic "That screen needs a company" (no screen name).
4. The other `rfd` file dialogs (GST, AI settings, report view, print, bank import) all need an
   open company, so the cold audit cannot reach them; they are not injectable.

## Table

Verdicts: matches = in line with the design; unstyled = raw widgets; broken = wrong behaviour.
"Mock pair" is the mock state compared; "none" means no mock state exists for a no-company run.

| Screen / state | Capture | Mock pair (SSIM) | Verdict |
|---|---|---|---|
| Setup welcome (cold start) | `light/setup.welcome.webp` | setup.welcome (0.8395) | matches (chrome gaps in PARITY-GAPS) |
| Welcome, Open card chosen | `audit/cold.start-open-selected.webp` | setup.welcome variant, none | matches; button reads "Open a Ledgerly file" |
| Collapsed sidebar rail | `audit/cold.rail.webp` | none | matches |
| Command palette over welcome | `audit/cold.palette.webp` | none | matches |
| New tab (Ctrl+T) | `audit/cold.new-tab.webp` | none | fixed: kit tab strip, "Company setup 2", kit New tab / Close tab buttons |
| Keyboard map | `audit/cold.keys-help.webp` | none | matches; list is clipped by design |
| Home via palette | `audit/cold.home.webp` | none | fixed: stays on welcome, toast "Home needs a company..." |
| Masters via palette | `audit/cold.masters.webp` | none | fixed: same toast |
| Review queue | `audit/cold.review.webp` | none | fixed: one clear "No company is open" note (was also "Loading") |
| Masters key `m` | `audit/cold.masters-m.webp` | none | fixed: toast with the key and a "Create a company" action |
| Sales F8, Purchase F9, Payment F5, Receipt F6 | `audit/cold.sales-f8.webp` etc. | none | fixed: toast with the key and a "Create a company" action |
| Day book `d`, Balance sheet `b`, GST `g` | `audit/cold.day-book-d.webp` etc. | none | fixed: toast with the key and a "Create a company" action |
| Bank, Contra, Journal | `audit/cold.bank.webp`, `cold.contra.webp`, `cold.journal.webp` | none | fixed: toast "That screen needs a company..." (generic wording) |
| Quick entry (plain words) | `audit/cold.quick-entry.webp` | none | matches (disabled note, clear) |
| Ask your books | `audit/cold.ask.webp` | none | matches (disabled note, clear) |
| Purchase from a bill photo | `audit/cold.purchase-bill.webp` | none | matches |
| AI and privacy | `audit/cold.ai-settings.webp` | none | matches; note card is bare |
| Import from another app | `audit/cold.import.webp` | none | fixed: page head, card, step line, kit buttons; step-by-step disabled with reason |

## Fixed in S8 (`tests/s8_cold_start.rs`)

- Home and Masters refuse to open with no company, instead of showing a raw error screen.
- The Review queue no longer says "Loading the queue from the engine" beside its own error.

## Fixed in S9 (test first, `tests/s9_cold_start.rs`; all 24 audit shots and the setup states retaken)

- Import screen restyled in the kit; the welcome's Import card is a real choice and opens it.
- F4-F9, D, B, M, G with no company say what they need in plain words, as a toast with an action.
- Refusals moved from the red key bar to the toast (wording changed; `s8_cold_start.rs` assertion
  updated to the new text, not loosened).
- Tab strip styled; same-title tabs are numbered.
- Setup inputs share the mock's input size; setup step markers shared with the import step line.
- The file picker is injectable (`with_picker`); the default raises no native dialog unless the
  real window enabled it (`enable_system_dialogs`, called only from `shell::run`).
