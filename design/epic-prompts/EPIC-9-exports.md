# EPIC-9 — Exports (`rivercrossing-htmlexport` + `rivercrossing-pdfexport`)

Load into a `tdd-rust-writer` agent. TDD (RS-14). Run after EPIC-3 and EPIC-6.

## Goal

Two crates: the HTML results/poster exporter and the PDF report/poster exporter.

## Context

The Python oracles are `/Users/mark/src/rivercrossing/src/rivercrossing/htmlexport/__init__.py`
(1,403 LOC) and `pdfexport.py` (1,934 LOC); tests in `tests/unit/test_htmlexport*.py` and
`test_pdfexport.py`. Jinja2 → **minijinja** (a custom formatter reproduces MarkupSafe escaping and the
integral-float `finalize`); fpdf2 → **craft-pdf (krilla)**. Vendored Tailwind CSS + woff2 fonts ship
with the htmlexport crate; Barlow/Barlow Condensed/DejaVu TTFs ship with the pdfexport crate (licence
notices redistributed — DejaVu is Bitstream Vera, not OFL). Golden files: the committed production
goldens `tests/unit/fixtures/htmlexport/epic-2026-results{,-no-times,-solo}.html` (their `race-data`
JSON is the oracle) and `tests/unit/fixtures/pdfexport/*.pdf` (reference only — cross-engine byte
parity is impossible).

## Refs

- `CODE_STRUCTURE.md` (rivercrossing-htmlexport, rivercrossing-pdfexport)
- `requirements.md` R-61, R-62, R-63, R-65, R-86 · `spec.md` §8, §8b
- Python `htmlexport/*.py` + `pdfexport.py` + their tests + golden fixtures

## Tests-first (Red)

Write failing tests for: the payload model (`ExportOptions`/`EventInfo`/`ResultRow`/`LapsBoardRow`/
`TimeBoardRow`/`RacePayload`/`Sections`/`CardPair`/`RideInfo`/`ExportInput`) + `build_payload`/
`sections`/`format_generated`/`png_data_uri`; `render` self-contained (no external refs, `</` escaped,
byte-identical to the three production goldens); `render_poster`/`render_wordpress`/
`render_poster_wordpress`; times opt-in (R-63); the self-test note (R-44); PDF deterministic-byte
output; `podium_poster`.

## Implement (Green → Refactor)

`build_payload`, `sections`, `format_generated`, `png_data_uri`, `render`, `render_poster`,
`render_wordpress`, `render_poster_wordpress`; `render`/`podium_poster` for PDF (a pure `layout` model
+ a thin `writer`). `thiserror`; `#![forbid(unsafe_code)]`; SPDX line.

## Done when

- `cargo nextest run -P unit` green for both crates; `cargo llvm-cov` ≥90/90.
- HTML renders byte-identically to the committed production goldens; the embedded `race-data`
  round-trips; PDF output is deterministic (same input ⇒ same bytes) and passes the section-keyed
  content oracle; Rust-owned PDF goldens are frozen and byte-compared.
- `cargo fmt --check` + `cargo check` pass.
