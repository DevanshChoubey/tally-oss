# 08 — Data, interoperability, multi-user and editions

Researcher area: how TallyPrime stores data, every documented way in and out of it, the connector
ecosystem, TDL customisation, multi-user/editions/pricing and the security model.
Date: 2026-11-20. Current vendor release line: TallyPrime 7.x (7.0 released 19 Dec 2025; 7.1 and 7.2
release notes exist on the help site).

Clean-room note: everything here comes from public vendor help pages, vendor marketing, third-party
blogs and the source/READMEs of public open-source connectors. No Tally binary was downloaded or run,
nothing was decompiled, and the on-disk file format was **not** examined. XML examples below were
written by me to illustrate documented shapes; they are not copied from Tally docs.

Labels: **[vendor]** = claim from Tally Solutions' own material; **[3p]** = third-party source;
**[code]** = observed in an open-source connector's source; **[unconfirmed]** = single weak source
or my inference.

Research limit: the shared web-search budget ran out part-way. Gaps are listed under Open questions.

---

## 1. Summary

1. **Live interop works through one door: TallyPrime's built-in HTTP server (default port 9000).**
   You POST an XML (or, from 7.0, JSON) envelope, and Tally answers. It can export almost anything
   (reports, collections, objects, and *inline TDL* that defines new reports on the fly) and import
   masters and vouchers (documented actions Create/Alter/Cancel; Delete [unconfirmed]). Tally must be running and the target company
   loaded. Every serious integrator (ClearTax, the open-source DB loaders, ERPNext migrators, MCP
   servers) uses this door [vendor][code][3p].
2. **The on-disk company folder is a proprietary, versioned binary store** (`.900` files up to
   TallyPrime 2.1, `.1800` from 3.0, plus `.tsf` state files) [vendor]. We must **not** parse it.
   Migration has to go through Tally itself (HTTP/XML/JSON, XML file export, Excel/ODBC), which
   means **the user needs a working TallyPrime install at migration time** (whether an
   educational-mode or lapsed install suffices is [unconfirmed]). That shapes the product: we ship a "Tally bridge" that talks to a local Tally.
3. **Two-way sync is feasible but needs care.** Change detection works via company-level `AltMstId`
   / `AltVchId` counters and per-object `AlterID` + `GUID`, which the MIT-licensed
   `tally-database-loader` uses for incremental sync [code]. Deletions are detected only by diffing
   GUID sets. Writes go back as `TALLYMESSAGE` imports. There are no webhooks or change feeds. Tally
   has to be polled, the endpoint has no authentication, and responses can contain characters that
   are not valid XML.
4. **The customisation ecosystem is large and runs on TDL** (Tally Definition Language). There are
   reportedly 28,000+ partners (2022, [3p]), TallyShop add-ons in about 17 categories [vendor], and
   invoice formats are the most common customisation. TallyPrime 7.1 moved basic invoice layout to
   built-in templates [vendor]. For us: a plugin system with declarative print templates, custom
   fields on masters and vouchers, validation hooks and custom reports covers most of what TDL
   add-ons do. We cannot run TDL, and should not try.
5. **Editions and pricing are simple and cheap**, so our pitch must rest on UX and openness rather
   than price alone. Silver (1 user) costs about ₹22,500 and Gold (unlimited users on one LAN) about
   ₹67,500, both one-time plus 18% GST. TSS renewal is ₹4,500 (Silver) or ₹13,500 (Gold) a year.
   Rental runs ₹750 to ₹2,250 a month. TallyPrime Server costs about ₹2,70,000. Cloud access is about
   ₹600 per user per month on top of a licence [vendor, via search snippets]. Remote access and
   browser reports require TSS and Tally.NET [vendor].

---

## 2. XML/HTTP integration interface

### 2.1 Transport

- Enable it under F12 / Advanced Configuration: TallyPrime acts as "Server" or "Both", and the port
  defaults to 9000. ODBC is enabled separately [vendor]. The same switch was documented for
  Tally.ERP 9 [3p: ClearTax setup docs].
- Plain HTTP POST to `http://<host>:9000/`, with the envelope as the body. SOAP is not native
  [vendor]. Opening the URL in a browser is a documented liveness check.
- **Encoding:** `tally-database-loader` sends and receives **UTF-16LE** with
  `Content-Type: text/xml;charset=utf-16` [code]. Whether UTF-8 is also accepted, and which
  encoding is canonical, is [unconfirmed]. Test non-ASCII names (Hindi ledger names, the ₹ sign).
- **Dirty output:** the C# `TallyConnector` library wraps responses in a cleaner that strips a
  stray `&#4;` sequence and escapes raw control characters 0x00–0x1F. Tally output is therefore not
  always well-formed XML [code]. Our parser must be lenient and must report what it repaired.
- **Volume:** the loader extracts vouchers in batches of 5,000 by date range. It warns that batches
  over about 10,000 can make Tally hang indefinitely [code]. Whether Tally serves one
  request at a time and blocks its UI meanwhile is [unconfirmed]. ClearTax's "use one tab only"
  note concerns its own UI.
- **No authentication and no TLS** on the port [vendor docs describe none; 3p advice is to firewall
  it or use a VPN]. Tally's own help publishes a *reference pattern*, not a product, for remote
  access: an on-premises connector that opens an outbound WSS connection to a cloud relay and
  forwards requests to the local HTTP port [vendor].

### 2.2 Envelope shape (documented)

