# EPIC prompts — rivercrossing-rs

Agent-ready prompts for the Rust port. Load each `EPIC-N.md` into a `tdd-rust-writer` agent to
execute that EPIC (one crate per EPIC, or a defined chunk of one). Execute **in order** — EPIC-1
first, then EPIC-2…EPIC-14; each EPIC's gate blocks the next. Run `tdd-rust-writer` agents
**sequentially**, never in parallel (one uncompilable module test fails every other module's build).

EPIC-10 (`EPIC-10-ui-automation-parity.md`) is the one deliberate owner exception: it is not a
single crate. Load its `.rs` work into `tdd-rust-writer`, keep the docs and the parity scaffold in the
main session, and give the shell/gate changes to `shell-code-writer`. It must land before EPIC-12 so
the UI is built against a test contract.

Each prompt is self-contained: it names its `CODE_STRUCTURE.md` entry, `spec.md` sections, the window
registry, and R-ids, plus the tests to write first and the `Done when` gate.

All reference paths below are **in this repository**:

- Order & dependencies: `../docs-md/project-plan.md`
- Architecture: `../docs-md/architecture.md`
- Crate APIs & build order: `../../CODE_STRUCTURE.md`
- Requirements (R-ids): `../docs-md/requirements.md` (the Python oracle, copied in; R-87…R-92 are
  Rust-added)
- Python window inventory + screenshots: `../docs-md/xrc-windows.md`, `../screenshots/`
- Engineering spec: `../docs-md/spec.md`
- Automation & parity: `../docs-md/automation.md`, `../docs-md/test-plan.md`
- Golden exports / vectors / fonts / assets: `../exports/`, `../fixtures/`, `../assets/`

The Python source tree at `/Users/mark/src/rivercrossing/src/` stays outside this repo as the
behaviour oracle; its unit tests (`tests/unit/*.py`) are the reference for the Rust fixtures.
