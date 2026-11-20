# 02 — User experience and keyboard workflow (TallyPrime 7.x reference)

Researcher area: UX and keyboard workflow. Research date: 2026-11-20.
Method: web research only (vendor help site help.tallysolutions.com, vendor blogs, review aggregators,
third-party comparison blogs). Nothing was downloaded or run. All content below is summarised in our
own words. We record **interaction patterns**, not visual trade dress: our app must not reuse Tally's
name, colours, logo, screen layout or wording.

Legend: **[vendor]** = claim from Tally Solutions' own material; **[3p]** = third-party blog/review;
**[unconfirmed]** = from memory or a single weak source, verify before relying on it.

Research limitations (be honest about gaps):
- reddit.com is not reachable from our research tooling, and several sites (tallysolutions.com blog,
  Capterra, G2, TrustRadius full pages) returned HTTP 403. Review sentiment below comes from search
  result snippets of those pages plus fetchable sources. Reddit/CAclubindia/Quora/YouTube sentiment is
  therefore **not directly sampled**; see Open questions.
- The web search budget for this session ran out before the voucher field-flow could be confirmed
  against a primary help page; those items are marked [unconfirmed].

---

## 1. Summary

1. **The whole product is one keyboard-driven screen stack.** A home menu ("Gateway") lists Masters,
   Vouchers (transactions), Utilities and Reports; every item has a single-letter hotkey (underlined
   letter). Enter goes deeper, Esc goes back one level. There is no visible window management; a
   breadcrumb/status path was deliberately removed in TallyPrime 1.0 [vendor].
2. **Command palette navigation** (Go To, Alt+G) was the headline change from Tally.ERP 9 to
   TallyPrime (2020). It searches reports/masters/voucher-creation by typed fragment and alias, and
   lets the user leave an unfinished voucher, look at a report, and come back to the voucher
   [vendor]. Switch To (Ctrl+G) is the same palette but *replaces* the current context instead of
   stacking on top of it [vendor].
3. **Function keys are a stable "verb vocabulary"** that accountants have in muscle memory since
   the DOS era: F4 Contra, F5 Payment, F6 Receipt, F7 Journal, F8 Sales, F9 Purchase, F2 date/period,
   F3 company, F11 features, F12 configure, Ctrl+A save, Esc back, Alt+C create-on-the-fly,
   Ctrl+Enter alter master, Alt+D delete, Alt+X cancel, Alt+2 duplicate, Alt+I insert. Most of these
   survived the ERP 9 to Prime transition unchanged [vendor].
4. **Data entry is a linear form**: Enter advances field-by-field, lookup fields pop up a filtered
   list that narrows as you type, unknown values can be created inline (Alt+C) without leaving the
   voucher, and sub-forms (bill allocation, bank details, stock batches) pop up automatically when the
   chosen ledger/item needs them.
5. **Reports are drillable and re-shapeable in place**: Enter drills into a line (group → ledger →
   voucher list → voucher), Esc climbs back, F2 changes period, Alt+F1/Alt+F5 toggles
   detailed/condensed, Ctrl+B "basis of values", Ctrl+H "change view", Ctrl+J exceptions, F12
   configuration, Ctrl+F filter (3.0+, was Alt+F12), Alt+C/Alt+N add/auto-repeat columns, and from
   a voucher list you can insert/duplicate/delete/cancel vouchers directly.
6. **User sentiment is split along a clear line**: trained accountants praise speed and no-mouse
   operation; newcomers, owners and younger staff find it intimidating, "typewriter-like", visually
   dated, with weak graphics, painful invoice customisation and slow/complex GST reports [3p].
7. **Multilingual support regressed in perception**: Tally.ERP 9 marketed ~12 Indian-language UIs
   with phonetic typing; TallyPrime's current help says UI *display* is only Arabic and Bangla (5.0+),
   while *data entry* can be in regional languages with per-master language aliases [vendor].

---

## 2. Navigation model

### 2.1 Home screen ("Gateway") structure (pattern only)
- Four groups: **Masters** (create / alter / chart of accounts), **Vouchers/Transactions**
  (voucher entry, day book), **Utilities** (banking, import, verification), **Reports**
  (balance sheet, P&L, stock summary, ratio analysis, display more reports, GST/TDS) [vendor].
- Each menu entry has an underlined single letter: pressing it activates the entry instantly
  (no Enter needed). Arrow keys + Enter also work; mouse click works since Prime [vendor].