```
ENVELOPE
├── HEADER
│   ├── VERSION        (1)
│   ├── TALLYREQUEST   Import | Export | Execute
│   ├── TYPE           Data | Collection | Object | Function | Action(Sync)
│   ├── SUBTYPE        (optional)
│   └── ID             report / collection / object / function name   (TYPE attr for Object: Name, GUID…)
└── BODY
    ├── DESC
    │   ├── STATICVARIABLES  SVCURRENTCOMPANY, SVFROMDATE, SVTODATE, SVEXPORTFORMAT …
    │   ├── REPEATVARIABLES
    │   ├── FETCHLIST / FETCH   (methods/fields to return)
    │   ├── FUNCPARAMLIST / PARAM
    │   └── TDL / TDLMESSAGE    (inline TDL: REPORT, FORM, PART, LINE, FIELD, COLLECTION, SYSTEM Formulae)
    └── DATA
        └── TALLYMESSAGE        (import payload: LEDGER, GROUP, STOCKITEM, UNIT, GODOWN, VOUCHER …)
```

Response header has `VERSION` and `STATUS` (1 = success, 0 = failure) [vendor]. The "Import
Data" form uses `REQUESTDESC/REPORTNAME` (`All Masters` or `Vouchers`) plus
`REQUESTDATA/TALLYMESSAGE` [vendor]. The `BODY/IMPORTDATA` wrapper in example (d) is the common
connector convention [unconfirmed against the vendor page].

Import result counters, documented [vendor] and confirmed in connector code [code]: `CREATED`,
`ALTERED`, `DELETED`, `COMBINED`, `IGNORED`, `ERRORS`, `CANCELLED`, `LASTVCHID` (the internal id
of the last voucher), `LASTMID` (always 0 per the docs), and `LINEERROR` with a text message. Tally
also writes an import log file, `Tally.imp`, in its install folder [vendor].

### 2.3 Examples (written by me, illustrating documented shapes)

**(a) Export a built-in report (Trial Balance) as XML for one company and period**

```xml
<ENVELOPE>
  <HEADER>
    <VERSION>1</VERSION>
    <TALLYREQUEST>Export</TALLYREQUEST>
    <TYPE>Data</TYPE>
    <ID>Trial Balance</ID>
  </HEADER>
  <BODY><DESC><STATICVARIABLES>
    <SVCURRENTCOMPANY>Acme Traders</SVCURRENTCOMPANY>
    <SVFROMDATE>20260401</SVFROMDATE>
    <SVTODATE>20270331</SVTODATE>
    <SVEXPORTFORMAT>$$SysName:XML</SVEXPORTFORMAT>
  </STATICVARIABLES></DESC></BODY>
</ENVELOPE>
```

**(b) Export a collection defined inline: all ledgers with chosen fields.** This is the workhorse
pattern for extraction.

```xml
<ENVELOPE>
  <HEADER>
    <VERSION>1</VERSION><TALLYREQUEST>Export</TALLYREQUEST>
    <TYPE>Collection</TYPE><ID>OssLedgers</ID>
  </HEADER>
  <BODY><DESC>
    <STATICVARIABLES><SVCURRENTCOMPANY>Acme Traders</SVCURRENTCOMPANY></STATICVARIABLES>
    <TDL><TDLMESSAGE>
      <COLLECTION NAME="OssLedgers">
        <TYPE>Ledger</TYPE>
        <FETCH>GUID, AlterID, Name, Parent, OpeningBalance, PartyGSTIN</FETCH>
        <FILTER>OssChangedSince</FILTER>
      </COLLECTION>
      <SYSTEM TYPE="Formulae" NAME="OssChangedSince">$AlterID &gt; 1234</SYSTEM>
    </TDLMESSAGE></TDL>
  </DESC></BODY>
</ENVELOPE>
```

The field names above (GUID, AlterID, Parent, OpeningBalance) match those used by open-source
loaders [code]. Treat each field name as unverified until it has been checked against a real
export.

**(c) Read the company-level change counters**, the trick used for incremental sync [code]: an
inline report over the `Company` collection, filtered to `##SVCurrentCompany`, that emits
`$AltMstId` and `$AltVchId`. If the counters have not moved since the last poll, nothing changed.

**(d) Import, creating a ledger** [vendor shape]

```xml
<ENVELOPE>
  <HEADER><TALLYREQUEST>Import Data</TALLYREQUEST></HEADER>
  <BODY><IMPORTDATA>
    <REQUESTDESC>
      <REPORTNAME>All Masters</REPORTNAME>
      <STATICVARIABLES><SVCURRENTCOMPANY>Acme Traders</SVCURRENTCOMPANY></STATICVARIABLES>
    </REQUESTDESC>
    <REQUESTDATA>
      <TALLYMESSAGE xmlns:UDF="TallyUDF">
        <LEDGER NAME="Sharma Hardware" ACTION="Create">
          <NAME>Sharma Hardware</NAME>
          <PARENT>Sundry Debtors</PARENT>
        </LEDGER>
      </TALLYMESSAGE>
    </REQUESTDATA>
  </IMPORTDATA></BODY>
</ENVELOPE>
```

**(e) Import, a simple sales voucher in accounting (non-invoice) view** [vendor shape]. The sign
convention is the trap here. A debit line carries `ISDEEMEDPOSITIVE=Yes` and a **negative**
`AMOUNT`. A credit line carries `No` and a positive amount [vendor sample page]. Each voucher must
balance to zero, or Tally returns a `LINEERROR`.

