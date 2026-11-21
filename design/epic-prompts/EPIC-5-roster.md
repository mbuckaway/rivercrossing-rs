# EPIC-5 — Roster (`rivercrossing-roster`)

Load into a `tdd-rust-writer` agent. TDD (RS-14). Run after EPIC-4.

## Goal

The `rivercrossing-roster` crate: riders, entries, teams, the plate/entry model, audit events, and the
roster mutators — **implementing `RosterView`** (declared in `rivercrossing-ride`).

## Context

The Python oracle is `/Users/mark/src/rivercrossing/src/rivercrossing/roster.py` (1,594 LOC); tests in
`tests/unit/test_roster.py`. Plate is the crossing's attribution key; the team is the result-posting
unit. `rivercrossing-roster` depends on `rivercrossing-ride` (for `RideStatus`/`RosterView`).

## Refs

- `CODE_STRUCTURE.md` (rivercrossing-roster) · `requirements.md` R-11, R-12, R-15, R-16, R-17, R-20,
  R-22, R-23, R-25
- `spec.md` §1, §3 · Python `roster.py` + `test_roster.py`

## Tests-first (Red)

Write failing tests for: enums; `Rider`/`Entry`/`AuditEvent`/`StartViolation`; `can_*` guards;
`rider_name_key`/`canonical_person_name`/`team_name_key`; `Roster` mutators; plate dispatch and the
lock matrix; retired-entry semantics (a pooled move that empties a data-bearing entry keeps it);
`validate_for_start`; `RosterView` implementation.

## Implement (Green → Refactor)

`EntryMode`, `PlateModel`, `EntryType`, `EntryStatus`, `Rider`, `Entry`, `Roster`, `AuditEvent`,
`StartViolation`, the `can_*` guards, name keys, and `impl RosterView for Roster`. `thiserror`;
`#![forbid(unsafe_code)]`; SPDX line.

## Done when

- `cargo nextest run -P unit -p rivercrossing-roster` green; `cargo llvm-cov` ≥90/90.
- Behaviour matches the Python `roster.py` tests, ported as Rust fixtures.
- `cargo fmt --check` + `cargo check` pass.
