# EPIC-11 — WordPress (`rivercrossing-wordpress`)

Load into a `tdd-rust-writer` agent. TDD (RS-14). Run after EPIC-9.

## Goal

The `rivercrossing-wordpress` crate: a single-site WordPress REST client.

## Context

The Python oracle is `/Users/mark/src/rivercrossing/src/rivercrossing/wordpress.py` (491 LOC); tests
in `tests/unit/test_wordpress.py`. BasicAuth + REST discovery + page publish; stdlib `urllib` →
**reqwest/ureq**. WP credential storage (plaintext settings JSON vs OS keychain) is an open decision —
record the choice in `docs/ui-porting-runbook.md`.

## Refs

- `CODE_STRUCTURE.md` (rivercrossing-wordpress) · `requirements.md` R-86
- Python `wordpress.py` + `test_wordpress.py`

## Tests-first (Red)

Write failing tests for: `BasicAuth`; `discover`; `find_page_by_slug`; `publish_page` (success +
failure); URL/response validation. Mock the HTTP boundary behind a trait (RS-10/RS-20).

## Implement (Green → Refactor)

`BasicAuth`, `SiteInfo`, `PublishedPage`, `discover`, `find_page_by_slug`, `publish_page`. `thiserror`;
`#![forbid(unsafe_code)]`; SPDX line.

## Done when

- `cargo nextest run -P unit -p rivercrossing-wordpress` green; `cargo llvm-cov` ≥90/90.
- Behaviour matches the Python `wordpress.py` tests.
- `cargo fmt --check` + `cargo check` pass.
