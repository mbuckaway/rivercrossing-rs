# EPIC-5 — Roster (`rivercrossing-roster`)

Load into a `tdd-rust-writer` agent. TDD (RS-14). Run after EPIC-4.

## Goal

The `rivercrossing-roster` crate: riders, entries, teams, the plate/entry model, the lock matrix, team
logos, audit events, the start gate, and the roster mutators — **implementing `RosterView`**. The trait
lives in `crates/rivercrossing-ride/src/roster_view.rs`; read it there, do not copy it (a copy drifts).

## Context

The Python oracle is `/Users/mark/src/rivercrossing/src/rivercrossing/roster.py` (1,594 LOC); tests in
`tests/unit/test_roster.py` (269 functions / 327 cases). Plate is the crossing's attribution key; the
team is the result-posting unit.

`rivercrossing-roster` depends on **`rivercrossing-ride`** (the shared `EntryMode`/`PlateModel`/
`RideStatus`/`StartViolation` types, `EntryId`, `EntrySnapshot`, `RosterView`, `RideError`) and on
**`rivercrossing-cards`** (`seeded_card_codes` builds the logo deck). It re-exports ride's enums rather
than redefining them; the oracle's `StartViolation` carries the whole `Entry`, ride's carries the plate.

## Refs

- `CODE_STRUCTURE.md` (rivercrossing-roster) · `requirements.md` R-11, R-12, R-15, R-16, R-17, R-20,
  R-22, R-23, R-25
- `spec.md` §1, §3 · Python `roster.py` + `test_roster.py`

## Tests-first (Red)

Write failing tests for: enums; `Rider`/`Entry`/`AuditEvent`; `can_*` guards and the lock matrix;
`rider_name_key`/`canonical_person_name`/`team_name_key`; `Roster` mutators; plate dispatch; the
whole-number plate-validation domain; team logos (`next_team_logo_card` strictly after, not at-or-after;
`random_team_card` with an injected chooser; `set_team_logo_card`); retired-entry semantics (a pooled move
that empties a data-bearing entry keeps it retired and invisible to `entries()`); the restore seams
(`load_entries`/`load_retired_entries`); `validate_for_start`; `RosterView` implementation.

## Implement (Green → Refactor)

`EntryType`, `EntryStatus`, `Rider`, `Entry`, `Roster`, `AuditEvent`, the `can_*` guards, name keys,
`RosterError`, and `impl RosterView for Roster`. Surface beyond the obvious mutators:
`create_empty_team`, `create_team_entry_of_one`, `next_free_plate`, `resolve_plate`, `entry_by_key`,
`take_audit_log`, `update_shape`, the ride-status setter, `change_*_plate`, `add_rider_to_team`,
`remove_rider`, `random_team_card`, `set_team_logo_card`, `load_entries`, `load_retired_entries`.
`thiserror`; `#![forbid(unsafe_code)]`; SPDX line.

The roster takes `entry_mode`/`plate_model`/`max_team_size` at construction — **no defaults**: the Python
skeleton's `SOLO`/4/pooled defaults contradict R-11's mixed default, which the ride config owns.

Note the trait's semantics, which the oracle does not make explicit: `entries()` is live-only while
`entry()` includes retired rows; `set_entry_dnf` is infallible, the engine always passes `rider = false`
(it keeps per-rider DNF itself), `plate`/`rider` are audit-only, and an unknown id is a silent no-op.

## Done when

- `cargo nextest run -P unit -p rivercrossing-roster` green; the roster's own `cargo llvm-cov` rows hold
  ≥90% line and ≥90% branch.
- Behaviour matches the Python `roster.py` tests, ported as Rust fixtures.
- `cargo fmt --check`, `cargo clippy -D warnings` and `cargo check` pass.
