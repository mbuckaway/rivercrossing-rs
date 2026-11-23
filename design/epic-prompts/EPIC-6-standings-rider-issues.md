# EPIC-6 — Standings + rider issues

Load into a `tdd-rust-writer` agent. TDD (RS-14). Run after EPIC-3 and EPIC-5.

## Goal

Two crates: `rivercrossing-standings` (ranking, tie-breaks, leaderboards, hand names) and
`rivercrossing-rider-issues` (the advisory roster defect report).

## Context

The Python oracles are `/Users/mark/src/rivercrossing/src/rivercrossing/standings.py` (706 LOC) and
`rider_issues.py` (257 LOC); tests in `tests/unit/test_standings.py` (85 fixed functions / 118 cases plus
8 Hypothesis tests) and `test_rider_issues_checker.py` (18 functions / 24 fixed nodes plus one property
test). Mixed-ride results split into Teams and Solo sections (`rank_by_kind`), each renumbered from 1.

`rivercrossing-standings` depends on **`rivercrossing-ride`** (its `TieBreak` is re-exported, plus
`EntryId`, `SnapshotKind` and `SnapshotEntry`), **`rivercrossing-hands`** (`HandClass`, `EvaluatedHand`,
`compare`) and **`rivercrossing-cards`** (`Card`, `draw_key`), plus `thiserror`.
`rivercrossing-rider-issues` depends on **`rivercrossing-roster`** only.

Ranking takes `&[EntryResult]`; `EntryResult::from(&SnapshotEntry)` is the seam from ride's result row,
so ride never depends on standings.

## Refs

- `CODE_STRUCTURE.md` (rivercrossing-standings, rivercrossing-rider-issues)
- `requirements.md` R-60, R-65, R-78 · `spec.md` §5 (`:157-159`) and §6 (`:161-169`)
- Python `standings.py`/`rider_issues.py` + their tests

## Tests-first (Red)

Standings: `TieBreak` re-export and spelling conversion (unknown spelling message); `ZeroLapMode`
(`Rank`/`Dns`/`Hide`); `EntryResult`/`Placed` (`place: Option<u32>`, `None` = DNS); `rank`/`rank_by_kind`
(hand order, the `HIGH_CARD_DRAW` barrier, competition numbering, the mixed split, DNF excluded entirely,
the DNS/HIDE/RANK partition and append order); `laps_leaderboard`/`time_leaderboard` (one key: laps DESC,
then total ASC; places computed before truncation; DNF excluded; zero-lap filtering is the caller's);
`hand_name` (the twelve golden em-dash strings, the wheel 5-high, five naturals name as QUADS, the empty
hand error).

Rider issues: `rider_issues(roster)` with the **seven** kinds in canonical order — `team-of-one`,
`missing-name`, `missing-number` (pooled only), `duplicate-name`, `duplicate-team-name`,
`near-duplicate-team-name` (a ⚠ warning, two-pass after the hard duplicates), `duplicate-number`
(blank plates skipped; pooled rider plates vs relay entry plates). Retired entries are invisible;
`entry.status` is never read (a DNF entry is still scanned).

## Implement (Green → Refactor)

`TieBreak` (re-exported from ride), `ZeroLapMode`, `EntryResult`, `Placed`, `StandingsError`, `rank`,
`rank_by_kind`, `laps_leaderboard`, `time_leaderboard`, `hand_name`, `tiebreak_order_from_spellings`, and
the constants `DEFAULT_TIEBREAK_ORDER` (high-card first), `LIVE_TIEBREAK_ORDER` (laps then time),
`DRAW_TIE_NOTE`, `DEFAULT_TOP`; `IssueKind`, `RiderIssue<'a>` (borrows the entry and optional rider),
`rider_issues(&Roster)`. `#![forbid(unsafe_code)]`; SPDX line.

Notes the oracle does not state:
- Default arguments do not exist in Rust — callers pass the constants explicitly.
- `f64` ordering uses `total_cmp`; a joker `tiebreak_card` is only read when the draw criterion is
  reached and every member of the group is drawn.
- `hand_name` returns `Result` for an empty hand; `MalformedHand` is the typed form of the oracle's
  `KeyError`.
- The oracle's `TypeError` cases (a non-`TieBreak` order member, `include_zero_laps`, a positional third
  `rank_by_kind` argument) are unrepresentable and are not ported.
- Port the Hypothesis invariants as fixed cases; no property tests.

## Done when

- `cargo nextest run -P unit -p rivercrossing-standings -p rivercrossing-rider-issues` green.
- Each crate's own `cargo llvm-cov` rows hold ≥90% line and ≥90% branch.
- Behaviour matches the Python standings and rider-issues tests.
- `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo check` pass.
