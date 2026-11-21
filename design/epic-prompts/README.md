# EPIC prompts — rivercrossing-rs

Agent-ready prompts for the Rust port. Load each `EPIC-N.md` into a `tdd-rust-writer` agent to
execute that EPIC (one crate per EPIC, or a defined chunk of one). Execute **in order** — EPIC-1
first, then EPIC-2…EPIC-13; each EPIC's gate blocks the next. Run `tdd-rust-writer` agents
**sequentially**, never in parallel (one uncompilable module test fails every other module's build).

Each prompt is self-contained: it names its `CODE_STRUCTURE.md` entry, `spec.md` sections, the window
registry, and R-ids, plus the tests to write first and the `Done when` gate.

All reference paths below are **in this repository**:

- Order & dependencies: `../docs-md/project-plan.md`
- Architecture: `../docs-md/architecture.md`
- Crate APIs & build order: `../../CODE_STRUCTURE.md`
- Requirements (R-ids): `../docs-md/requirements.md` (the Python oracle, copied in)
- Python window inventory + screenshots: `../docs-md/xrc-windows.md`, `../screenshots/`
- Engineering spec: `../docs-md/spec.md`
- Golden exports / vectors / fonts / assets: `../exports/`, `../fixtures/`, `../assets/`

The Python source tree at `/Users/mark/src/rivercrossing/src/` stays outside this repo as the
behaviour oracle; its unit tests (`tests/unit/*.py`) are the reference for the Rust fixtures.
