# RiverCrossing-rs — Project Plan

The Rust project plan. It supersedes the Python nine-EPIC plan; it is derived from the Rust
architecture (`architecture.md`) and the actual Python code's module structure (`CODE_STRUCTURE.md`),
with acceptance criteria traced to the Python R-ids in `requirements.md`. The Python plan is a
reference for *scope*, not a template.

## Method

- **TDD, always.** Each EPIC is executed by the `tdd-rust-writer` agent, test-first (RS-14), one crate
  at a time, **sequentially** (concurrent whole-crate test compiles collide).
- **Order** is the crate dependency order (leaf-first): `cards` → `hands` → `standings` → `ride` →
  `roster` → `rider-issues` → `store` → `csvio` → `htmlexport` → `pdfexport` → `wordpress` → `ui-egui`.
- **Gates** per EPIC: `scripts/validate-rust.sh` green; ≥90% line and ≥90% branch on the delivered
  crate(s); the crate's public API matches `CODE_STRUCTURE.md`.
- **EPIC-1 (empty egui shell) is the de-risk gate** — it proves the egui/craft-libs toolchain before
  any domain code lands.
- **EPIC-13 (CLI + MCP) is new** — beyond the Python app's feature set, added only after 100% parity.

## EPICs

### EPIC-0 — Repository & documentation (this phase)
- **Scope:** GitHub repo, Cargo workspace scaffold, coding standards, `AGENTS.md`, `CODE_STRUCTURE.md`,
  `architecture.md`, this plan, and the EPIC prompts.
- **Acceptance:** every file in `CODE_STRUCTURE.md`/this plan exists; `cargo metadata` parses the
  workspace; the traillens-agents header-policy prerequisite is pushed.
- **R-ids:** none (infrastructure). Sets up the enforcement of R-70/R-75.

### EPIC-1 — Empty egui shell (de-risk)
- **Crates:** `rivercrossing-ui-egui` + `apps/rivercrossing`.
- **Scope:** an `impl eframe::App` opening an empty, titled, resizable window via the `craft` facade;
  lock the craft-libs rev + egui/winit `[patch.crates-io]` forks; an open/quit smoke test.
- **Acceptance:** `cargo run --release -p rivercrossing` opens a window on macOS; smoke + `cargo
  fmt --check` + `cargo check` pass; the pin rev is recorded in `AGENTS.md`.
- **R-ids:** R-01 (runs), R-05 (the UI framework that replaces XRC).

### EPIC-2 — Cards (`rivercrossing-cards`)
- **Scope:** `Suit`/`Rank`/`Card`, seeded CPython-compatible Fisher-Yates `Shoe`,
  deal/restitute/reshuffle/close/reopen/replay, `JokersMode`, `CardsError`, `draw_key`,
  `high_card_draw`, `seeded_card_codes`.
- **Acceptance:** deterministic deal reproducible from the seed; the reshuffle advances to
  `seed + (cycle - 1)` with the stored seed unchanged; the oracle-generated golden fixture
  (`crates/rivercrossing-cards/tests/fixtures/shoe-golden.json`) and the raw RNG vectors pass.
  No card-deal CSV vector set exists — `design/fixtures/vectors/` holds the hands fixtures only.
- **R-ids:** R-40 (seeded shoe), R-13 (shoe config data shape), R-14 (high-card draw).

### EPIC-3 — Hands (`rivercrossing-hands`)
- **Scope:** `HandClass`, `EvaluatedHand`, natural `eval5`/`classify`, wild/joker layer,
  five-of-a-kind, `best_hand` (best-5-of-N, cap), `compare` total order, `self_test`.
- **Acceptance:** six self-test checks pass (7,462-rank sweep + joker vectors + five-of-a-kind +
  total-order sweep + joker-count bound); whole-field 180×12 timing advisory.
- **R-ids:** R-41, R-42, R-43, R-44, R-45.

### EPIC-4 — Ride engine (`rivercrossing-ride`)
- **Scope:** `RideStatus`/`RideConfig`/`RideEngine`, `Event`/`Crossing`/`CrossingResult`/
  `HeldCrossing`/`PendingMiss`, `CardCheck`, `estimate_cards_needed`; **declares `trait RosterView`**;
  corrections (add/edit/void/reassign/deal_manual/void_card/DNF/move/reopen).
- **Acceptance:** the four-state machine (DRAFT/RUNNING/FINISHED/REOPENED) and corrections match the
  Python oracle; `RosterView` breaks the ride↔roster cycle.
- **R-ids:** R-10..R-18 (config/editing), R-30, R-33, R-34, R-35, R-36 (console state), R-14 (tie-breaks).

