# EPIC-9 — Exports (`rivercrossing-htmlexport` + `rivercrossing-pdfexport`)

Load into a `tdd-rust-writer` agent. TDD (RS-14). Run after EPIC-3 and EPIC-6.

## Goal

Two crates: the HTML results/poster exporter and the PDF report/poster exporter.

## Context

The Python oracles are `/Users/mark/src/rivercrossing/src/rivercrossing/htmlexport/__init__.py`
(1,403 LOC) and `pdfexport.py` (1,934 LOC); tests in `tests/unit/test_htmlexport*.py` and
`test_pdfexport.py`. Jinja2 → **minijinja/askama**; fpdf2 → **printpdf/lopdf**. Vendored Tailwind CSS +
woff2 fonts ship with the htmlexport crate; Barlow/Barlow Condensed/DejaVu TTFs ship with the pdfexport
crate (OFL notices redistributed). Golden files: `design/exports/*.html` (their `race-data` JSON is the
oracle) and `tests/unit/fixtures/pdfexport/*.pdf`.

## Refs

- `CODE_STRUCTURE.md` (rivercrossing-htmlexport, rivercrossing-pdfexport)
- `requirements.md` R-61, R-62, R-63, R-65 · `spec.md` §8, §8b
- Python `htmlexport/*.py` + `pdfexport.py` + their tests + golden fixtures

## Tests-first (Red)

Write failing tests for: the payload model (`ExportOptions`/`EventInfo`/`ResultRow`/`LapsBoardRow`/
`TimeBoardRow`/`RacePayload`/`Sections`) + `build_payload`/`sections`/`format_generated`; `render`
self-contained (no external refs, `</` escaped); `render_poster`/`render_wordpress`/
`render_poster_wordpress`; times opt-in (R-63); PDF deterministic-byte output; `podium_poster`.

## Implement (Green → Refactor)

`build_payload`, `sections`, `format_generated`, `render`, `render_poster`, `render_wordpress`,
`render_poster_wordpress`; `render`/`podium_poster` for PDF. `thiserror`; `#![forbid(unsafe_code)]`;
SPDX line.

## Done when

- `cargo nextest run -P unit` green for both crates; `cargo llvm-cov` ≥90/90.
- Golden HTML/PDF fixtures round-trip byte-faithfully.
- `cargo fmt --check` + `cargo check` pass.
