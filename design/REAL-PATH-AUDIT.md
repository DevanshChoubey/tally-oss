# Real-path audit (2026-11-24)

Every screen and state reachable from the real cold start (`Shell::cold_start()`, no company
open), captured in a real window through the same construction path as `main`. Captures live in
`design/app-captures/audit/cold.<id>.webp` (made with `capture.sh --audit`; `compare.mjs` skips
this folder because these shots have no mock state). Only `setup.welcome` has a mock pair; its
light SSIM is 0.8199 (see `design/COMPARE.md`). All 24 images were looked at.

## Worst first (still open)

1. **Import from another app is raw egui** (`cold.import`): unstyled buttons and text boxes,
   plain-text step line, no card layout, no page head. It also contradicts the setup welcome,
   whose Import card says "Not available in this wizard yet" while the palette opens a working
   import screen. Needs a designed screen (a screen-sized job, not a quick fix).
2. **Function-key and letter shortcuts do nothing on a cold start, with no feedback**
   (`cold.sales-f8`, `purchase-f9`, `payment-f5`, `receipt-f6`, `day-book-d`, `balance-sheet-b`,
   `masters-m`, `gst-g`, all identical to the welcome). The wizard swallows the keys. Some
   screens (Bank, Contra, Journal) only say so in the bottom bar.
3. **Refusal messages land in the bottom key bar in red** (`cold.home`, `cold.masters`,
   `cold.bank`): it shares the line with the key hints, and the wording ("create one or open a
   file in Company setup first (Home)") is developer text. Should be a toast or a note on the
   welcome.
4. **Tab strip is unstyled** (`cold.new-tab`): two tabs both titled "Company setup", "+" and
   "Close tab" drawn as plain outline buttons.
5. **Setup wizard has no hotkeys of its own beyond Enter/Esc/Tab** and the first-launch chrome
   differs from the mock (see `design/PARITY-GAPS.md`, setup section).
6. Dark and high-contrast `setup.welcome` captures were taken before this change and have not
   been reshot cold (dark 0.7540, hc 0.6955); the light one has.

## Table

Verdicts: matches = in line with the design; unstyled = raw widgets; broken = wrong behaviour.
"Mock pair" is the mock state compared; "none" means no mock state exists for a no-company run.

| Screen / state | Capture | Mock pair (SSIM) | Verdict |
|---|---|---|---|
| Setup welcome (cold start) | `light/setup.welcome.webp` | setup.welcome (0.8199) | matches (chrome gaps in PARITY-GAPS) |
| Welcome, Open card chosen | `audit/cold.start-open-selected.webp` | setup.welcome variant, none | matches; button reads "Open a Ledgerly file" |
| Collapsed sidebar rail | `audit/cold.rail.webp` | none | matches |
| Command palette over welcome | `audit/cold.palette.webp` | none | matches |
| New tab (Ctrl+T) | `audit/cold.new-tab.webp` | none | unstyled tab strip, duplicate titles |
| Keyboard map | `audit/cold.keys-help.webp` | none | matches; list is clipped by design |
| Home via palette | `audit/cold.home.webp` | none | fixed: stays on welcome (was a raw error page); message wording poor |
| Masters via palette | `audit/cold.masters.webp` | none | fixed: same as Home |
| Review queue | `audit/cold.review.webp` | none | fixed: one clear "No company is open" note (was also "Loading") |
| Masters key `m` | `audit/cold.masters-m.webp` | none | broken: key ignored, no feedback |
| Sales F8, Purchase F9, Payment F5, Receipt F6 | `audit/cold.sales-f8.webp` etc. | none | broken: keys ignored, no feedback |
| Day book `d`, Balance sheet `b`, GST `g` | `audit/cold.day-book-d.webp` etc. | none | broken: keys ignored, no feedback |
| Bank, Contra, Journal | `audit/cold.bank.webp`, `cold.contra.webp`, `cold.journal.webp` | none | broken: refusal only in the key bar |
| Quick entry (plain words) | `audit/cold.quick-entry.webp` | none | matches (disabled note, clear) |
| Ask your books | `audit/cold.ask.webp` | none | matches (disabled note, clear) |
| Purchase from a bill photo | `audit/cold.purchase-bill.webp` | none | matches |
| AI and privacy | `audit/cold.ai-settings.webp` | none | matches; note card is bare |
| Import from another app | `audit/cold.import.webp` | none | unstyled |

## Fixed in this change (test first, `tests/s8_cold_start.rs`)

- Home and Masters refuse to open with no company, instead of showing a raw error screen.
- The Review queue no longer says "Loading the queue from the engine" beside its own error.