### EPIC-5 — Roster (`rivercrossing-roster`)
- **Scope:** `EntryMode`/`PlateModel`/`EntryType`/`EntryStatus`, `Rider`/`Entry`/`Roster`/`AuditEvent`/
  `StartViolation`, `can_*` guards, name keys; **implements `RosterView`**.
- **Acceptance:** plate dispatch + lock matrix + retired-entry semantics match the Python oracle.
- **R-ids:** R-11, R-12, R-15, R-16, R-17, R-20, R-22, R-23, R-25.

### EPIC-6 — Standings + rider issues
- **Crates:** `rivercrossing-standings`, `rivercrossing-rider-issues`.
- **Scope:** `TieBreak`/`EntryResult`/`Placed`/`rank`/`rank_by_kind`, leaderboards, `hand_name`;
  `RiderIssue` + `rider_issues(roster)`.
- **Acceptance:** ranking + mixed-ride split (`rank_by_kind`) and the issue report match the oracle.
- **R-ids:** R-60, R-65, R-78.

### EPIC-7 — Store / persistence (`rivercrossing-store`)
- **Scope:** schema + migrations (v1→v2→v3), `Store` facade + session state, backup (hourly, rotate 20).
- **Acceptance:** WAL commit-per-crossing, full-table-rebuild migrations, partial unique index,
  `wal_checkpoint(TRUNCATE)` before backup; resume/backup/restore match the oracle.
- **R-ids:** R-50, R-51, R-52, R-54, R-55, R-10, R-18.

### EPIC-8 — CSV (`rivercrossing-csvio`)
- **Scope:** header-mapped roster CSV preview/commit/export + standings export.
- **Acceptance:** the R-24 header resolution, conflicts/warnings, and plate assignment match the oracle.
- **R-ids:** R-21, R-24.

### EPIC-9 — Exports (`rivercrossing-htmlexport` + `rivercrossing-pdfexport`)
- **Scope:** results payload model; HTML (minijinja/askama + vendored Tailwind CSS + woff2 fonts);
  PDF (printpdf/lopdf + embedded TTFs); podium poster.
- **Acceptance:** golden `design/exports/*.html` byte-faithful; `race-data` JSON round-trips; PDF
  deterministic; times opt-in.
- **R-ids:** R-61, R-62, R-63, R-65.

### EPIC-10 — WordPress (`rivercrossing-wordpress`)
- **Scope:** single-site REST client (`BasicAuth`/`discover`/`publish_page`).
- **Acceptance:** discover/publish match the oracle; credential handling decided (parity vs keychain).
- **R-ids:** R-86.

### EPIC-11 — egui UI + Action registry (`rivercrossing-ui-egui`)
- **Scope:** reproduce the Python screens from `design/screenshots/` + `xrc-windows.md`; the
  `Action`/command registry (§15 menu map + §15b frozen ids); presenters' decision logic; console,
  riders/teams editors, results, audit, settings, dialogs; sound (rodio).
- **Acceptance:** every EPIC-1..11 acceptance scenario passes against the Python screens; ids-drift
  and `xtask layers` checks green; keyboard-first + dialog/destructive/menu/a11y per
  `CODINGSTANDARDS-UX-DESKTOP.md`.
- **R-ids:** R-02, R-03, R-04, R-05, R-30..R-38, R-51, R-73, R-76, R-79, R-81, R-82, R-83, R-84, R-85.

### EPIC-12 — Installers & release
- **Scope:** macOS **ARM64-only** DMG (`hdiutil`, `aarch64-apple-darwin`; no Intel) → Windows x64+arm64 MSI (WiX v5) + portable zip → Linux x86_64
  `.deb` + `.rpm` (`nfpm`); `SHA256SUMS.txt`; signing/notarization in a release EPIC.
- **Acceptance:** each installer builds and smoke-installs; release assets + checksums produced.
- **R-ids:** R-01, R-75.

### EPIC-13 — CLI + MCP (parity complete)
- **Scope:** expose the `Action` registry via `apps/rivercrossing-cli` (hand-rolled subcommands, no
  `clap`), a JSON control channel, and an MCP server built on the shared **`craft-mcp-server`** crate
  (craft-libs; JSON-RPC 2.0 over stdio).
- **Acceptance:** CLI + MCP execute the same commands as the GUI.
- **R-ids:** none (new Rust feature, beyond the Python app).

## R-id coverage

Every Python R-id (R-01…R-85, minus the retired R-53/R-80) is traced to at least one EPIC above;
R-70 (TDD) and R-72 (card vectors) are cross-cutting method requirements enforced across all code
EPICs.