```xml
<TALLYMESSAGE>
  <VOUCHER VCHTYPE="Sales" ACTION="Create">
    <DATE>20261115</DATE>
    <VOUCHERTYPENAME>Sales</VOUCHERTYPENAME>
    <VOUCHERNUMBER>S-0042</VOUCHERNUMBER>
    <PARTYLEDGERNAME>Sharma Hardware</PARTYLEDGERNAME>
    <PERSISTEDVIEW>Accounting Voucher View</PERSISTEDVIEW>
    <LEDGERENTRIES.LIST>
      <LEDGERNAME>Sharma Hardware</LEDGERNAME>
      <ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE>
      <AMOUNT>-11800.00</AMOUNT>
    </LEDGERENTRIES.LIST>
    <LEDGERENTRIES.LIST>
      <LEDGERNAME>Sales @18%</LEDGERNAME>
      <ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>
      <AMOUNT>10000.00</AMOUNT>
    </LEDGERENTRIES.LIST>
    <LEDGERENTRIES.LIST>
      <LEDGERNAME>Output IGST</LEDGERNAME>
      <ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>
      <AMOUNT>1800.00</AMOUNT>
    </LEDGERENTRIES.LIST>
  </VOUCHER>
</TALLYMESSAGE>
```

Invoice view uses `ISINVOICE=Yes`, `ALLINVENTORYENTRIES.LIST` (with batch and accounting
allocations) and `BILLALLOCATIONS.LIST` for bill-wise references [vendor]. To alter or cancel a
voucher, identify it by date, voucher type and a tag/value pair such as the voucher number, then set
`ACTION="Alter"` or `ACTION="Cancel"` [vendor]. `ACTION="Delete"` is [unconfirmed]: I saw it neither
on the vendor sample page nor in the connector code I read. Only the `DELETED` counter is documented. Masters must exist before any voucher that references
them [vendor].

**(f) JSON (TallyPrime 7.0+).** The routing fields move into HTTP headers: `content-type:
application/json`, `version`, `tallyrequest: Import|Export`, `type: Data|Collection|Object|Function`,
`id`, and an optional `detailed-response`. The payload uses Tally's native object structure. There
are two dialects: "JSON" (dating back to Tally.ERP 9 6.2, via TDL) and "JSONEx" (native from 7.0)
[vendor]. I did not obtain a full JSONEx schema. **XML remains the baseline**, because every
deployed Tally (ERP 9 included) speaks it.

### 2.4 Limitations relevant to us

- Tally must be running, with the company open (or openable) and the HTTP server enabled. A headless
  or service mode exists only in the separate TallyPrime Server product [vendor].
- There is no push notification: polling only.
- No authentication, and the endpoint is single-threaded in practice.
- Field-level semantics live in TDL object methods (`$Name`, `$Parent`, `$ClosingBalance`). The
  official reference for every method exists in Tally Developer docs and is extensive but not
  exhaustive [vendor; 3p].
- Computed reports such as GST returns and stock valuation can be exported as *Tally's numbers*. We
  recommend using them as **observed expected values in reconciliation tests**, recorded with their
  provenance (TallyPrime version and date).
- Data integrity rules (balanced vouchers, master existence, locked periods, Edit Log) are enforced
  by Tally on import. Expect rejections, and surface every `LINEERROR` to the user.

---

## 3. Import/export paths ranked

### 3.1 For a one-shot migration tool (Tally → us)

| Rank | Path | Fidelity | Effort | Notes |
|---|---|---|---|---|
| 1 | **HTTP XML collections with inline TDL** (live Tally on the same PC or LAN) | Highest: GUIDs, AlterIDs, bill-wise, cost centres, batches, godowns, GST fields, payroll | Medium | Proven by `tally-database-loader` (33 tables, including its own `config` table, covering masters, vouchers, accounting, inventory, bills, bank, batch, cost centres, payroll) [code]. Needs Tally running. Batch by date range. |
| 2 | **Offline XML file export** from Tally's Export menu (masters and Day Book/vouchers as `.xml`) | High for masters and vouchers; less control over fields | Low for the user, medium for us | Works when the user (or their CA) can export but cannot run us beside Tally. Same parser as path 1. |
| 3 | **JSON (7.0+) via HTTP or file** | Probably equal to XML [unconfirmed] | Medium | Only for 7.x users. Schema not fully public in what I found. |
| 4 | **ODBC** (Tally as ODBC server, DSN `TallyODBC_<port>`, SQL-like `SELECT $Name FROM Ledger`) | Medium: only collections flagged `IsODBCTable`; awkward for nested voucher lines | Medium; driver platform support [unconfirmed] | Useful for Excel/Power BI users. Not our primary path [vendor]. |
| 5 | **Excel/CSV exports of reports** | Low: report-shaped, not ledger-postings-shaped | Low | Last resort: opening balances plus current-year Day Book. |
| ✗ | Parsing `.900`/`.1800` company files directly | n/a | n/a | **Prohibited for us**: proprietary format, and reverse-engineering it is legally risky (section 7). |

Recommended migration recipe: (1) detect Tally on `localhost:9000` and list its companies;
(2) pull masters in dependency order (groups → ledgers → units → stock groups/categories → stock
items → godowns → cost categories/centres → voucher types → employees/pay heads); (3) pull vouchers
in date batches, capturing raw XML to disk first; (4) stage everything keyed by **Tally GUID**;
(5) reconcile our Trial Balance, stock summary and party outstandings against Tally's own exported
reports for the same dates, which gives *observed* expected values; (6) produce a signed
reconciliation report. The MIT ERPNext migrator follows almost this exact architecture
(raw capture → SQLite staging → target API → GL reconciliation) [code].

Older data: Tally.ERP 9 (5.0+) speaks the same XML. Tally 7.2/5.4 data must be upgraded through
Tally's own migration tool chain first [vendor]. We should support only TallyPrime and Tally.ERP 9
as live sources.

### 3.2 For live two-way sync (coexistence period)

