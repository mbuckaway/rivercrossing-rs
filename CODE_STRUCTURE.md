# CODE_STRUCTURE.md — rivercrossing-rs

The structural contract for the Rust workspace: crate boundaries, per-crate public APIs, dependency
order, and build order. Generated from the Python `design/docs-md/module-skeletons.md` and the actual
Python code, translated to the Rust layered-crate architecture in `design/docs-md/architecture.md`.

## Layered workspace

```
apps/rivercrossing                 thin eframe binary (craft facade + ui-egui)
apps/rivercrossing-cli             thin CLI, hand-rolled parser over the Action registry (EPIC-13)
crates/rivercrossing-mcp           MCP server over the Action registry on craft-mcp-server (EPIC-13)
crates/rivercrossing-ui-egui       L6 — egui front end (the only crate allowed egui/eframe/winit)
crates/rivercrossing-pdfexport     ┐
crates/rivercrossing-htmlexport    ├ L0–L3 domain crates (UI-agnostic, forbid unsafe)
crates/rivercrossing-wordpress     │
crates/rivercrossing-csvio         │
crates/rivercrossing-store         │
crates/rivercrossing-rider-issues  │
crates/rivercrossing-roster        │
crates/rivercrossing-ride          │
crates/rivercrossing-standings     │
crates/rivercrossing-hands         │
crates/rivercrossing-cards         ┘
```

Layering rule (`cargo xtask layers`): nothing below `rivercrossing-ui-egui` may depend on
`egui`/`eframe`/`winit`/`rfd`.

## Dependency order (leaf-first)

```
cards          (leaf)
hands        → cards
ride         → cards, hands               (declares trait RosterView; owns SnapshotEntry)
roster       → ride, cards                 (implements RosterView; cards seeds the logo deck)
standings    → hands, cards, ride          (ranks ride's SnapshotEntry)
rider_issues → roster
store        → ride, cards, roster
csvio        → roster, standings, ride, cards
htmlexport   → standings, cards, hands
pdfexport    → htmlexport, standings, hands
wordpress    (leaf)
ui-egui      → every domain crate
```

The Python `ride`↔`roster` cycle (roster imports `RideStatus` from ride at runtime; ride imports
roster types under `TYPE_CHECKING` only and duck-types the `roster` parameter) is broken in Rust by
`trait RosterView` — declared in `rivercrossing-ride`, implemented by `rivercrossing-roster`. This is
a new Rust design; no such trait exists in the Python code.

## Crate public APIs

### rivercrossing-cards
`Suit`, `Rank`, `Card` (code/parse, serde), `JokersMode`, `Shoe` (seeded CPython-compatible Fisher-Yates
shuffle; deal / restitute / reshuffle / close / reopen / replay), `CardsError`, `draw_key`,
`high_card_draw`, `seeded_card_codes`. The shoe takes its jokers mode as a required argument — the ride
owns the default.

### rivercrossing-hands
`HandClass`, `EvaluatedHand`, `eval5`, `best_hand`, `compare` (total order: class, then kickers, then
fewer jokers wins), `self_test` (`SelfTestReport`/`SelfTestCheck`; six checks, five blocking and one
advisory timing check). The 7,462-row rank sweep and joker vectors ship as `fixtures/*.csv`, embedded
via `include_str!` and parsed by the crate so the runtime self-test reads the same data the tests do.

### rivercrossing-standings
Re-exports ride's `TieBreak` (the config and ranking share one type); owns `ZeroLapMode`
(`Rank`/`Dns`/`Hide`), `EntryResult`, `Placed` (`place: Option<u32>`, `None` = DNS), `StandingsError`,
`rank`, `rank_by_kind`, `laps_leaderboard`, `time_leaderboard`, `hand_name`,
`tiebreak_order_from_spellings`, and the constants `DEFAULT_TIEBREAK_ORDER`, `LIVE_TIEBREAK_ORDER`,
`DRAW_TIE_NOTE`, `DEFAULT_TOP`. Ranking takes `&[EntryResult]`; `EntryResult::from(&SnapshotEntry)` is
the seam from ride's result row. Both leaderboards share one key — laps DESC, then total ASC — and DNS
filtering is the caller's.

