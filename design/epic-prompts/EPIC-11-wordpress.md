# EPIC-11 — WordPress (`rivercrossing-wordpress`)

Load into a `tdd-rust-writer` agent. TDD (RS-14). Run after EPIC-9.

## Goal

The `rivercrossing-wordpress` crate: a single-site WordPress REST client.

## Context

The Python oracle is `/Users/mark/src/rivercrossing/src/rivercrossing/wordpress.py` (491 LOC); tests
in `tests/unit/test_wordpress.py`. BasicAuth + REST discovery + page publish; stdlib `urllib` →
**ureq 3.5.2** (blocking, `rustls`, `http_status_as_error(false)` so 4xx/5xx keep their WordPress
error body, 30 s global timeout, 64 MiB body limit), with `url` for `form_urlencoded` only. The
HTTP boundary is the `pub(crate) trait Transport` seam (one `UreqTransport` plus a test mock). WP
credential storage is decided: **parity — plaintext settings JSON**; this crate stores nothing and
holds credentials in memory only (recorded in `docs/ui-porting-runbook.md` and
`design/docs-md/project-plan.md`).

## Refs

- `CODE_STRUCTURE.md` (rivercrossing-wordpress) · `requirements.md` R-86
- Python `wordpress.py` + `test_wordpress.py`

## Tests-first (Red)

Write failing tests for: `BasicAuth`; `discover`; `find_page_by_slug`; `publish_page` (success +
failure); URL/response validation. Mock the HTTP boundary behind a trait (RS-10/RS-20).

## Implement (Green → Refactor)

`BasicAuth`, `SiteInfo`, `PublishedPage`, `PageDraft`, `WordPressError`, `discover`,
`find_page_by_slug`, `publish_page`. `PageDraft { title, slug, content, status, parent, page_id }`
with `PageDraft::new(title, slug, content, status)` replacing the oracle's eight parameters.
One `thiserror` enum; `#![forbid(unsafe_code)]`; SPDX line.

## Done when

- `cargo nextest run -P unit -p rivercrossing-wordpress` green; `cargo llvm-cov` ≥90/90.
- Behaviour matches the Python `wordpress.py` tests.
- `cargo fmt --check` + `cargo check` pass.
