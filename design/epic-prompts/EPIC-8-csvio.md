# EPIC-8 — CSV (`rivercrossing-csvio`)

Load into a `tdd-rust-writer` agent. TDD (RS-14). Run after EPIC-4, EPIC-5 and EPIC-6.

## Goal

The `rivercrossing-csvio` crate: the unified, header-mapped roster CSV import (preview → commit) and
both CSV exporters (`export`, `export_standings`).

## Context

The Python oracle is `/Users/mark/src/rivercrossing/src/rivercrossing/csvio.py` (1,895 LOC); tests in
`tests/unit/test_csvio.py`; fixtures in `tests/unit/fixtures/csv/`. One unified format (R-21/R-24):
export header `FIRSTNAME,LASTNAME,TYPE,TEAMNAME,PLATE,NOTES,SEX`; ordered case-insensitive header
matchers; one row per rider; plate auto-assign; team grouping across the whole file; preview writes
nothing; commit applies through the roster's audited mutators.

Two explicit, off-by-default import options are part of the contract (R-21/W10): **Map unknown sex
to Male** and **Convert teams of 1 to solo** (DRAFT-only, both plate models). Team-of-one is a DRAFT
warning and a blocking conflict in any later state (R-78). `export_standings` is the §15 standings
CSV, including the R-14 `draw` card and the `sex` column (R-65).

The crate's fixture corpus lives at `crates/rivercrossing-csvio/fixtures/` — copy the synthetic
Python fixtures and use the scrubbed GORBA derivative; never copy the real
`tests/unit/fixtures/csv/gorba_epic.csv` (personal data; see the fixtures README).

## Refs

- `CODE_STRUCTURE.md` (rivercrossing-csvio) · `requirements.md` R-21, R-24, R-65 (standings-CSV
  clause), R-78 · `spec.md` §5, §7, §15 · `xrc-windows.md` `csv_preview_dlg` · Python `csvio.py` +
  `test_csvio.py` + `tests/unit/fixtures/csv/`

## Tests-first (Red)

Write failing tests for: the seven ordered header matchers (first match claims a column, repeated
field keeps the first, unmatched ignored, no name column = one row-1 header conflict); footer/blank
rows vs missing-name conflicts; sex alias folding, unknown-sex conflict and the map-unknown opt-in;
TYPE derivation and unknown values; team grouping (trim/collapse/lowercase key, first-seen display
casing) and the `"; "` notes join; name canonicalisation; plate auto-assign (skips roster and
file-named plates; pooled keeps each rider's plate, relay shares the team's single plate, two plates
= shape conflict); duplicate plates; team size floor (DRAFT warning vs later conflict) and ceiling;
solo-only-ride team rows; the three warnings (team-of-one, case-insensitive duplicate rider name,
near-duplicate team name); the convert-teams-of-one opt-in on both plate models; preview purity
(roster, audit log and target path untouched); commit insert/rename-in-place/DRAFT reshape/post-start
refusal and never-delete-absent; `ImportReport` counts and audit events; the atomic write; `export`
headers (draft and finished), per-model plate, notes on the first member row, the finished columns
(`laps,cards,best_hand,total_time`), formula neutralisation; `export_standings` columns, DNS/DNF,
draw, sex and `show_times`.

## Implement (Green → Refactor)

`CsvIoError`, `ImportOptions`, `ParsedRider`/`ParsedEntry`, `ProblemCode`/`ImportProblem`,
`ImportPreview`/`ImportReport`, `preview`, `commit`, `export`, `export_standings`, the CSV wire
reader/writer and the atomic writer. `thiserror`; `#![forbid(unsafe_code)]`; SPDX line. Use the
roster's audited mutators and the `next_free_plate_skipping`, `update_rider` and `move_rider_at`
seams.

## Done when

- `cargo nextest run -P unit -p rivercrossing-csvio --locked` green; `cargo llvm-cov` ≥90 % line
  **and** ≥90 % branch.
- Behaviour matches the Python `csvio.py` tests; the fixtures produce the pins recorded in
  `crates/rivercrossing-csvio/fixtures/README.md`.
- Both exporters are byte-identical to their goldens and survive a forced rename failure with the
  destination intact.
- `./scripts/validate-rust.sh` passes, and the crate builds and tests on macOS.