| Rank | Direction | Mechanism | Risk |
|---|---|---|---|
| 1 | Tally → us (read) | Poll `AltMstId`/`AltVchId` every N minutes. On change, pull `$AlterID > last` per collection, then diff GUID sets for deletions | Low to medium. Proven [code]. The loader itself warns incremental mode is fragile and recommends single-FY use |
| 2 | Us → Tally (write) | `TALLYMESSAGE` import with `ACTION=Create/Alter`. Read back `LASTVCHID`, then fetch the new voucher's GUID/AlterID | Medium. Name-based master references (ledger *names*, not IDs) mean renames break links. Need a name↔GUID map and a pre-check that every referenced name exists (the approach `dave8172/tally-aiagent` advertises) |
| 3 | Conflict handling | Our own vector clock: (our version, Tally AlterID) per object. Treat Tally as source of truth for periods it has locked | High. Simultaneous edits, Edit Log companies, and TDL add-ons that add UDF fields we do not know about |
| 4 | Remote sync (Tally in office, us elsewhere) | Our small local agent next to Tally, opening an outbound TLS tunnel (the pattern Tally itself documents for WSS) | Medium. We must ship and secure the agent ourselves |

Design implications: keep a `tally_guid`, `tally_alterid`, `tally_master_id` and the source company
name on every imported object. Preserve unknown UDF (`UDF:` namespace) fields as an opaque blob so
round-trips do not drop add-on data. Make "Tally bridge" a separate headless crate with no UI
dependency, plus a CLI.

### 3.3 Us → Tally (exit path, also needed for CA hand-off)

Generating Tally-importable XML files is easy, given the many small MIT and unlicensed
Excel/JSON→Tally-XML converters on GitHub, and doubles as a CA hand-off feature: "send books to your
CA who uses Tally". It is cheap, valuable, and lowers switching anxiety.

### 3.4 Tally's own Excel import (TallyPrime 4.0+)

Since 4.0, Tally can import masters and vouchers from **any** Excel layout via saved "mapping
templates". Masters need Name and Group. Vouchers need type, date, number, ledger and amount, and
lines may span rows. Templates are shareable `.tsf` files kept in an `excelmaps` folder [vendor].
For us this sets the baseline: our importer must match it, mapping arbitrary sheets to our objects,
with saved reusable mappings and an error log. The AI assistant can propose the mapping, and the
user confirms it.

### 3.5 How Tally stores data, and its backup, split, repair and migration (public facts only)

- **Data path and company folders** [vendor]: each company is a folder under the configured data
  path, named by its company number. Numbers were 5 digits (for example `10000`) before
  TallyPrime 3.0 and are 6 digits after migration to 3.0 or later. The migrated 6-digit company is
  created in the same data folder as the old one, and the original 5-digit copy is kept.
- **File families by release** [vendor]: `.500` (Tally 5.4/6.3/7.2), `.900` (Tally 9, Tally.ERP 9,
  TallyPrime up to 2.1) and `.1800` (TallyPrime 3.0 onwards, for example `Company.1800` and
  `TranMgr.1800`), plus `.tsf` state, message and access files used for synchronisation and
  recovery. No newer family is documented for 6.x or 7.x. Release 3.0 data is larger because Tally
  adds internal "purpose" data for GST and Edit Log reports. **We treat all of these as opaque**,
  but a migration wizard may *locate* company folders by name so it can tell the user which company
  to open in Tally.
- **Forward-only compatibility** [vendor]: data from a higher release won't open in a lower one.
  After migration, saving in the old release is blocked. From 7.0, users of 2.1 or earlier, or of
  Tally.ERP 9 5.0+, can keep working in the old release during migration, and can pause and resume
  it. For 3.0+ the advice is to wait until migration finishes. Migration can't be stopped safely
  once started, so back up first. In practice, the Tally version a user runs determines which
  fields our bridge can see.
- **Migration chains** [vendor]: Tally.ERP 9 5.0+ migrates straight to current TallyPrime. Tally
  7.2/5.4/6.3 data goes through Tally.ERP 9's migration tool first, and 4.5 data must be converted
  to 7.2 first. Old `.tcp` customisations need conversion. For us: support TallyPrime and
  Tally.ERP 9 as live sources, and tell users with older data to upgrade it in Tally first.
- **Repair** [vendor]: Data → Repair, or a "Repair Required" prompt when a company is loaded. It
  needs free disk of at least twice the data size and a prior backup. Repair, Split or Restore must
  not run during a migration. Third-party recovery guides warn that repair can drop records it
  considers bad [3p], so our migrator should run **after** a successful Repair and Verify, and
  should reconcile totals.
- **Backup and restore** [vendor]: manual or scheduled backups (once, daily, weekly, monthly) to a
  local drive or TallyDrive (7.0+, needs TSS and a Tally.NET ID). Backups are compressed and can
  have an optional backup password, with a recovery key emailed to the Tally.NET ID. TallyDrive
  quota depends on Silver vs Gold, with an alert at 80%. When TSS lapses, new cloud backups stop,
  and existing ones stay for 90 days before deletion. Data entry is blocked while a backup runs.
  Restore offers Replace (destructive) or Skip. Scheduled backup works only for data migrated to
  7.0. The backup file format is undocumented, so we **do not read Tally backups**.
- **Split company data** [vendor]: Data → Split → Verify Data first, then split from a chosen date
  into either (a) a company from the split date onward, (b) a company before it, or (c) two
  companies. The prerequisites are finalised books, adjusted forex, resolved pending bills,
  completed TDS/TCS and GST filings and reconciliation, and a backup. How opening balances carry
  over, and how GUIDs behave across split companies, is not documented [unconfirmed]. That matters
  when a user wants multi-year history merged into one of our books: we will need to stitch
  per-FY Tally companies together, using their opening balances as checkpoints.

---

## 4. Existing open-source connectors

Stars and licences checked via the GitHub API on 2026-11-20.

