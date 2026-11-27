# Ledgerly UI mock v2: design

`index.html` is the design source. It is one self-contained file: inline CSS and JS, Google Fonts
as the only network request, and it opens from `file://`. Every screen and state opens from a URL
hash (see "Capture contract"). All companies, people, GSTINs, banks and amounts are fictional.

A small mock engine inside the page computes every figure from sample books, using integer paise.
On load it prints a self-check to the console:
- every voucher has Dr equal to Cr;
- the trial balance is zero;
- both sides of the balance sheet agree, at ₹56,09,537.00;
- GSTR-3B outward tax equals the tax on the vouchers;
- GSTR-1 taxable value equals the 3B taxable value;
- the bank statement adds up.

## 1. Design principles

1. **The engine owns every number.** Screens show values from the books, from a document or from
   your own words, or values the engine computed. Each one says which. The AI never shows a figure.
2. **AI output is a proposal, never an action.** Every AI result uses the same proposal component,
   with the same three keys everywhere: Accept `Ctrl+A`, Edit `Enter`, Reject `Alt+D`.
3. **Say where it came from and how sure it is.** A proposal shows its source chips (file, bank line,
   your words, model) and a confidence band. Every field carries a provenance tag.
4. **Keyboard first, and still obvious.** Every action has a key, and the key is shown on the
   control and in the bottom key bar. Mouse users get the same controls, labelled in words.
5. **Calm, dense, readable.** The surfaces are warm neutral. One accent for "you are here and the
   primary action". One AI colour. Tabular numbers, right-aligned, in Indian grouping.
6. **Fail loudly and kindly.** Errors say what is wrong, what was not changed, and the next step.
   No silent defaults: a blank stays blank and blocks Accept.
7. **Local first.** AI runs on this computer by default. Cloud AI is opt-in per company, with a
   confirmation for each document.

## 2. Information architecture and navigation

### Shell

