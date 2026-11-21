# EPIC-6 — Standings + rider issues

Load into a `tdd-rust-writer` agent. TDD (RS-14). Run after EPIC-3 and EPIC-5.

## Goal

Two crates: `rivercrossing-standings` (ranking, tie-breaks, leaderboards) and
`rivercrossing-rider-issues` (the pre-start defect report).

## Context

The Python oracles are `/Users/mark/src/rivercrossing/src/rivercrossing/standings.py` (706 LOC) and
`rider_issues.py` (257 LOC); tests in `tests/unit/test_standings.py` and `test_rider_issues_checker.py`.
Mixed-ride results split into Teams and Solo sections (`rank_by_kind`), each renumbered from 1.

## Refs

- `CODE_STRUCTURE.md` (rivercrossing-standings, rivercrossing-rider-issues)
- `requirements.md` R-60, R-65, R-78 · `spec.md` §5
- Python `standings.py`/`rider_issues.py` + their tests

## Tests-first (Red)

Write failing tests for: `TieBreak` ordering + spelling conversion; `rank`/`rank_by_kind` (mixed split,
DNF excluded); `laps_leaderboard`/`time_leaderboard`; `hand_name`; `EntryResult`/`Placed`;
`rider_issues(roster)` (team-of-one, missing name/number, duplicate/near-duplicate name, duplicate
number).

## Implement (Green → Refactor)

`TieBreak`, `ZeroLapMode`, `EntryResult`, `Placed`, `rank`, `rank_by_kind`, leaderboards, `hand_name`,
`tiebreak_order_from_spellings`; `RiderIssue` + `rider_issues(roster)`. `#![forbid(unsafe_code)]`;
SPDX line.

## Done when

- `cargo nextest run -P unit` green for both crates; `cargo llvm-cov` ≥90/90.
- Behaviour matches the Python standings/rider-issues tests.
- `cargo fmt --check` + `cargo check` pass.