- A **top menu bar** holds global, cross-cutting actions available on every screen: Company
  (Alt+K), Data (Alt+Y), Exchange (Alt+Z), Go To (Alt+G), Import (Alt+O), Export (Alt+E),
  Share/Email (Alt+M), Print (Alt+P), Help (F1). Single underline = Alt+letter, double underline =
  Ctrl+letter [vendor].
- A **right-hand vertical button bar** lists context-specific actions with their key (F2 Date,
  F3 Company, F11 Features, F12 Configure, plus screen-specific ones). Holding Alt or Ctrl swaps
  the bar to show the Alt-/Ctrl- variants (e.g. F2 Date → Alt+F2 Period) [vendor]. TallyPrime keeps
  button *positions* uniform across screens and shows blank slots rather than shifting buttons
  [vendor]. This is a discoverability device: the key legend is always on screen.
- A **bottom bar** (more actions) was hidden in Prime 1.0–2.0 and re-enabled by default from 2.0.1
  after feedback; it can be switched off [vendor]. (Signals that users missed the visible key legend.)
- Home also shows a panel of the currently loaded companies with current period/date.

### 2.2 Screen stack, Esc and Ctrl+Q
- Every action pushes a screen; Esc pops it (and on a dirty voucher asks to quit without saving
  [unconfirmed exact prompt]). Esc in a field first clears the typed input [vendor].
- Ctrl+Q exits a screen or the application; Alt+F4 quits [vendor].
- No breadcrumb: Prime removed the path display ("navigation is non-linear") [vendor]; instead
  **Show Open Reports** under Go To lists the open screens and lets you jump to any of them [vendor].
- Only one screen is visible at a time; viewing two reports side by side is not supported [vendor FAQ].

### 2.3 Go To (Alt+G) vs Switch To (Ctrl+G)
- Go To: type-ahead palette over reports, master create/alter/display, voucher creation, features
  and settings; matches on name fragments and **aliases** (e.g. common abbreviations); has a
  "Common Reports" group and a "Show Inactive" toggle that lists reports for features not yet enabled
  in F11 (with a hint to enable them) [vendor].
- Go To *stacks*: you can be mid-voucher, open a ledger report, then Esc back and resume the
  voucher exactly where you left it [vendor]. This is TallyPrime's "multitasking" [vendor].
- Switch To *replaces*: you cannot Esc back to where you were [vendor].
- Limitation: Go To cannot open an existing voucher by number or a master by name for display
  (per FAQ) [vendor] — a gap we can fill.

### 2.4 Companies and periods
- F3 switch company among loaded ones; Alt+F3 open company from another data path; Ctrl+F3 shut
  companies (was Alt+F1 in ERP 9) [vendor].
- F2 changes "current date" (voucher date) or report period; Alt+F2 changes company period [vendor].

### 2.5 Masters vs vouchers lists
- F10 inside create/alter toggles between "list of masters" and "list of vouchers" menus [vendor].
- Chart of Accounts screen handles multi-master create/alter [vendor].

---

## 3. Shortcut table

"Keep compatible?" = should our default keymap honour the same key for the same action, so that
Tally-trained users transfer muscle memory. Keyboard bindings are functional conventions, not trade
dress; we still name and label everything in our own words. A Tally-compatible keymap could be a
selectable preset rather than the only default (see Recommendations).

### 3.1 Global / navigation

