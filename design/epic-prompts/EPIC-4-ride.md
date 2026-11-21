# EPIC-4 — Ride engine (`rivercrossing-ride`)

Load into a `tdd-rust-writer` agent. TDD (RS-14). Run after EPIC-2.

## Goal

The `rivercrossing-ride` crate: the ride state machine, crossing recording, corrections, and the
`RosterView` trait that breaks the ride↔roster cycle.

## Context

The Python oracle is `/Users/mark/src/rivercrossing/src/rivercrossing/ride.py` (3,752 LOC); tests in
`tests/unit/test_ride*.py`. The Python ride↔roster cycle is resolved in Rust by declaring
`trait RosterView` here (implemented later by `rivercrossing-roster`); no such trait exists in
Python — it is new. This crate is large: split into config/card-check, engine lifecycle/timing/undo,
corrections, and replay+`RosterView` chunks, in that order.

## Refs

- `CODE_STRUCTURE.md` (rivercrossing-ride) · `requirements.md` R-10..R-18, R-30, R-33, R-34, R-35,
  R-36, R-14
- `spec.md` §1 (event model), §3 (state machine), §6 (timing) · Python `ride.py` + `test_ride*.py`

## Tests-first (Red)

Write failing tests for: `setup_minimum_violations`; the four-state machine (DRAFT/RUNNING/FINISHED/
REOPENED) + start/stop/finish/reopen; `record_crossing` timing, undo, held/short-lap cards, misses;
corrections (add/edit/void/reassign/deal_manual/void_card/DNF/move/reopen); `apply()`/replay
equivalence; card-sufficiency (`estimate_cards_needed`, `check_card_sufficiency`); `RosterView` seam.

## Implement (Green → Refactor)

`RideStatus`, `RideConfig`, `RideEngine`, `Event`/`Crossing`/`CrossingResult`/`HeldCrossing`/
`PendingMiss`/`CardCheck`, `estimate_cards_needed`, `check_card_sufficiency`; **`trait RosterView`**.
`thiserror`; `#![forbid(unsafe_code)]`; SPDX line.

## Done when

- `cargo nextest run -P unit -p rivercrossing-ride` green; `cargo llvm-cov` ≥90/90.
- Behaviour matches the Python `ride.py` tests, ported as Rust fixtures.
- `cargo fmt --check` + `cargo check` pass.
