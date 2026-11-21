# EPIC-3 — Hands (`rivercrossing-hands`)

Load into a `tdd-rust-writer` agent. TDD (RS-14). Run after EPIC-2.

## Goal

The `rivercrossing-hands` crate: five-card poker evaluation with jokers wild, best-5-of-N selection,
a total-order `compare`, and the blocking self-test.

## Context

Jokers fully wild, five of a kind above royal flush, and — on equal class and kickers — fewer jokers
above more. The Python oracle is `/Users/mark/src/rivercrossing/src/rivercrossing/hands.py`
(1,367 LOC); tests in `tests/unit/test_hands.py`. The rank table and vectors become Rust fixtures
(`vectors/rank_sweep.csv`, 7,462 rows; `vectors/joker_vectors.csv`).

## Refs

- `CODE_STRUCTURE.md` (rivercrossing-hands) · `requirements.md` R-41, R-42, R-43, R-44, R-45
- `spec.md` §5 (hand evaluation) · Python `hands.py` + `tests/unit/test_hands.py`

## Tests-first (Red)

Write failing tests for:
- natural `eval5`/`classify` against the 7,462-row rank sweep;
- wild/joker layer and five-of-a-kind ordering;
- `best_hand` best-5-of-N (0/1/2+ jokers, card cap, partial hands);
- `compare` total order;
- `self_test` (six checks: rank sweep + joker vectors + five-of-a-kind + total-order sweep +
  joker-count bound + whole-field timing advisory).

## Implement (Green → Refactor)

`HandClass`, `EvaluatedHand`, `eval5`, `classify`, `best_hand`, `compare`, `self_test`.
`#![forbid(unsafe_code)]`; SPDX line. Load the vectors via `include_str!`/fixtures.

## Done when

- `cargo nextest run -P unit -p rivercrossing-hands` green; `cargo llvm-cov` ≥90/90.
- `self_test` passes all six checks.
- `cargo fmt --check` + `cargo check` pass.
