# EPIC-7 — Store / persistence (`rivercrossing-store`)

Load into a `tdd-rust-writer` agent. TDD (RS-14). Run after EPIC-4 and EPIC-5.

## Goal

The `rivercrossing-store` crate: SQLite schema + migrations, the `Store` facade (rides, session state,
save/load), and hourly backups.

## Context

The Python oracle is `/Users/mark/src/rivercrossing/src/rivercrossing/store/` (`__init__.py` 1,713 LOC,
`schema.py` 352, `migrations.py` 295, `backup.py` 292); tests in `tests/unit/test_store.py` and
`test_backup.py`. Use **rusqlite (bundled)** — WAL, foreign keys, `CHECK` constraints, a partial
unique index (`entry_plate_live_unique WHERE retired=0`), full-table-rebuild migrations (SQLite cannot
drop table-level UNIQUE), `uuid`, JSON audit payloads, `wal_checkpoint(TRUNCATE)` before backup. This
crate is large: split into schema+migrations, the `Store` facade, and backup.

## Refs

- `CODE_STRUCTURE.md` (rivercrossing-store) · `requirements.md` R-50, R-51, R-52, R-54, R-55, R-10,
  R-18
- `spec.md` §2 (database) · Python `store/*.py` + `test_store.py`/`test_backup.py`

## Tests-first (Red)

Write failing tests for: `ensure_schema` + PRAGMAs; migrations v1→v2→v3 (idempotent); the partial
unique index; `Store` open/create/delete/duplicate/update; session state (clean-quit vs crash);
`save_roster`/`roster_for`/`append`/`append_roster_event`/`load_engine`/`audit_rows`; backup
run/restore/hourly (rotate 20) with `wal_checkpoint(TRUNCATE)`.

## Implement (Green → Refactor)

`schema`/`migrations`/`backup`/`Store` with `rusqlite` (bundled) + `uuid` + `serde_json`. `thiserror`;
`#![forbid(unsafe_code)]`; SPDX line.

## Done when

- `cargo nextest run -P unit -p rivercrossing-store` green; `cargo llvm-cov` ≥90/90.
- Behaviour matches the Python `store` tests; the v2→v3 idempotency gotcha is preserved.
- `cargo fmt --check` + `cargo check` pass.
