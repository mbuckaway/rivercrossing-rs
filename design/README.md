# design/ — the RiverCrossing-rs build contract

This directory holds the Rust migration's design contract and the Python oracle material it was derived
from. Every path the EPIC prompts reference resolves inside this repository.

## Rust docs (authored for the port)

| Path | Role |
|---|---|
| `docs-md/architecture.md` | Rust architecture: layered crates, Action registry, persistence, exports, installer |
| `docs-md/project-plan.md` | Rust EPIC plan (EPIC-0…15) with R-id traceability |
| `docs-md/automation.md` | UI automation decision, live control channel, selector and parity contract |
| `docs-md/test-plan.md` | Test tiers, flow → mechanism → assertion matrix, golden policy |
| `docs-md/ui-contract.md` | Consolidated UI contract (30 windows, 53 actions, controls, flows, gates, parity gaps) |
| `docs-md/visual-style.md` | ArtCraft design system (tokens, typography, spacing, widget vocabulary, appearance) |
| `docs-md/control-mapping.md` | Frozen §15b id → `craft_ui` widget + accessible label/role |
| `docs-md/screen-specs.md` | Per-window layout, states and interactions |
| `mockups/` | Approved ArtCraft HTML mockups (light/dark) |
| `epic-prompts/` | Agent-ready prompts (EPIC-1…15) + README |
| `diagrams/` | Rendered Mermaid diagrams (SVG + PNG) |
| `README.md` | This index |

## Python oracle (copied from `/Users/mark/src/rivercrossing`)

| Path | Role |
|---|---|
| `docs-md/requirements.md` | Numbered R-ids — the acceptance authority |
| `docs-md/spec.md` | Engineering spec §1–§15b |
| `docs-md/xrc-windows.md` | The Python window inventory + frozen names (source for `gui-windows.md`) |
| `docs-md/module-skeletons.md` | Python module APIs + build order (source for `CODE_STRUCTURE.md`) |
| `docs-md/task-briefs.md` | The Python task briefs |
| `docs-md/sound-cues.md` | The three WAV console cues |
| `screenshots/` | Retired Python wx screenshots — historical only; the approved mockups are the visual source |
| `exports/` | Golden HTML results (their `race-data` JSON is the export oracle) |
| `templates/` | Jinja2 results templates |
| `fixtures/vectors/` | eval5 rank sweep + joker vectors (hands fixtures) |
| `assets/` | Card PNGs, sound WAVs, org logo, PDF fonts (+ OFL), htmlexport runtime templates |
| `README-python.md`, `REVIEW.md` | The Python design index + final audit (historical) |

The Python source tree itself (`/Users/mark/src/rivercrossing/src/`) stays outside this repo as the
behaviour oracle; the Rust port mirrors its unit tests as fixtures. The CSV fixture corpus lives with
its crate at `crates/rivercrossing-csvio/fixtures/`; its README records the provenance, hashes and
pins, including the scrubbed GORBA derivative (the real registration export stays on the oracle
machine).
