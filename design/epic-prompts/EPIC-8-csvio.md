# EPIC-8 — CSV (`rivercrossing-csvio`)

Load into a `tdd-rust-writer` agent. TDD (RS-14). Run after EPIC-4, EPIC-5, EPIC-6.

## Goal

The `rivercrossing-csvio` crate: unified header-mapped roster CSV import/export + standings export.

## Context

The Python oracle is `/Users/mark/src/rivercrossing/src/rivercrossing/csvio.py` (1,895 LOC); tests in
`tests/unit/test_csvio.py`. One unified format (R-24): export header
`FIRSTNAME,LASTNAME,TYPE,TEAMNAME,PLATE,NOTES,SEX`; ordered case-insensitive header matchers; one row
per rider; plate auto-assign; team-of-one / duplicate-name / near-duplicate warnings vs hard conflicts.

## Refs

- `CODE_STRUCTURE.md` (rivercrossing-csvio) · `requirements.md` R-21, R-24
- `spec.md` §7 (CSV) · Python `csvio.py` + `test_csvio.py`

## Tests-first (Red)

Write failing tests for: header resolution; preview counts + conflicts (duplicate plate blocks; team-of-
one / case-insensitive duplicate name / near-duplicate team name warn); name canonicalisation; plate
auto-assign (pooled vs relay); `commit` (pooled + relay assembly); `export`; `export_standings`
(with `type` column and the R-14 drawn card).

## Implement (Green → Refactor)

`CsvIoError`, `ParsedRider`/`ParsedEntry`, `ImportConflict`/`ImportPreview`/`ImportReport`, `preview`,
`commit`, `export`, `export_standings`. `thiserror`; `#![forbid(unsafe_code)]`; SPDX line.

## Done when

- `cargo nextest run -P unit -p rivercrossing-csvio` green; `cargo llvm-cov` ≥90/90.
- Behaviour matches the Python `csvio.py` tests; the fixture CSVs (`tests/unit/fixtures/csv/*.csv`)
  round-trip.
- `cargo fmt --check` + `cargo check` pass.