| Action | Tally key | Notes | Keep compatible? |
|---|---|---|---|
| Command palette that stacks on current screen | Alt+G | Search reports/masters/voucher creation; resume after Esc | **yes** (also add Ctrl+K / Ctrl+P-style palette) |
| Command palette that replaces current screen | Ctrl+G | No return to previous context | yes |
| Back / close current screen; clear field | Esc | Prompts when unsaved [unconfirmed exact wording] | **yes** |
| Accept / save current form | Ctrl+A | Universal save in Tally | **yes** (also Ctrl+S) |
| Exit screen / app | Ctrl+Q; Alt+F4 quits | | yes |
| Help menu / contextual help | F1 / Ctrl+F1 | ERP 9 used Alt+H | yes (F1) |
| Change current date or report period | F2 | Context dependent | **yes** |
| Change company period | Alt+F2 | | yes |
| Switch / select / shut company | F3 / Alt+F3 / Ctrl+F3 | | yes |
| Company features (feature toggles) | F11 | | yes |
| Configure current screen | F12 | Per-report/per-voucher options | **yes** |
| Toggle masters/vouchers list | F10 | | no (low value) |
| Top menus: Company, Data, Exchange, Import, Export, Share, Print | Alt+K, Alt+Y, Alt+Z, Alt+O, Alt+E, Alt+M, Alt+P | | partial: yes for Alt+P/Alt+E/Alt+M |
| Find bar | Alt+F | | no — use Ctrl+F convention? conflicts with Tally filter; decide |
| Display language / data-entry language | Ctrl+K / Ctrl+W | ERP 9: Alt+G / Alt+K (reassigned in Prime) | no (Ctrl+K is our palette candidate; put language in settings) |
| Notifications | Ctrl+Alt+N | | no |
| Copy / paste in field | Ctrl+C / Ctrl+V | ERP 9 needed Ctrl+Alt+C/V | yes (OS standard) |
| Next/prev master or voucher | PgDn / PgUp | Browse records while in alter mode | **yes** |
| Next/prev date or report in sequence | + / – (in reports) | | yes (in reports only) |
| Expand/collapse group row | Alt+Enter / Shift+Enter | | yes |
| Calculator panel | Ctrl+N (toggle); Alt+C from an amount field | ERP 9 used Ctrl+M to hide | maybe — we prefer inline arithmetic in amount fields |

### 3.2 Voucher creation (from anywhere in voucher screen or Gateway voucher menu)

| Action | Tally key | Notes | Keep compatible? |
|---|---|---|---|
| Contra | F4 | Cash↔bank, bank↔bank | **yes** |
| Payment | F5 | | **yes** |
| Receipt | F6 | | **yes** |
| Journal | F7 | | **yes** |
| Sales | F8 | | **yes** |
| Purchase | F9 | | **yes** |
| Debit note / Credit note | Alt+F5 / Alt+F6 | | yes |
| Stock journal / Physical stock | Alt+F7 / Ctrl+F7 | | yes |
| Delivery note / Sales order | Alt+F8 / Ctrl+F8 | | yes |
| Receipt note / Purchase order | Alt+F9 / Ctrl+F9 | | yes |
| Payroll | Ctrl+F4 | | yes |
| Rejection out / in | Ctrl+F5 / Ctrl+F6 | | yes |
| Change voucher mode (item invoice / accounting invoice / single vs double entry) | Ctrl+H | Prime replaced ERP 9's mode-switching function keys | yes |
| More details for current line/field | Ctrl+I | Opens optional sub-fields | yes |
| Create master on the fly from a lookup field | Alt+C | Opens master form; on save returns with it selected | **yes** — critical |
| Alter the master in the current lookup field | Ctrl+Enter | Edit ledger/item without leaving voucher | **yes** — critical |
| Delete current line | Ctrl+D | | yes |
| Delete whole voucher | Alt+D | Confirmation prompt | **yes** |
| Cancel voucher (keep number, zero values) | Alt+X | Distinct from delete; keeps audit trail/number | **yes** |
| Mark post-dated | Ctrl+T | | yes |
| Mark optional (memo, not posted) | Ctrl+L | | yes |
| Retrieve narration of last voucher for same party / same type | Alt+R / Ctrl+R | Big speed trick | **yes** |
| Stock query for item | Alt+S | | yes |
| Related reports | Ctrl+O | | maybe |
| GST tax analysis of current voucher | Alt+A | | yes |
| Pick unused voucher number | Ctrl+Alt+U | | no |
| Autofill statutory details (AI "docs" autofill) | Ctrl+F | Recent release; named AI assistant [unconfirmed scope] | no |

### 3.3 Reports