| Area | Content |
|---|---|
| Top bar (48 px) | Brand mark; company `F3`; date `F2`; lock chip; search/palette `Ctrl+K`; AI tier chip (privacy cue); theme; keyboard map |
| Side bar (208 px, rail 60 px with `Ctrl+\`) | Overview, Enter, Books, Compliance, Settings. Each item shows its key |
| Main | Breadcrumbs, page head (title, one-line purpose, actions with keys), content |
| Key bar (32 px) | Keys for this screen. Hold `Alt` or `Ctrl` to see those layers |

There is no gateway menu. Home is a dashboard with work to do. Navigation goes through the side
bar, the palette and direct keys.

### Side bar groups

| Group | Screens |
|---|---|
| Overview | Home, Review queue (count of open proposals) |
| Enter | Sales `F8`, Purchase `F9`, Payment `F5`, Receipt `F6`, Plain-words entry |
| Books | Day book, Balance sheet, Masters |
| Compliance | GST returns (GSTR-1, GSTR-3B, 2B and IMS), Bank reconciliation |
| Settings | AI and privacy, Keyboard map `?`, Backups and updates, Company setup |

### Command palette (`Ctrl+K`, also `Alt+G` and `Ctrl+G`)

One box for everything. A prefix narrows the search:

| Prefix | Meaning | Example |
|---|---|---|
| none | Screens, reports, ledgers, vouchers | `bal` |
| `#` | Voucher number | `#0412` |
| `@` | Party | `@mehta` |
| `>` | Command | `>theme` |
| `?` | Ask your books | `? who owes us more than 30 days` |
| `+` | Plain-words entry | `+ paid rent 25k` |

In the palette, `Enter` opens the result on top of the stack and `Shift+Enter` replaces the current
screen. `Esc` closes the palette and leaves you where you were.

### Depth model

Screens form a stack. `Esc` always goes back one level and never loses a saved voucher. Reports
drill in with `Enter` (Balance sheet, then group, then ledger, then voucher) and back out with
`Esc`, and the row you came from stays selected.

### Review queue

All open proposals sit in one list: bill photos, bank suggestions, 2B explanations. A proposal is
the same object in the queue and in context. Deciding in either place clears it from both.

## 3. Visual system

### Colour tokens and contrast

The tokens live on `:root[data-theme=light|dark|hc]`. The accent is "Jamun", a deep plum. The AI
colour is copper. Neither colour is blue or yellow, so the product cannot read as the incumbent's
trade dress. Ratios were computed with the WCAG 2.x formula by script.

| Token | Light | Dark | High contrast | Use |
|---|---|---|---|---|
| `--bg` | #F6F5F2 | #141312 | #000000 | Page |
| `--surface` | #FFFFFF | #1C1B19 | #000000 | Cards, tables |
| `--sunken` | #EFEDE8 | #0F0E0D | #000000 | Wells, skeletons |
| `--text` | #1C1B19 | #EEEBE5 | #FFFFFF | Body |
| `--text-2` | #4A4741 | #C9C4BA | #FFFFFF | Secondary |
| `--text-3` | #615D55 | #A8A296 | #E6E6E6 | Meta, captions |
| `--accent` | #6B2D73 | #D9A6E0 | #F5B8FF | Selection, primary |
| `--ai` | #9A4211 | #F2A36E | #FFB37A | AI proposals only |
| `--ok` | #1E6E3F | #7FCF9A | #8CF5A9 | Matched, passed |
| `--warn` | #7E4C00 | #E9B860 | #FFD27A | Needs attention |
| `--bad` | #B3261E | #F28B82 | #FF9C94 | Errors, differences |
| `--border-strong` | #8F897E | #77726A | #FFFFFF | Input borders |

Each colour also has a `-soft` tint and an `-ink` shade for text on that tint.

| Pair (text on background) | Light | Dark | HC |
|---|---|---|---|
| text on surface | 17.21 | 14.47 | 21.00 |
| text-2 on surface | 9.26 | 9.91 | 21.00 |
| text-3 on surface | 6.55 | 6.78 | 16.83 |
| text-3 on sunken | 5.60 | 7.60 | 16.83 |
| accent on surface | 9.34 | 8.59 | 13.18 |
| on-accent on accent | 9.34 | 8.69 | 13.18 |
| accent-ink on accent-soft | 9.68 | 9.58 | 11.09 |
| ai on surface | 6.64 | 8.39 | 11.98 |
| ai-ink on ai-soft | 7.04 | 8.74 | 11.28 |
| ok on ok-soft | 5.40 | 7.36 | 12.17 |
| warn on warn-soft | 6.37 | 7.57 | 11.15 |
| bad on bad-soft | 5.59 | 6.34 | 8.75 |
| border-strong on surface (UI, needs 3:1) | 3.47 | 3.61 | 21.00 |

All text pairs pass AA at 4.5:1. High contrast also sets a 2 px border width (`--bw`), outlines
badges and notes, and keeps the dashed AI border.

AI is never shown by colour alone. A proposal has a dashed border, a heavy left bar, a sparkle icon
and the words "suggested by …".

### Type

The UI font is Inter. Noto Sans Devanagari is the fallback for Hindi names. Numbers use
`tabular-nums`.

| Token | Size | Use |
|---|---|---|
| `--t-cap` | 11 | Badges, tags |
| `--t-meta` | 12 | Hints, captions, labels |
| `--t-body` | 13 | Tables, text |
| `--t-input` / `--t-h3` | 14 | Inputs, card titles |
| `--t-h2` | 16 | Section titles, totals |
| `--t-h1` | 20 | Page title |
| `--t-fig` | 26 | KPI figures |

### Spacing, radii, elevation

- Spacing is on a 4-point grid: 4, 8, 12, 16, 20, 24, 32.
- Radii: `--r-xs` 4 (kbd, tags), `--r-s` 6 (buttons, inputs), `--r-m` 10 (cards), `--r-l` 14 (palette, dialogs).
- Elevation is a hairline border by default. Only overlays (palette, dialog, typeahead, toast) get a
  single soft shadow. There is no blur and no glass.

### Components

| Component | Notes |
|---|---|
| Button | Default, primary, ok, danger, ghost, disabled. The key hint sits inside: "Save `Ctrl+A`" |
| Chip | Top-bar status and controls |
| Badge | Status with an icon and a word: Matched, Not in books, Provisional, Locked |
| Provenance tag (`.prov`) | from bill · AI · your words · engine · books · rule. Has a tooltip |
| Confidence band (`.conf`) | Three bars plus a word: High, Medium, Low, or "Rule, deterministic" |
| Source chip | File, bank line, model. Shows where a proposal came from |
| Proposal (`.prop`) | Head (kind, by, sources, confidence), body, checks list, footer (why plus Accept, Edit, Reject) |
| Checks list | ok, warn, bad, info rows written by engine validators. A bad row blocks Accept |
| Voucher sheet | Header grid, line table, footer with narration and tax box |
| Tax box | Engine rows. The total row is tagged "engine" and flashes when it recomputes |
| Typeahead | Lists masters only, with a "Create … `Alt+C`" row. It never accepts free text as a ledger |
| Note | info, ok, warn, bad. Used for lock bars, offline cues and errors |
| Empty state | Icon, one sentence on what goes here, the action with its key |
| Skeleton | Loading rows. A stale result is never shown as current |
| Palette, dialog, popover, toast | Overlays. A toast offers Undo `Ctrl+Z` |

## 4. Keyboard model

### Presets

Two presets ship. Classic is the default.

| Action | Classic accountant keys | Standard desktop keys |
|---|---|---|
| Sales, Purchase, Payment, Receipt | F8, F9, F5, F6 | Ctrl+Shift+S, P, Y, R (F-keys stay as aliases) |
| Contra, Journal | F4, F7 | Ctrl+Shift+C, J |
| Save voucher / accept proposal | Ctrl+A | Ctrl+S |
| Next field | Enter | Enter or Tab |
| Back | Esc | Esc |
| Date, company | F2, F3 | F2, Ctrl+Shift+O |

### Rules

- `Enter` moves to the next field. `Shift+Enter` moves back. `Enter` on an empty item line goes to
  narration. `Enter` on narration asks "Save?" (`Enter` yes, `Esc` no).
- `Esc` goes back one level. In a field, the first `Esc` leaves the field.
- `Alt+C` creates a master from any master field. `Ctrl+Enter` alters it.
- Classic report keys stay the same: `Alt+F1` detailed, `Alt+F2` period, `Ctrl+E` export,
  `Ctrl+J` exceptions.
- Proposals use the same keys everywhere: `Ctrl+A` accept, `Enter` edit, `Alt+D` reject, `Ctrl+I` source.
- Lists and queues: `J`/`K` or arrows move, `Y` accept, `N` reject, `E` edit. Single letters only
  work when no field has focus.
- GST 2B: `A`/`R`/`P` set the IMS action, `Y` takes the suggestion, `X` explains. Tabs are `1 2 3`.
- Holding `Alt` or `Ctrl` swaps the key bar to that layer, so you find keys without reading docs.
- `?` opens the full keyboard map. `Ctrl+K` reaches anything by name.
- The first mouse click on a control that has a key shows a one-time tip: "next time press …".

### Collisions avoided

| Key | Meaning in Ledgerly | Not used for |
|---|---|---|
| Ctrl+A | Accept or save, the classic habit | Select all (only inside text fields) |
| Alt+D | Delete or reject | Browser address bar (not relevant in a desktop app) |
| F9 | Purchase | Recalculate |
| Ctrl+K | Palette | Insert link |

## 5. AI integration model

### Where AI appears

| Flow | Where | Proposal | Source shown | Confidence |
|---|---|---|---|---|
| Bill photo to draft purchase | Purchase `Alt+O`, review queue | Draft purchase voucher | Image with boxes around read fields; model | Per field and overall |
| Plain words to draft voucher | Home box, palette `+`, Plain-words screen | Draft payment or receipt; links a bank line | Your words (marked up); model | Overall |
| Bank lines | Bank reconciliation | Match to a voucher, or create a voucher or ledger | Bank narration; past postings | Per line |
| Ask your books | Palette `?`, Ask screen | A ReportQuery shown as chips | Your question; model | Overall, "pick one" when low |
| Nudges | Home, review queue | Due dates, duplicates, IMS reminders | The rule that fired | "Rule, deterministic" |
| GST mismatch explanation | 2B and IMS, `X` | Explanation plus a draft fix | 2B record, voucher, attached image | Overall |

Nudges are deterministic rules, not AI. They use the same card, tagged "Rule".

### Trust and confirmation

- **One component.** Every proposal has the same layout and keys, so trust carries from one place
  to the next.
- **Provenance on each value.** "from bill" (dashed field), "AI match", "your words", "engine",
  "books", "rule". Fields read from a document are dashed until a person touches them.
- **Checks before Accept.** Engine validators run on the draft: Dr equals Cr, GSTIN check
  character, lock date, printed total against engine total, duplicate image or bill number, open
  bill not over-settled. A failing check disables Accept and says why in a tooltip and in words.
- **Show the source.** `Ctrl+I`, hover or focus highlights the box on the bill image the field
  came from.
- **Template prose.** In Ask and in 2B explanations, sentences are templates. The only digits are
  bound engine values or query parameters, shown as highlighted chips. A filter removes any other
  digit.
- **Audit.** Accepting writes the AI event (model, input hash, fields, edits) and the person. The
  day book shows an "AI draft, accepted" badge. `Ctrl+I` on a row shows the original proposal.
- **Reject teaches locally.** An optional reason ("wrong ledger") updates local rules only.

### Failure states (each is a deep-linked state)

| State | What the user sees |
|---|---|
| Loading | Skeleton plus the step ("matching items to your masters") and `Esc` to cancel |
| Low confidence | The field is left blank, red dashed box on the image, Accept blocked, "nothing was guessed" |
| Mismatch | Printed total and engine total differ; Accept blocked until fixed or a reason is recorded |
| Duplicate | The same image hash or bill number was found; link to the other draft |
| Unknown ledger | "No ledger named Ramesh"; pick or `Alt+C`; never auto-created |
| Unsupported question | Ask says what it can do and offers the nearest reports |
| File error | Password-protected PDF, wrong-GSTIN 2B file, a statement that does not add up: nothing imported, reason and next step |
| Offline / AI off | The AI entry points are hidden. Manual paths and deterministic matching remain |

### Privacy and offline mode

| Tier | Default | What leaves the computer | Cue |
|---|---|---|---|
| Off | No | Nothing | Top chip "AI off"; AI entry points hidden |
| On this computer | Yes | Nothing | Top chip "AI on this computer" (CPU icon) |
| Cloud, your own key | No | Only the one document you confirm, after redaction | Top chip "AI cloud, your key"; a consent dialog for each document |

- The tier is set per company.
- Before sending, redaction removes bank account numbers, PAN, Aadhaar, phone numbers and emails.
- Ledger balances and report figures are never sent.
- The AI activity log is kept with the company file and can be exported.

### What AI may never do

- Show a figure it worked out.
- Post, alter, cancel or delete a voucher without Accept.
- Create a ledger, item or party on its own.
- Change a locked period, an IMS action or a filed return.
- File anything, generate an IRN or send anything. v1 only exports JSON.
- Send data to the cloud without the tier being on and the document confirmed.

## 6. What changed from v1 and why

| v1 | v2 | Why |
|---|---|---|
| Indigo and turmeric on blue-ruled paper | Warm stone neutrals, plum accent, copper AI | Indigo plus turmeric read as blue and yellow, too close to the incumbent's trade dress |
| A real bank name in the sample | Fictional banks and merchants | Clean room: no real branding |
| An assistant side panel with its own "Post" key | One proposal component in context, same three keys everywhere, plus a review queue | AI where the work is; one way to trust it |
| AI and engine boxes side by side | Provenance tag on every value, plus a checks list | Users need per-field truth, not per-box |
| Balance sheet labelled after a statutory format | Neutral "Balance sheet" in vertical form | Do not imply statutory compliance the mock cannot prove |
| `--ink-3` meta text below 4.5:1 | Every text pair 5.4:1 or better | WCAG AA |
| Key collisions (save, delete, palette) | Classic preset plus a standard preset, with a collision table | Keep habits, avoid surprises |
| Static screens | 89 deep-linked states, 15 frozen-frame transitions | The mock is the design source and drives screenshots |
| No first run | 7-step setup with GSTIN checking, keyboard preset and AI tier | First-time users need a path without training |
| Light and dark only | Light, dark and high contrast, all AA | Accessibility |

## 7. egui notes

- **Layout.** The shell maps to `TopBottomPanel` (top bar, key bar), `SidePanel` (nav) and
  `CentralPanel`. Cards are `Frame`s with stroke and rounding. Tables use `egui_extras::TableBuilder`
  with right-aligned, monospaced-digit numbers (`FontFeatures` tnum, or a numeric font).
- **Tokens.** Use one `Theme` struct per theme that fills `Visuals` (`panel_fill`, `extreme_bg_color`,
  `widgets.*`, `selection`). The `-soft`, `-ink` and `--bw` values go into the struct. High contrast
  sets `widgets.*.bg_stroke.width = 2.0`.
- **Proposal component.** A `Frame` with a dashed stroke. egui has no dashed frame stroke, so paint
  `Shape::dashed_line` around the rect, plus a filled 6 px left rect. Head, body and footer are
  vertical sections.
- **Icons.** Use `egui-phosphor` (MIT). The mock's inline paths are placeholders in the same stroke style.
- **Fonts.** Inter plus Noto Sans Devanagari, loaded through `FontDefinitions` with fallback order.
- **Motion.** Use `ctx.animate_value_with_time` with the mock's tokens. 120, 180, 240 and 600 ms
  map to `emath::easing::cubic_out`, `cubic_in` and `cubic_in_out`. Only fade (alpha), slide (offset),
  scale (palette only) and highlight (fill alpha) are used. Reduced motion means a duration of 0.
- **Typeahead.** An `Area` anchored under the field, `Order::Foreground`, with arrow keys handled
  before the text edit consumes them.
- **Bill image boxes.** Paint rects over the `Image` in image coordinates from the reader's output.
  Focus on a field sets the highlighted box.
- **Key bar.** Read `ctx.input(|i| i.modifiers)` every frame to swap the Alt and Ctrl layers.
- **Not used:** blur (the unreadable bill field is faded ink, not blurred), backdrop filter, gradients, CSS grid tricks or hover-only actions. `color-mix`
  in the mock is a lerp between two tokens in egui.

## 8. Capture contract

The page is the single source for screenshots. A capture script loads URLs, waits for a ready flag
and takes a 1280×800 screenshot.

### URL hash

`index.html#screen=<id>&state=<id>[&param=value...]&theme=light|dark|hc`

| Param | Meaning | Example |
|---|---|---|
| `screen` | Screen id | `sales-invoice`, `bank`, `palette` |
| `state` | State id; the screen default when missing | `gst-inter-state`, `ai-suggestions` |
| `theme` | `light`, `dark` or `hc`; otherwise the stored choice or the OS | `theme=dark` |
| `base` | The screen under the palette | `screen=palette&base=home` |
| `query` | Palette text | `query=bal`, `query=%3F%20who%20owes` |
| `g`, `l`, `v` | Balance sheet drill: group, ledger, voucher | `state=ledger&g=debtors&l=sgt` |
| `sel` | Row to select (balance sheet, by name) | `sel=Sundry%20debtors` |
| `accepted` | Bank lines already accepted | `accepted=L09` |
| `toast=none` | Render without the toast | `state=saved&toast=none` |
| `tx` | Play or freeze a transition on this render | `tx=drill-down` |

The page sets `document.documentElement.dataset.ready = '1'` once fonts are ready and the first
frame is painted. Wait for that before capturing.

A screen's `mount(state, params)` runs as soon as the screen renders, before the fonts are
ready. Any layout that depends on text size, such as scrolling a selected row to the centre,
goes in the screen's `settle(state, params)`. It runs after the fonts are ready and before the
ready flag is set. Done in `mount`, the scroll depends on whether the fonts arrived first, and
two captures of the same state differ.

### states.json

```json
{ "version": 1, "viewport": {"width":1280,"height":800}, "themes": ["light","dark","hc"],
  "ready": "document.documentElement.dataset.ready === '1'",
  "url": "index.html#{hash}&theme={theme}",
  "states": [ {"id":"bank.ai-suggestions","kind":"happy","hash":"screen=bank&state=ai-suggestions",
               "title":"…","description":"one line"} ] }
```

`kind` is one of: happy, loading, empty, offline-ai, low-confidence, error, validation-error,
period-locked. The capture runs every state in every theme (89 × 3).

### transitions.json

```json
{ "url": "index.html?freeze={ms}#{render_hash}&tx={id}&theme={theme}",
  "motion_tokens": {"--dur-base":"180ms", "--ease-out":"cubic-bezier(.33,1,.68,1)"},
  "transitions": [ {"id":"drill-down","start":"screen=balance-sheet&state=default",
     "end":"screen=balance-sheet&state=group","render":"end",
     "targets":[{"selector":"#report-body","keyframes":"drill-in"}],
     "duration":"--dur-base","duration_ms":180,"easing":"--ease-out","frames_ms":[0,45,90,180]} ] }
```

- `render: "end"` means the page renders the end hash and plays the motion into it (push, drill,
  arrive). `render: "start"` means it renders the start hash and plays the motion out of it
  (palette close, accept, reject, toast out).
- `targets` lists the elements and keyframes. A target may override `duration_ms`.
- `frames_ms` lists suggested still frames.
- The transitions are: screen push and pop, palette open and close, drill down and up, AI proposal
  arriving, proposal accept and reject, bank match accept, live GST recompute, toast in and out,
  theme switch and setup step.

### Freeze

`?freeze=<ms>` (in the query string, or `&freeze=` in the hash) renders that moment as a still. Each
target animation gets `animation-delay: -<ms>` and `animation-play-state: paused`, and other CSS
transitions are turned off. Without `freeze`, `&tx=<id>` plays the motion live. For
`render: "start"` it then navigates to the end hash.

### One source

The registry is embedded in `index.html` as `<script id="registry" type="application/json">`.
`node extract-registry.js` regenerates `states.json` and `transitions.json` from it, so the page and
the JSON files cannot drift. To add a state, add it to the registry and to the screen's `render`,
then run the script.

### Capturing

The mock is the central design record, kept in git together with its captures. Every change to
the mock commits fresh captures in the same commit.

```bash
node extract-registry.js   # after changing the registry in index.html
node capture.mjs           # every state and transition, all themes, then the catalog
node capture.mjs --check   # exit 1 when captures/ is older than the mock
node capture.mjs --only bank   # ids containing "bank" only (does not rewrite the catalog)
```

- It needs `google-chrome` and an `ffmpeg` built with libwebp. No npm packages are used: Node's
  built-in WebSocket drives Chrome over the DevTools protocol.
- States go to `captures/states/<theme>/<id>.webp`. Transition frames go to
  `captures/transitions/<id>/<theme>/<ms>ms.webp`, with one `<theme>.gif` per transition.
- Images are lossless WebP. That is half the size of PNG with the same pixels, and the same render
  gives the same bytes, so git stores an unchanged screen only once.
- `captures/CATALOG.md` shows every state and transition with its images.
- `captures/manifest.json` records the sha256 of the sources, which is what `--check` compares.
- Before each shot the script hides the text caret and stops the skeleton shimmer, which would
  otherwise land on a different frame each time. A focused field shows no caret in captures.
- Chrome runs with spell checking off and software raster, and each shot is taken again until two
  in a row match. Two runs of the script give the same bytes for every image.
- Google Fonts must be reachable. Offline, the system-font fallback changes every image.

## 9. Open questions

1. **Ctrl+A for accept.** It matches the classic save habit, but in text fields it means select all.
   Should the standard preset be the default for new users who never used the incumbent?
2. **Confidence thresholds.** Which band auto-fills a field and which leaves it blank? The mock
   uses Medium. This needs data from real bills.
3. **Bulk accept.** The mock allows bulk accept only for Exact and Rule bank matches. Should
   High-confidence AI matches be allowed in bulk with one confirm?
4. **Reject reasons.** Do they train only local rules (the mock's assumption), or can they be
   shared with consent?
5. **Cloud redaction.** Is redacting PAN, bank and phone numbers enough? Should party names also be
   masked, given that the model then reads less well?
6. **IMS actions.** v1 records decisions and lists them for the portal. Is a later direct portal
   action in scope, given the legal research?
7. **Overdraft policy.** Block, Warn or Allow per company: what should the default be?
8. **Hinglish and other languages.** Where does language detection run, and what is the fallback
   when the local model is weak in Marathi or Gujarati?
9. **Provisional stock.** Should a provisional closing stock block the balance sheet export, or
   export with a watermark?
10. **Brand.** "Ledgerly" and the plum mark are working choices that need a trademark check.
