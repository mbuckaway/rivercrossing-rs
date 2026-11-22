# EPIC-2 — Cards (`rivercrossing-cards`)

Load into a `tdd-rust-writer` agent. TDD (RS-14). Run after EPIC-1.

## Goal

The `rivercrossing-cards` crate: `Suit`, `Rank`, `Card`, the seeded Fisher-Yates `Shoe`, and the draw
helpers — the deterministic dealing foundation.

## Context

The shoe is Fisher-Yates shuffled from a stored seed; replaying the seed reproduces every card. The
Python oracle is `/Users/mark/src/rivercrossing/src/rivercrossing/cards.py` (546 LOC) with tests in
`tests/unit/test_cards.py`.

## Refs

- `CODE_STRUCTURE.md` (rivercrossing-cards) · `requirements.md` R-40, R-13, R-14
- `spec.md` §4 (the shoe) · Python `cards.py` + `tests/unit/test_cards.py`

## Tests-first (Red)

Write failing `rstest` tests (in `src/<module>/tests.rs`) for:
- seeded deal determinism (same seed → same sequence) against the oracle-generated golden fixture;
- empty-shoe reshuffle (`cycle + 1`, rebuilt under `seed + (cycle - 1)`) and `restitute`/`close`/`reopen`/`replay`;
- `draw_key`, `high_card_draw` (rank first, then suit, spades highest);
- `seeded_card_codes`.

## Implement (Green → Refactor)

`Suit`/`Rank`/`Card` (serde), `Shoe` (deal, restitute, close, reopen, replay, reshuffle),
`draw_key`, `high_card_draw`, `seeded_card_codes`. `thiserror` for the crate error type;
`#![forbid(unsafe_code)]`; SPDX line.

The shoe carries **no default jokers mode**: `JokersMode` is a required argument, and the ride crate
supplies the setup default (total) per spec §4 and R-13 — the Python skeleton's
`jokers_total: bool = False` contradicts both and is not ported.

The shuffle ports **CPython's `random.Random(seed)` and `random.shuffle`** (Mersenne Twister with CPython
int seeding and the getrandbits-based `_randbelow`), stable from CPython 3.8 through the 3.14 the oracle
runs. That is what lets the Rust deal reproduce the oracle's card sequences exactly.

The crate writes **no audit entry**; the ride crate records the reshuffle event. `Shoe::reconfigure`
is not part of this crate — ride adds it with its Edit Ride caller in EPIC-4.

## Done when

- `cargo nextest run -P unit -p rivercrossing-cards` green; `cargo llvm-cov` ≥90% line and ≥90% branch.
- The oracle-generated fixture (`tests/fixtures/shoe-golden.json`, 60 configs) and the raw RNG vectors
  (`tests/fixtures/mt19937.json`) pass. No card-deal CSV vector set exists; this replaces that clause.
- `cargo fmt --check` + `cargo check` pass.