### rivercrossing-ride
`RideStatus`, `RideConfig`, `RideEngine`, `Event`/`EventAction`, `Crossing`, `CrossingResult`,
`HeldCrossing`, `PendingMiss`, `CardCheck`, `Sufficiency`, `SnapshotEntry` (the result row standings
ranks), `Timestamp`, `estimate_cards_needed`, `check_card_sufficiency`, `format_elapsed`; **declares
`trait RosterView`** (reads plus the roster mutations the engine performs) and `trait Clock`.

### rivercrossing-roster
Re-exports ride's `EntryMode`, `PlateModel`, `RideStatus`, `StartViolation`; owns `EntryType`,
`EntryStatus`, `Rider`, `Entry`, `AuditEvent`, `RosterError`, the `can_*` lock matrix,
`rider_name_key` / `canonical_person_name` / `team_name_key`. `Roster` covers construction
(`create_solo_entry` / `create_team_entry` / `create_team_entry_of_one` / `create_empty_team`), edits,
plate dispatch (`resolve_plate`, `next_free_plate`, `change_*_plate`), moves
(`move_rider` / `extract_rider_to_solo` / `add_rider_to_team` / `remove_rider`), retirement, the restore
seams (`load_entries` / `load_retired_entries`), team logos (`next_team_logo_card` / `random_team_card` /
`set_team_logo_card`), `validate_for_start`, and the audit log (`audit_log` / `take_audit_log`);
**implements `RosterView`**.

### rivercrossing-rider-issues
`IssueKind` (the seven kinds), `RiderIssue<'a>` (borrows the entry and optional rider, plus the message),
`rider_issues(&Roster)` — the advisory roster defect report (R-78), not R-12's start gate.

### rivercrossing-store
`Store`, `SessionState`, `PreviousSession`, `RideRow`, `default_db_path`, `ensure_schema`,
`MIGRATIONS`/`run_migrations`, `backup::run/restore/schedule_hourly`. SQLite via **rusqlite (bundled)**:
WAL, foreign keys, CHECK constraints, partial unique index (`entry_plate_live_unique WHERE retired=0`),
full-table-rebuild migrations, `uuid`, JSON audit payloads, `wal_checkpoint(TRUNCATE)` before backup.

### rivercrossing-csvio
`CsvIoError`, `ParsedRider`/`ParsedEntry`, `ImportConflict`/`ImportPreview`/`ImportReport`, `preview`,
`commit`, `export`, `export_standings`.

### rivercrossing-htmlexport
Shared results model (`ExportOptions`, `EventInfo`, `ResultRow`, `LapsBoardRow`, `TimeBoardRow`,
`RacePayload`, `Sections`) + `build_payload`, `sections`, `format_generated`, `render`,
`render_poster`, `render_wordpress`, `render_poster_wordpress`. Jinja2 → **minijinja/askama**; vendored
Tailwind CSS and woff2 fonts ship with the crate.

### rivercrossing-pdfexport
`render`, `podium_poster`, deterministic-byte stream helpers. fpdf2 → **printpdf/lopdf** (or craft-pdf);
Barlow / Barlow Condensed / DejaVu TTFs embedded, OFL notices redistributed.

### rivercrossing-wordpress
`BasicAuth`, `SiteInfo`, `PublishedPage`, `discover`, `find_page_by_slug`, `publish_page` — a
single-site REST client. stdlib `urllib` → **reqwest/ureq**.

### rivercrossing-ui-egui
The egui front end: panels/dialogs for the Python window inventory, the `Action`/command registry
(§15 menu map + §15b frozen ids), and the presenters' decision logic. Sound via **rodio**.

## Build order

Port in dependency order (leaf-first), each crate by a **sequential** `tdd-rust-writer` agent:

`cards` → `hands` → `ride` → `roster` → `standings` → `rider-issues` → `store` → `csvio` →
`htmlexport` → `pdfexport` → `wordpress` → `ui-egui` → `apps/rivercrossing`.

The empty-shell EPIC (EPIC-1) creates `rivercrossing-ui-egui` + `apps/rivercrossing` first (an empty
egui window, no business logic) to prove the egui/craft-libs toolchain before the domain crates land.

EPIC-13 (parity complete) adds `crates/rivercrossing-mcp` — built on the shared **`craft-mcp-server`**
crate (JSON-RPC 2.0 over stdio; the app implements `Tools`) — and `apps/rivercrossing-cli` (a
hand-rolled subcommand parser, no `clap`), both dispatching the same `Action` registry.