| Repo | Licence | Stars | Lang | What it does / what we learn |
|---|---|---|---|---|
| [dhananjay1405/tally-database-loader](https://github.com/dhananjay1405/tally-database-loader) | MIT | 165 | TS/Node | Tally → SQL Server/MySQL/Postgres/BigQuery/CSV/JSON. Inline-TDL collection extraction, full + **incremental (AlterID)** sync, 33-table schema, batch-size warning. TallyPrime only (ERP 9 support dropped). Best reference for extraction. |
| [dhananjay1405/tally-mcp-server](https://github.com/dhananjay1405/tally-mcp-server) | MIT | 114 | — | MCP server feeding Tally data to LLMs. Same author. |
| [Accounting-Companion/TallyConnector](https://github.com/Accounting-Companion/TallyConnector) | **none declared** (treat as all rights reserved) | 96 | C# | .NET library with typed models and source-generated TDL requests. Claims TallyPrime 4–7. Its response model confirms the import counters, and its cleaner shows Tally emits invalid XML characters. **Read for learning only; do not copy code.** |
| [ComplyEaze/bridge](https://github.com/ComplyEaze/bridge) | Apache-2.0 | 36 | Rust | TallyPrime MCP server for CA firms. Read-only by default, with guarded writes. Its README says Tally Solutions ships its own TallyPrime MCP plug-in [unconfirmed]. A Rust precedent. |
| [ramajayam-CA/Tally-Connector](https://github.com/ramajayam-CA/Tally-Connector) | none | 28 | TDL | Excel connector via TDL and ODBC. |
| [rudra-sah00/TallyWeb](https://github.com/rudra-sah00/TallyWeb) | MIT | 23 | Go/React | Browser dashboard over the XML API. |
| [GreenHacker420/Tally_sync](https://github.com/GreenHacker420/Tally_sync) | MIT | 21 | — | Unmaintained Tally ↔ cloud ERP sync prototype. |
| [Forgesaroj/tally-xml-kit](https://github.com/Forgesaroj/tally-xml-kit) / [tally-mock-server](https://github.com/Forgesaroj/tally-mock-server) | MIT | 9 / 4 | TS/JS | Typed envelope builders, response-counter parser, and a **deterministic mock Tally HTTP server for tests**. The mock idea is worth adopting (with our own fixtures, marked synthetic). |
| [manikantadm09/tally-prime-erpnext-migrator](https://github.com/manikantadm09/tally-prime-erpnext-migrator) | MIT | 7 | Python | Auditable Tally → ERPNext migration: raw capture, staging keyed by GUID, explicit write confirmation, GL reconciliation. Closest analogue to our migration tool. |
| [Wadhawnaiya/TallyMind](https://github.com/Wadhawnaiya/TallyMind), [taxor-ai/tally-mcp](https://github.com/taxor-ai/tally-mcp), others | MIT / none | <10 | Py/Go | More MCP servers. The 2026 trend is to put AI on top of Tally via XML. |
| [labs-infinitum/tallyprime-sdk](https://github.com/labs-infinitum/tallyprime-sdk), [Shivvam/tallyprimemcp](https://github.com/Shivvam/tallyprimemcp) | Apache-2.0 / other | 1 | Rust | Small Rust SDK and MCP server. Too small to depend on. |
| [khataos/khataos](https://github.com/khataos/khataos) | AGPL-3.0 | 3 | Rust | Competing offline OSS accounting app (India/UAE/KSA…). Watch it. |
| svnix-solutions/tally_erpnext, jobin05/erpnext_tally_migrator | GPL-3.0 | ~5 | Python | ERPNext-side Tally import. GPL: don't copy code. |
| Odoo `tally_xml_connector` | LGPL-3 (paid) | — | Python | Odoo → Tally XML vouchers. |
| CData Tally ODBC/JDBC/ADO drivers | commercial | — | — | Show there is demand for SQL access to Tally. |

Commercial integrators: **ClearTax** GST connector reads from Tally through the port-9000 server
(Tally set to "Both" with ODBC enabled), needs the company open, pulls only GST-applicable vouchers,
and warns against running in parallel tabs [3p: ClearTax docs]. **Zoho Books**' official migration
is a file-based remap (accounts → chart of accounts, debtors/creditors → contacts, opening balances),
not live sync. Third-party guides note cost centres, batches, memo vouchers and edit logs don't carry
over, and most firms migrate opening balances plus the current year only [3p]. **Suvit, Vyapar**: no
data gathered (search budget exhausted) [unconfirmed; reportedly XML/HTTP-based].

Licence takeaway: MIT/Apache repos are safe to *learn from and cite*. Our Rust implementation should
be written fresh. Copying would anyway require attribution, and the most-starred C# library has no
licence at all.

---

## 5. TDL and customisation landscape: plugin-system implications

**What TDL is** [vendor]: Tally Definition Language, the declarative and procedural language that
the whole Tally UI is written in. It defines Reports → Forms → Parts → Lines → Fields over
Collections of Objects, plus Functions and Formulae, and lets third parties alter or extend any
built-in definition. Add-ons ship as `.tdl` source or compiled `.tcp` files, loaded via the
TDL/add-on configuration. Old 7.2-era `.tcp` files need conversion for TallyPrime [vendor]. TDL also
powers integrations: the inline TDL in XML requests is the same language, and TDL can call ODBC,
HTTP/JSON and Excel data sources (Excel source objects added in Developer Release 4.0) [vendor].
TallyPrime Developer is the vendor IDE. Its price was not gathered [unconfirmed].

**Ecosystem size:** 28,000+ partners in 2022 [3p, Wikipedia], over 9.25 lakh customers on
Release 6.x [vendor partner update], and TallyShop add-ons in about 17 categories such as
Invoicing, Workflow, Business Verticals and Alerts & Controls [vendor]. A large part of the partner
channel earns from TDL customisation and AMC.

**What businesses rely on most** (synthesis of vendor and third-party material; ranking
[unconfirmed]):
1. Custom invoice and print formats: logos, signatures, terms, QR/UPI codes, extra columns, multiple
   copies, industry layouts. Tally itself acknowledged this by shipping 7 built-in templates and
   template personalisation in **7.1** ("no TDL needed for layout") [vendor].
2. Extra fields (UDFs) on masters and vouchers: vehicle number, transporter, salesperson, site,
   batch attributes.
3. Controls and validations: block negative stock, credit limits, mandatory fields, rate-change
   locks (Tally's FAQ says rate-change restriction *needs customisation*) [vendor].
4. Custom MIS reports and registers, often exported to Excel.
5. Vertical modules: construction, pharma (batch and expiry), textile, jewellery, transport.
6. Integration add-ons: SMS/WhatsApp, e-commerce, bank, barcode, auto-backup.

**Implications for our plugin system:**
- A **declarative template engine for documents** (invoice, challan, voucher print), with versioned
  templates, conditional blocks, per-party and per-voucher-type selection, and PDF output. This
  alone covers the #1 use case without code. Templates are data, so they can be shared.
- **Custom fields** on any master or voucher (typed, validated, printable, reportable, importable),
  and round-tripping of unknown Tally UDFs.
- **Hooks**: pre-save validation (deny with reason), post-save side effects, computed defaults.
  Run sandboxed: WASM components are the obvious choice in a Rust app, with capability-scoped
  access (read ledgers, propose vouchers) and no raw DB writes.
- **Custom reports** defined as queries over our engine. The engine computes and the plugin only
  lays out, so every displayed number stays engine-computed (recommended principle).
- **Import/export adapters** as plugins (bank formats, marketplaces, Tally XML itself).
- A marketplace and signing story for partners. The CA/partner channel is the distribution moat, so
  give them a way to earn (paid plugins, implementation services).
- We will **not** execute TDL. A best-effort "TDL add-on inventory" during migration would help:
  list the add-ons and UDFs found, so a partner can rebuild them.

---

## 6. Multi-user, editions, pricing

### 6.1 Editions [vendor unless marked]

| Edition | Users | Notes |
|---|---|---|
| **Silver** | 1 user on one PC | Single-user licence. |
| **Gold** | Unlimited concurrent users **on one LAN** | Unlimited concurrent users on a single LAN [vendor guide]. The client/host data mechanism is [unconfirmed]. Cross-branch or genuinely remote use needs Tally.NET remote access or Server [vendor guide]. |
| **Edit Log** variants (Silver/Gold) | as above | The same product with the Edit Log (audit trail) **always on** and impossible to disable, for companies under the MCA audit-trail rule [vendor] (rule citation and effective date [unconfirmed] here; see the reports/audit research). Data moves between the standard and Edit Log 3.0+ releases in both directions [vendor]. |
| **TallyPrime Server** | Gold add-on | Runs as a Windows service. Gives true concurrent access (users don't block each other on load, save, report or backup), a monitor (who is logged in, open companies, disconnect users), server-side permissions and backup control. Clients don't need the physical data path [vendor]. |
| **Educational / trial** | — | The 7-day trial is Gold with TSS [vendor]. Educational mode limits date entry (well known; [unconfirmed] details). |

### 6.2 Pricing (India, ex-GST 18%)

From Tally's own TCO and renewal pages, seen via search snippets (direct fetch was blocked with
HTTP 403), so treat as **[vendor, unconfirmed-current]**:

| Item | Price |
|---|---|
| Silver perpetual (includes 1 year TSS) | ₹22,500 |
| Gold perpetual (includes 1 year TSS) | ₹67,500 |
| TSS renewal, 1 year | Silver ₹4,500 · Gold ₹13,500 (2-year: ₹8,100 / ₹24,300; about 10% offers common) |
| Rental (includes TSS) | Silver ₹750/mo, ₹2,138/3 mo, ₹8,100/yr · Gold ₹2,250/mo, ₹24,300/yr |
| TallyPrime Server | ₹2,70,000 (Bangladesh page: USD 9,450) |
| TallyPrime Cloud Access (hosted, browser) | from ₹600/user/month + GST, **licence bought separately**. Personal pack: 2 vCPU, 4 GB RAM, 5 GB storage + 5 GB backup. Pages variously say AWS or Oracle Cloud [vendor, inconsistent] |
| TallyDrive cloud backup | needs TSS; about 3 GB free [vendor] |

Conflicting third-party figures exist (Silver ₹18,000 / Gold ₹54,000, Gold renewal ₹10,800–₹12,150),
likely older lists or partner discounts [3p].

**What TSS buys** (assembled from the vendor feature pages above; full list [unconfirmed]): product updates (new releases and statutory updates for GST, TDS and
e-invoice), Tally.NET connected services (remote access, browser reports, sync), TallyDrive, e-way
bill and e-invoice connectivity, PrimeBanking, and support. Without TSS the product keeps working
but freezes at its version. **Compliance updates are the real lock-in lever.** Our free updates are
a strong counter-pitch.

### 6.3 Remote and browser access [vendor]

- **Remote Access via Tally.NET:** a remote user with a TallyPrime install and a Tally.NET ID
  connects to the office machine. Data stays on the host and traffic is encrypted. It requires TSS,
  F11 → Online Access, and Company → Connect.
- **Reports in Browser:** read-only viewing of reports in any browser, with no data entry. It
  requires a Tally.NET ID, a connected company and TSS. An ID used for browser access can't also be
  used for remote access.
- **Hosted Tally on cloud:** both the vendor's own Cloud Access and a large third-party market
  (Tally-on-cloud resellers, RDP/RemoteApp hosting) exist because Tally is a Windows desktop app.
  This is a market signal: users want anywhere-access but keep a desktop-grade UX.

Implications for us: local-first single-user should be free. For multi-user, offer LAN sync (one
machine as a host with real concurrency, comparable to TallyPrime Server rather than plain Gold), optional self-hosted sync, and
**browser read-only reports**. Matching Tally Server's features (monitor sessions, kick users,
central backup) is a v2 goal.

---

## 7. Security model (Tally, as documented)

- **Security control:** enabled per company. Users have a username and password, and an **Admin**
  (Owner) user exists. Two built-in security levels, **Owner** and **Data Entry**; admins define
  more [vendor].
- **Security levels / user roles:** allow or disallow per feature or report, with access types full
  access, create, alter, display, print and preview. "Days allowed" and a cut-off date restrict
  back-dated entry. Deletion and alteration are restricted by withholding Full Access [vendor].
- **Password policy:** strength, expiry, history and self-change [vendor].
- **Tally.NET users and auditors:** remote identities. Auditors get Owner rights by default, and
  separate GST/TDS/TCS/VAT auditors can be given different rights. Tally.NET authentication can
  replace local passwords [vendor].
- **TallyVault:** company-level encryption with a password that is not stored. Tally validates via
  "an advanced algorithm" (unnamed) and keeps no decrypted copy on disk. **A lost password means the
  data is lost permanently** [vendor]. No algorithm or KDF is published. Vendor marketing calls the
  encryption "unbreakable": a marketing claim, unverified.
- **Lost admin password:** there is no recovery. The documented workaround is to restore an older
  backup without a password, or export and re-import into a new company [vendor].
- **Backups (7.0):** scheduled to a local drive or TallyDrive, optionally protected with a backup
  password and recovery key, with versioned restore [vendor].
- **Edit Log:** audit trail of creation, alteration and deletion with versions; mandatory in Edit
  Log releases [vendor].
- **Integration surface:** the XML/ODBC port has **no authentication**, so any process that can
  reach port 9000 can read *and write* the open company [vendor docs silent; 3p advice to firewall
  it]. Whether security-level permissions apply to XML imports is **[unconfirmed]**.

Implications: publish our crypto (for example XChaCha20-Poly1305 with an Argon2id KDF) and **offer
a recovery key**. TallyVault's "lose the password, lose the data" behaviour hurts users, and our
recovery key is a differentiator. Make role-based access, period locks and an immutable audit log
(MCA-compliant by default, not a separate SKU) first-class. Any local API we expose must be
authenticated (a per-app token, localhost-only by default).

---

## 8. Legal cautions for interoperability

(Not legal advice. Flag these for review by Indian counsel.)

1. **Use only documented interfaces.** XML/JSON over HTTP, XML file export, ODBC and Excel are
   features Tally publishes and documents for third-party integration. Using them is ordinary
   interoperability. Parsing `.900`/`.1800` files, decompiling `tally.exe`, or sniffing Tally.NET
   traffic is not something we should do. Indian copyright law reportedly has a narrow
   interoperability exception for computer programs [unconfirmed; ask counsel]. A clean, documented-API approach avoids
   relying on it.
2. **EULA.** Tally's licence terms likely prohibit reverse engineering. Using the product's own
   export features on the user's own data, run by the user under their own licence, is the safest
   posture. Our tool should run against **the user's own licensed Tally**, and we should never ship
   or redistribute Tally components (DLLs, the ODBC driver, `.tcp` files).
3. **Data ownership:** the books belong to the user. Migration runs locally, and nothing is sent to
   us. This matters under India's DPDP Act 2023 when books hold personal data of customers or
   employees (payroll).
4. **Trademarks:** "Tally", "TallyPrime", "Tally.ERP 9", "TallyVault", "TDL" and "TallyShop" are
   Tally Solutions marks. Use them only nominatively ("Import from TallyPrime") with a
   non-affiliation disclaimer, and never in our product name, logo or UI styling. (Note: the
   provisional project dir name `tally-oss` should **not** become the product name.)
5. **Docs are copyrighted.** Write our own XML tag reference from observed behaviour and our own
   tests, not by copying help pages. We recommend that expected values in tests carry provenance:
   *observed* (Tally version and date) from a real Tally export, never copied from vendor sample
   pages as if they were facts.
6. **Licences of OSS connectors:** MIT and Apache code can be studied and attributed. GPL/AGPL code
   (ERPNext migrators, khataos) and unlicensed repos (TallyConnector C#) must not be copied.
7. **Tally's own AI/MCP plug-in** [unconfirmed: one third-party README cites Tally help pages dated 18 Sep 2026; the
   TallyPrime 7.2 release notes (12 Oct 2026) do not mention it] means
   the vendor is moving into "AI over Tally". We can't rely on interface stability. Tally could
   restrict or license its XML port in future releases (no evidence they plan to). Keep the
   file-export path working as a fallback.

---

## 9. Open questions

1. Exact JSONEx (7.0+) schema and whether it covers every object XML does, including UDFs. This
   needs testing against a real 7.x install.
2. Canonical encoding (UTF-8 vs UTF-16) per TallyPrime version, and handling of Indic scripts.
3. Do security levels or user roles apply to HTTP/XML imports? Does Edit Log record XML-originated
   changes, and with what user?
4. Is there a documented way to get deleted-object tombstones, as opposed to GUID set diffing?
5. Behaviour of the HTTP server on TallyPrime Server (multi-client) vs Gold, and whether several
   integrations can poll at once.
6. Can the HTTP server open a closed company on request (`SVCURRENTCOMPANY` of a non-loaded
   company)? Behaviour seems version-dependent [unconfirmed].
7. Suvit, Vyapar and Busy importers: how they read Tally (not gathered: search budget exhausted).
8. Official current prices: confirm Silver, Gold, Server and TSS on tallysolutions.com in a browser,
   since the direct fetch was blocked with HTTP 403. Edit Log edition pricing is also unknown.
9. TallyPrime Developer licence cost, and the size of the partner TDL market (₹ and number of
   add-ons).
10. How common TallyVault is in the field. If it is common, migration requires the user to open the
    company in Tally first (fine for us, since we use the live API).
11. Whether Educational mode's HTTP server allows full export. If it does, unlicensed ex-users could
    still migrate.
12. Data Split: what it produces (separate company per FY) and how GUIDs behave across split
    companies. This matters when merging multi-year history into one of our books.

---

## 10. Sources

Vendor (Tally Solutions):
- XML integration overview: https://help.tallysolutions.com/xml-integration/
- Understanding Tally XML tags: https://help.tallysolutions.com/understanding-tally-xml-tags/
- Sample XML (masters, vouchers, responses): https://help.tallysolutions.com/sample-xml/
- XML interface / message formats: https://help.tallysolutions.com/xml-interface/ , https://help.tallysolutions.com/xml-message-formats
- TallyPrime as server / prerequisites: https://help.tallysolutions.com/tallyprime-as-server/ , https://help.tallysolutions.com/pre-requisites-for-integrations/
- JSON integration (7.0, JSONEx): https://help.tallysolutions.com/tally-prime-integration-using-json-1/ , https://help.tallysolutions.com/json-based-export-import/
- Secure WebSocket integration pattern: https://help.tallysolutions.com/secure-websocket-integration-with-tallyprime/
- ODBC: https://help.tallysolutions.com/using-odbc-interface/ , https://help.tallysolutions.com/odbc-integrations/
- Excel import / mapping templates: https://help.tallysolutions.com/import-data-using-any-excel-file/ , https://help.tallysolutions.com/release-notes-tallyprime-4/
- Data migration FAQ (file extensions, company number, repair): https://help.tallysolutions.com/data-migration-faq/ , https://help.tallysolutions.com/data-migration-data-corruption-faq/
- Release notes 7.0 / 7.1 / 7.2: https://help.tallysolutions.com/release-notes-tallyprime-7-0 , https://help.tallysolutions.com/release-notes-tallyprime-7-1/ , https://help.tallysolutions.com/release-notes-tallyprime-7-2/
- Backup / TallyDrive: https://help.tallysolutions.com/backup-data-tallydrive/ , https://tallysolutions.com/tally/automatic-backup/ , https://help.tallysolutions.com/tally-prime/data-management/backup-restore-data/
- Split company data: https://help.tallysolutions.com/split-company-data-tally/
- TallyPrime Server: https://help.tallysolutions.com/tally-prime-server/ , https://tallysolutions.com/tally-prime-server/
- Remote access / browser reports: https://help.tallysolutions.com/remote-access-tally/ , https://help.tallysolutions.com/browser-reports-faq-tally/
- Data security FAQ (TallyVault, security levels): https://help.tallysolutions.com/data-security-faq/ , https://tallysolutions.com/features/enhanced-security-management/
- Edit Log: https://tallysolutions.com/tally/audit-trail-in-tallyprime , https://help.tallysolutions.com/tally-prime/edit-log/get-started-with-edit-log/
- Pricing: https://tallysolutions.com/business-guides/tallyprime-total-cost-of-ownership-india/ (403 on direct fetch; figures via search snippets), https://tipl.tallysolutions.com/renew-tss , https://tallysolutions.com/tally/tallyprime-pricing-for-seasonal-businesses/ , https://tallysolutions.com/tally-software-services/
- Cloud access: https://tallysolutions.com/tallyprime-cloud-access/ , https://tallysolutions.com/tally/tally-cloud-solution-pricing-plans/
- Invoice templates (7.1): https://tallysolutions.com/tally/custom-invoice-templates-in-tally-prime/ , https://help.tallysolutions.com/guidelines-template-based-invoice-customisation/
- Extensions / TallyShop: https://tallysolutions.com/mena/extensions-integrations/ , https://help.tallysolutions.com/tally/extend-using-tally-shop-add-ons-tally

Third-party:
- ClearTax Tally connector setup: https://docs.cleartax.in/product-help-and-support/for-large-businesses/gst-compliance/tally-connector-for-gst/configuring-tally
- Zoho Books migration from Tally: https://www.zoho.com/books/help/migration/tally-to-zoho-books.html , https://www.patronaccounting.com/blog/tally-to-zoho-migration-checklist
- ERPResearch pricing: https://www.erpresearch.com/pricing/tallyprime
- Tally Solutions (Wikipedia via Wikiwand, partner count): https://wikiwand.com/en/articles/Tally_Solutions
- Univik (file extensions, 5- vs 6-digit folders): https://univik.com/blog/tally-company-number-10000-or-100000.html
- Bison KB (firewalling the Tally port): https://knowledgebase.bison.co.in/view_article.php?id=1963

Open-source code read (via GitHub API, 2026-11-20): repositories listed in section 4, in particular
`dhananjay1405/tally-database-loader` (`dist/tally.mjs`, `docs/incremental-sync.md`,
`database-structure.sql`, README), `Accounting-Companion/TallyConnector`
(`ResponseEnvelope.cs`, `TallyResponseCleaner.cs`), `Forgesaroj/tally-xml-kit` README,
`manikantadm09/tally-prime-erpnext-migrator` README, and `ComplyEaze/bridge` README.
