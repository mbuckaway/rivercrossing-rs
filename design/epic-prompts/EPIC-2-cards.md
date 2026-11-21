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

Write failing `rstest` tests (in `src/lib.rs` `#[cfg(test)] mod tests;`) for:
- seeded deal determinism (same seed → same sequence);
- empty-shoe reshuffle (audit entry) and `restitute`/`close`/`replay`;
- `draw_key`, `high_card_draw` (rank first, then suit, spades highest);
- `seeded_card_codes`.

## Implement (Green → Refactor)

`Suit`/`Rank`/`Card` (serde), `Shoe` (seed, deal, restitute, close, replay, reshuffle audit),
`draw_key`, `high_card_draw`, `seeded_card_codes`. `thiserror` for the crate error type;
`#![forbid(unsafe_code)]`; SPDX line.

## Done when

- `cargo nextest run -P unit -p rivercrossing-cards` green; `cargo llvm-cov` ≥90% line and ≥90% branch.
- The card vectors (`vectors/*.csv`, copied into `tests/fixtures/`) pass.
- `cargo fmt --check` + `cargo check` pass.