| Action | Tally key | Notes | Keep compatible? |
|---|---|---|---|
| Drill down into line | Enter | group → ledger → vouchers → voucher | **yes** |
| Drill to voucher (bypass intermediate) | Ctrl+Enter | | yes |
| Back up one level | Esc | | **yes** |
| Expand/collapse a single line | Shift+Enter | | yes |
| Detailed / condensed toggle for whole report | Alt+F1 (also Alt+F5 in Prime) | ERP 9: Alt+F1 only | **yes** (Alt+F1) |
| Change period | F2 | | **yes** |
| Filter / range filter | Ctrl+F (3.0+); Alt+F12 (≤2.1, and ERP 9) | Ctrl+Alt+F shows filter details (7.0+) | **yes Ctrl+F**, also accept Alt+F12 |
| Balances computed from filtered rows | Ctrl+F then F7 (3.0+); Ctrl+F12 earlier | | maybe |
| Basis of values (scale, valuation, include optional/post-dated) | Ctrl+B | | yes |
| Change view / related reports | Ctrl+H | | yes |
| Exception reports (negative stock, optional, post-dated, etc.) | Ctrl+J | | yes |
| Configure report | F12 | Toggle columns/details, stripe view | **yes** |
| Narrow by context (voucher type, stock group) | F4 | | yes |
| Add / alter / delete column; auto-repeat columns | Alt+C / Alt+A / Alt+D / Alt+N | Comparative columns (period, company, cost centre) | yes |
| Monthly / periodic breakdown | ERP 9 users recall Alt+F5 "monthly"; Prime FAQ points to Ctrl+F6 or Ctrl+H/F12 | Conflicting sources | **[unconfirmed]** — decide our own |
| Insert voucher above selection | Alt+I | From Day Book / registers | yes |
| Duplicate selected voucher | Alt+2 | Copies, then change date, Ctrl+A | **yes** |
| Delete / cancel voucher from list | Alt+D / Alt+X | | yes |
| Hide (remove) a line from view / restore last / restore all | Ctrl+R / Ctrl+U / Alt+U | ERP 9 used Alt+U for restore last | yes |
| Select row / range / all / invert | Space; Shift+Up/Down; Ctrl+Space; Ctrl+Alt+I | Then act on selection (bulk delete, print) | yes |

### 3.4 Output

| Action | Tally key | Notes | Keep compatible? |
|---|---|---|---|
| Print current | Ctrl+P (Alt+P = print menu) | Configure sub-screen with copies, paper saving, page numbers | **yes** |
| Export current | Ctrl+E (Alt+E = menu) | PDF/Excel/XML/JSON etc. | **yes** |
| Email current | Ctrl+M (Alt+M = share menu) | | **yes** |
| WhatsApp current | Ctrl+Alt+W | Vendor integration | no (provide "share" generically) |

---

## 4. Entry-screen flow patterns

(Field order is a *pattern*; exact labels and layout are Tally's and must not be copied.)

- **Opening**: from anywhere press a voucher key (F4–F9 etc.) or Alt+G → "create voucher"; the
  screen opens with the cursor on the first editable field. Voucher number is auto-assigned; date
  defaults to the session "current date" and is changed with F2 (not by tabbing into a date field)
  [vendor/3p].
- **Enter is the universal "next field"**; Tab/Shift+Tab also move; Backspace in an empty field
  moves back [unconfirmed]. Fields not needed are skipped automatically depending on configuration
  and the masters chosen (e.g. bill-wise prompts only for party ledgers with bill-wise enabled).
- **Lookup fields pop a list on focus** (right side panel), grouped and expandable; typing filters
  by prefix/fragment and aliases; Enter selects the highlighted item; "Create" sits at the top of the
  list (Alt+C) so creating a missing ledger/item is one keystroke away [vendor]. After saving the
  new master you are returned to the voucher with it selected.
- **Ctrl+Enter on a filled lookup field** opens that master for alteration, then returns [vendor].
- **Sales/purchase flow (item invoice mode)**: party → (optional dispatch/order details pop-up) →
  sales ledger → item lines (item → qty → rate → amount auto-computed; Enter on empty item ends lines)
  → tax/discount ledgers below items with auto-calculated GST → narration → save [vendor partial;
  sub-step order unconfirmed].
- **Accounting vouchers** can be keyed as single-entry (account at top, many particulars) or
  double-entry (Dr/Cr or By/To lines); Ctrl+H switches mode per voucher without changing settings
  [vendor]. Amount on the balancing line is pre-filled with the remaining difference [unconfirmed
  but widely taught].
- **Contextual sub-screens pop up automatically**: bill-wise allocation (new ref / against ref /
  advance / on account), bank allocation (instrument number, date, transfer mode), cost-centre
  allocation, batch/godown allocation, GST details. Each sub-screen accepts with Enter on the last
  field or Ctrl+A and returns to the parent line.
- **Ctrl+I "more details"** reveals optional fields for the current line instead of showing every
  field always [vendor]; the technique keeps the default form short.
- **Save**: Ctrl+A anywhere, or Enter through the last field. ERP 9 asked a yes/no "accept?" at the
  end; Prime's default behaviour here is [unconfirmed]. After save, a *fresh* voucher of the same
  type opens immediately (continuous entry) [unconfirmed but consistent with training material].
- **Alter/delete/cancel/duplicate**: open from Day Book or any voucher register (Enter on row);
  in alter mode PgUp/PgDn walks to neighbouring vouchers; Alt+D deletes (with confirm), Alt+X
  cancels (keeps the number as void), Alt+2 duplicates (change date, Ctrl+A), Alt+I inserts a new
  voucher at that position (numbering follows) [vendor].
- **Calculator / arithmetic**: amount field supports a calculator panel (Ctrl+N / Alt+C) [vendor].
- **Speed tricks accountants use** (from training/third-party material; vendor confirmed where noted):
  - short **aliases** and codes on ledgers/items so type-ahead hits in 2–3 characters [vendor feature];
  - **Alt+R / Ctrl+R** to recall the previous narration [vendor];
  - **Alt+2 duplicate** for recurring entries (rent, salary) [vendor];
  - **Alt+C create on the fly** instead of pre-creating masters [vendor];
  - set **F2 current date** once and key a whole day's vouchers without touching date [3p];
  - voucher **classes / default ledgers** so tax and round-off lines auto-fill [unconfirmed detail];
  - stay in the voucher and use **Alt+G** to peek at a ledger balance, then Esc back [vendor];
  - **Alt+S** stock query while billing [vendor];
  - Excel/XML import (Alt+O) for bulk masters and bank statements [vendor].

---

## 5. Report interaction patterns

- **Drill-down chain**: summary report (Balance Sheet/P&L/Trial Balance) → group summary → ledger
  vouchers / monthly summary → voucher in alter mode. Enter goes down, Esc comes back to the same
  row selection [vendor].
- **In-place reshaping** without opening a new report: period (F2), detailed/condensed (Alt+F1),
  basis of values (Ctrl+B), change view (Ctrl+H), exceptions (Ctrl+J), context narrowing (F4),
  columns (Alt+C/N), filter (Ctrl+F), configure (F12) [vendor].
- **Monthly view**: ledger and group drill-downs commonly land on a month-wise summary before the
  voucher list (ERP 9 users associate this with "Alt+F5 / F5 monthly") [unconfirmed for Prime].
- **Comparative columns**: add a column for another period/company/cost centre; auto-repeat
  generates e.g. one column per month or per company [vendor].
- **Actions from reports**: insert/duplicate/delete/cancel vouchers directly in Day Book and
  registers; select rows with Space and act in bulk [vendor].
- **Hiding lines** temporarily (Ctrl+R) and restoring (Ctrl+U / Alt+U) to tidy a report before
  printing [vendor].
- **Output from any report**: Ctrl+P / Ctrl+E / Ctrl+M with a configure sub-screen; export format,
  folder, email settings consolidated in one "Configure" so the landing screen is one-key send
  [vendor].
- **Multitasking**: one visible screen; reports remain open in a stack; "Show Open Reports" to jump
  between them [vendor]. No split view [vendor].
- **Stripe view** (zebra rows) is an F12 option per report [vendor].

---

## 6. What users love / hate (with sources)

Caveat: Reddit, CAclubindia threads, Quora and YouTube comments could not be fetched by our
tooling; items below come from review aggregators (via search snippets), vendor FAQ and
third-party blogs. Treat as directional.

### Love
- **Keyboard speed / no mouse**: reviewers say shortcuts are "very simple" and it is very easy
  to navigate with keyboard shortcuts (G2, Capterra snippets) [3p]. Vendor positions the product as
  "keyboard-first" and fastest for data entry [vendor claim].
- **Learnability for accountants**: many reviewers say it is easy to pick up even without deep
  accounting knowledge [3p]; vendor claims non-accountants learn basics in 30–60 minutes [vendor
  claim, unverified].
- **Familiarity and transferability**: most Indian CAs/accounting staff learned on Tally; muscle
  memory is a moat (mith.tech, satvasolutions) [3p].
- **Offline, local data, reliability** and built-in GST/TDS compliance (satvasolutions, mith.tech) [3p].
- **Prime improvements**: Go To palette, more-details, change-mode, mouse support, consistent
  shortcuts, single-window multitasking (techjockey, vendor FAQ) [3p/vendor].

### Hate
- **Dated, "typewriter" look**: a TrustRadius reviewer wants the aesthetic modernised because using
  it feels like typing on a typewriter; a Capterra reviewer's main dislike is that the UI has barely
  changed over the years [3p].
- **Keyboard-dependence / weak mouse role**: G2 reviewer says the mouse plays a minimal role [3p].
- **Intimidating to newcomers**: fast once learned but intimidating for younger staff and
  non-accountant owners (finexo/indiasbestaitools-type 2026 blogs) [3p].
- **Reports, especially GST**: Prime made GST reports more complex and time-consuming; custom
  reports are slow to build (G2 snippets) [3p].
- **Invoice customisation**: limited / dated default invoice formats; customisation needs TDL
  specialists (Capterra, mith.tech) [3p].
- **Weak graphics/visualisation** (techjockey) [3p].
- **Performance**: occasional slow saves (Capterra) [3p].
- **Desktop-bound / remote access** via RDP workarounds (satvasolutions, mith.tech) [3p].
- **ERP 9 → Prime friction** (vendor FAQ reveals pain points): removed status/path bar, removed
  offline help, no classic colour scheme, bottom bar initially hidden then restored in 2.0.1,
  relocated shortcuts (help, filter, restore line, languages, shut company), Stock Query button
  removed, Daily button removed from registers [vendor]. A Techjockey Q&A says switching is
  "generally not difficult" because most keys stayed [3p].
- **Multilingual**: Prime's UI display languages are limited (Arabic, Bangla per current help) vs
  ERP 9's multi-Indian-language UI; numerals only in English (FAQ) [vendor].

---

## 7. Recommendations for our UI (keyboard-first + modern)

1. **Ship a "Classic keys" keymap preset ON by default for Indian users**, honouring the "yes"
   rows in section 3 (F4–F9 voucher keys, F2, F3, F11, F12, Ctrl+A, Esc, Alt+C, Ctrl+Enter, Alt+D,
   Alt+X, Alt+2, Alt+I, Alt+R, Ctrl+P/E/M, Enter-drill, Alt+F1, Ctrl+F). Also accept modern aliases
   (Ctrl+S save, Ctrl+K palette, Ctrl+Z undo). Keymap fully rebindable, exported as a text file.
   Use our own labels/wording.
2. **One command palette, two modes**: "open on top" (default, stackable, resumable) and
   "replace". Make it search *everything*: reports, masters by name/alias/GSTIN, vouchers by
   number/party/amount/date, settings, and natural-language AI intents ("sales to X last month").
   This fills Tally's gap of not finding vouchers by number.
3. **Keep the stack model but make it visible**: a slim breadcrumb + open-screens strip (tabs),
   because Prime users missed the path bar and the bottom bar. Allow **split view** (report beside
   voucher), which Tally cannot do.
4. **Always-visible key legend** (context action rail) that updates when Alt/Ctrl is held — proven
   discoverability device. Our layout/position/style must differ; e.g. a bottom hint bar or a
   collapsible side rail with our own design language.
5. **Entry forms: Enter-to-advance, auto-skip, inline create (Alt+C), inline alter (Ctrl+Enter)**,
   auto-popping sub-forms (bill allocation, bank details, batches) and "more details" (Ctrl+I) to
   keep default forms short. After save, open a fresh voucher of the same type. Balancing line
   pre-filled. Inline arithmetic in amount fields (type `1200*18%`).
6. **Type-ahead lists**: fuzzy + alias + code matching, recents first, shows balance/GSTIN/stock
   alongside each candidate, "create new" as first option when no exact match.
7. **Reports reshape in place** (period, detail level, basis, view, filters, columns) with
   Enter/Esc drill and *preserved selection* on return; add saved views and true charts (common
   complaint: weak graphics). Every number drillable to vouchers.
8. **Safety for delete vs cancel**: keep the two distinct semantics; add undo and an audit trail.
9. **Modern visual layer for newcomers**: full mouse/touch parity, readable typography, dark mode,
   but zero mouse requirement. Onboarding "key coach" that shows the shortcut when a user clicks.
10. **Multilingual**: full UI in major Indian languages (at least Hindi, Bengali, Marathi, Tamil,
    Telugu, Gujarati, Kannada, Malayalam, Odia, Punjabi), built-in phonetic transliteration input
    (no OS keyboard needed), per-master native-script aliases searchable from Latin input, Indian
    digit grouping (lakh/crore) and optional native numerals in print.
11. **Accessibility**: screen-reader labels on every field and list (Tally help says nothing on
    this; likely weak — unconfirmed), high-contrast theme, scalable fonts, focus always visible,
    no colour-only signals.
12. **Output**: Ctrl+P/E/M with one remembered configuration per document type; preview before
    send; PDF/Excel/JSON/XML.
13. **AI assistant**: palette-integrated; produces draft vouchers/filters the user reviews with the
    same Enter/Ctrl+A flow — never silently posts.

---

## 8. Open questions

- Exact Prime 7.x behaviour at end of voucher: is there still an "accept yes/no" prompt by default,
  and does a new blank voucher auto-open after save? [unconfirmed]
- Monthly breakdown key in Prime reports (Alt+F5 vs Ctrl+F6 vs drill default) — sources conflict.
- Backspace-to-previous-field behaviour and Tab vs Enter differences in Prime.
- Real practitioner sentiment from r/CAIndia, r/IndiaTax, CAclubindia forum, Quora, YouTube
  comments — not reachable with our tooling; a human should skim a sample.
- Extent and naming of Prime 7.x AI features (an autofill for statutory/document details is bound
  to Ctrl+F in voucher entry per the shortcut page; scope unconfirmed).
- Accessibility: any screen-reader support in TallyPrime? Nothing found.
- Whether ERP 9's Indian-language UI is truly gone in Prime or available via downloadable
  dictionaries (current help says display only Arabic/Bangla; another help page lists many Indian
  languages for data entry).
- Should our default be the Tally-compatible keymap or a modern one with the compatible keymap as a
  preset? Recommend compatible-by-default for Indian accounting users; needs user testing.

---

## 9. Sources

Vendor (Tally Solutions):
- Keyboard shortcuts (with ERP 9 comparison): https://help.tallysolutions.com/keyboard-shortcuts-tally-prime/
- Keyboard shortcuts (current): https://help.tallysolutions.com/tally-prime/keyboard-shortcuts/keyboard-shortcuts-tally-prime/
- TallyPrime FAQ (navigation, Go To/Switch To, changes from ERP 9, printing, languages): https://help.tallysolutions.com/tallyprime-faq/
- Release notes 1.0 (Go To, consistency, status bar removal, Ctrl+B/Ctrl+H/Ctrl+I): https://help.tallysolutions.com/tally-prime/release-notes-tally-prime/
- Working with reports: https://help.tallysolutions.com/working-with-reports/
- Use TallyPrime in your preferred language: https://help.tallysolutions.com/use-tallyprime-in-your-preferred-language/
- Multilingual (Indian languages list): https://help.tallysolutions.com/?p=108162
- Accounting entry / post-dated: https://help.tallysolutions.com/accounting-and-inventory/accounting-entry-tally/
- Home screen / Gateway (search snippet): https://help.tallysolutions.com/?p=108854
- Go To blog (403 to fetch; snippet only): https://tallysolutions.com/tally/tallyprimes-go-to-a-powerful-capability-to-discover
- Multitasking blog (403; snippet only): https://tallysolutions.com/tally/multi-tasking-just-got-easier-with-tallyprime
- Keyboard-first marketing (403; snippet only): https://tallysolutions.com/business-guides/tallyprime-keyboard-first-design-fast-data-entry/

Third party:
- Capterra reviews (snippets): https://www.capterra.com/p/127762/Tally-ERP-9/reviews
- G2 reviews (snippets): https://www.g2.com/products/tallyprime/reviews
- TrustRadius review (snippet): https://www.trustradius.com/reviews/tallyprime-2025-05-28-02-02-49
- Techjockey ERP 9 vs Prime: https://www.techjockey.com/blog/tally-erp-9-vs-tally-prime
- Techjockey Q&A on switching: https://www.techjockey.com/question/7058/is-it-difficult-to-operater-to-switch-from-erp9-to-prime-for-key-board-operation
- MithTech "Is Tally still relevant in 2026": https://mith.tech/blog/is-tally-still-relevant-2026
- Satva Solutions, Tally Prime automation: https://satvasolutions.com/blog/tally-prime-automation-indian-finance-teams
- Finexo, Tally alternatives 2026: https://finexo.in/blog/accounting/best-tally-alternatives-for-indian-businesses-2026
- CAclubindia ERP 9 shortcut article: https://www.caclubindia.com/articles/tally-erp-9-short-cut-function-keys-combination-31681.asp
